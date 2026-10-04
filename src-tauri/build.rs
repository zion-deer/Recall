// Every IPC command must be listed here AND granted in capabilities/default.json.
// Commands that are not granted cannot be called from the webview.
const COMMANDS: &[&str] = &[
    "get_app_info",
    "get_settings",
    "update_settings",
    "pause_recording",
    "resume_recording",
    "get_recorder_status",
    "get_browser_statuses",
    "list_events",
    "search_events",
    "get_event",
    "get_app_usage",
    "get_memory_stats",
    "delete_event",
    "delete_events_in_range",
    "delete_screenshots",
    "delete_all_memories",
    "list_exclusions",
    "add_exclusion",
    "remove_exclusion",
    "export_data",
    "get_permissions",
    "request_permission",
    "open_data_folder",
    "open_log_folder",
    "open_url",
    "open_path",
    "get_ai_status",
    "download_ai_model",
    "cancel_ai_download",
    "remove_ai_model",
    "ask_recall",
    "get_app_icon",
    "check_for_update",
    "install_update",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run tauri build script");
}
