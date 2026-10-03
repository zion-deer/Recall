//! The IPC boundary. Every argument arriving here is untrusted and validated
//! before it reaches the core modules.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::ai::{self, llama::AiStatus, AskResponse};
use crate::error::{AppError, AppResult};
use crate::memory::browser::{sanitize_url, BrowserCollector, BrowserStatus};
use crate::memory::export::{self, ExportResult};
use crate::memory::privacy::{self, Exclusion, ExclusionKind};
use crate::memory::recorder::RecorderStatus;
use crate::memory::store::{self, AppUsage, EventQuery, MemoryStats, SearchQuery};
use crate::memory::MemoryEvent;
use crate::platform::PermissionInfo;
use crate::settings::{Settings, PAUSE_INDEFINITELY};
use crate::state::AppState;
use crate::storage::{migrations, now_ms};
use crate::updates::{self, UpdateOffer};

pub const EVENT_MEMORY_CHANGED: &str = "memory:changed";
pub const EVENT_SETTINGS_CHANGED: &str = "settings:changed";

const MAX_PAUSE_MINUTES: u32 = 24 * 60;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: &'static str,
    platform: &'static str,
    platform_limitation: Option<&'static str>,
    data_dir: String,
    log_dir: String,
    schema_version: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddExclusionResult {
    exclusion: Exclusion,
    removed_memories: usize,
}

fn memory_changed(app: &AppHandle) {
    let _ = app.emit(EVENT_MEMORY_CHANGED, ());
}

fn settings_changed(app: &AppHandle, s: &Settings) {
    let _ = app.emit(EVENT_SETTINGS_CHANGED, s);
}

fn validate_id(id: i64) -> AppResult<i64> {
    if id <= 0 {
        return Err(AppError::invalid("Invalid memory id"));
    }
    Ok(id)
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        platform: state.platform.platform_name(),
        platform_limitation: state.platform.limitation(),
        data_dir: state.data_dir.to_string_lossy().into_owned(),
        log_dir: state.log_dir.to_string_lossy().into_owned(),
        schema_version: migrations::current_version(&state.db.conn())?,
    })
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> AppResult<Settings> {
    let saved = state.save_settings(settings)?;
    settings_changed(&app, &saved);
    Ok(saved)
}

/// Pauses recording for `minutes`, or until resumed when `minutes` is absent.
#[tauri::command]
pub fn pause_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    minutes: Option<u32>,
) -> AppResult<Settings> {
    let until = match minutes {
        None => PAUSE_INDEFINITELY,
        Some(m) if (1..=MAX_PAUSE_MINUTES).contains(&m) => now_ms() + i64::from(m) * 60_000,
        Some(_) => {
            return Err(AppError::invalid(
                "Pause must be between 1 minute and 24 hours",
            ))
        }
    };
    let saved = state.set_paused_until(Some(until))?;
    settings_changed(&app, &saved);
    Ok(saved)
}

#[tauri::command]
pub fn resume_recording(app: AppHandle, state: State<'_, AppState>) -> AppResult<Settings> {
    let saved = state.set_paused_until(None)?;
    settings_changed(&app, &saved);
    Ok(saved)
}

#[tauri::command]
pub fn get_recorder_status(state: State<'_, AppState>) -> RecorderStatus {
    state.recorder.status()
}

#[tauri::command]
pub fn get_browser_statuses(state: State<'_, AppState>) -> Vec<BrowserStatus> {
    let statuses = state
        .config
        .browser_statuses
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if statuses.is_empty() {
        BrowserCollector::default().statuses()
    } else {
        statuses
    }
}

#[tauri::command]
pub fn list_events(state: State<'_, AppState>, query: EventQuery) -> AppResult<Vec<MemoryEvent>> {
    store::list_events(&state.db, &query)
}

#[tauri::command]
pub fn search_events(
    state: State<'_, AppState>,
    query: SearchQuery,
) -> AppResult<Vec<MemoryEvent>> {
    store::search_events(&state.db, &query)
}

