use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{MemoryEvent, NewEvent};
use crate::error::{AppError, AppResult};
use crate::storage::{now_ms, Database};

pub const MAX_PAGE_SIZE: u32 = 1000;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventQuery {
    /// Inclusive lower bound (Unix ms). Events overlapping the range are returned.
    pub start: Option<i64>,
    /// Exclusive upper bound (Unix ms).
    pub end: Option<i64>,
    pub app_name: Option<String>,
    pub kind: Option<String>,
    pub limit: Option<u32>,
}

impl EventQuery {
    pub fn validate(&self) -> AppResult<()> {
        if let (Some(s), Some(e)) = (self.start, self.end) {
            if s > e {
                return Err(AppError::invalid("Start time must be before end time"));
            }
        }
        if let Some(limit) = self.limit {
            if limit == 0 || limit > MAX_PAGE_SIZE {
                return Err(AppError::invalid(format!(
                    "Limit must be between 1 and {MAX_PAGE_SIZE}"
                )));
            }
        }
        if let Some(app) = &self.app_name {
            if app.len() > 512 {
                return Err(AppError::invalid("Application filter is too long"));
            }
        }
        if let Some(kind) = &self.kind {
            if super::EventKind::parse(kind).is_none() {
                return Err(AppError::invalid("Unknown memory type"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsage {
    pub app_name: String,
    pub total_ms: i64,
    pub sessions: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStats {
    pub total_events: i64,
    pub oldest_at: Option<i64>,
    pub newest_at: Option<i64>,
    pub database_bytes: u64,
}

const EVENT_COLUMNS: &str =
    "id, kind, source, started_at, ended_at, app_name, app_id, window_title, url, file_path";

fn map_event(row: &Row<'_>) -> rusqlite::Result<MemoryEvent> {
    Ok(MemoryEvent {
        id: row.get(0)?,
        kind: row.get(1)?,
        source: row.get(2)?,
        started_at: row.get(3)?,
        ended_at: row.get(4)?,
        app_name: row.get(5)?,
        app_id: row.get(6)?,
        window_title: row.get(7)?,
        url: row.get(8)?,
        file_path: row.get(9)?,
    })
}

pub fn insert_event(db: &Database, e: &NewEvent) -> AppResult<i64> {
    if e.ended_at < e.started_at {
        return Err(AppError::invalid("Event ends before it starts"));
    }
    let conn = db.conn();
    conn.execute(
        "INSERT INTO events (kind, source, started_at, ended_at, app_name, app_id, window_title, url, file_path, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            e.kind.as_str(),
            e.source,
            e.started_at,
            e.ended_at,
            e.app_name,
            e.app_id,
            e.window_title,
            e.url,
            e.file_path,
            now_ms()
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Extends an open session. Returns false if the event no longer exists
/// (for example, the user deleted it while it was still in progress).
pub fn set_event_end(db: &Database, id: i64, ended_at: i64) -> AppResult<bool> {
    let n = db.conn().execute(
        "UPDATE events SET ended_at = MAX(started_at, ?2) WHERE id = ?1",
        params![id, ended_at],
    )?;
    Ok(n > 0)
}

pub fn get_event(db: &Database, id: i64) -> AppResult<Option<MemoryEvent>> {
    let conn = db.conn();
    Ok(conn
        .query_row(
            &format!("SELECT {EVENT_COLUMNS} FROM events WHERE id = ?1"),
            [id],
            map_event,
        )
        .optional()?)
}

pub fn list_events(db: &Database, q: &EventQuery) -> AppResult<Vec<MemoryEvent>> {
    q.validate()?;
    let conn = db.conn();
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {EVENT_COLUMNS} FROM events
         WHERE (?1 IS NULL OR ended_at >= ?1)
           AND (?2 IS NULL OR started_at < ?2)
           AND (?3 IS NULL OR app_name = ?3 COLLATE NOCASE)
           AND (?4 IS NULL OR kind = ?4)
         ORDER BY started_at DESC, id DESC
         LIMIT ?5"
    ))?;
    let limit = q.limit.unwrap_or(200);
    let rows = stmt.query_map(
        params![q.start, q.end, q.app_name, q.kind, limit],
        map_event,
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Per-application time totals for events overlapping `[start, end)`,
/// clipped to the range.
pub fn app_usage(db: &Database, start: i64, end: i64) -> AppResult<Vec<AppUsage>> {
    if start > end {
        return Err(AppError::invalid("Start time must be before end time"));
    }
    let conn = db.conn();
    let mut stmt = conn.prepare_cached(
        "SELECT app_name,
                SUM(MIN(ended_at, ?2) - MAX(started_at, ?1)) AS total,
                COUNT(*)
         FROM events
         WHERE ended_at >= ?1 AND started_at < ?2 AND app_name IS NOT NULL
         GROUP BY app_name COLLATE NOCASE
         ORDER BY total DESC",
    )?;
    let rows = stmt.query_map(params![start, end], |r| {
        Ok(AppUsage {
            app_name: r.get(0)?,
            total_ms: r.get::<_, i64>(1)?.max(0),
            sessions: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn delete_event(db: &Database, id: i64) -> AppResult<bool> {
    Ok(db
        .conn()
        .execute("DELETE FROM events WHERE id = ?1", [id])?
        > 0)
}

/// Deletes every event that overlaps `[start, end)`.
pub fn delete_range(db: &Database, start: i64, end: i64) -> AppResult<usize> {
    if start > end {
        return Err(AppError::invalid("Start time must be before end time"));
    }
    Ok(db.conn().execute(
        "DELETE FROM events WHERE ended_at >= ?1 AND started_at < ?2",
        params![start, end],
    )?)
}

pub fn delete_ids(db: &Database, ids: &[i64]) -> AppResult<usize> {
    let mut conn = db.conn();
    let tx = conn.transaction()?;
    let mut n = 0;
    {
        let mut stmt = tx.prepare_cached("DELETE FROM events WHERE id = ?1")?;
        for id in ids {
            n += stmt.execute([id])?;
        }
    }
    tx.commit()?;
    Ok(n)
}

pub fn delete_all(db: &Database) -> AppResult<usize> {
    let n = db.conn().execute("DELETE FROM events", [])?;
    db.compact()?;
    Ok(n)
}

pub fn purge_older_than(db: &Database, cutoff: i64) -> AppResult<usize> {
    Ok(db
        .conn()
        .execute("DELETE FROM events WHERE ended_at < ?1", [cutoff])?)
}

pub fn stats(db: &Database) -> AppResult<MemoryStats> {
    let (total, oldest, newest) = db.conn().query_row(
        "SELECT COUNT(*), MIN(started_at), MAX(ended_at) FROM events",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(MemoryStats {
        total_events: total,
        oldest_at: oldest,
        newest_at: newest,
        database_bytes: db.file_size_bytes(),
    })
}

/// Streams all events in chronological order. Used by export and by
/// retroactive exclusion; avoids loading the whole table into memory.
pub fn for_each_event(
    db: &Database,
    mut f: impl FnMut(MemoryEvent) -> AppResult<()>,
) -> AppResult<()> {
    let conn = db.conn();
    let mut stmt = conn.prepare(&format!(
        "SELECT {EVENT_COLUMNS} FROM events ORDER BY started_at ASC, id ASC"
    ))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        f(map_event(row)?)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample(app: &str, title: &str, start: i64, end: i64) -> NewEvent {
    NewEvent {
        kind: super::EventKind::AppActivity,
        source: "test",
        started_at: start,
        ended_at: end,
        app_name: Some(app.into()),
        app_id: None,
        window_title: Some(title.into()),
        url: None,
        file_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_with_events() -> Database {
        let db = Database::open_in_memory().unwrap();
        insert_event(&db, &sample("Code", "main.rs", 1_000, 5_000)).unwrap();
        insert_event(&db, &sample("Chrome", "Rust docs", 5_000, 9_000)).unwrap();
        insert_event(&db, &sample("code", "lib.rs", 10_000, 12_000)).unwrap();
        db
    }

    #[test]
    fn insert_and_get() {
        let db = Database::open_in_memory().unwrap();
        let id = insert_event(&db, &sample("Code", "x", 10, 20)).unwrap();
        let e = get_event(&db, id).unwrap().unwrap();
        assert_eq!(e.app_name.as_deref(), Some("Code"));
        assert_eq!(e.kind, "app_activity");
        assert_eq!((e.started_at, e.ended_at), (10, 20));
        assert!(get_event(&db, id + 1).unwrap().is_none());
    }

    #[test]
    fn rejects_negative_duration() {
        let db = Database::open_in_memory().unwrap();
        assert!(insert_event(&db, &sample("Code", "x", 20, 10)).is_err());
    }

    #[test]
    fn extends_open_session_but_never_backwards() {
        let db = Database::open_in_memory().unwrap();
        let id = insert_event(&db, &sample("Code", "x", 100, 100)).unwrap();
        assert!(set_event_end(&db, id, 500).unwrap());
        assert_eq!(get_event(&db, id).unwrap().unwrap().ended_at, 500);
        set_event_end(&db, id, 50).unwrap();
        assert_eq!(get_event(&db, id).unwrap().unwrap().ended_at, 100);
        delete_event(&db, id).unwrap();
        assert!(!set_event_end(&db, id, 600).unwrap());
    }

    #[test]
    fn lists_newest_first_with_filters() {
        let db = db_with_events();
        let all = list_events(&db, &EventQuery::default()).unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].window_title.as_deref(), Some("lib.rs"));

        let ranged = list_events(
            &db,
            &EventQuery {
                start: Some(6_000),
                end: Some(10_500),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ranged.len(), 2, "overlapping events are included");

        let code = list_events(
            &db,
            &EventQuery {
                app_name: Some("CODE".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(code.len(), 2, "app filter is case-insensitive");

        let limited = list_events(
            &db,
            &EventQuery {
                limit: Some(1),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(limited.len(), 1);
    }

    #[test]
    fn query_validation() {
        let db = Database::open_in_memory().unwrap();
        for q in [
            EventQuery {
                start: Some(10),
                end: Some(5),
                ..Default::default()
            },
            EventQuery {
                limit: Some(0),
                ..Default::default()
            },
            EventQuery {
                limit: Some(MAX_PAGE_SIZE + 1),
                ..Default::default()
            },
            EventQuery {
                kind: Some("nope".into()),
                ..Default::default()
            },
            EventQuery {
                app_name: Some("x".repeat(600)),
                ..Default::default()
            },
        ] {
            assert!(list_events(&db, &q).is_err());
        }
    }

    #[test]
    fn app_usage_clips_to_range() {
        let db = db_with_events();
        let usage = app_usage(&db, 3_000, 11_000).unwrap();
        let code = usage
            .iter()
            .find(|u| u.app_name.eq_ignore_ascii_case("code"))
            .unwrap();
        assert_eq!(code.total_ms, 2_000 + 1_000);
        assert_eq!(code.sessions, 2);
        let chrome = usage.iter().find(|u| u.app_name == "Chrome").unwrap();
        assert_eq!(chrome.total_ms, 4_000);
        assert_eq!(usage[0].app_name, "Chrome", "sorted by time spent");
    }

    #[test]
    fn deletes_single_range_and_all() {
        let db = db_with_events();
        let first = list_events(&db, &EventQuery::default()).unwrap()[0].id;
        assert!(delete_event(&db, first).unwrap());
        assert!(!delete_event(&db, first).unwrap());

        assert_eq!(delete_range(&db, 4_000, 4_500).unwrap(), 1);
        assert_eq!(stats(&db).unwrap().total_events, 1);

        insert_event(&db, &sample("A", "b", 1, 2)).unwrap();
        assert_eq!(delete_all(&db).unwrap(), 2);
        let s = stats(&db).unwrap();
        assert_eq!(s.total_events, 0);
        assert!(s.oldest_at.is_none());
    }

    #[test]
    fn retention_purge() {
        let db = db_with_events();
        assert_eq!(purge_older_than(&db, 9_500).unwrap(), 2);
        assert_eq!(stats(&db).unwrap().total_events, 1);
    }

    #[test]
    fn titles_with_sql_metacharacters_are_stored_verbatim() {
        let db = Database::open_in_memory().unwrap();
        let title = "'); DROP TABLE events; --";
        let id = insert_event(&db, &sample("Code", title, 1, 2)).unwrap();
        assert_eq!(
            get_event(&db, id).unwrap().unwrap().window_title.as_deref(),
            Some(title)
        );
    }
}
