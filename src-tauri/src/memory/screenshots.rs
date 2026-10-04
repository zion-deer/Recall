//! Optional local screenshots. Images stay in the app data directory; the
//! memory database stores only their metadata and path.

use std::path::{Component, Path};

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};
use sha2::{Digest, Sha256};

use super::privacy::{Candidate, PrivacyFilter};
use super::{store, EventKind, NewEvent};
use crate::error::{AppError, AppResult};
use crate::platform::PlatformAdapter;
use crate::settings::Settings;
use crate::storage::{now_ms, Database};

pub const SOURCE: &str = "screenshot";
const MAX_EDGE: u32 = 1600;
const QUALITY: u8 = 60;

pub struct ScreenshotCollector {
    last_at: i64,
    last_window: Option<String>,
    directory: std::path::PathBuf,
    own_pid: u64,
}

impl ScreenshotCollector {
    pub fn new(directory: std::path::PathBuf, own_pid: u64) -> Self {
        Self {
            last_at: 0,
            last_window: None,
            directory,
            own_pid,
        }
    }

    pub fn maybe_capture(
        &mut self,
        db: &Database,
        settings: &Settings,
        filter: &PrivacyFilter,
        platform: &dyn PlatformAdapter,
        now: i64,
    ) -> AppResult<bool> {
        if !settings.recording_enabled || !settings.screenshots_enabled || settings.is_paused(now) {
            return Ok(false);
        }
        let window = platform.active_window().unwrap_or(None);
        let window_key = window.as_ref().map(|w| {
            format!(
                "{}|{}|{}",
                w.app_name,
                w.app_id.as_deref().unwrap_or(""),
                w.title.as_deref().unwrap_or("")
            )
        });
        if settings.screenshot_interval_secs == 0 {
            if self.last_window == window_key {
                return Ok(false);
            }
        } else {
            let interval_ms = i64::from(settings.screenshot_interval_secs) * 1000;
            if now.saturating_sub(self.last_at) < interval_ms {
                return Ok(false);
            }
        }
        if platform
            .idle_seconds()
            .is_some_and(|idle| idle >= u64::from(settings.idle_threshold_secs))
        {
            return Ok(false);
        }
        if let Some(window) = &window {
            if window.process_id == self.own_pid {
                self.last_window = window_key;
                return Ok(false);
            }
            let candidate = Candidate {
                app_name: Some(&window.app_name),
                app_id: window.app_id.as_deref(),
                window_title: window.title.as_deref(),
                url: None,
                file_path: None,
            };
            if filter.excludes(&candidate) {
                self.last_window = window_key;
                return Ok(false);
            }
        }

        // Advance even when capture fails so a broken display server cannot
        // turn the recorder into a tight retry loop.
        self.last_at = now;
        self.last_window = window_key;
        if !platform.screen_capture_allowed() {
            return Ok(false);
        }
        let jpeg = capture_jpeg().inspect_err(|_| {
            log::warn!("screenshot capture failed");
        })?;
        std::fs::create_dir_all(&self.directory)?;
        let path = self
            .directory
            .join(format!("{now}-{}.jpg", now_ms() % 1_000_000));
        std::fs::write(&path, &jpeg)?;
        let hash = hex_encode(&Sha256::digest(&jpeg));
        let metadata = serde_json::json!({
            "bytes": jpeg.len(),
            "sha256": hash,
        })
        .to_string();
        let id = store::insert_event(
            db,
            &NewEvent {
                kind: EventKind::Screenshot,
                source: SOURCE,
                started_at: now,
                ended_at: now,
                app_name: window.as_ref().map(|w| w.app_name.clone()),
                app_id: window.as_ref().and_then(|w| w.app_id.clone()),
                window_title: window.as_ref().and_then(|w| w.title.clone()),
                url: None,
                file_path: Some(path.to_string_lossy().into_owned()),
            },
        )?;
        db.conn().execute(
            "UPDATE events SET metadata = ?1 WHERE id = ?2",
            rusqlite::params![metadata, id],
        )?;
        Ok(true)
    }
}

fn capture_jpeg() -> AppResult<Vec<u8>> {
    let monitors = xcap::Monitor::all()
        .map_err(|e| AppError::Internal(format!("screen capture is unavailable: {e}")))?;
    let monitor = monitors
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Unsupported("No display was found".into()))?;
    let image = monitor
        .capture_image()
        .map_err(|e| AppError::Internal(format!("screen capture failed: {e}")))?;
    let (width, height) = image.dimensions();
    let longest = width.max(height).max(1);
    let rgb = if longest > MAX_EDGE {
        let scale = f64::from(MAX_EDGE) / f64::from(longest);
        image::imageops::resize(
            &image,
            ((f64::from(width) * scale).round() as u32).max(1),
            ((f64::from(height) * scale).round() as u32).max(1),
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    let rgb = image::DynamicImage::ImageRgba8(rgb).to_rgb8();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, QUALITY)
        .write_image(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            ExtendedColorType::Rgb8,
        )
        .map_err(|e| AppError::Internal(format!("Could not compress the screenshot: {e}")))?;
    Ok(bytes)
}

/// Deletes a screenshot image only when the stored path is a plain JPEG inside
/// a directory named `screenshots`. Memory deletion cannot be turned into
/// arbitrary file deletion.
pub fn remove_stored_file(path: &str) {
    let path = Path::new(path);
    let managed = path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, Component::ParentDir))
        && path.extension().and_then(|ext| ext.to_str()) == Some("jpg")
        && path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            == Some("screenshots");
    if managed {
        if let Err(error) = std::fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("could not delete screenshot file: {error}");
            }
        }
    } else if !path.as_os_str().is_empty() {
        log::warn!("refused to delete a file outside screenshot storage");
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_managed_jpeg_files() {
        let dir = tempfile::tempdir().unwrap();
        let shots = dir.path().join("screenshots");
        std::fs::create_dir(&shots).unwrap();
        let image = shots.join("1.jpg");
        std::fs::write(&image, b"jpeg").unwrap();
        let other = dir.path().join("notes.txt");
        std::fs::write(&other, b"keep").unwrap();

        remove_stored_file(image.to_str().unwrap());
        remove_stored_file(other.to_str().unwrap());
        remove_stored_file("../secrets.jpg");
        assert!(!image.exists());
        assert_eq!(std::fs::read(&other).unwrap(), b"keep");
    }

    #[test]
    fn captures_a_jpeg_when_a_display_exists() {
        if std::env::var_os("DISPLAY").is_none() {
            return;
        }
        let bytes = capture_jpeg().expect("screen capture");
        assert!(bytes.starts_with(&[0xff, 0xd8]), "output is a JPEG");
        assert!(bytes.len() > 1000);
    }
}
