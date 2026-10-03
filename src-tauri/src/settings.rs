//! User settings. Stored as one JSON document in the `settings` table so that
//! adding a field never needs a schema migration; unknown or missing fields
//! fall back to defaults.

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::storage::{now_ms, Database};

const SETTINGS_KEY: &str = "user_settings";

/// `paused_until` value meaning "until the user resumes". Kept within
/// JavaScript's safe integer range so it survives the IPC round trip.
pub const PAUSE_INDEFINITELY: i64 = 9_007_199_254_740_991;

/// Allowed memory retention choices, in days. `None` means forever.
pub const RETENTION_CHOICES: &[u32] = &[7, 30, 180, 365];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub onboarding_completed: bool,
    /// Master switch for all memory collection.
    pub recording_enabled: bool,
    /// Unix ms until which recording is paused. See [`PAUSE_INDEFINITELY`].
    pub paused_until: Option<i64>,
    pub app_activity_enabled: bool,
    /// When off, only the application name is stored, never window titles.
    pub window_titles_enabled: bool,
    pub retention_days: Option<u32>,
    pub idle_threshold_secs: u32,
    pub poll_interval_secs: u32,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarding_completed: false,
            recording_enabled: false,
            paused_until: None,
            app_activity_enabled: true,
            window_titles_enabled: true,
            retention_days: Some(180),
            idle_threshold_secs: 300,
            poll_interval_secs: 2,
            theme: Theme::System,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> AppResult<()> {
        if let Some(days) = self.retention_days {
            if !RETENTION_CHOICES.contains(&days) {
                return Err(AppError::invalid(format!(
                    "Unsupported retention period: {days} days"
                )));
            }
        }
        if !(60..=3600).contains(&self.idle_threshold_secs) {
            return Err(AppError::invalid(
                "Idle threshold must be between 1 and 60 minutes",
            ));
        }
        if !(1..=30).contains(&self.poll_interval_secs) {
            return Err(AppError::invalid(
                "Activity check interval must be between 1 and 30 seconds",
            ));
        }
        if let Some(until) = self.paused_until {
            if !(0..=PAUSE_INDEFINITELY).contains(&until) {
                return Err(AppError::invalid("Invalid pause time"));
            }
        }
        Ok(())
    }

    pub fn is_paused(&self, now: i64) -> bool {
        self.paused_until.is_some_and(|until| until > now)
    }
}

pub fn load(db: &Database) -> AppResult<Settings> {
    let conn = db.conn();
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTINGS_KEY],
            |r| r.get(0),
        )
        .optional()?;
    let Some(raw) = raw else {
        return Ok(Settings::default());
    };
    match serde_json::from_str::<Settings>(&raw) {
        Ok(s) if s.validate().is_ok() => Ok(s),
        Ok(_) | Err(_) => {
            log::warn!("stored settings were invalid; using defaults");
            Ok(Settings::default())
        }
    }
}

pub fn save(db: &Database, settings: &Settings) -> AppResult<()> {
    settings.validate()?;
    let json = serde_json::to_string(settings)?;
    db.conn().execute(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        rusqlite::params![SETTINGS_KEY, json, now_ms()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_private() {
        let s = Settings::default();
        s.validate().unwrap();
        assert!(
            !s.recording_enabled,
            "recording must be opt-in during onboarding"
        );
        assert!(!s.onboarding_completed);
    }

    #[test]
    fn round_trips_through_database() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(load(&db).unwrap(), Settings::default());
        let s = Settings {
            recording_enabled: true,
            retention_days: None,
            theme: Theme::Dark,
            ..Settings::default()
        };
        save(&db, &s).unwrap();
        assert_eq!(load(&db).unwrap(), s);
    }

    #[test]
    fn rejects_invalid_values() {
        let bad = [
            Settings {
                retention_days: Some(3),
                ..Default::default()
            },
            Settings {
                idle_threshold_secs: 5,
                ..Default::default()
            },
            Settings {
                poll_interval_secs: 0,
                ..Default::default()
            },
            Settings {
                paused_until: Some(-1),
                ..Default::default()
            },
        ];
        let db = Database::open_in_memory().unwrap();
        for s in bad {
            assert!(s.validate().is_err());
            assert!(save(&db, &s).is_err());
        }
    }

    #[test]
    fn missing_fields_use_defaults() {
        let s: Settings = serde_json::from_str(r#"{"recordingEnabled":true}"#).unwrap();
        assert!(s.recording_enabled);
        assert_eq!(s.poll_interval_secs, Settings::default().poll_interval_secs);
    }

    #[test]
    fn corrupt_settings_fall_back_to_defaults() {
        let db = Database::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, 'not json', 0)",
                [SETTINGS_KEY],
            )
            .unwrap();
        assert_eq!(load(&db).unwrap(), Settings::default());
    }

    #[test]
    fn pause_state() {
        let mut s = Settings::default();
        assert!(!s.is_paused(1000));
        s.paused_until = Some(2000);
        assert!(s.is_paused(1000));
        assert!(!s.is_paused(2000));
        s.paused_until = Some(PAUSE_INDEFINITELY);
        assert!(s.is_paused(PAUSE_INDEFINITELY - 1));
        assert!(Settings {
            paused_until: Some(i64::MAX),
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}
