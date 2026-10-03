//! Browser-memory providers.
//!
//! Providers read only the normal browsing-history SQLite database written by
//! each browser. Private/incognito visits are intentionally unavailable there,
//! which gives us a stronger privacy boundary than inspecting browser windows
//! or accessibility trees. Recall never opens cookie, login, autofill, payment,
//! or preference stores.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use url::Url;

use super::privacy::{Candidate, PrivacyFilter};
use super::{store, EventKind, NewEvent};
use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::storage::{now_ms, Database};

const CHROMIUM_EPOCH_OFFSET_MS: i64 = 11_644_473_600_000;
const SAFARI_EPOCH_OFFSET_MS: i64 = 978_307_200_000;
const MAX_VISITS_PER_SCAN: usize = 500;
const SAME_PAGE_MERGE_MS: i64 = 5 * 60_000;
pub const SCAN_INTERVAL_MS: i64 = 15_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub supported: bool,
    pub installed: bool,
    pub profile_count: usize,
    pub state: &'static str,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
struct BrowserProfile {
    /// Non-sensitive profile label (for example "Default" or "Profile 2").
    id: String,
    history_path: PathBuf,
}

impl BrowserProfile {
    /// Full path is used only as a local cursor key and is never sent to the UI
    /// or logs.
    fn cursor_id(&self) -> String {
        self.history_path.to_string_lossy().into_owned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserVisit {
    raw_time: i64,
    timestamp_ms: i64,
    title: String,
    url: String,
}

trait BrowserProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn supported(&self) -> bool {
        true
    }
    fn enabled(&self, settings: &Settings) -> bool;
    fn profiles(&self) -> Vec<BrowserProfile>;
    fn latest_visit(&self, profile: &BrowserProfile) -> AppResult<i64>;
    fn visits_after(&self, profile: &BrowserProfile, cursor: i64) -> AppResult<Vec<BrowserVisit>>;
}

#[derive(Clone, Copy)]
enum HistoryFormat {
    Chromium,
    Firefox,
    Safari,
}

struct SqliteHistoryProvider {
    id: &'static str,
    name: &'static str,
    format: HistoryFormat,
    roots: Vec<PathBuf>,
    supported: bool,
}

impl BrowserProvider for SqliteHistoryProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn supported(&self) -> bool {
        self.supported
    }

    fn enabled(&self, settings: &Settings) -> bool {
        match self.id {
            "chrome" => settings.browser_chrome_enabled,
            "edge" => settings.browser_edge_enabled,
            "firefox" => settings.browser_firefox_enabled,
            "safari" => settings.browser_safari_enabled,
            _ => false,
        }
    }

    fn profiles(&self) -> Vec<BrowserProfile> {
        if !self.supported {
            return Vec::new();
        }
        let mut out = Vec::new();
        for root in &self.roots {
            match self.format {
                HistoryFormat::Chromium => {
                    add_if_file(&mut out, root.join("History"));
                    if let Ok(entries) = std::fs::read_dir(root) {
                        for entry in entries.flatten() {
                            let name = entry.file_name();
                            let name = name.to_string_lossy();
                            if name == "Default" || name.starts_with("Profile ") {
                                add_if_file(&mut out, entry.path().join("History"));
                            }
                        }
                    }
                }
                HistoryFormat::Firefox => {
                    if let Ok(entries) = std::fs::read_dir(root) {
                        for entry in entries.flatten() {
                            add_if_file(&mut out, entry.path().join("places.sqlite"));
                        }
                    }
                }
                HistoryFormat::Safari => add_if_file(&mut out, root.join("History.db")),
            }
        }
        out
    }

    fn latest_visit(&self, profile: &BrowserProfile) -> AppResult<i64> {
        let snapshot = HistorySnapshot::open(&profile.history_path)?;
        let sql = match self.format {
            HistoryFormat::Chromium => "SELECT COALESCE(MAX(visit_time), 0) FROM visits",
            HistoryFormat::Firefox => "SELECT COALESCE(MAX(visit_date), 0) FROM moz_historyvisits",
            HistoryFormat::Safari => {
                "SELECT CAST(COALESCE(MAX(visit_time), 0) AS INTEGER) FROM history_visits"
            }
        };
        Ok(snapshot.conn.query_row(sql, [], |r| r.get(0))?)
    }

