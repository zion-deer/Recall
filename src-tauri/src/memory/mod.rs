//! The memory pipeline: collectors produce [`NewEvent`]s, the privacy filter
//! drops anything excluded, and the store persists what remains.
//!
//! Collectors are independent. V0.1 ships the application activity collector
//! ([`recorder`]); browser history and screenshots plug into the same event model.

pub mod export;
pub mod privacy;
pub mod recorder;
pub mod store;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    AppActivity,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppActivity => "app_activity",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "app_activity" => Some(Self::AppActivity),
            _ => None,
        }
    }
}

/// An event produced by a collector, before it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct NewEvent {
    pub kind: EventKind,
    pub source: &'static str,
    pub started_at: i64,
    pub ended_at: i64,
    pub app_name: Option<String>,
    pub app_id: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub file_path: Option<String>,
}

/// A stored memory, as shown to the user. Timestamps are Unix milliseconds (UTC).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEvent {
    pub id: i64,
    /// Kept as a string so data written by newer versions still loads.
    pub kind: String,
    pub source: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub app_name: Option<String>,
    pub app_id: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub file_path: Option<String>,
}
