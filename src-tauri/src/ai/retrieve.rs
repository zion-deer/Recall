//! Turns a natural-language memory question into a small set of recorded events.
//! Time phrases are interpreted directly so the small local model never has to
//! search the database itself.

use chrono::{DateTime, Local, NaiveTime, TimeZone};

use crate::error::AppResult;
use crate::memory::store::{self, EventQuery, SearchQuery};
use crate::memory::MemoryEvent;
use crate::storage::Database;

const MAX_MEMORIES: usize = 12;
const MAX_CONTEXT_CHARS: usize = 3_200;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TimeRange {
    start: i64,
    end: i64,
}

pub struct Retrieval {
    pub events: Vec<MemoryEvent>,
    pub context: String,
}

pub fn retrieve(db: &Database, question: &str, now_ms: i64) -> AppResult<Retrieval> {
    let now = Local
        .timestamp_millis_opt(now_ms)
        .single()
        .unwrap_or_else(Local::now);
    let range = time_range(question, now);
    let keywords = keywords(question);
    let mut events = if keywords.is_empty() {
        store::list_events(
            db,
            &EventQuery {
                start: range.as_ref().map(|r| r.start),
                end: range.as_ref().map(|r| r.end),
                limit: Some(40),
                ..EventQuery::default()
            },
        )?
    } else {
        store::search_events(
            db,
            &SearchQuery {
                text: keywords,
                start: range.as_ref().map(|r| r.start),
                end: range.as_ref().map(|r| r.end),
                kind: None,
                limit: Some(40),
            },
        )?
    };
    events = dedupe(events);
    events.truncate(MAX_MEMORIES);
    let context = build_context(&events);
    Ok(Retrieval { events, context })
}

pub const INSUFFICIENT_MEMORY: &str = "I don't have enough recorded information to answer that.";

pub fn build_context(events: &[MemoryEvent]) -> String {
    let mut lines = Vec::new();
    let mut used = 0usize;
    for event in events {
        let when = Local
            .timestamp_millis_opt(event.started_at)
            .single()
            .map(|time| time.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown time".into());
        let mut line = format!(
            "- {when} | {} | {}",
            event.app_name.as_deref().unwrap_or("Unknown app"),
            event.window_title.as_deref().unwrap_or("untitled")
        );
        if let Some(url) = &event.url {
            line.push_str(" | ");
            line.push_str(url);
        }
        if event.kind == "screenshot" {
            line.push_str(" | screenshot");
        }
        if line.chars().count() > 360 {
            line = line.chars().take(360).collect();
        }
        if used + line.len() > MAX_CONTEXT_CHARS {
            break;
        }
        used += line.len();
        lines.push(line);
    }
    lines.join("\n")
}

fn dedupe(events: Vec<MemoryEvent>) -> Vec<MemoryEvent> {
    let mut out = Vec::new();
    for event in events {
        let duplicate = out.iter().any(|prior: &MemoryEvent| {
            prior.kind == event.kind
                && prior.app_name == event.app_name
                && prior.window_title == event.window_title
                && prior.url == event.url
                && event.started_at.abs_diff(prior.started_at) < 30 * 60_000
        });
        if !duplicate {
            out.push(event);
        }
    }
    out
}

fn keywords(question: &str) -> String {
    const DROP: &[&str] = &[
        "what",
        "was",
        "were",
        "i",
        "me",
        "my",
        "doing",
        "do",
        "did",
        "working",
        "on",
        "the",
        "a",
        "an",
        "about",
        "around",
        "yesterday",
        "today",
        "morning",
        "afternoon",
        "evening",
        "between",
        "and",
        "last",
        "week",
        "this",
        "at",
        "pm",
        "am",
        "when",
        "where",
        "that",
        "show",
        "find",
        "please",
        "recall",
        "summarize",
        "summary",
        "website",
        "websites",
        "looking",
        "page",
        "browser",
        "file",
        "files",
        "article",
        "information",
        "using",
        "seen",
        "read",
        "open",
        "opened",
    ];
    question
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.len() > 2 && !DROP.contains(&word.to_lowercase().as_str()))
        .filter(|word| word.parse::<u32>().is_err())
        .take(8)
        .collect::<Vec<_>>()
        .join(" ")
}

fn time_range(question: &str, now: DateTime<Local>) -> Option<TimeRange> {
    let text = question.to_lowercase();
    let today = start_of_day(now);
    let tomorrow = today + chrono::Duration::days(1);
    if text.contains("yesterday") {
        return Some(TimeRange {
            start: (today - chrono::Duration::days(1)).timestamp_millis(),
            end: today.timestamp_millis(),
        });
    }
    if text.contains("this morning") {
        return Some(clock_window(today, 5, 0, 12, 0));
    }
    if text.contains("this afternoon") {
        return Some(clock_window(today, 12, 0, 17, 0));
    }
    if text.contains("this evening") || text.contains("tonight") {
        return Some(clock_window(today, 17, 0, 23, 59));
    }
    if text.contains("today") || text.contains("this morning") {
        return Some(TimeRange {
            start: today.timestamp_millis(),
            end: tomorrow.timestamp_millis(),
        });
    }
    if text.contains("last week") || text.contains("past week") {
        return Some(TimeRange {
            start: (now - chrono::Duration::days(7)).timestamp_millis(),
            end: now.timestamp_millis(),
        });
    }
    if let Some(range) = between_range(&text, today) {
        return Some(range);
    }
    around_range(&text, today)
}

