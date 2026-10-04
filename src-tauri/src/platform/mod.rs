//! Platform abstraction. All OS-specific behavior lives behind
//! [`PlatformAdapter`]; the rest of Recall never calls OS APIs directly.

pub mod icons;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::path::Path;

use serde::Serialize;

use crate::error::{AppError, AppResult};

/// The window the user is currently focused on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWindow {
    pub app_name: String,
    /// Executable path (Windows/Linux) or app bundle path (macOS).
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub process_id: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub reason: &'static str,
    /// `None` when the OS cannot report the status.
    pub granted: Option<bool>,
}

pub trait PlatformAdapter: Send + Sync {
    fn platform_name(&self) -> &'static str;

    /// Returns `Ok(None)` when there is no focused application window.
    fn active_window(&self) -> AppResult<Option<ActiveWindow>>;

    /// Seconds since the last keyboard or mouse input, if the OS reports it.
    fn idle_seconds(&self) -> Option<u64>;

    /// OS permissions that affect what Recall can record.
    fn permissions(&self) -> Vec<PermissionInfo>;

    /// Asks the OS for a permission, or opens the relevant system settings page.
    fn request_permission(&self, id: &str) -> AppResult<()>;

    /// A short note when activity tracking is limited on this platform.
    fn limitation(&self) -> Option<&'static str> {
        None
    }

    /// Shows a Recall-owned folder (data, logs) in the system file manager.
    fn reveal_folder(&self, path: &Path) -> AppResult<()>;
}

pub fn current() -> Box<dyn PlatformAdapter> {
    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsAdapter);
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacAdapter);
    #[cfg(target_os = "linux")]
    return Box::new(linux::LinuxAdapter::new());
}

/// Converts the cross-platform window probe into our model.
fn from_probe(w: active_win_pos_rs::ActiveWindow) -> Option<ActiveWindow> {
    let app_id = w.process_path.to_string_lossy().trim().to_string();
    let mut app_name = w.app_name.trim().to_string();
    if app_name.is_empty() {
        app_name = w
            .process_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    if app_name.is_empty() {
        return None;
    }
    let title = w.title.trim();
    Some(ActiveWindow {
        app_name,
        app_id: (!app_id.is_empty()).then_some(app_id),
        title: (!title.is_empty()).then(|| title.to_string()),
        process_id: w.process_id,
    })
}

/// Opens a folder with the OS file manager. The path is passed as a single
/// argument (never through a shell) and must be an existing directory.
fn open_folder_with(program: &str, path: &Path) -> AppResult<()> {
    if !path.is_dir() {
        return Err(AppError::NotFound("That folder does not exist yet".into()));
    }
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("Could not open the folder: {e}")))
}