    fn visits_after(&self, profile: &BrowserProfile, cursor: i64) -> AppResult<Vec<BrowserVisit>> {
        let snapshot = HistorySnapshot::open(&profile.history_path)?;
        match self.format {
            HistoryFormat::Chromium => query_visits(
                &snapshot.conn,
                "SELECT v.visit_time, COALESCE(u.title, ''), u.url
                 FROM visits v JOIN urls u ON u.id = v.url
                 WHERE v.visit_time > ?1
                 ORDER BY v.visit_time ASC LIMIT ?2",
                cursor,
                |raw| raw / 1_000 - CHROMIUM_EPOCH_OFFSET_MS,
            ),
            HistoryFormat::Firefox => query_visits(
                &snapshot.conn,
                "SELECT v.visit_date, COALESCE(p.title, ''), p.url
                 FROM moz_historyvisits v JOIN moz_places p ON p.id = v.place_id
                 WHERE v.visit_date > ?1
                 ORDER BY v.visit_date ASC LIMIT ?2",
                cursor,
                |raw| raw / 1_000,
            ),
            HistoryFormat::Safari => query_visits(
                &snapshot.conn,
                "SELECT CAST(v.visit_time AS INTEGER), COALESCE(v.title, ''), i.url
                 FROM history_visits v JOIN history_items i ON i.id = v.history_item
                 WHERE v.visit_time > ?1
                 ORDER BY v.visit_time ASC LIMIT ?2",
                cursor,
                |raw| raw.saturating_mul(1_000) + SAFARI_EPOCH_OFFSET_MS,
            ),
        }
    }
}

fn add_if_file(out: &mut Vec<BrowserProfile>, path: PathBuf) {
    if path.is_file() {
        let id = path
            .parent()
            .and_then(Path::file_name)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Default".into());
        out.push(BrowserProfile {
            id,
            history_path: path,
        });
    }
}

fn query_visits(
    conn: &Connection,
    sql: &str,
    cursor: i64,
    convert_time: impl Fn(i64) -> i64,
) -> AppResult<Vec<BrowserVisit>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![cursor, MAX_VISITS_PER_SCAN as i64], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut visits = Vec::new();
    for row in rows {
        let (raw_time, title, raw_url) = row?;
        let Some(url) = sanitize_url(&raw_url) else {
            continue;
        };
        let timestamp_ms = convert_time(raw_time);
        if timestamp_ms <= 0 || timestamp_ms > now_ms() + 5 * 60_000 {
            continue;
        }
        visits.push(BrowserVisit {
            raw_time,
            timestamp_ms,
            title: sanitize_title(&title),
            url,
        });
    }
    Ok(visits)
}

/// A private short-lived copy avoids browser locks without ever modifying the
/// browser's database. WAL/SHM companions are copied when present.
struct HistorySnapshot {
    conn: Connection,
    path: PathBuf,
}

impl HistorySnapshot {
    fn open(source: &Path) -> AppResult<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("recall-history-{}-{n}.sqlite", std::process::id()));
        std::fs::copy(source, &path).map_err(|e| {
            AppError::Internal(format!("browser history is temporarily unavailable: {e}"))
        })?;
        for suffix in ["-wal", "-shm"] {
            let from = PathBuf::from(format!("{}{suffix}", source.to_string_lossy()));
            if from.is_file() {
                let to = PathBuf::from(format!("{}{suffix}", path.to_string_lossy()));
                let _ = std::fs::copy(from, to);
            }
        }
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        Ok(Self { conn, path })
    }
}

impl Drop for HistorySnapshot {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path.to_string_lossy()));
        }
    }
}

/// Strips credentials, fragments, and values of query parameters commonly
/// used for authentication. Only HTTP(S) page URLs are accepted.
pub fn sanitize_url(raw: &str) -> Option<String> {
    if raw.len() > 16_384 || raw.chars().any(char::is_control) {
        return None;
    }
    let mut url = Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    url.set_fragment(None);
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| {
            let sensitive = matches!(
                k.to_ascii_lowercase().as_str(),
                "access_token"
                    | "auth"
                    | "authorization"
                    | "code"
                    | "key"
                    | "password"
                    | "session"
                    | "sessionid"
                    | "token"
            );
            (
                k.into_owned(),
                if sensitive {
                    "[redacted]".into()
                } else {
                    v.into_owned()
                },
            )
        })
        .collect();
    if !pairs.is_empty() {
        url.query_pairs_mut().clear().extend_pairs(pairs);
    }
    let value = url.to_string();
    (value.len() <= 4_096).then_some(value)
}

