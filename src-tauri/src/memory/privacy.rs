//! Exclusion rules. Every collected event passes through [`PrivacyFilter`]
//! before it is stored; excluded activity is dropped entirely.
//!
//! Matching deliberately errs toward excluding too much rather than too little
//! (for example, matching is case-insensitive on every platform).

use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{store, MemoryEvent};
use crate::error::{AppError, AppResult};
use crate::storage::{now_ms, Database};

const MAX_PATTERN_LEN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExclusionKind {
    App,
    Website,
    Folder,
    Title,
}

impl ExclusionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Website => "website",
            Self::Folder => "folder",
            Self::Title => "title",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "app" => Some(Self::App),
            "website" => Some(Self::Website),
            "folder" => Some(Self::Folder),
            "title" => Some(Self::Title),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exclusion {
    pub id: i64,
    pub kind: ExclusionKind,
    pub pattern: String,
    pub created_at: i64,
}

/// The fields of an activity that exclusion rules are checked against.
#[derive(Debug, Default, Clone, Copy)]
pub struct Candidate<'a> {
    pub app_name: Option<&'a str>,
    pub app_id: Option<&'a str>,
    pub window_title: Option<&'a str>,
    pub url: Option<&'a str>,
    pub file_path: Option<&'a str>,
}

impl<'a> From<&'a MemoryEvent> for Candidate<'a> {
    fn from(e: &'a MemoryEvent) -> Self {
        Self {
            app_name: e.app_name.as_deref(),
            app_id: e.app_id.as_deref(),
            window_title: e.window_title.as_deref(),
            url: e.url.as_deref(),
            file_path: e.file_path.as_deref(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct PrivacyFilter {
    rules: Vec<(ExclusionKind, String)>,
}

impl PrivacyFilter {
    pub fn new(exclusions: &[Exclusion]) -> Self {
        Self {
            rules: exclusions
                .iter()
                .map(|e| (e.kind, normalize_for_match(e.kind, &e.pattern)))
                .collect(),
        }
    }

    pub fn excludes(&self, c: &Candidate<'_>) -> bool {
        self.rules.iter().any(|(kind, p)| rule_matches(*kind, p, c))
    }
}

fn rule_matches(kind: ExclusionKind, pattern: &str, c: &Candidate<'_>) -> bool {
    let title = c.window_title.map(str::to_lowercase);
    match kind {
        ExclusionKind::App => {
            let by_name = c.app_name.is_some_and(|n| normalize_app(n) == pattern);
            let by_id = c.app_id.is_some_and(|id| {
                id.eq_ignore_ascii_case(pattern) || normalize_app(file_stem(id)) == pattern
            });
            by_name || by_id
        }
        ExclusionKind::Title => title.is_some_and(|t| t.contains(pattern)),
        ExclusionKind::Website => {
            let by_url = c
                .url
                .and_then(url_host)
                .is_some_and(|host| host == pattern || host.ends_with(&format!(".{pattern}")));
            by_url || title.is_some_and(|t| t.contains(pattern))
        }
        ExclusionKind::Folder => {
            let in_folder = |path: &str| {
                let path = normalize_path(path);
                path == pattern || path.starts_with(&format!("{pattern}/"))
            };
            c.file_path.is_some_and(in_folder)
                || title.is_some_and(|t| t.replace('\\', "/").contains(pattern))
        }
    }
}

fn normalize_for_match(kind: ExclusionKind, pattern: &str) -> String {
    match kind {
        ExclusionKind::App => normalize_app(pattern),
        ExclusionKind::Folder => normalize_path(pattern),
        ExclusionKind::Title | ExclusionKind::Website => pattern.trim().to_lowercase(),
    }
}

fn normalize_app(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    lower
        .strip_suffix(".exe")
        .or_else(|| lower.strip_suffix(".app"))
        .unwrap_or(&lower)
        .to_string()
}

fn normalize_path(path: &str) -> String {
    let p = path.trim().replace('\\', "/").to_lowercase();
    let trimmed = p.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".into()
    } else {
        trimmed.to_string()
    }
}

fn file_stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name)
}

