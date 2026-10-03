//! The IPC boundary. Every argument arriving here is untrusted and validated
//! before it reaches the core modules.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, AppResult};
use crate::memory::export::{self, ExportResult};
use crate::memory::privacy::{self, Exclusion, ExclusionKind};
use crate::memory::recorder::RecorderStatus;
use crate::memory::store::{self, AppUsage, EventQuery, MemoryStats};
use crate::memory::MemoryEvent;
use crate::platform::PermissionInfo;
use crate::settings::Settings;
use crate::state::AppState;
use crate::storage::{migrations, now_ms};

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
        None => i64::MAX,
        Some(m) if (1..=MAX_PAUSE_MINUTES).contains(&m) => now_ms() + i64::from(m) * 60_000,
        Some(_) => return Err(AppError::invalid("Pause must be between 1 minute and 24 hours")),
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
pub fn list_events(state: State<'_, AppState>, query: EventQuery) -> AppResult<Vec<MemoryEvent>> {
    store::list_events(&state.db, &query)
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
    Ok(AddExclusionResult { exclusion, removed_memories: removed })
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