fn sanitize_title(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .take(1_024)
        .collect::<String>()
        .trim()
        .to_string()
}

fn provider_roots() -> Vec<Box<dyn BrowserProvider>> {
    #[cfg(not(target_os = "windows"))]
    let home = home_dir();
    #[cfg(target_os = "windows")]
    let (chrome, edge, firefox, safari, safari_supported) = {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_default();
        let roaming = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default();
        (
            vec![local.join("Google/Chrome/User Data")],
            vec![local.join("Microsoft/Edge/User Data")],
            vec![roaming.join("Mozilla/Firefox/Profiles")],
            Vec::new(),
            false,
        )
    };
    #[cfg(target_os = "macos")]
    let (chrome, edge, firefox, safari, safari_supported) = (
        vec![home.join("Library/Application Support/Google/Chrome")],
        vec![home.join("Library/Application Support/Microsoft Edge")],
        vec![home.join("Library/Application Support/Firefox/Profiles")],
        vec![home.join("Library/Safari")],
        true,
    );
    #[cfg(target_os = "linux")]
    let (chrome, edge, firefox, safari, safari_supported) = (
        vec![
            home.join(".config/google-chrome"),
            home.join(".config/chromium"),
        ],
        vec![home.join(".config/microsoft-edge")],
        vec![home.join(".mozilla/firefox")],
        Vec::new(),
        false,
    );

    vec![
        Box::new(SqliteHistoryProvider {
            id: "chrome",
            name: "Google Chrome",
            format: HistoryFormat::Chromium,
            roots: chrome,
            supported: true,
        }),
        Box::new(SqliteHistoryProvider {
            id: "edge",
            name: "Microsoft Edge",
            format: HistoryFormat::Chromium,
            roots: edge,
            supported: true,
        }),
        Box::new(SqliteHistoryProvider {
            id: "firefox",
            name: "Firefox",
            format: HistoryFormat::Firefox,
            roots: firefox,
            supported: true,
        }),
        Box::new(SqliteHistoryProvider {
            id: "safari",
            name: "Safari",
            format: HistoryFormat::Safari,
            roots: safari,
            supported: safari_supported,
        }),
    ]
}

#[cfg(not(target_os = "windows"))]
fn home_dir() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .unwrap_or_default()
}

pub struct BrowserCollector {
    providers: Vec<Box<dyn BrowserProvider>>,
    last_scan: i64,
    statuses: Vec<BrowserStatus>,
    active_providers: HashSet<&'static str>,
}

/// Selected browsers are handled by the browser provider instead of the
/// generic window-title collector. This prevents duplicate timeline entries
/// and, critically, ensures a website exclusion cannot leak the excluded
/// page title through a generic app-activity event.
pub fn handles_window(settings: &Settings, app_name: &str, app_id: Option<&str>) -> bool {
    if !settings.browser_activity_enabled {
        return false;
    }
    let haystack = format!(
        "{} {}",
        app_name.to_ascii_lowercase(),
        app_id.unwrap_or_default().to_ascii_lowercase()
    );
    (settings.browser_chrome_enabled
        && (haystack.contains("google chrome")
            || haystack.contains("google-chrome")
            || haystack.contains("chrome.exe")
            || haystack.contains("/chrome")))
        || (settings.browser_edge_enabled
            && (haystack.contains("microsoft edge")
                || haystack.contains("msedge")
                || haystack.contains("/edge")))
        || (settings.browser_firefox_enabled && haystack.contains("firefox"))
        || (settings.browser_safari_enabled
            && (haystack.contains("safari.app") || app_name.eq_ignore_ascii_case("safari")))
}

impl Default for BrowserCollector {
    fn default() -> Self {
        Self {
            providers: provider_roots(),
            last_scan: 0,
            statuses: Vec::new(),
            active_providers: HashSet::new(),
        }
    }
}

impl BrowserCollector {
    pub fn statuses(&self) -> Vec<BrowserStatus> {
        if self.statuses.is_empty() {
            return self
                .providers
                .iter()
                .map(|p| status_for(p.as_ref(), None))
                .collect();
        }
        self.statuses.clone()
    }

