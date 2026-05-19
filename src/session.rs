use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use directories::BaseDirs;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::error::{OccError, OccResult};
use crate::output;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub backend_session_id: Option<String>,
    #[serde(rename = "agent")]
    pub profile: String,
    #[serde(rename = "cli")]
    pub backend: String,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub latest_run_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIndexEntry {
    pub session_id: String,
    #[serde(rename = "agent")]
    pub profile: String,
    #[serde(rename = "cli")]
    pub backend: String,
    pub cwd: PathBuf,
    #[serde(default)]
    pub session_path: Option<PathBuf>,
    pub updated_at: DateTime<Utc>,
}

struct SessionRow {
    session_id: String,
    backend_session_id: Option<String>,
    profile: String,
    backend: String,
    cwd: String,
    model: Option<String>,
    effort: Option<String>,
    latest_run_id: Option<String>,
    created_at: String,
    updated_at: String,
}

struct SessionEntryRow {
    session_id: String,
    profile: String,
    backend: String,
    cwd: String,
    updated_at: String,
}

impl SessionRecord {
    pub fn new(
        session_id: String,
        profile: String,
        backend: String,
        cwd: PathBuf,
        model: Option<String>,
        effort: Option<String>,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            session_id,
            backend_session_id: None,
            profile,
            backend,
            cwd,
            model,
            effort,
            latest_run_id: None,
            created_at: now,
            updated_at: now,
        }
    }
}

impl SessionRow {
    fn into_record(self) -> OccResult<SessionRecord> {
        Ok(SessionRecord {
            session_id: self.session_id,
            backend_session_id: self.backend_session_id,
            profile: self.profile,
            backend: self.backend,
            cwd: PathBuf::from(self.cwd),
            model: self.model,
            effort: self.effort,
            latest_run_id: self.latest_run_id,
            created_at: parse_time(&self.created_at, "created_at")?,
            updated_at: parse_time(&self.updated_at, "updated_at")?,
        })
    }
}

impl SessionEntryRow {
    fn into_entry(self) -> OccResult<SessionIndexEntry> {
        Ok(SessionIndexEntry {
            session_id: self.session_id,
            profile: self.profile,
            backend: self.backend,
            cwd: PathBuf::from(self.cwd),
            session_path: None,
            updated_at: parse_time(&self.updated_at, "updated_at")?,
        })
    }
}

pub fn save(session: &SessionRecord) -> OccResult<()> {
    let conn = open_user_db()?;
    let cwd = path_to_string(&session.cwd);
    let cwd_key = normalize_path(&session.cwd);
    let created_at = format_time(&session.created_at);
    let updated_at = format_time(&session.updated_at);
    conn.execute(
        r#"INSERT INTO sessions (
            session_id,
            backend_session_id,
            profile,
            backend,
            cwd,
            cwd_key,
            model,
            effort,
            latest_run_id,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(session_id) DO UPDATE SET
            backend_session_id = excluded.backend_session_id,
            profile = excluded.profile,
            backend = excluded.backend,
            cwd = excluded.cwd,
            cwd_key = excluded.cwd_key,
            model = excluded.model,
            effort = excluded.effort,
            latest_run_id = excluded.latest_run_id,
            created_at = excluded.created_at,
            updated_at = excluded.updated_at"#,
        params![
            &session.session_id,
            session.backend_session_id.as_deref(),
            &session.profile,
            &session.backend,
            &cwd,
            &cwd_key,
            session.model.as_deref(),
            session.effort.as_deref(),
            session.latest_run_id.as_deref(),
            &created_at,
            &updated_at,
        ],
    )
    .map_err(|error| sqlite_error("Failed to save session", error))?;
    Ok(())
}

pub fn append_run(session_id: &str, run_id: &str, created_at: DateTime<Utc>) -> OccResult<()> {
    let conn = open_user_db()?;
    let created_at = format_time(&created_at);
    conn.execute(
        r#"INSERT OR REPLACE INTO session_runs (
            session_id,
            run_id,
            created_at
        ) VALUES (?1, ?2, ?3)"#,
        params![session_id, run_id, &created_at],
    )
    .map_err(|error| sqlite_error("Failed to save session run", error))?;
    Ok(())
}

pub fn load_from_path(path: &Path) -> OccResult<SessionRecord> {
    let text = fs::read_to_string(path).map_err(|error| {
        OccError::io(
            "session_not_found",
            format!("Failed to read '{}'", output::display_path(path)),
            error,
        )
    })?;
    toml::from_str(&text).map_err(|error| {
        OccError::new(
            "session_parse_failed",
            format!(
                "Failed to parse '{}': {}",
                output::display_path(path),
                error
            ),
        )
    })
}

pub fn load_by_id(doc_root: &Path, session_id: &str) -> OccResult<SessionRecord> {
    if let Some(session) = load_from_db(session_id)? {
        return Ok(session);
    }
    if let Some(session) = load_from_legacy(doc_root, session_id)? {
        return Ok(session);
    }

    Err(OccError::new(
        "session_not_found",
        format!("Session '{}' was not found.", session_id),
    ))
}

