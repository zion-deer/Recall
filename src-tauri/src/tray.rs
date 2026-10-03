//! System tray (menu bar on macOS): lets Recall keep recording with the window
//! closed, and keeps pause one click away.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::commands::EVENT_SETTINGS_CHANGED;
use crate::memory::recorder::{RecorderState, RecorderStatus};
use crate::settings::PAUSE_INDEFINITELY;
use crate::state::AppState;
use crate::storage::now_ms;

pub struct TrayMenu {
    status: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<TrayMenu> {
    let status = MenuItem::with_id(app, "status", "Starting…", false, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Pause recording", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Recall", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Recall", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &toggle,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Recall")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => toggle_pause(app),
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(TrayMenu { status, toggle })
}

pub fn update(menu: &TrayMenu, status: &RecorderStatus) {
    let label = match status.state {
        RecorderState::Recording => "Recording",
        RecorderState::Paused => "Recording paused",
        RecorderState::Off => "Recording is off",
        RecorderState::Idle => "Waiting — you're away",
        RecorderState::Excluded | RecorderState::RecallFocused => "Not recording this window",
        RecorderState::NoWindow | RecorderState::Starting => "Recording",
        RecorderState::Unavailable => "Activity tracking unavailable",
    };
    let toggle = if status.state == RecorderState::Paused {
        "Resume recording"
    } else {
        "Pause recording"
    };
    let _ = menu.status.set_text(label);
    let _ = menu.toggle.set_text(toggle);
    let _ = menu.toggle.set_enabled(status.state != RecorderState::Off);
}

fn toggle_pause(app: &AppHandle) {
    let state = app.state::<AppState>();
    let paused = state.settings().is_paused(now_ms());
    let result = state.set_paused_until(if paused {
        None
    } else {
        Some(PAUSE_INDEFINITELY)
    });
    match result {
        Ok(s) => {
            let _ = app.emit(EVENT_SETTINGS_CHANGED, &s);
        }
        Err(e) => log::error!("could not toggle pause from tray: {e}"),
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