    /// Scans at most every 15 seconds. A disabled/paused collector does not
    /// open browser history at all. When re-enabled, it establishes a new
    /// high-water mark before importing, so disabled activity is never backfilled.
    pub fn scan(
        &mut self,
        db: &Database,
        settings: &Settings,
        filter: &PrivacyFilter,
        now: i64,
        force: bool,
    ) -> AppResult<usize> {
        if !force && now - self.last_scan < SCAN_INTERVAL_MS {
            return Ok(0);
        }
        self.last_scan = now;
        let may_store = settings.recording_enabled
            && settings.browser_activity_enabled
            && !settings.is_paused(now);
        if !may_store {
            self.active_providers.clear();
            return Ok(0);
        }
        let mut inserted = 0;
        let mut statuses = Vec::new();

        for provider in &self.providers {
            let profiles = provider.profiles();
            let mut provider_error = None;
            if !provider.enabled(settings) {
                self.active_providers.remove(provider.id());
                statuses.push(status_for_with_profiles(
                    provider.as_ref(),
                    profiles.len(),
                    None,
                ));
                continue;
            }
            let just_enabled = self.active_providers.insert(provider.id());
            for profile in &profiles {
                let cursor_id = profile.cursor_id();
                let cursor = load_cursor(db, provider.id(), &cursor_id)?;
                if cursor.is_none() || just_enabled {
                    match provider.latest_visit(profile) {
                        Ok(latest) => save_cursor(db, provider.id(), &cursor_id, latest)?,
                        Err(e) => provider_error = Some(e.to_string()),
                    }
                    continue;
                }
                match provider.visits_after(profile, cursor.unwrap_or_default()) {
                    Ok(visits) => {
                        let mut high_water = cursor.unwrap_or_default();
                        for visit in visits {
                            high_water = high_water.max(visit.raw_time);
                            let candidate = Candidate {
                                app_name: Some(provider.name()),
                                app_id: Some(provider.id()),
                                window_title: Some(&visit.title),
                                url: Some(&visit.url),
                                file_path: None,
                            };
                            if filter.excludes(&candidate) {
                                continue;
                            }
                            let event = NewEvent {
                                kind: EventKind::BrowserActivity,
                                source: provider.id(),
                                started_at: visit.timestamp_ms,
                                ended_at: visit.timestamp_ms,
                                app_name: Some(provider.name().into()),
                                app_id: Some(profile.id.clone()),
                                window_title: (!visit.title.is_empty()).then_some(visit.title),
                                url: Some(visit.url),
                                file_path: None,
                            };
                            if store::insert_or_merge_browser_visit(db, &event, SAME_PAGE_MERGE_MS)?
                            {
                                inserted += 1;
                            }
                        }
                        save_cursor(db, provider.id(), &cursor_id, high_water)?;
                    }
                    Err(e) => provider_error = Some(e.to_string()),
                }
            }
            statuses.push(status_for_with_profiles(
                provider.as_ref(),
                profiles.len(),
                provider_error,
            ));
        }
        self.statuses = statuses;
        Ok(inserted)
    }
}

fn status_for(provider: &dyn BrowserProvider, error: Option<String>) -> BrowserStatus {
    let profiles = provider.profiles();
    status_for_with_profiles(provider, profiles.len(), error)
}

fn status_for_with_profiles(
    provider: &dyn BrowserProvider,
    profile_count: usize,
    error: Option<String>,
) -> BrowserStatus {
    let supported = provider.supported();
    BrowserStatus {
        id: provider.id(),
        name: provider.name(),
        supported,
        installed: profile_count > 0,
        profile_count,
        state: if !supported {
            "unsupported"
        } else if error.is_some() {
            "unavailable"
        } else if profile_count > 0 {
            "ready"
        } else {
            "not_installed"
        },
        message: error.map(|_| {
            if provider.id() == "safari" {
                "Safari history is unavailable. Grant Recall Full Disk Access in macOS Settings, then it will retry automatically.".into()
            } else {
                "History is temporarily unavailable. Recall will try again automatically.".into()
            }
        }),
    }
}

fn load_cursor(db: &Database, provider: &str, profile: &str) -> AppResult<Option<i64>> {
    Ok(db
        .conn()
        .query_row(
            "SELECT last_visit FROM browser_cursors WHERE provider_id = ?1 AND profile_id = ?2",
            params![provider, profile],
            |r| r.get(0),
        )
        .optional()?)
}