fn url_host(url: &str) -> Option<String> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?.trim_end_matches('.').to_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Validates and canonicalizes a user-entered pattern before storing it.
pub fn validate_pattern(kind: ExclusionKind, raw: &str) -> AppResult<String> {
    let p = raw.trim();
    if p.is_empty() {
        return Err(AppError::invalid("Enter something to exclude"));
    }
    if p.chars().count() > MAX_PATTERN_LEN {
        return Err(AppError::invalid("That exclusion is too long"));
    }
    if p.chars().any(char::is_control) {
        return Err(AppError::invalid(
            "That exclusion contains invalid characters",
        ));
    }
    match kind {
        ExclusionKind::Website => {
            let host = url_host(p).unwrap_or_default();
            let host = host.strip_prefix("www.").unwrap_or(&host).to_string();
            let valid = !host.is_empty()
                && host.contains('.')
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
                && !host.starts_with('.')
                && !host.contains("..");
            if !valid {
                return Err(AppError::invalid("Enter a website like example.com"));
            }
            Ok(host)
        }
        ExclusionKind::Folder => {
            let normalized = p.replace('\\', "/");
            let absolute = normalized.starts_with('/')
                || normalized.starts_with("//")
                || (normalized.len() >= 3
                    && normalized.as_bytes()[0].is_ascii_alphabetic()
                    && &normalized[1..3] == ":/");
            if !absolute {
                return Err(AppError::invalid(
                    "Enter a full folder path, like /Users/me/Private or C:\\Private",
                ));
            }
            let trimmed = p.trim_end_matches(['/', '\\']);
            Ok(if trimmed.is_empty() {
                p[..1].to_string()
            } else {
                trimmed.to_string()
            })
        }
        ExclusionKind::App | ExclusionKind::Title => Ok(p.to_string()),
    }
}

pub fn list(db: &Database) -> AppResult<Vec<Exclusion>> {
    let conn = db.conn();
    let mut stmt = conn.prepare_cached(
        "SELECT id, kind, pattern, created_at FROM exclusions ORDER BY kind, pattern COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        let kind: String = r.get(1)?;
        Ok((r.get(0)?, kind, r.get(2)?, r.get(3)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, kind, pattern, created_at): (i64, String, String, i64) = row?;
        if let Some(kind) = ExclusionKind::parse(&kind) {
            out.push(Exclusion {
                id,
                kind,
                pattern,
                created_at,
            });
        }
    }
    Ok(out)
}

pub fn add(db: &Database, kind: ExclusionKind, raw: &str) -> AppResult<Exclusion> {
    let pattern = validate_pattern(kind, raw)?;
    let conn = db.conn();
    let created_at = now_ms();
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO exclusions (kind, pattern, created_at) VALUES (?1, ?2, ?3)",
        params![kind.as_str(), pattern, created_at],
    )?;
    if inserted == 0 {
        return Err(AppError::invalid("That exclusion already exists"));
    }
    Ok(Exclusion {
        id: conn.last_insert_rowid(),
        kind,
        pattern,
        created_at,
    })
}

pub fn remove(db: &Database, id: i64) -> AppResult<bool> {
    Ok(db
        .conn()
        .execute("DELETE FROM exclusions WHERE id = ?1", [id])?
        > 0)
}

