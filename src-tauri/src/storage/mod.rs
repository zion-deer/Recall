//! Local SQLite storage. All Recall data lives in a single database file inside
//! the per-user application data directory.

pub mod migrations;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub struct Database {
    conn: Mutex<Connection>,
    path: Option<PathBuf>,
}

impl Database {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
            restrict_permissions(dir, true);
        }

        let existed = path.exists();
        let mut conn = Connection::open(path)?;
        configure(&conn)?;
        restrict_permissions(path, false);

        let version = migrations::current_version(&conn)?;
        if existed && version > 0 && version < migrations::latest_version() {
            backup_before_migration(path, version)?;
        }
        migrations::migrate(&mut conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
            path: Some(path.to_path_buf()),
        })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        let mut conn = Connection::open_in_memory()?;
        configure(&conn)?;
        migrations::migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            path: None,
        })
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves SQLite itself consistent
        // (transactions roll back), so recovering the guard is safe.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn file_size_bytes(&self) -> u64 {
        self.path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0)
    }

    /// Rewrites the database file so deleted content no longer occupies disk pages.
    pub fn compact(&self) -> AppResult<()> {
        let conn = self.conn();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        Ok(())
    }
}

fn configure(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA secure_delete = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    Ok(())
}

fn backup_before_migration(path: &Path, version: u32) -> AppResult<()> {
    let backup = path.with_extension(format!("db.v{version}.bak"));
    if backup.exists() {
        return Ok(());
    }
    let conn = Connection::open(path)?;
    conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
        .map_err(|e| {
            AppError::Internal(format!("could not back up database before upgrade: {e}"))
        })?;
    restrict_permissions(&backup, false);
    log::info!("backed up schema v{version} database before migrating");
    Ok(())
}

#[cfg(unix)]
fn restrict_permissions(path: &Path, is_dir: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if is_dir { 0o700 } else { 0o600 };
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
        log::warn!("could not restrict permissions on data path: {e}");
    }
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path, _is_dir: bool) {
    // On Windows the per-user AppData directory is already ACL'd to the user.
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_file_database_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("recall.db");
        {
            let db = Database::open(&path).unwrap();
            db.conn()
                .execute(
                    "INSERT INTO settings (key, value, updated_at) VALUES ('k', '\"v\"', 0)",
                    [],
                )
                .unwrap();
        }
        let db = Database::open(&path).unwrap();
        let v: String = db
            .conn()
            .query_row("SELECT value FROM settings WHERE key = 'k'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, "\"v\"");
        assert!(db.file_size_bytes() > 0);
    }

    #[cfg(unix)]
    #[test]
    fn database_file_is_private_to_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recall.db");
        let _db = Database::open(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
