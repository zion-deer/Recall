use std::path::Path;

use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

use super::{from_probe, open_folder_with, ActiveWindow, PermissionInfo, PlatformAdapter};
use crate::error::{AppError, AppResult};

pub struct WindowsAdapter;

impl PlatformAdapter for WindowsAdapter {
    fn platform_name(&self) -> &'static str {
        "windows"
    }

    fn active_window(&self) -> AppResult<Option<ActiveWindow>> {
        // Fails when the foreground window is the desktop, the lock screen, or
        // an elevated process we are not allowed to inspect. That is "no window".
        Ok(active_win_pos_rs::get_active_window()
            .ok()
            .and_then(from_probe))
    }

    fn idle_seconds(&self) -> Option<u64> {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: `info` is a properly sized, initialized LASTINPUTINFO.
        let ok = unsafe { GetLastInputInfo(&mut info) } != 0;
        if !ok {
            return None;
        }
        // SAFETY: no preconditions.
        let now = unsafe { GetTickCount() };
        Some(u64::from(now.wrapping_sub(info.dwTime)) / 1000)
    }

    fn permissions(&self) -> Vec<PermissionInfo> {
        // Windows does not gate foreground-window information behind a permission.
        Vec::new()
    }

    fn request_permission(&self, id: &str) -> AppResult<()> {
        Err(AppError::invalid(format!("Unknown permission: {id}")))
    }

    fn reveal_folder(&self, path: &Path) -> AppResult<()> {
        open_folder_with("explorer.exe", path)
    }
}