#[tauri::command]
pub fn get_event(state: State<'_, AppState>, id: i64) -> AppResult<MemoryEvent> {
    store::get_event(&state.db, validate_id(id)?)?
        .ok_or_else(|| AppError::NotFound("That memory no longer exists".into()))
}

#[tauri::command]
pub fn get_app_usage(state: State<'_, AppState>, start: i64, end: i64) -> AppResult<Vec<AppUsage>> {
    store::app_usage(&state.db, start, end)
}

#[tauri::command]
pub fn get_memory_stats(state: State<'_, AppState>) -> AppResult<MemoryStats> {
    store::stats(&state.db)
}

#[tauri::command]
pub fn delete_event(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<bool> {
    let deleted = store::delete_event(&state.db, validate_id(id)?)?;
    state.recorder.reset();
    memory_changed(&app);
    Ok(deleted)
}

#[tauri::command]
pub fn delete_events_in_range(
    app: AppHandle,
    state: State<'_, AppState>,
    start: i64,
    end: i64,
) -> AppResult<usize> {
    let n = store::delete_range(&state.db, start, end)?;
    state.recorder.reset();
    memory_changed(&app);
    log::info!("user deleted {n} memories in a time range");
    Ok(n)
}

#[tauri::command]
pub fn delete_screenshots(app: AppHandle, state: State<'_, AppState>) -> AppResult<usize> {
    let n = store::delete_screenshots(&state.db)?;
    state.recorder.reset();
    memory_changed(&app);
    Ok(n)
}

#[tauri::command]
pub fn delete_all_memories(app: AppHandle, state: State<'_, AppState>) -> AppResult<usize> {
    state.recorder.reset();
    let n = store::delete_all(&state.db)?;
    state.recorder.reset();
    memory_changed(&app);
    log::info!("user deleted all memories ({n})");
    Ok(n)
}

#[tauri::command]
pub fn list_exclusions(state: State<'_, AppState>) -> AppResult<Vec<Exclusion>> {
    privacy::list(&state.db)
}

/// Adds an exclusion and removes any existing memories that match it.
#[tauri::command]
pub fn add_exclusion(
    app: AppHandle,
    state: State<'_, AppState>,
    kind: ExclusionKind,
    pattern: String,
) -> AppResult<AddExclusionResult> {
    let exclusion = privacy::add(&state.db, kind, &pattern)?;
    state.reload_filter()?;
    let only_new = privacy::PrivacyFilter::new(std::slice::from_ref(&exclusion));
    let removed = privacy::purge_matching(&state.db, &only_new)?;
    if removed > 0 {
        state.recorder.reset();
        memory_changed(&app);
    }
    Ok(AddExclusionResult {
        exclusion,
        removed_memories: removed,
    })
}

#[tauri::command]
pub fn remove_exclusion(state: State<'_, AppState>, id: i64) -> AppResult<bool> {
    let removed = privacy::remove(&state.db, validate_id(id)?)?;
    state.reload_filter()?;
    Ok(removed)
}

#[tauri::command]
pub fn export_data(state: State<'_, AppState>, path: String) -> AppResult<ExportResult> {
    let result = export::export_to(&state.db, &path, env!("CARGO_PKG_VERSION"))?;
    log::info!("exported {} memories", result.event_count);
    Ok(result)
}

#[tauri::command]
pub fn get_permissions(state: State<'_, AppState>) -> Vec<PermissionInfo> {
    state.platform.permissions()
}

#[tauri::command]
pub fn request_permission(state: State<'_, AppState>, id: String) -> AppResult<()> {
    if id.len() > 64 {
        return Err(AppError::invalid("Unknown permission"));
    }
    state.platform.request_permission(&id)
}

#[tauri::command]
pub fn open_data_folder(state: State<'_, AppState>) -> AppResult<()> {
    state.platform.reveal_folder(&state.data_dir)
}

#[tauri::command]
pub fn open_log_folder(state: State<'_, AppState>) -> AppResult<()> {
    state.platform.reveal_folder(&state.log_dir)
}

#[tauri::command]
pub fn open_path(path: String) -> AppResult<()> {
    use tauri_plugin_opener::open_path;
    let path = std::path::PathBuf::from(&path);
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(AppError::invalid("That file path is not safe to open"));
    }
    if !path.is_file() {
        return Err(AppError::NotFound(
            "That file is no longer available".into(),
        ));
    }
    open_path(path, None::<&str>)
        .map_err(|e| AppError::Internal(format!("Could not open the file: {e}")))
}