fn save_cursor(db: &Database, provider: &str, profile: &str, cursor: i64) -> AppResult<()> {
    db.conn().execute(
        "INSERT INTO browser_cursors (provider_id, profile_id, last_visit, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(provider_id, profile_id) DO UPDATE SET
           last_visit = excluded.last_visit, updated_at = excluded.updated_at",
        params![provider, profile, cursor, now_ms()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::privacy::{Exclusion, ExclusionKind};
    use crate::memory::store::{self, EventQuery};

    struct FakeProvider {
        visits: Vec<BrowserVisit>,
    }

    impl BrowserProvider for FakeProvider {
        fn id(&self) -> &'static str {
            "chrome"
        }
        fn name(&self) -> &'static str {
            "Google Chrome"
        }
        fn enabled(&self, s: &Settings) -> bool {
            s.browser_chrome_enabled
        }
        fn profiles(&self) -> Vec<BrowserProfile> {
            vec![BrowserProfile {
                id: "profile".into(),
                history_path: PathBuf::from("Default"),
            }]
        }
        fn latest_visit(&self, _: &BrowserProfile) -> AppResult<i64> {
            Ok(self.visits.last().map(|v| v.raw_time).unwrap_or(0))
        }
        fn visits_after(&self, _: &BrowserProfile, cursor: i64) -> AppResult<Vec<BrowserVisit>> {
            Ok(self
                .visits
                .iter()
                .filter(|v| v.raw_time > cursor)
                .cloned()
                .collect())
        }
    }

    fn visit(raw: i64, title: &str, url: &str) -> BrowserVisit {
        BrowserVisit {
            raw_time: raw,
            timestamp_ms: raw,
            title: title.into(),
            url: url.into(),
        }
    }

    fn collector(visits: Vec<BrowserVisit>) -> BrowserCollector {
        BrowserCollector {
            providers: vec![Box::new(FakeProvider { visits })],
            last_scan: 0,
            statuses: vec![],
            active_providers: HashSet::from(["chrome"]),
        }
    }

    fn enabled() -> Settings {
        Settings {
            recording_enabled: true,
            browser_activity_enabled: true,
            ..Settings::default()
        }
    }

    #[test]
    fn sanitizes_urls_and_secrets() {
        assert_eq!(
            sanitize_url("https://example.com/a?q=ok&token=secret#part").unwrap(),
            "https://example.com/a?q=ok&token=%5Bredacted%5D"
        );
        assert!(sanitize_url("javascript:alert(1)").is_none());
        assert!(sanitize_url("file:///etc/passwd").is_none());
        assert!(sanitize_url("https://user:pass@example.com").is_none());
        assert!(sanitize_url("not a url").is_none());
        assert!(sanitize_url(&format!("https://example.com/{}", "x".repeat(5_000))).is_none());
    }

    #[test]
    fn first_scan_sets_cursor_without_importing_old_history() {
        let db = Database::open_in_memory().unwrap();
        let mut c = collector(vec![visit(10, "Old", "https://old.example/")]);
        assert_eq!(
            c.scan(&db, &enabled(), &PrivacyFilter::default(), 100, true)
                .unwrap(),
            0
        );
        assert_eq!(store::stats(&db).unwrap().total_events, 0);
    }

    #[test]
    fn stores_new_visits_once_and_supports_deletion() {
        let db = Database::open_in_memory().unwrap();
        save_cursor(&db, "chrome", "Default", 10).unwrap();
        let mut c = collector(vec![
            visit(10, "Old", "https://old.example/"),
            visit(20, "SQLite FTS5", "https://sqlite.org/fts5.html"),
        ]);
        assert_eq!(
            c.scan(&db, &enabled(), &PrivacyFilter::default(), 100, true)
                .unwrap(),
            1
        );
        assert_eq!(
            c.scan(&db, &enabled(), &PrivacyFilter::default(), 200, true)
                .unwrap(),
            0
        );
        let events = store::list_events(&db, &EventQuery::default()).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "browser_activity");
        assert_eq!(
            events[0].url.as_deref(),
            Some("https://sqlite.org/fts5.html")
        );
        assert_eq!(store::delete_range(&db, 19, 21).unwrap(), 1);
        assert_eq!(store::stats(&db).unwrap().total_events, 0);
    }

    #[test]
    fn merges_repeated_visits_to_the_same_page() {
        let db = Database::open_in_memory().unwrap();
        save_cursor(&db, "chrome", "Default", 10).unwrap();
        let mut c = collector(vec![
            visit(20, "YouTube", "https://youtube.com/watch?v=1"),
            visit(30, "YouTube", "https://youtube.com/watch?v=1"),
            visit(40, "Different", "https://example.com/"),
        ]);
        assert_eq!(
            c.scan(&db, &enabled(), &PrivacyFilter::default(), 100, true)
                .unwrap(),
            3
        );
        let events = store::list_events(&db, &EventQuery::default()).unwrap();
        assert_eq!(events.len(), 2);
        let youtube = events
            .iter()
            .find(|event| event.url.as_deref() == Some("https://youtube.com/watch?v=1"))
            .unwrap();
        assert_eq!((youtube.started_at, youtube.ended_at), (20, 30));
    }

    #[test]
    fn website_exclusions_drop_url_and_title_together() {
        let db = Database::open_in_memory().unwrap();
        save_cursor(&db, "chrome", "Default", 10).unwrap();
        let filter = PrivacyFilter::new(&[Exclusion {
            id: 1,
            kind: ExclusionKind::Website,
            pattern: "bank.com".into(),
            created_at: 0,
        }]);
        let mut c = collector(vec![visit(
            20,
            "Private bank statement",
            "https://secure.bank.com/account",
        )]);
        assert_eq!(c.scan(&db, &enabled(), &filter, 100, true).unwrap(), 0);
        assert_eq!(store::stats(&db).unwrap().total_events, 0);
    }

    #[test]
    fn pause_advances_cursor_without_storing_or_later_backfill() {
        let db = Database::open_in_memory().unwrap();
        save_cursor(&db, "chrome", "Default", 10).unwrap();
        let mut c = collector(vec![visit(20, "During pause", "https://example.com/")]);
        let paused = Settings {
            paused_until: Some(1_000),
            ..enabled()
        };
        assert_eq!(
            c.scan(&db, &paused, &PrivacyFilter::default(), 100, true)
                .unwrap(),
            0
        );
        assert_eq!(
            c.scan(&db, &enabled(), &PrivacyFilter::default(), 2_000, true)
                .unwrap(),
            0
        );
    }

    #[test]
    fn reads_chromium_firefox_and_safari_schemas() {
        let dir = tempfile::tempdir().unwrap();
        let cases = [
            (
                HistoryFormat::Chromium,
                "History",
                "CREATE TABLE urls(id INTEGER PRIMARY KEY, url TEXT, title TEXT);
                 CREATE TABLE visits(id INTEGER PRIMARY KEY, url INTEGER, visit_time INTEGER);
                 INSERT INTO urls VALUES(1, 'https://chromium.example/page', 'Chromium page');
                 INSERT INTO visits VALUES(1, 1, 11644473600000000 + 42000000);",
                11_644_473_600_000_000 + 42_000_000,
                42_000,
            ),
            (
                HistoryFormat::Firefox,
                "places.sqlite",
                "CREATE TABLE moz_places(id INTEGER PRIMARY KEY, url TEXT, title TEXT);
                 CREATE TABLE moz_historyvisits(id INTEGER PRIMARY KEY, place_id INTEGER, visit_date INTEGER);
                 INSERT INTO moz_places VALUES(1, 'https://firefox.example/page', 'Firefox page');
                 INSERT INTO moz_historyvisits VALUES(1, 1, 42000000);",
                42_000_000,
                42_000,
            ),
            (
                HistoryFormat::Safari,
                "History.db",
                "CREATE TABLE history_items(id INTEGER PRIMARY KEY, url TEXT);
                 CREATE TABLE history_visits(id INTEGER PRIMARY KEY, history_item INTEGER, visit_time REAL, title TEXT);
                 INSERT INTO history_items VALUES(1, 'https://safari.example/page');
                 INSERT INTO history_visits VALUES(1, 1, 42.0, 'Safari page');",
                42,
                SAFARI_EPOCH_OFFSET_MS + 42_000,
            ),
        ];

        for (i, (format, name, schema, raw, expected_ms)) in cases.into_iter().enumerate() {
            let profile_dir = dir.path().join(format!("profile-{i}"));
            std::fs::create_dir(&profile_dir).unwrap();
            let path = profile_dir.join(name);
            Connection::open(&path)
                .unwrap()
                .execute_batch(schema)
                .unwrap();
            let provider = SqliteHistoryProvider {
                id: "test",
                name: "Test",
                format,
                roots: vec![],
                supported: true,
            };
            let profile = BrowserProfile {
                id: "profile".into(),
                history_path: path,
            };
            assert_eq!(provider.latest_visit(&profile).unwrap(), raw);
            let visits = provider.visits_after(&profile, 0).unwrap();
            assert_eq!(visits.len(), 1);
            assert_eq!(visits[0].timestamp_ms, expected_ms);
            assert!(visits[0].url.starts_with("https://"));
        }
    }
}
