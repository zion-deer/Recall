use std::path::Path;

use super::{from_probe, open_folder_with, ActiveWindow, PermissionInfo, PlatformAdapter};
use crate::error::{AppError, AppResult};

const SCREEN_RECORDING: &str = "screen_recording";
const SCREEN_RECORDING_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

const COMBINED_SESSION_STATE: i32 = 0;
const ANY_INPUT_EVENT: u32 = u32::MAX;

pub struct MacAdapter;

impl PlatformAdapter for MacAdapter {
    fn platform_name(&self) -> &'static str {
        "macos"
    }

    fn active_window(&self) -> AppResult<Option<ActiveWindow>> {
        // The recorder runs on a long-lived background thread; drain the
        // Objective-C objects created by each probe so they never accumulate.
        let probe = objc::rc::autoreleasepool(active_win_pos_rs::get_active_window);
        // Without Screen Recording permission macOS omits window titles; the
        // application name is still available.
        Ok(probe.ok().and_then(from_probe))
    }

    fn idle_seconds(&self) -> Option<u64> {
        // SAFETY: pure query with constant arguments.
        let secs = unsafe {
            CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT)
        };
        (secs.is_finite() && secs >= 0.0).then_some(secs as u64)
    }

    fn permissions(&self) -> Vec<PermissionInfo> {
        // SAFETY: no arguments; available on macOS 10.15+.
        let granted = unsafe { CGPreflightScreenCaptureAccess() };
        vec![PermissionInfo {
            id: SCREEN_RECORDING,
            name: "Screen Recording",
            reason: "macOS only shares window titles (like document and page names) with apps that have this permission. Without it, Recall remembers which apps you used but not what was in them.",
            granted: Some(granted),
        }]
    }

    fn request_permission(&self, id: &str) -> AppResult<()> {
        if id != SCREEN_RECORDING {
            return Err(AppError::invalid(format!("Unknown permission: {id}")));
        }
        // SAFETY: no arguments. Shows the system prompt the first time only.
        let granted = unsafe { CGRequestScreenCaptureAccess() };
        if !granted {
            std::process::Command::new("open")
                .arg(SCREEN_RECORDING_SETTINGS)
                .spawn()
                .map_err(|e| AppError::Internal(format!("Could not open System Settings: {e}")))?;
        }
        Ok(())
    }

    fn reveal_folder(&self, path: &Path) -> AppResult<()> {
        open_folder_with("open", path)
    }
}
