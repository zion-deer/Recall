//! Development-only adapter. Linux is not a supported release platform yet;
//! this exists so Recall can be built and exercised on Linux (X11) machines.

use std::path::Path;

use super::{from_probe, open_folder_with, ActiveWindow, PermissionInfo, PlatformAdapter};
use crate::error::{AppError, AppResult};

pub struct LinuxAdapter {
    has_x11: bool,
}

impl LinuxAdapter {
    pub fn new() -> Self {
        let has_x11 = std::env::var_os("DISPLAY").is_some();
        Self { has_x11 }
    }
}

impl PlatformAdapter for LinuxAdapter {
    fn platform_name(&self) -> &'static str {
        "linux"
    }

    fn active_window(&self) -> AppResult<Option<ActiveWindow>> {
        if !self.has_x11 {
            return Err(AppError::Unsupported(
                "Activity tracking needs an X11 session on Linux".into(),
            ));
        }
        Ok(active_win_pos_rs::get_active_window().ok().and_then(from_probe))
    }

    fn idle_seconds(&self) -> Option<u64> {
        None
    }

    fn permissions(&self) -> Vec<PermissionInfo> {
        Vec::new()
    }

    fn request_permission(&self, id: &str) -> AppResult<()> {
        Err(AppError::invalid(format!("Unknown permission: {id}")))
    }

    fn limitation(&self) -> Option<&'static str> {
        Some("Linux support is experimental: idle detection is unavailable and Wayland sessions are not supported.")
    }

    fn reveal_folder(&self, path: &Path) -> AppResult<()> {
        open_folder_with("xdg-open", path)
    }
}
