use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::error::{OccError, OccResult};
use crate::run_record::append_json_line;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub backend_session_id: Option<String>,
    pub profile: String,
    pub backend: String,
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub latest_run_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIndexEntry {
    pub session_id: String,
    pub profile: String,
    pub backend: String,
    pub cwd: PathBuf,
    pub session_path: PathBuf,
    pub updated_at: DateTime<Utc>,
}

impl SessionRecord {
    pub fn new(
        session_id: String,
        profile: String,
        backend: String,
        cwd: PathBuf,
        model: Option<String>,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            session_id,
            backend_session_id: None,
            profile,
            backend,
            cwd,
            model,
            latest_run_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn session_path(&self, doc_root: &Path) -> PathBuf {
        doc_root
            .join("sessions")
            .join(&self.session_id)
            .join("session.toml")
    }

    pub fn index_entry(&self, doc_root: &Path) -> SessionIndexEntry {
        SessionIndexEntry {
            session_id: self.session_id.clone(),
            profile: self.profile.clone(),
            backend: self.backend.clone(),
            cwd: self.cwd.clone(),
            session_path: self.session_path(doc_root),
            updated_at: self.updated_at,
        }
    }
}

pub fn save(doc_root: &Path, session: &SessionRecord) -> OccResult<()> {
    let session_dir = doc_root.join("sessions").join(&session.session_id);
    fs::create_dir_all(&session_dir).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to create '{}'", session_dir.display()),
            error,
        )
    })?;
    let text = toml::to_string_pretty(session).map_err(|error| {
        OccError::new(
            "config_parse_failed",
            format!("Failed to serialize session TOML: {}", error),
        )
    })?;
    fs::write(session_dir.join("session.toml"), text).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!(
                "Failed to write '{}'",
                session_dir.join("session.toml").display()
            ),
            error,
        )
    })?;
    append_json_line(
        &doc_root.join("session-index.jsonl"),
        &session.index_entry(doc_root),
    )?;
    if let Some(user_index) = user_session_index_path() {
        append_json_line(&user_index, &session.index_entry(doc_root))?;
    }
    Ok(())
}

pub fn append_run(
    doc_root: &Path,
    session_id: &str,
    run_id: &str,
    created_at: DateTime<Utc>,
) -> OccResult<()> {
    let path = doc_root
        .join("sessions")
        .join(session_id)
        .join("runs.jsonl");
    append_json_line(
        &path,
        &serde_json::json!({
            "run_id": run_id,
            "created_at": created_at,
        }),
    )
}

pub fn load_from_path(path: &Path) -> OccResult<SessionRecord> {
    let text = fs::read_to_string(path).map_err(|error| {
        OccError::io(
            "session_not_found",
            format!("Failed to read '{}'", path.display()),
            error,
        )
    })?;
    toml::from_str(&text).map_err(|error| {
        OccError::new(
            "config_parse_failed",
            format!("Failed to parse '{}': {}", path.display(), error),
        )
    })
}

pub fn load_by_id(doc_root: &Path, session_id: &str) -> OccResult<SessionRecord> {
    let local_path = doc_root
        .join("sessions")
        .join(session_id)
        .join("session.toml");
    if local_path.exists() {
        return load_from_path(&local_path);
    }

    for entry in all_index_entries(doc_root)? {
        if entry.session_id == session_id && entry.session_path.exists() {
            return load_from_path(&entry.session_path);
        }
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
    let cwd_string = cwd.map(normalize_path);
    let mut entries = all_index_entries(doc_root)?;
    entries.sort_by_key(|entry| entry.updated_at);
    entries.reverse();
    Ok(entries.into_iter().find(|entry| {
        profile.map(|value| entry.profile == value).unwrap_or(true)
            && backend.map(|value| entry.backend == value).unwrap_or(true)
            && cwd_string
                .as_deref()
                .map(|value| normalize_path(&entry.cwd) == value)
                .unwrap_or(true)
    }))
}

pub fn list(doc_root: &Path, limit: usize) -> OccResult<Vec<SessionIndexEntry>> {
    let mut entries = all_index_entries(doc_root)?;
    entries.sort_by_key(|entry| entry.updated_at);
    entries.reverse();
    entries.truncate(limit);
    Ok(entries)
}

pub fn all_index_entries(doc_root: &Path) -> OccResult<Vec<SessionIndexEntry>> {
    let mut entries = read_index(&doc_root.join("session-index.jsonl"))?;
    if let Some(user_index) = user_session_index_path() {
        entries.extend(read_index(&user_index)?);
    }
    let mut deduped = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in entries.into_iter().rev() {
        if seen.insert(entry.session_id.clone()) {
            deduped.push(entry);
        }
    }
    deduped.reverse();
    Ok(deduped)
}

pub fn user_session_index_path() -> Option<PathBuf> {
    BaseDirs::new().map(|base_dirs| {
        base_dirs
            .home_dir()
            .join(".occ")
            .join("session-index.jsonl")
    })
}

fn read_index(path: &Path) -> OccResult<Vec<SessionIndexEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|error| {
        OccError::io(
            "config_parse_failed",
            format!("Failed to read '{}'", path.display()),
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

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}
