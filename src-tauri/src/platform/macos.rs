use std::path::Path;

use objc::{class, msg_send, sel, sel_impl};

use super::{from_probe, open_folder_with, ActiveWindow, PermissionInfo, PlatformAdapter};
use crate::error::{AppError, AppResult};

const SCREEN_RECORDING: &str = "screen_recording";
const SCREEN_RECORDING_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
const FULL_DISK_ACCESS: &str = "full_disk_access";
const FULL_DISK_ACCESS_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
    fn CGPreflightScreenCaptureAccess() -> bool;
}

const COMBINED_SESSION_STATE: i32 = 0;
const ANY_INPUT_EVENT: u32 = u32::MAX;

pub struct MacAdapter;

impl PlatformAdapter for MacAdapter {
    fn platform_name(&self) -> &'static str {
        "macos"
    }

    fn active_window(&self) -> AppResult<Option<ActiveWindow>> {
        // Listing windows asks macOS for Screen Recording and raises the
        // Deny / System Settings dialog. Until that access is already on,
        // only the frontmost app name is read, which does not prompt.
        if !screen_capture_granted() {
            return Ok(frontmost_app());
        }
        let probe = objc::rc::autoreleasepool(active_win_pos_rs::get_active_window);
        Ok(probe.ok().and_then(from_probe))
    }

    fn screen_capture_allowed(&self) -> bool {
        screen_capture_granted()
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
        vec![
            PermissionInfo {
                id: SCREEN_RECORDING,
                name: "Screen Recording",
                reason: "macOS only shares window titles (like document and page names) with apps that have this permission. Without it, Recall remembers which apps you used but not what was in them.",
                granted: Some(granted),
            },
            PermissionInfo {
                id: FULL_DISK_ACCESS,
                name: "Full Disk Access",
                reason: "Safari protects its local history with this macOS permission. It is needed only to remember Safari pages; Chrome, Edge, Firefox, and app activity work without it.",
                // macOS has no reliable public API for querying this permission.
                granted: None,
            },
        ]
    }

    fn request_permission(&self, id: &str) -> AppResult<()> {
        if id == FULL_DISK_ACCESS {
            std::process::Command::new("open")
                .arg(FULL_DISK_ACCESS_SETTINGS)
                .spawn()
                .map_err(|e| AppError::Internal(format!("Could not open System Settings: {e}")))?;
            return Ok(());
        }
        if id != SCREEN_RECORDING {
            return Err(AppError::invalid(format!("Unknown permission: {id}")));
        }
        // Do not call CGRequestScreenCaptureAccess. That is the dialog with
        // Deny and Open System Settings. Open the settings page only when
        // the user taps Enable inside Recall.
        if !screen_capture_granted() {
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

fn screen_capture_granted() -> bool {
    // SAFETY: no arguments; available on macOS 10.15+. Does not prompt.
    unsafe { CGPreflightScreenCaptureAccess() }
}

/// Frontmost app via NSWorkspace. This does not request Screen Recording.
#[allow(unexpected_cfgs)]
fn frontmost_app() -> Option<ActiveWindow> {
    use objc::runtime::Object;
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let name = nsstring_to_string(msg_send![app, localizedName]);
        if name.is_empty() {
            return None;
        }
        let bundle: *mut Object = msg_send![app, bundleURL];
        let app_id = if bundle.is_null() {
            None
        } else {
            let path = nsstring_to_string(msg_send![bundle, path]);
            (!path.is_empty()).then_some(path)
        };
        let pid: i32 = msg_send![app, processIdentifier];
        Some(ActiveWindow {
            app_name: name,
            app_id,
            title: None,
            process_id: pid.max(0) as u64,
        })
    }
}

#[allow(unexpected_cfgs)]
fn nsstring_to_string(value: *mut objc::runtime::Object) -> String {
    if value.is_null() {
        return String::new();
    }
    unsafe {
        let bytes: *const i8 = msg_send![value, UTF8String];
        if bytes.is_null() {
            return String::new();
        }
        std::ffi::CStr::from_ptr(bytes)
            .to_string_lossy()
            .into_owned()
    }
}