pub fn latest(
    doc_root: &Path,
    profile: Option<&str>,
    backend: Option<&str>,
    cwd: Option<&Path>,
) -> OccResult<Option<SessionIndexEntry>> {
    let cwd_key = cwd.map(normalize_path);
    let mut entries = db_entries(profile, backend, cwd, None)?;
    entries.extend(
        legacy_index_entries(doc_root)?
            .into_iter()
            .filter(|entry| matches_entry(entry, profile, backend, cwd_key.as_deref())),
    );
    Ok(sort_and_dedupe(entries).into_iter().next())
}

pub fn list(doc_root: &Path, limit: usize) -> OccResult<Vec<SessionIndexEntry>> {
    let mut entries = all_index_entries(doc_root)?;
    entries.truncate(limit);
    Ok(entries)
}

pub fn all_index_entries(doc_root: &Path) -> OccResult<Vec<SessionIndexEntry>> {
    let mut entries = db_entries(None, None, None, None)?;
    entries.extend(legacy_index_entries(doc_root)?);
    Ok(sort_and_dedupe(entries))
}

pub fn user_session_db_path() -> OccResult<PathBuf> {
    BaseDirs::new()
        .map(|base_dirs| base_dirs.home_dir().join(".occ").join("sessions.sqlite"))
        .ok_or_else(|| {
            OccError::new(
                "home_not_found",
                "Unable to locate the user home directory.",
            )
        })
}

pub fn user_session_index_path() -> Option<PathBuf> {
    BaseDirs::new().map(|base_dirs| {
        base_dirs
            .home_dir()
            .join(".occ")
            .join("session-index.jsonl")
    })
}

pub fn check_user_store() -> OccResult<PathBuf> {
    let path = user_session_db_path()?;
    let _conn = open_user_db()?;
    Ok(path)
}

fn open_user_db() -> OccResult<Connection> {
    let path = user_session_db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OccError::io(
                "session_store_not_writable",
                format!("Failed to create '{}'", output::display_path(parent)),
                error,
            )
        })?;
    }
    let conn = Connection::open(&path).map_err(|error| {
        sqlite_error(
            format!("Failed to open '{}'", output::display_path(&path)),
            error,
        )
    })?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|error| sqlite_error("Failed to set WAL journal mode", error))?;
    init_db(&conn)?;
    Ok(conn)
}

fn init_db(conn: &Connection) -> OccResult<()> {
    conn.execute_batch(
        r#"CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY NOT NULL,
            backend_session_id TEXT,
            profile TEXT NOT NULL,
            backend TEXT NOT NULL,
            cwd TEXT NOT NULL,
            cwd_key TEXT NOT NULL,
            model TEXT,
            effort TEXT,
            latest_run_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
            ON sessions(updated_at);
        CREATE INDEX IF NOT EXISTS idx_sessions_profile_backend_cwd_updated
            ON sessions(profile, backend, cwd_key, updated_at);
        CREATE TABLE IF NOT EXISTS session_runs (
            session_id TEXT NOT NULL,
            run_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY(session_id, run_id)
        );
        CREATE INDEX IF NOT EXISTS idx_session_runs_session_created
            ON session_runs(session_id, created_at);"#,
    )
    .map_err(|error| sqlite_error("Failed to initialize session store", error))?;
    let _ = conn.execute("ALTER TABLE sessions ADD COLUMN effort TEXT", []);
    Ok(())
}

fn load_from_db(session_id: &str) -> OccResult<Option<SessionRecord>> {
    let conn = open_user_db()?;
    let mut stmt = conn
        .prepare(
            r#"SELECT
                session_id,
                backend_session_id,
                profile,
                backend,
                cwd,
                model,
                effort,
                latest_run_id,
                created_at,
                updated_at
            FROM sessions
            WHERE session_id = ?1"#,
        )
        .map_err(|error| sqlite_error("Failed to prepare session lookup", error))?;
    let row = stmt
        .query_row(params![session_id], read_session_row)
        .optional()
        .map_err(|error| sqlite_error("Failed to query session", error))?;
    row.map(SessionRow::into_record).transpose()
}

fn db_entries(
    profile: Option<&str>,
    backend: Option<&str>,
    cwd: Option<&Path>,
    limit: Option<usize>,
) -> OccResult<Vec<SessionIndexEntry>> {
    let conn = open_user_db()?;
    let cwd_key = cwd.map(normalize_path);
    let limit = limit.map(|value| value as i64).unwrap_or(-1);
    let mut stmt = conn
        .prepare(
            r#"SELECT
                session_id,
                profile,
                backend,
                cwd,
                updated_at
            FROM sessions
            WHERE (?1 IS NULL OR profile = ?1)
              AND (?2 IS NULL OR backend = ?2)
              AND (?3 IS NULL OR cwd_key = ?3)
            ORDER BY updated_at DESC
            LIMIT ?4"#,
        )
        .map_err(|error| sqlite_error("Failed to prepare session list", error))?;
    let rows = stmt
        .query_map(
            params![profile, backend, cwd_key.as_deref(), limit],
            read_session_entry_row,
        )
        .map_err(|error| sqlite_error("Failed to query sessions", error))?;
    let mut entries = Vec::new();
    for row in rows {
        entries.push(
            row.map_err(|error| sqlite_error("Failed to read session", error))?
                .into_entry()?,
        );
    }
    Ok(entries)
}

