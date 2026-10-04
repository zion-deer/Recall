mod ai;
mod commands;
mod error;
mod memory;
mod platform;
mod settings;
mod state;
mod storage;
mod tray;
mod updates;

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use memory::recorder::{Notifier, RecorderHandle, RecorderStatus};
use state::AppState;
use storage::Database;

pub const EVENT_RECORDER_STATUS: &str = "recorder:status";

struct TauriNotifier(AppHandle);

impl Notifier for TauriNotifier {
    fn status_changed(&self, status: &RecorderStatus) {
        let _ = self.0.emit(EVENT_RECORDER_STATUS, status);
        if let Some(menu) = self.0.try_state::<tray::TrayMenu>() {
            tray::update(&menu, status);
        }
    }

    fn memory_changed(&self) {
        let _ = self.0.emit(commands::EVENT_MEMORY_CHANGED, ());
    }
}

fn init(app: &AppHandle) -> Result<AppState, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let log_dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    let db = Arc::new(Database::open(&data_dir.join("recall.db")).map_err(|e| e.to_string())?);
    let config = Arc::new(AppState::load_config(&db).map_err(|e| e.to_string())?);
    let platform: Arc<dyn platform::PlatformAdapter> = Arc::from(platform::current());
    let screenshots_dir = data_dir.join("screenshots");
    let recorder = RecorderHandle::spawn(
        db.clone(),
        config.clone(),
        platform.clone(),
        screenshots_dir,
        TauriNotifier(app.clone()),
    );
    let ai = Arc::new(ai::llama::AiEngine::new(data_dir.join("models")));
    Ok(AppState {
        db,
        config,
        recorder,
        platform,
        ai,
        data_dir,
        log_dir,
    })
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("unknown");
        log::error!("panic at {location}: {payload}");
        default_hook(info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("recall".into()),
                    }),
                ])
                .level(log::LevelFilter::Info)
                .max_file_size(5 * 1024 * 1024)
                .rotation_strategy(RotationStrategy::KeepSome(5))
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            log::info!("Recall {} starting", env!("CARGO_PKG_VERSION"));
            let handle = app.handle().clone();
            match init(&handle) {
                Ok(state) => {
                    app.manage(state);
                }
                Err(e) => {
                    log::error!("startup failed: {e}");
                    handle
                        .dialog()
                        .message(format!("Recall couldn't open its memory database.\n\n{e}"))
                        .title("Recall couldn't start")
                        .kind(MessageDialogKind::Error)
                        .blocking_show();
                    std::process::exit(1);
                }
            }
            let update_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                // Let the window and first memory query finish before the update request.
                tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                match updates::check(&update_handle).await {
                    Ok(Some(update)) => {
                        let _ = update_handle.emit("update:available", update);
                    }
                    Ok(None) => {}
                    Err(error) => log::info!("update check skipped: {error}"),
                }
            });
            match tray::create(&handle) {
                Ok(menu) => {
                    app.manage(menu);
                }
                Err(e) => log::warn!("system tray unavailable: {e}"),
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // With a tray icon, closing the window keeps Recall running in the background.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.app_handle().try_state::<tray::TrayMenu>().is_some() {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_settings,
            commands::update_settings,
            commands::pause_recording,
            commands::resume_recording,
            commands::get_recorder_status,
            commands::get_browser_statuses,
            commands::list_events,
            commands::search_events,
            commands::get_event,
            commands::get_app_usage,
            commands::get_memory_stats,
            commands::delete_event,
            commands::delete_events_in_range,
            commands::delete_screenshots,
            commands::delete_all_memories,
            commands::list_exclusions,
            commands::add_exclusion,
            commands::remove_exclusion,
            commands::export_data,
            commands::get_permissions,
            commands::request_permission,
            commands::open_data_folder,
            commands::open_log_folder,
            commands::open_url,
            commands::open_path,
            commands::get_ai_status,
            commands::download_ai_model,
            commands::cancel_ai_download,
            commands::remove_ai_model,
            commands::ask_recall,
            commands::get_app_icon,
            commands::check_for_update,
            commands::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Recall");

    app.run(|app, event| match event {
        RunEvent::Exit => {
            if let Some(state) = app.try_state::<AppState>() {
                state.recorder.stop();
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => tray::show_main_window(app),
        _ => {}
    });
}
