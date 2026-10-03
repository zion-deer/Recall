//! Versioned schema migrations, tracked with SQLite's `user_version` pragma.
//!
//! Rules:
//! - Migrations are append-only. Never edit or reorder a migration that has shipped.
//! - Each migration runs in its own transaction; a failure leaves the previous version intact.
//! - A database written by a newer version of Recall is never opened (no silent downgrades).

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub struct Migration {
    pub version: u32,
    pub description: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "events, settings and exclusions",
        sql: r#"
        CREATE TABLE events (
            id           INTEGER PRIMARY KEY,
            kind         TEXT    NOT NULL,
            source       TEXT    NOT NULL,
            started_at   INTEGER NOT NULL,
            ended_at     INTEGER NOT NULL,
            app_name     TEXT,
            app_id       TEXT,
            window_title TEXT,
            url          TEXT,
            file_path    TEXT,
            metadata     TEXT    NOT NULL DEFAULT '{}',
            created_at   INTEGER NOT NULL,
            CHECK (ended_at >= started_at)
        );
        CREATE INDEX idx_events_started_at ON events (started_at);
        CREATE INDEX idx_events_app_name ON events (app_name COLLATE NOCASE);

        CREATE TABLE settings (
            key        TEXT PRIMARY KEY,
            value      TEXT    NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE exclusions (
            id         INTEGER PRIMARY KEY,
            kind       TEXT    NOT NULL CHECK (kind IN ('app', 'website', 'folder', 'title')),
            pattern    TEXT    NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE (kind, pattern COLLATE NOCASE)
        );

        -- Privacy-leaning defaults. Users can remove any of these.
        INSERT INTO exclusions (kind, pattern, created_at) VALUES
            ('app', '1Password', 0),
            ('app', 'Bitwarden', 0),
            ('app', 'KeePass', 0),
            ('app', 'KeePassXC', 0),
            ('app', 'LastPass', 0),
            ('app', 'Dashlane', 0),
            ('app', 'Enpass', 0),
            ('app', 'Proton Pass', 0),
            ('app', 'Keychain Access', 0),
            ('app', 'Passwords', 0),
            ('app', 'Credential Manager', 0),
            ('app', 'Signal', 0),
            ('title', 'Private Browsing', 0),
            ('title', 'Incognito', 0),
            ('title', 'InPrivate', 0),
            ('website', '1password.com', 0),
            ('website', 'bitwarden.com', 0),
            ('website', 'lastpass.com', 0);
    "#,
    },
    Migration {
        version: 2,
        description: "browser memory and full-text search",
        sql: r#"
        -- A provider cursor prevents Recall from importing old history on
        -- startup and makes each incremental scan inexpensive.
        CREATE TABLE browser_cursors (
            provider_id TEXT NOT NULL,
            profile_id  TEXT NOT NULL,
            last_visit  INTEGER NOT NULL,
            updated_at  INTEGER NOT NULL,
            PRIMARY KEY (provider_id, profile_id)
        );

        -- The source visit id is not globally meaningful, so the provider and
        -- profile (stored in source/app_id) are part of the dedupe key.
        CREATE UNIQUE INDEX idx_events_browser_dedupe
            ON events (source, app_id, started_at, url)
            WHERE kind = 'browser_activity';
        CREATE INDEX idx_events_url
            ON events (url COLLATE NOCASE)
            WHERE url IS NOT NULL;

        CREATE VIRTUAL TABLE events_fts USING fts5(
            app_name,
            window_title,
            url,
            file_path,
            content='events',
            content_rowid='id',
            tokenize='unicode61 remove_diacritics 2'
        );

        CREATE TRIGGER events_fts_insert AFTER INSERT ON events BEGIN
            INSERT INTO events_fts(rowid, app_name, window_title, url, file_path)
            VALUES (new.id, new.app_name, new.window_title, new.url, new.file_path);
        END;
        CREATE TRIGGER events_fts_delete AFTER DELETE ON events BEGIN
            INSERT INTO events_fts(events_fts, rowid, app_name, window_title, url, file_path)
            VALUES ('delete', old.id, old.app_name, old.window_title, old.url, old.file_path);
        END;
        CREATE TRIGGER events_fts_update AFTER UPDATE ON events BEGIN
            INSERT INTO events_fts(events_fts, rowid, app_name, window_title, url, file_path)
            VALUES ('delete', old.id, old.app_name, old.window_title, old.url, old.file_path);
            INSERT INTO events_fts(rowid, app_name, window_title, url, file_path)
            VALUES (new.id, new.app_name, new.window_title, new.url, new.file_path);
        END;

        -- Index memories written by v0.1.0 before this migration.
        INSERT INTO events_fts(rowid, app_name, window_title, url, file_path)
            SELECT id, app_name, window_title, url, file_path FROM events;
    "#,
    },
];

pub fn latest_version() -> u32 {
    MIGRATIONS.last().map(|m| m.version).unwrap_or(0)
}

pub fn current_version(conn: &Connection) -> AppResult<u32> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

/// Applies all pending migrations. Returns the number applied.
pub fn migrate(conn: &mut Connection) -> AppResult<usize> {
    let current = current_version(conn)?;
    let latest = latest_version();
    if current > latest {
        return Err(AppError::Unsupported(format!(
            "This memory database was created by a newer version of Recall (schema {current}, \
             this version supports {latest}). Please update Recall."
        )));
    }

    let mut applied = 0;
    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        log::info!(
            "applying migration {} ({})",
            migration.version,
            migration.description
        );
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.pragma_update(None, "user_version", migration.version)?;
        tx.commit()?;
        applied += 1;
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_strictly_increasing() {
        let mut prev = 0;
        for m in MIGRATIONS {
            assert!(m.version > prev, "migration {} out of order", m.version);
            prev = m.version;
        }
    }

    #[test]
    fn migrates_fresh_database_to_latest() {
        let mut conn = Connection::open_in_memory().unwrap();
        assert_eq!(current_version(&conn).unwrap(), 0);
        let applied = migrate(&mut conn).unwrap();
        assert_eq!(applied, MIGRATIONS.len());
        assert_eq!(current_version(&conn).unwrap(), latest_version());
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(migrate(&mut conn).unwrap(), 0);
    }

    #[test]
    fn refuses_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", latest_version() + 1)
            .unwrap();
        assert!(matches!(migrate(&mut conn), Err(AppError::Unsupported(_))));
    }

    #[test]
    fn seeds_default_exclusions() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM exclusions", [], |r| r.get(0))
            .unwrap();
        assert!(n > 0);
    }

    #[test]
    fn upgrades_v1_without_losing_events_and_builds_search_index() {
        let mut conn = Connection::open_in_memory().unwrap();
        let first = &MIGRATIONS[0];
        conn.execute_batch(first.sql).unwrap();
        conn.pragma_update(None, "user_version", first.version)
            .unwrap();
        conn.execute(
            "INSERT INTO events
             (kind, source, started_at, ended_at, app_name, window_title, metadata, created_at)
             VALUES ('app_activity', 'test', 1, 2, 'Code', 'migration note', '{}', 1)",
            [],
        )
        .unwrap();

        assert_eq!(migrate(&mut conn).unwrap(), 1);
        assert_eq!(current_version(&conn).unwrap(), 2);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let indexed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM events_fts WHERE events_fts MATCH 'migration'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(indexed, 1);
    }
}