fn read_session_row(row: &Row<'_>) -> rusqlite::Result<SessionRow> {
    Ok(SessionRow {
        session_id: row.get(0)?,
        backend_session_id: row.get(1)?,
        profile: row.get(2)?,
        backend: row.get(3)?,
        cwd: row.get(4)?,
        model: row.get(5)?,
        effort: row.get(6)?,
        latest_run_id: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn read_session_entry_row(row: &Row<'_>) -> rusqlite::Result<SessionEntryRow> {
    Ok(SessionEntryRow {
        session_id: row.get(0)?,
        profile: row.get(1)?,
        backend: row.get(2)?,
        cwd: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn load_from_legacy(doc_root: &Path, session_id: &str) -> OccResult<Option<SessionRecord>> {
    let local_path = doc_root
        .join("sessions")
        .join(session_id)
        .join("session.toml");
    if local_path.exists() {
        return load_from_path(&local_path).map(Some);
    }

    for entry in legacy_index_entries(doc_root)? {
        if entry.session_id == session_id {
            if let Some(path) = entry.session_path {
                if path.exists() {
                    return load_from_path(&path).map(Some);
                }
            }
        }
    }
    Ok(None)
}

fn legacy_index_entries(doc_root: &Path) -> OccResult<Vec<SessionIndexEntry>> {
    let mut entries = read_index(&doc_root.join("session-index.jsonl"))?;
    if let Some(user_index) = user_session_index_path() {
        entries.extend(read_index(&user_index)?);
    }
    Ok(entries)
}

fn read_index(path: &Path) -> OccResult<Vec<SessionIndexEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|error| {
        OccError::io(
            "session_parse_failed",
            format!("Failed to read '{}'", output::display_path(path)),
            error,
        )
    })?;
    let mut entries = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(entry) = serde_json::from_str::<SessionIndexEntry>(line) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

fn matches_entry(
    entry: &SessionIndexEntry,
    profile: Option<&str>,
    backend: Option<&str>,
    cwd_key: Option<&str>,
) -> bool {
    profile.map(|value| entry.profile == value).unwrap_or(true)
        && backend.map(|value| entry.backend == value).unwrap_or(true)
        && cwd_key
            .map(|value| normalize_path(&entry.cwd) == value)
            .unwrap_or(true)
}

fn sort_and_dedupe(mut entries: Vec<SessionIndexEntry>) -> Vec<SessionIndexEntry> {
    entries.sort_by_key(|entry| entry.updated_at);
    entries.reverse();
    let mut deduped = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries {
        if seen.insert(entry.session_id.clone()) {
            deduped.push(entry);
        }
    }
    deduped
}

fn format_time(value: &DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn parse_time(value: &str, field: &str) -> OccResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| {
            OccError::new(
                "timestamp_parse_failed",
                format!("Failed to parse session {} '{}': {}", field, value, error),
            )
        })
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn sqlite_error(action: impl Into<String>, error: rusqlite::Error) -> OccError {
    OccError::new(
        "session_store_failed",
        format!("{}: {}", action.into(), error),
    )
}

/// Migrate legacy session-index.jsonl entries into the SQLite session store.
/// Returns the number of sessions imported.
pub fn migrate_legacy(doc_root: &Path) -> OccResult<usize> {
    let entries = legacy_index_entries(doc_root)?;
    if entries.is_empty() {
        return Ok(0);
    }
    let conn = open_user_db()?;
    let mut count = 0;
    for entry in &entries {
        // Check if already in SQLite
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM sessions WHERE session_id = ?1",
                params![entry.session_id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if exists {
            continue;
        }
        // Try to load full record from TOML file
        if let Some(path) = &entry.session_path {
            if path.exists() {
                if let Ok(record) = load_from_path(path) {
                    save(&record)?;
                    count += 1;
                    continue;
                }
            }
        }
        // Otherwise create a minimal record from the index entry
        let record = SessionRecord {
            session_id: entry.session_id.clone(),
            backend_session_id: None,
            profile: entry.profile.clone(),
            backend: entry.backend.clone(),
            cwd: entry.cwd.clone(),
            model: None,
            effort: None,
            latest_run_id: None,
            created_at: entry.updated_at,
            updated_at: entry.updated_at,
        };
        save(&record)?;
        count += 1;
    }
    Ok(count)
}
