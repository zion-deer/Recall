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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub kind: Option<String>,
    pub limit: Option<u32>,
}

impl SearchQuery {
    fn validate(&self) -> AppResult<()> {
        if self.text.chars().count() > 500 {
            return Err(AppError::invalid("Search is too long"));
        }
        if let (Some(start), Some(end)) = (self.start, self.end) {
            if start > end {
                return Err(AppError::invalid("Start time must be before end time"));
            }
        }
        if let Some(kind) = &self.kind {
            if super::EventKind::parse(kind).is_none() {
                return Err(AppError::invalid("Unknown memory type"));
            }
        }
        if let Some(limit) = self.limit {
            if limit == 0 || limit > MAX_PAGE_SIZE {
                return Err(AppError::invalid(format!(
                    "Limit must be between 1 and {MAX_PAGE_SIZE}"
                )));
            }
        }
        Ok(())
    }
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

/// Inserts an event unless a database uniqueness rule identifies it as a
/// duplicate. Browser collectors use this to make repeated scans idempotent.
pub fn insert_event_if_new(db: &Database, e: &NewEvent) -> AppResult<Option<i64>> {
    if e.ended_at < e.started_at {
        return Err(AppError::invalid("Event ends before it starts"));
    }
    let conn = db.conn();
    let changed = conn.execute(
        "INSERT OR IGNORE INTO events
         (kind, source, started_at, ended_at, app_name, app_id, window_title, url, file_path, created_at)
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
    Ok((changed > 0).then(|| conn.last_insert_rowid()))
}

/// Inserts a browser visit, or extends the immediately preceding visit when
/// the same page is recorded again within `merge_window_ms` (for example a
/// refresh). Returns true when stored data changed.
pub fn insert_or_merge_browser_visit(
    db: &Database,
    e: &NewEvent,
    merge_window_ms: i64,
) -> AppResult<bool> {
    if e.kind != super::EventKind::BrowserActivity {
        return Err(AppError::invalid("Expected a browser memory"));
    }
    let conn = db.conn();
    let previous: Option<(i64, Option<String>, i64)> = conn
        .query_row(
            "SELECT id, url, ended_at FROM events
             WHERE kind = 'browser_activity' AND source = ?1 AND app_id = ?2
             ORDER BY started_at DESC, id DESC LIMIT 1",
            params![e.source, e.app_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    if let Some((id, url, ended_at)) = previous {
        if url == e.url && e.started_at >= ended_at && e.started_at - ended_at <= merge_window_ms {
            let changed = conn.execute(
                "UPDATE events SET ended_at = ?2,
                    window_title = COALESCE(?3, window_title)
                 WHERE id = ?1 AND ended_at < ?2",
                params![id, e.started_at, e.window_title],
            )?;
            return Ok(changed > 0);
        }
    }
    drop(conn);
    Ok(insert_event_if_new(db, e)?.is_some())
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

/// Full-text search over app names, titles, URLs, and file paths.
pub fn search_events(db: &Database, q: &SearchQuery) -> AppResult<Vec<MemoryEvent>> {
    q.validate()?;
    let match_query = fts_query(&q.text);
    if match_query.is_empty() {
        return Ok(Vec::new());
    }
    let conn = db.conn();
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT e.{}
         FROM events_fts f JOIN events e ON e.id = f.rowid
         WHERE events_fts MATCH ?1
           AND (?2 IS NULL OR e.ended_at >= ?2)
           AND (?3 IS NULL OR e.started_at < ?3)
           AND (?4 IS NULL OR e.kind = ?4)
         ORDER BY bm25(events_fts), e.started_at DESC
         LIMIT ?5",
        EVENT_COLUMNS.replace(", ", ", e.")
    ))?;
    let rows = stmt.query_map(
        params![match_query, q.start, q.end, q.kind, q.limit.unwrap_or(100)],
        map_event,
    )?;
    let mut events = rows.collect::<Result<Vec<_>, _>>()?;
    rerank(&q.text, &mut events);
    Ok(events)
}

/// Exact title and URL matches stay ahead of the remaining FTS order.
fn rerank(query: &str, events: &mut [MemoryEvent]) {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return;
    }
    events.sort_by_key(|event| {
        let title = event.window_title.as_deref().unwrap_or("").to_lowercase();
        let url = event.url.as_deref().unwrap_or("").to_lowercase();
        let app = event.app_name.as_deref().unwrap_or("").to_lowercase();
        if title == needle || url.contains(&needle) {
            0
        } else if title.contains(&needle) || app == needle {
            1
        } else {
            2
        }
    });
}

fn fts_query(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .take(20)
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
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
    let paths = screenshot_paths(db, "id = ?1", params![id])?;
    let deleted = db
        .conn()
        .execute("DELETE FROM events WHERE id = ?1", [id])?
        > 0;
    if deleted {
        remove_screenshot_files(&paths);
    }
    Ok(deleted)
}

/// Deletes every event that overlaps `[start, end)`.
pub fn delete_range(db: &Database, start: i64, end: i64) -> AppResult<usize> {
    if start > end {
        return Err(AppError::invalid("Start time must be before end time"));
    }
    let paths = screenshot_paths(
        db,
        "ended_at >= ?1 AND started_at < ?2",
        params![start, end],
    )?;
    let deleted = db.conn().execute(
        "DELETE FROM events WHERE ended_at >= ?1 AND started_at < ?2",
        params![start, end],
    )?;
    remove_screenshot_files(&paths);
    Ok(deleted)
}

pub fn delete_ids(db: &Database, ids: &[i64]) -> AppResult<usize> {
    let mut paths = Vec::new();
    for id in ids {
        paths.extend(screenshot_paths(db, "id = ?1", params![id])?);
    }
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
    remove_screenshot_files(&paths);
    Ok(n)
}

pub fn delete_all(db: &Database) -> AppResult<usize> {
    let paths = screenshot_paths(db, "1 = 1", ())?;
    let n = db.conn().execute("DELETE FROM events", [])?;
    remove_screenshot_files(&paths);
    db.compact()?;
    Ok(n)
}

pub fn purge_older_than(db: &Database, cutoff: i64) -> AppResult<usize> {
    let paths = screenshot_paths(db, "ended_at < ?1 AND kind <> 'screenshot'", [cutoff])?;
    let n = db.conn().execute(
        "DELETE FROM events WHERE ended_at < ?1 AND kind <> 'screenshot'",
        [cutoff],
    )?;
    remove_screenshot_files(&paths);
    Ok(n)
}

pub fn delete_screenshots(db: &Database) -> AppResult<usize> {
    let paths = screenshot_paths(db, "1 = 1", ())?;
    let n = db
        .conn()
        .execute("DELETE FROM events WHERE kind = 'screenshot'", [])?;
    remove_screenshot_files(&paths);
    Ok(n)
}

pub fn purge_screenshots_older_than(db: &Database, cutoff: i64) -> AppResult<usize> {
    let paths = screenshot_paths(db, "kind = 'screenshot' AND ended_at < ?1", [cutoff])?;
    let n = db.conn().execute(
        "DELETE FROM events WHERE kind = 'screenshot' AND ended_at < ?1",
        [cutoff],
    )?;
    remove_screenshot_files(&paths);
    Ok(n)
}

fn screenshot_paths(
    db: &Database,
    predicate: &str,
    values: impl rusqlite::Params,
) -> AppResult<Vec<String>> {
    let conn = db.conn();
    let mut stmt = conn.prepare(&format!(
        "SELECT file_path FROM events WHERE kind = 'screenshot' AND file_path IS NOT NULL AND {predicate}"
    ))?;
    let rows = stmt.query_map(values, |row| row.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn remove_screenshot_files(paths: &[String]) {
    for path in paths {
        super::screenshots::remove_stored_file(path);
    }
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

    #[test]
    fn searches_browser_titles_urls_and_dates() {
        let db = Database::open_in_memory().unwrap();
        insert_event(
            &db,
            &NewEvent {
                kind: crate::memory::EventKind::BrowserActivity,
                source: "chrome",
                started_at: 5_000,
                ended_at: 5_000,
                app_name: Some("Google Chrome".into()),
                app_id: Some("profile".into()),
                window_title: Some("SQLite FTS5 Documentation".into()),
                url: Some("https://sqlite.org/fts5.html".into()),
                file_path: None,
            },
        )
        .unwrap();
        insert_event(&db, &sample("Code", "sqlite_notes.rs", 10_000, 12_000)).unwrap();

        let browser = search_events(
            &db,
            &SearchQuery {
                text: "SQLite FTS".into(),
                start: Some(0),
                end: Some(9_000),
                kind: Some("browser_activity".into()),
                limit: None,
            },
        )
        .unwrap();
        assert_eq!(browser.len(), 1);
        assert_eq!(
            browser[0].url.as_deref(),
            Some("https://sqlite.org/fts5.html")
        );

        assert_eq!(
            search_events(
                &db,
                &SearchQuery {
                    text: "sqlite.org".into(),
                    start: None,
                    end: None,
                    kind: None,
                    limit: Some(10),
                },
            )
            .unwrap()
            .len(),
            1
        );
    }

    #[test]
    fn search_index_tracks_deletion() {
        let db = db_with_events();
        assert_eq!(
            search_events(
                &db,
                &SearchQuery {
                    text: "Rust".into(),
                    start: None,
                    end: None,
                    kind: None,
                    limit: None,
                },
            )
            .unwrap()
            .len(),
            1
        );
        delete_all(&db).unwrap();
        assert!(search_events(
            &db,
            &SearchQuery {
                text: "Rust".into(),
                start: None,
                end: None,
                kind: None,
                limit: None,
            },
        )
        .unwrap()
        .is_empty());
    }
}
