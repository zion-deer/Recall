//! Application activity collector.
//!
//! A background thread samples the focused window every few seconds. Each run
//! of the same app + window title becomes one event whose end time is extended
//! while it stays in focus. Ends are flushed periodically (not on every sample)
//! to keep disk writes low.
//!
//! Logs from this module must never include app names, titles, or URLs.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use serde::Serialize;

use super::privacy::{Candidate, PrivacyFilter};
use super::{store, EventKind, NewEvent};
use crate::error::AppResult;
use crate::platform::PlatformAdapter;
use crate::settings::Settings;
use crate::storage::{now_ms, Database};

pub const SOURCE: &str = "app_activity";
const FLUSH_INTERVAL_MS: i64 = 15_000;
const RETENTION_SWEEP_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecorderState {
    Starting,
    Recording,
    Paused,
    Off,
    Idle,
    Excluded,
    RecallFocused,
    NoWindow,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecorderStatus {
    pub state: RecorderState,
    pub current_app: Option<String>,
    pub paused_until: Option<i64>,
    pub message: Option<String>,
}

impl RecorderStatus {
    fn new(state: RecorderState) -> Self {
        Self { state, current_app: None, paused_until: None, message: None }
    }
}

/// Live configuration shared between IPC commands and the recorder thread.
pub struct SharedConfig {
    pub settings: RwLock<Settings>,
    pub filter: RwLock<PrivacyFilter>,
}

pub trait Notifier: Send + 'static {
    fn status_changed(&self, status: &RecorderStatus);
    fn memory_changed(&self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionKey {
    app_name: String,
    app_id: Option<String>,
    title: Option<String>,
}

struct OpenSession {
    id: i64,
    key: SessionKey,
    last_flushed: i64,
}

#[derive(Default)]
struct SessionTracker {
    open: Option<OpenSession>,
}

impl SessionTracker {
    /// Records that `key` is in focus at `now`. Returns true if stored data changed.
    fn observe(&mut self, db: &Database, key: SessionKey, now: i64) -> AppResult<bool> {
        if let Some(open) = &mut self.open {
            if open.key == key {
                if now - open.last_flushed < FLUSH_INTERVAL_MS {
                    return Ok(false);
                }
                if store::set_event_end(db, open.id, now)? {
                    open.last_flushed = now;
                    return Ok(true);
                }
                // The user deleted the in-progress memory; start a fresh one.
                self.open = None;
            } else {
                self.close(db, now)?;
            }
        }

        let id = store::insert_event(
            db,
            &NewEvent {
                kind: EventKind::AppActivity,
                source: SOURCE,
                started_at: now,
                ended_at: now,
                app_name: Some(key.app_name.clone()),
                app_id: key.app_id.clone(),
                window_title: key.title.clone(),
                url: None,
                file_path: None,
            },
        )?;
        self.open = Some(OpenSession { id, key, last_flushed: now });
        Ok(true)
    }

    fn close(&mut self, db: &Database, at: i64) -> AppResult<bool> {
        match self.open.take() {
            Some(open) => store::set_event_end(db, open.id, at),
            None => Ok(false),
        }
    }

    /// Forgets the open session without writing (its row may have been deleted).
    fn reset(&mut self) {
        self.open = None;
    }
}

struct RecorderCore {
    tracker: SessionTracker,
    own_pid: u64,
}

impl RecorderCore {
    fn new(own_pid: u64) -> Self {
        Self { tracker: SessionTracker::default(), own_pid }
    }

    /// Takes one activity sample. Returns the resulting status and whether stored data changed.
    fn tick(
        &mut self,
        db: &Database,
        settings: &Settings,
        filter: &PrivacyFilter,
        platform: &dyn PlatformAdapter,
        now: i64,
    ) -> AppResult<(RecorderStatus, bool)> {
        use RecorderState::*;

        if !settings.recording_enabled || !settings.app_activity_enabled {
            return Ok((RecorderStatus::new(Off), self.tracker.close(db, now)?));
        }
        if settings.is_paused(now) {
            let changed = self.tracker.close(db, now)?;
            let status = RecorderStatus { paused_until: settings.paused_until, ..RecorderStatus::new(Paused) };
            return Ok((status, changed));
        }
        if let Some(idle) = platform.idle_seconds() {
            if idle >= u64::from(settings.idle_threshold_secs) {
                let idle_since = now - (idle as i64).saturating_mul(1000);
                return Ok((RecorderStatus::new(Idle), self.tracker.close(db, idle_since)?));
            }
        }

        let window = match platform.active_window() {
            Ok(w) => w,
            Err(e) => {
                let changed = self.tracker.close(db, now)?;
                let status = RecorderStatus { message: Some(e.to_string()), ..RecorderStatus::new(Unavailable) };
                return Ok((status, changed));
            }
        };
        let Some(window) = window else {
            return Ok((RecorderStatus::new(NoWindow), self.tracker.close(db, now)?));
        };
        if window.process_id == self.own_pid {
            return Ok((RecorderStatus::new(RecallFocused), self.tracker.close(db, now)?));
        }

        // Exclusions are checked against the real title even when titles are
        // not being stored, so that e.g. private browsing windows stay excluded.
        let candidate = Candidate {
            app_name: Some(&window.app_name),
            app_id: window.app_id.as_deref(),
            window_title: window.title.as_deref(),
            url: None,
            file_path: None,
        };
        if filter.excludes(&candidate) {
            return Ok((RecorderStatus::new(Excluded), self.tracker.close(db, now)?));
        }

        let key = SessionKey {
            app_name: window.app_name.clone(),
            app_id: window.app_id,
            title: if settings.window_titles_enabled { window.title } else { None },
        };
        let changed = self.tracker.observe(db, key, now)?;
        let status = RecorderStatus { current_app: Some(window.app_name), ..RecorderStatus::new(Recording) };
        Ok((status, changed))
    }
}

enum Command {
    /// Settings or exclusions changed; re-evaluate now.
    Refresh,
    /// Stored memories were deleted; drop the open session reference.
    Reset,
    Stop,
}

pub struct RecorderHandle {
    tx: Sender<Command>,
    status: Arc<Mutex<RecorderStatus>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl RecorderHandle {
    pub fn spawn(
        db: Arc<Database>,
        config: Arc<SharedConfig>,
        platform: Arc<dyn PlatformAdapter>,
        notifier: impl Notifier,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(RecorderStatus::new(RecorderState::Starting)));
        let thread_status = status.clone();
        let thread = std::thread::Builder::new()
            .name("recall-recorder".into())
            .spawn(move || run(db, config, platform, notifier, rx, thread_status))
            .expect("failed to start recorder thread");
        Self { tx, status, thread: Mutex::new(Some(thread)) }
    }

    pub fn status(&self) -> RecorderStatus {
        self.status.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn refresh(&self) {
        let _ = self.tx.send(Command::Refresh);
    }

    pub fn reset(&self) {
        let _ = self.tx.send(Command::Reset);
    }

    /// Closes the open session and stops the thread.
    pub fn stop(&self) {
        let _ = self.tx.send(Command::Stop);
        if let Some(t) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = t.join();
        }
    }
}

fn run(
    db: Arc<Database>,
    config: Arc<SharedConfig>,
    platform: Arc<dyn PlatformAdapter>,
    notifier: impl Notifier,
    rx: Receiver<Command>,
    status: Arc<Mutex<RecorderStatus>>,
) {
    log::info!("recorder started");
    let mut core = RecorderCore::new(u64::from(std::process::id()));
    let mut last_sweep = 0i64;

    loop {
        let now = now_ms();
        let settings = config.settings.read().unwrap_or_else(|e| e.into_inner()).clone();
        let filter = config.filter.read().unwrap_or_else(|e| e.into_inner()).clone();

        let mut changed = false;
        match core.tick(&db, &settings, &filter, platform.as_ref(), now) {
            Ok((new_status, c)) => {
                changed = c;
                let mut current = status.lock().unwrap_or_else(|e| e.into_inner());
                if *current != new_status {
                    if current.state != new_status.state {
                        log::info!("recorder state: {:?} -> {:?}", current.state, new_status.state);
                    }
                    *current = new_status.clone();
                    drop(current);
                    notifier.status_changed(&new_status);
                }
            }
            Err(e) => log::error!("recorder sample failed: {e}"),
        }

        if now - last_sweep >= RETENTION_SWEEP_MS {
            last_sweep = now;
            if let Some(days) = settings.retention_days {
                match store::purge_older_than(&db, now - i64::from(days) * DAY_MS) {
                    Ok(0) => {}
                    Ok(n) => {
                        log::info!("retention removed {n} memories older than {days} days");
                        changed = true;
                    }
                    Err(e) => log::error!("retention sweep failed: {e}"),
                }
            }
        }

        if changed {
            notifier.memory_changed();
        }

        let wait = Duration::from_secs(u64::from(settings.poll_interval_secs.max(1)));
        match rx.recv_timeout(wait) {
            Ok(Command::Refresh) | Err(RecvTimeoutError::Timeout) => {}
            Ok(Command::Reset) => core.tracker.reset(),
            Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => {
                if let Err(e) = core.tracker.close(&db, now_ms()) {
                    log::error!("could not close session on shutdown: {e}");
                }
                log::info!("recorder stopped");
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use crate::memory::privacy::{Exclusion, ExclusionKind};
    use crate::platform::{ActiveWindow, PermissionInfo};
    use std::path::Path;

    #[derive(Default)]
    struct FakePlatform {
        window: Mutex<Option<ActiveWindow>>,
        idle: Mutex<Option<u64>>,
        fail: Mutex<bool>,
    }

    impl FakePlatform {
        fn focus(&self, app: &str, title: &str, pid: u64) {
            *self.window.lock().unwrap() = Some(ActiveWindow {
                app_name: app.into(),
                app_id: None,
                title: Some(title.into()),
                process_id: pid,
            });
        }
    }

    impl PlatformAdapter for FakePlatform {
        fn platform_name(&self) -> &'static str { "test" }
        fn active_window(&self) -> AppResult<Option<ActiveWindow>> {
            if *self.fail.lock().unwrap() {
                return Err(AppError::Unsupported("no display".into()));
            }
            Ok(self.window.lock().unwrap().clone())
        }
        fn idle_seconds(&self) -> Option<u64> { *self.idle.lock().unwrap() }
        fn permissions(&self) -> Vec<PermissionInfo> { vec![] }
        fn request_permission(&self, _: &str) -> AppResult<()> { Ok(()) }
        fn reveal_folder(&self, _: &Path) -> AppResult<()> { Ok(()) }
    }

    fn enabled() -> Settings {
        Settings { recording_enabled: true, ..Settings::default() }
    }

    fn events(db: &Database) -> Vec<crate::memory::MemoryEvent> {
        let mut v = store::list_events(db, &Default::default()).unwrap();
        v.reverse();
        v
    }

    #[test]
    fn records_sessions_and_transitions() {
        let db = Database::open_in_memory().unwrap();
        let p = FakePlatform::default();
        let mut core = RecorderCore::new(1);
        let f = PrivacyFilter::default();
        let s = enabled();

        p.focus("Code", "main.rs", 10);
        let (st, changed) = core.tick(&db, &s, &f, &p, 0).unwrap();
        assert_eq!(st.state, RecorderState::Recording);
        assert_eq!(st.current_app.as_deref(), Some("Code"));
        assert!(changed);

        let (_, changed) = core.tick(&db, &s, &f, &p, 2_000).unwrap();
        assert!(!changed, "same window does not write before the flush interval");
        let (_, changed) = core.tick(&db, &s, &f, &p, 20_000).unwrap();
        assert!(changed, "flushes end time periodically");

        p.focus("Code", "lib.rs", 10);
        core.tick(&db, &s, &f, &p, 22_000).unwrap();
        p.focus("Chrome", "Docs", 11);
        core.tick(&db, &s, &f, &p, 30_000).unwrap();

        let e = events(&db);
        assert_eq!(e.len(), 3);
        assert_eq!((e[0].started_at, e[0].ended_at), (0, 22_000));
        assert_eq!((e[1].started_at, e[1].ended_at), (22_000, 30_000));
        assert_eq!(e[2].app_name.as_deref(), Some("Chrome"));
        assert_eq!(e[2].source, SOURCE);
    }

    #[test]
    fn respects_off_and_pause() {
        let db = Database::open_in_memory().unwrap();
        let p = FakePlatform::default();
        let mut core = RecorderCore::new(1);
        let f = PrivacyFilter::default();
        p.focus("Code", "main.rs", 10);

        let off = Settings::default();
        assert_eq!(core.tick(&db, &off, &f, &p, 0).unwrap().0.state, RecorderState::Off);
        assert!(events(&db).is_empty());

        let s = enabled();
        core.tick(&db, &s, &f, &p, 1_000).unwrap();
        let paused = Settings { paused_until: Some(100_000), ..enabled() };
        let (st, _) = core.tick(&db, &paused, &f, &p, 5_000).unwrap();
        assert_eq!(st.state, RecorderState::Paused);
        assert_eq!(st.paused_until, Some(100_000));
        assert_eq!(events(&db)[0].ended_at, 5_000, "pausing closes the open session");
        core.tick(&db, &paused, &f, &p, 50_000).unwrap();
        assert_eq!(events(&db).len(), 1);

        assert_eq!(core.tick(&db, &paused, &f, &p, 100_000).unwrap().0.state, RecorderState::Recording, "pause expires");
        assert_eq!(events(&db).len(), 2);
    }

    #[test]
    fn idle_closes_session_at_last_input() {
        let db = Database::open_in_memory().unwrap();
        let p = FakePlatform::default();
        let mut core = RecorderCore::new(1);
        let f = PrivacyFilter::default();
        let s = enabled();
        p.focus("Code", "main.rs", 10);
        core.tick(&db, &s, &f, &p, 0).unwrap();
        *p.idle.lock().unwrap() = Some(400);
        let (st, _) = core.tick(&db, &s, &f, &p, 500_000).unwrap();
        assert_eq!(st.state, RecorderState::Idle);
        assert_eq!(events(&db)[0].ended_at, 100_000);
    }

    #[test]
    fn excluded_and_own_windows_are_never_stored() {
        let db = Database::open_in_memory().unwrap();
        let p = FakePlatform::default();
        let mut core = RecorderCore::new(42);
        let f = PrivacyFilter::new(&[
            Exclusion { id: 1, kind: ExclusionKind::App, pattern: "1Password".into(), created_at: 0 },
            Exclusion { id: 2, kind: ExclusionKind::Title, pattern: "incognito".into(), created_at: 0 },
        ]);
        let s = Settings { window_titles_enabled: false, ..enabled() };

        p.focus("1Password", "Vault", 10);
        assert_eq!(core.tick(&db, &s, &f, &p, 0).unwrap().0.state, RecorderState::Excluded);
        p.focus("Chrome", "New Tab (Incognito)", 11);
        assert_eq!(core.tick(&db, &s, &f, &p, 1).unwrap().0.state, RecorderState::Excluded);
        p.focus("Recall", "Recall", 42);
        assert_eq!(core.tick(&db, &s, &f, &p, 2).unwrap().0.state, RecorderState::RecallFocused);
        assert!(events(&db).is_empty());

        p.focus("Chrome", "Bank statement", 11);
        core.tick(&db, &s, &f, &p, 3).unwrap();
        let e = events(&db);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].window_title, None, "titles are dropped when disabled");
    }

    #[test]
    fn recovers_when_open_memory_is_deleted() {
        let db = Database::open_in_memory().unwrap();
        let p = FakePlatform::default();
        let mut core = RecorderCore::new(1);
        let f = PrivacyFilter::default();
        let s = enabled();
        p.focus("Code", "main.rs", 10);
        core.tick(&db, &s, &f, &p, 0).unwrap();
        store::delete_all(&db).unwrap();
        core.tick(&db, &s, &f, &p, 20_000).unwrap();
        let e = events(&db);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].started_at, 20_000);
    }

    #[test]
    fn reports_platform_failures() {
        let db = Database::open_in_memory().unwrap();
        let p = FakePlatform::default();
        *p.fail.lock().unwrap() = true;
        let mut core = RecorderCore::new(1);
        let (st, _) = core.tick(&db, &enabled(), &PrivacyFilter::default(), &p, 0).unwrap();
        assert_eq!(st.state, RecorderState::Unavailable);
        assert!(st.message.is_some());
    }
}
