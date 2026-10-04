//! Release comparison and update offers.
//!
//! The installed app only accepts updater packages whose minisign signature
//! matches the public key compiled into it. A failed download or signature
//! check leaves the current installation unchanged.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOffer {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(value: &str) -> Vec<u64> {
        value
            .trim()
            .trim_start_matches('v')
            .split(|c: char| !c.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse().unwrap_or(0))
            .take(3)
            .collect()
    }
    let next = parts(candidate);
    let installed = parts(current);
    next > installed
}

pub async fn check(app: &AppHandle) -> AppResult<Option<UpdateOffer>> {
    let updater = app
        .updater()
        .map_err(|e| AppError::Internal(format!("Updater is unavailable: {e}")))?;
    match updater.check().await {
        Ok(Some(update)) if is_newer(&update.version, &update.current_version) => {
            Ok(Some(UpdateOffer {
                current_version: update.current_version.clone(),
                version: update.version.clone(),
                notes: update.body.clone(),
            }))
        }
        Ok(None) | Ok(Some(_)) => Ok(None),
        Err(error) => {
            let text = error.to_string();
            // Releases only publish macOS and Windows. Other systems have nothing to install.
            if text.contains("platforms") {
                log::info!("no update package for this system");
                return Ok(None);
            }
            log::warn!("update check failed: {text}");
            Err(AppError::Internal(
                "Couldn't check for updates. Your current version is unchanged.".into(),
            ))
        }
    }
}

pub async fn install(app: &AppHandle) -> AppResult<()> {
    let updater = app
        .updater()
        .map_err(|e| AppError::Internal(format!("Updater is unavailable: {e}")))?;
    let Some(update) = updater.check().await.map_err(|error| {
        log::warn!("update recheck failed: {error}");
        AppError::Internal(
            "Couldn't reach the update server. The installed version is unchanged.".into(),
        )
    })?
    else {
        return Err(AppError::NotFound("Recall is already up to date.".into()));
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| {
            log::warn!("update installation failed: {error}");
            AppError::Internal(
                "The update could not be verified or installed. Your current version is unchanged."
                    .into(),
            )
        })?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_release_versions() {
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.1"));
        assert!(!is_newer("not-a-version", "0.1.0"));
    }
}
