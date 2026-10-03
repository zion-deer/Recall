//! Data export. Produces a single, self-describing JSON document; the format
//! is documented in PRIVACY.md ("Exporting your data").

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::{privacy, store};
use crate::error::{AppError, AppResult};
use crate::settings;
use crate::storage::{migrations, now_ms, Database};

pub const EXPORT_FORMAT: &str = "recall-export";
pub const EXPORT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub event_count: usize,
}

/// Ensures an export destination is a plain `.json` file path in an existing directory.
pub fn validate_destination(path: &str) -> AppResult<PathBuf> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(AppError::invalid("Choose a full location to save the export"));
    }
    if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err(AppError::invalid("Export location cannot contain '..'"));
    }
    let is_json = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if !is_json {
        return Err(AppError::invalid("Exports must be saved as a .json file"));
    }
    if p.is_dir() {
        return Err(AppError::invalid("Choose a file name, not a folder"));
    }
    match p.parent() {
        Some(dir) if dir.is_dir() => Ok(p.to_path_buf()),
        _ => Err(AppError::invalid("The chosen folder does not exist")),
    }
}

pub fn export_to(db: &Database, path: &str, app_version: &str) -> AppResult<ExportResult> {
    let dest = validate_destination(path)?;
    let tmp = dest.with_extension("json.partial");

    let result = write_export(db, &tmp, app_version);
    match result {
        Ok(count) => {
            std::fs::rename(&tmp, &dest)?;
            Ok(ExportResult { path: dest.to_string_lossy().into_owned(), event_count: count })
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn write_export(db: &Database, path: &Path, app_version: &str) -> AppResult<usize> {
    let settings = settings::load(db)?;
    let exclusions = privacy::list(db)?;
    let schema_version = migrations::current_version(&db.conn())?;

    let mut w = BufWriter::new(std::fs::File::create(path)?);
    let header = serde_json::json!({
        "format": EXPORT_FORMAT,
        "formatVersion": EXPORT_FORMAT_VERSION,
        "appVersion": app_version,
        "schemaVersion": schema_version,
        "exportedAt": now_ms(),
        "settings": settings,
        "exclusions": exclusions,
    });
    // Write the header object without its closing brace, then stream events
    // so large histories never need to be held in memory at once.
    let header = serde_json::to_string_pretty(&header)?;
    let header = header.trim_end().strip_suffix('}').unwrap_or(&header);
    write!(w, "{header},\n  \"events\": [")?;

    let mut count = 0usize;
    store::for_each_event(db, |e| {
        if count > 0 {
            w.write_all(b",")?;
        }
        w.write_all(b"\n    ")?;
        serde_json::to_writer(&mut w, &e)?;
        count += 1;
        Ok(())
    })?;
    w.write_all(b"\n  ]\n}\n")?;
    w.flush()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_valid_json_with_all_events() {
        let db = Database::open_in_memory().unwrap();
        store::insert_event(&db, &store::sample("Code", "a \"quoted\" title", 1, 2)).unwrap();
        store::insert_event(&db, &store::sample("Chrome", "docs", 3, 4)).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");

        let res = export_to(&db, path.to_str().unwrap(), "0.1.0").unwrap();
        assert_eq!(res.event_count, 2);

        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["format"], EXPORT_FORMAT);
        assert_eq!(v["events"].as_array().unwrap().len(), 2);
        assert_eq!(v["events"][0]["windowTitle"], "a \"quoted\" title");
        assert!(v["exclusions"].as_array().unwrap().len() > 0);
        assert!(!dir.path().join("export.json.partial").exists());
    }

    #[test]
    fn exports_empty_database() {
        let db = Database::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("e.json");
        export_to(&db, path.to_str().unwrap(), "0.1.0").unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["events"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn rejects_unsafe_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path().to_str().unwrap();
        assert!(validate_destination("relative.json").is_err());
        assert!(validate_destination(&format!("{d}/../x.json")).is_err());
        assert!(validate_destination(&format!("{d}/x.exe")).is_err());
        assert!(validate_destination(&format!("{d}/missing/x.json")).is_err());
        std::fs::create_dir(dir.path().join("folder.json")).unwrap();
        assert!(validate_destination(&format!("{d}/folder.json")).is_err());
        assert!(validate_destination(&format!("{d}/ok.JSON")).is_ok());
    }
}