#[tauri::command]
pub fn get_ai_status(state: State<'_, AppState>) -> AiStatus {
    let mut status = state.ai.status();
    if !state.settings().ai_enabled
        && matches!(
            status.phase,
            crate::ai::llama::AiPhase::Ready | crate::ai::llama::AiPhase::NotInstalled
        )
    {
        status.message = Some("Local AI is turned off.".into());
    }
    status
}

#[tauri::command]
pub async fn download_ai_model(state: State<'_, AppState>) -> AppResult<()> {
    state.ai.download().await
}

#[tauri::command]
pub fn cancel_ai_download(state: State<'_, AppState>) {
    state.ai.cancel();
}

#[tauri::command]
pub fn remove_ai_model(state: State<'_, AppState>) -> AppResult<()> {
    state.ai.remove()
}

#[tauri::command]
pub async fn ask_recall(state: State<'_, AppState>, question: String) -> AppResult<AskResponse> {
    let db = state.db.clone();
    let settings = state.settings();
    let engine = state.ai.clone();
    tauri::async_runtime::spawn_blocking(move || ai::ask(&db, &settings, &engine, &question))
        .await
        .map_err(|_| AppError::Internal("The AI request was interrupted".into()))?
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> AppResult<Option<UpdateOffer>> {
    updates::check(&app).await
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> AppResult<()> {
    updates::install(&app).await
}

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let safe =
        sanitize_url(&url).ok_or_else(|| AppError::invalid("That URL is not safe to open"))?;
    app.opener()
        .open_url(safe, None::<&str>)
        .map_err(|e| AppError::Internal(format!("Could not open the website: {e}")))
}

#[cfg(test)]
mod tests {
    use crate::memory::privacy::ExclusionKind;
    use crate::memory::store::EventQuery;
    use crate::settings::{Settings, PAUSE_INDEFINITELY};

    #[test]
    fn event_query_accepts_camel_case_and_nulls() {
        let q: EventQuery =
            serde_json::from_str(r#"{"start":1,"end":2,"appName":"Code","kind":null,"limit":10}"#)
                .unwrap();
        assert_eq!(q.app_name.as_deref(), Some("Code"));
        assert_eq!(q.limit, Some(10));
        let empty: EventQuery = serde_json::from_str("{}").unwrap();
        assert!(empty.start.is_none());
    }

    #[test]
    fn event_query_rejects_wrong_types() {
        assert!(serde_json::from_str::<EventQuery>(r#"{"limit":-1}"#).is_err());
        assert!(serde_json::from_str::<EventQuery>(r#"{"start":"yesterday"}"#).is_err());
    }

    #[test]
    fn exclusion_kind_is_a_closed_set() {
        assert_eq!(
            serde_json::from_str::<ExclusionKind>(r#""website""#).unwrap(),
            ExclusionKind::Website
        );
        assert!(serde_json::from_str::<ExclusionKind>(r#""shell""#).is_err());
    }

    #[test]
    fn settings_round_trip_through_javascript_numbers() {
        let s = Settings {
            paused_until: Some(PAUSE_INDEFINITELY),
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        // JavaScript parses numbers as f64; the sentinel must survive unchanged.
        let as_f64: f64 = serde_json::from_str::<serde_json::Value>(&json).unwrap()["pausedUntil"]
            .as_f64()
            .unwrap();
        assert_eq!(as_f64 as i64, PAUSE_INDEFINITELY);
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
