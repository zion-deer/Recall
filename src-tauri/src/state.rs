use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::error::AppResult;
use crate::memory::privacy::{self, PrivacyFilter};
use crate::memory::recorder::{RecorderHandle, SharedConfig};
use crate::platform::PlatformAdapter;
use crate::settings::{self, Settings};
use crate::storage::Database;

pub struct AppState {
    pub db: Arc<Database>,
    pub config: Arc<SharedConfig>,
    pub recorder: RecorderHandle,
    pub platform: Arc<dyn PlatformAdapter>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl AppState {
    pub fn load_config(db: &Database) -> AppResult<SharedConfig> {
        Ok(SharedConfig {
            settings: RwLock::new(settings::load(db)?),
            filter: RwLock::new(PrivacyFilter::new(&privacy::list(db)?)),
            browser_statuses: RwLock::new(Vec::new()),
        })
    }

    pub fn settings(&self) -> Settings {
        self.config
            .settings
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Persists settings, then applies them to the running recorder.
    pub fn save_settings(&self, new: Settings) -> AppResult<Settings> {
        settings::save(&self.db, &new)?;
        *self
            .config
            .settings
            .write()
            .unwrap_or_else(|e| e.into_inner()) = new.clone();
        self.recorder.refresh();
        Ok(new)
    }

    pub fn reload_filter(&self) -> AppResult<PrivacyFilter> {
        let filter = PrivacyFilter::new(&privacy::list(&self.db)?);
        *self
            .config
            .filter
            .write()
            .unwrap_or_else(|e| e.into_inner()) = filter.clone();
        self.recorder.refresh();
        Ok(filter)
    }

    pub fn set_paused_until(&self, until: Option<i64>) -> AppResult<Settings> {
        let mut s = self.settings();
        s.paused_until = until;
        self.save_settings(s)
    }
}