/// Removes already-stored memories that match `filter`. Used when the user
/// adds a new exclusion so that it also applies to the past.
pub fn purge_matching(db: &Database, filter: &PrivacyFilter) -> AppResult<usize> {
    let mut ids = Vec::new();
    store::for_each_event(db, |e| {
        if filter.excludes(&Candidate::from(&e)) {
            ids.push(e.id);
        }
        Ok(())
    })?;
    store::delete_ids(db, &ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: ExclusionKind, pattern: &str) -> PrivacyFilter {
        PrivacyFilter::new(&[Exclusion {
            id: 1,
            kind,
            pattern: pattern.into(),
            created_at: 0,
        }])
    }

    fn app(name: &str) -> Candidate<'_> {
        Candidate {
            app_name: Some(name),
            ..Default::default()
        }
    }

    #[test]
    fn app_rules_match_name_and_executable() {
        let f = rule(ExclusionKind::App, "1Password");
        assert!(f.excludes(&app("1password")));
        assert!(f.excludes(&app("1Password.exe")));
        assert!(!f.excludes(&app("1Password Helper Notes")));
        assert!(f.excludes(&Candidate {
            app_name: Some("Something"),
            app_id: Some(r"C:\Program Files\1Password\1Password.exe"),
            ..Default::default()
        }));
        assert!(f.excludes(&Candidate {
            app_id: Some("/Applications/1Password.app"),
            ..Default::default()
        }));

        let bundle = rule(ExclusionKind::App, "com.example.secret");
        assert!(bundle.excludes(&Candidate {
            app_id: Some("com.example.Secret"),
            ..Default::default()
        }));
    }

    #[test]
    fn title_rules_are_case_insensitive_substrings() {
        let f = rule(ExclusionKind::Title, "Incognito");
        assert!(f.excludes(&Candidate {
            window_title: Some("New Tab - Google Chrome (incognito)"),
            ..Default::default()
        }));
        assert!(!f.excludes(&Candidate {
            window_title: Some("Rust docs"),
            ..Default::default()
        }));
        assert!(!f.excludes(&Candidate::default()));
    }

    #[test]
    fn website_rules_match_host_and_subdomains() {
        let f = rule(ExclusionKind::Website, "bank.com");
        let url = |u| Candidate {
            url: Some(u),
            ..Default::default()
        };
        assert!(f.excludes(&url("https://bank.com/login")));
        assert!(f.excludes(&url("https://secure.BANK.com:443/x")));
        assert!(f.excludes(&url("https://user:pw@bank.com")));
        assert!(!f.excludes(&url("https://notbank.com")));
        assert!(!f.excludes(&url("https://bank.com.evil.io")));
        assert!(f.excludes(&Candidate {
            window_title: Some("Login | bank.com - Chrome"),
            ..Default::default()
        }));
    }

    #[test]
    fn folder_rules_match_paths_inside_folder() {
        let f = rule(ExclusionKind::Folder, r"C:\Users\me\Private\");
        let file = |p| Candidate {
            file_path: Some(p),
            ..Default::default()
        };
        assert!(f.excludes(&file(r"c:\users\me\private\taxes.pdf")));
        assert!(f.excludes(&file("C:/Users/me/Private")));
        assert!(!f.excludes(&file(r"C:\Users\me\PrivateNotes\a.txt")));
        assert!(f.excludes(&Candidate {
            window_title: Some(r"taxes.pdf - C:\Users\me\Private - Explorer"),
            ..Default::default()
        }));
    }

    #[test]
    fn validates_patterns() {
        use ExclusionKind::*;
        assert!(validate_pattern(App, "  ").is_err());
        assert!(validate_pattern(App, &"x".repeat(300)).is_err());
        assert!(validate_pattern(Title, "bad\u{0007}").is_err());
        assert_eq!(validate_pattern(App, " Slack ").unwrap(), "Slack");

        assert_eq!(
            validate_pattern(Website, "https://www.Example.com/path?q").unwrap(),
            "example.com"
        );
        assert!(validate_pattern(Website, "not a site").is_err());
        assert!(validate_pattern(Website, "localhost").is_err());
        assert!(validate_pattern(Website, "a..b.com").is_err());

        assert_eq!(
            validate_pattern(Folder, "/Users/me/Private/").unwrap(),
            "/Users/me/Private"
        );
        assert_eq!(
            validate_pattern(Folder, r"C:\Secret\").unwrap(),
            r"C:\Secret"
        );
        assert_eq!(validate_pattern(Folder, "/").unwrap(), "/");
        assert!(validate_pattern(Folder, "relative/path").is_err());
        assert!(validate_pattern(Folder, "../etc").is_err());
    }

    #[test]
    fn crud_and_duplicates() {
        let db = Database::open_in_memory().unwrap();
        let before = list(&db).unwrap().len();
        let e = add(&db, ExclusionKind::App, "Slack").unwrap();
        assert!(
            add(&db, ExclusionKind::App, "slack").is_err(),
            "duplicates are case-insensitive"
        );
        assert_eq!(list(&db).unwrap().len(), before + 1);
        assert!(remove(&db, e.id).unwrap());
        assert!(!remove(&db, e.id).unwrap());
        assert_eq!(list(&db).unwrap().len(), before);
    }

    #[test]
    fn purges_existing_matching_memories() {
        let db = Database::open_in_memory().unwrap();
        store::insert_event(&db, &store::sample("Slack", "general", 1, 2)).unwrap();
        store::insert_event(&db, &store::sample("Code", "main.rs", 3, 4)).unwrap();
        let excl = add(&db, ExclusionKind::App, "slack").unwrap();
        let removed = purge_matching(&db, &PrivacyFilter::new(&[excl])).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store::stats(&db).unwrap().total_events, 1);
    }

    #[test]
    fn default_exclusions_cover_password_managers() {
        let db = Database::open_in_memory().unwrap();
        let f = PrivacyFilter::new(&list(&db).unwrap());
        assert!(f.excludes(&app("1Password")));
        assert!(f.excludes(&app("KeePassXC")));
        assert!(f.excludes(&Candidate {
            window_title: Some("Mozilla Firefox Private Browsing"),
            ..Default::default()
        }));
    }
}