fn between_range(text: &str, day: DateTime<Local>) -> Option<TimeRange> {
    let marker = "between ";
    let start = text.find(marker)?;
    let rest = &text[start + marker.len()..];
    let (left, right) = rest.split_once(" and ")?;
    let start_clock = parse_clock(left)?;
    let end_clock = parse_clock(right)?;
    Some(TimeRange {
        start: at_clock(day, start_clock).timestamp_millis(),
        end: at_clock(day, end_clock).timestamp_millis(),
    })
}

fn around_range(text: &str, day: DateTime<Local>) -> Option<TimeRange> {
    let marker = text.find("around ").or_else(|| text.find("at "))?;
    let word = if text[marker..].starts_with("around ") {
        "around "
    } else {
        "at "
    };
    parse_clock(&text[marker + word.len()..]).map(|clock| {
        let center = at_clock(day, clock);
        TimeRange {
            start: (center - chrono::Duration::minutes(45)).timestamp_millis(),
            end: (center + chrono::Duration::minutes(45)).timestamp_millis(),
        }
    })
}

fn parse_clock(text: &str) -> Option<NaiveTime> {
    let token = text
        .split_whitespace()
        .next()?
        .trim_matches(|c: char| !c.is_ascii_digit() && c != ':');
    let afternoon = text.to_lowercase().contains("pm");
    let morning = text.to_lowercase().contains("am");
    let (hour, minute) = if let Some((h, m)) = token.split_once(':') {
        (h.parse().ok()?, m.parse().ok()?)
    } else {
        (token.parse().ok()?, 0)
    };
    let hour = match hour {
        1..=11 if afternoon => hour + 12,
        12 if morning => 0,
        0..=23 => hour,
        _ => return None,
    };
    NaiveTime::from_hms_opt(hour, minute, 0)
}

fn clock_window(day: DateTime<Local>, sh: u32, sm: u32, eh: u32, em: u32) -> TimeRange {
    TimeRange {
        start: at_clock(day, NaiveTime::from_hms_opt(sh, sm, 0).unwrap()).timestamp_millis(),
        end: at_clock(day, NaiveTime::from_hms_opt(eh, em, 0).unwrap()).timestamp_millis(),
    }
}

fn at_clock(day: DateTime<Local>, clock: NaiveTime) -> DateTime<Local> {
    day.date_naive()
        .and_time(clock)
        .and_local_timezone(Local)
        .single()
        .unwrap_or(day)
}

fn start_of_day(now: DateTime<Local>) -> DateTime<Local> {
    now.date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|time| time.and_local_timezone(Local).single())
        .unwrap_or(now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{EventKind, NewEvent};
    use chrono::Timelike;

    fn event(title: &str, url: Option<&str>, at: i64) -> NewEvent {
        NewEvent {
            kind: if url.is_some() {
                EventKind::BrowserActivity
            } else {
                EventKind::AppActivity
            },
            source: "test",
            started_at: at,
            ended_at: at,
            app_name: Some(
                if url.is_some() {
                    "Google Chrome"
                } else {
                    "VS Code"
                }
                .into(),
            ),
            app_id: None,
            window_title: Some(title.into()),
            url: url.map(str::to_string),
            file_path: None,
        }
    }

    #[test]
    fn answers_from_relevant_browser_and_app_records() {
        let db = Database::open_in_memory().unwrap();
        let now = Local.with_ymd_and_hms(2026, 10, 3, 18, 0, 0).unwrap();
        let yesterday = now - chrono::Duration::days(1);
        store::insert_event(
            &db,
            &event(
                "SQLite FTS5 Documentation",
                Some("https://sqlite.org/fts5.html"),
                yesterday.timestamp_millis(),
            ),
        )
        .unwrap();
        store::insert_event(
            &db,
            &event(
                "recall/src/search.rs",
                None,
                yesterday.timestamp_millis() + 60_000,
            ),
        )
        .unwrap();
        store::insert_event(&db, &event("Unrelated mail", None, now.timestamp_millis())).unwrap();

        let found = retrieve(
            &db,
            "What website was I looking at about SQLite yesterday?",
            now.timestamp_millis(),
        )
        .unwrap();
        assert_eq!(found.events.len(), 1);
        assert!(found.context.contains("sqlite.org/fts5.html"));
        assert!(!found.context.contains("Unrelated"));
    }

    #[test]
    fn empty_context_is_explicit() {
        let db = Database::open_in_memory().unwrap();
        let found = retrieve(
            &db,
            "What was I doing yesterday?",
            chrono::Local::now().timestamp_millis(),
        )
        .unwrap();
        assert!(found.events.is_empty());
        assert!(found.context.is_empty());
    }

    #[test]
    fn parses_a_clock_range() {
        let now = Local.with_ymd_and_hms(2026, 10, 3, 18, 0, 0).unwrap();
        let range = time_range("Summarize what I was doing between 2 PM and 4 PM", now).unwrap();
        let start = Local.timestamp_millis_opt(range.start).single().unwrap();
        let end = Local.timestamp_millis_opt(range.end).single().unwrap();
        assert_eq!((start.hour(), end.hour()), (14, 16));
    }
}
