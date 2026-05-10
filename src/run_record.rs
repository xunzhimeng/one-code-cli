use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{OccError, OccResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub session_id: String,
    pub profile: String,
    pub backend: String,
    pub model: Option<String>,
    pub cwd: PathBuf,
    pub prompt_source: String,
    pub interactive: bool,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub result_path: PathBuf,
    pub metadata_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunIndexEntry {
    pub run_id: String,
    pub session_id: String,
    pub profile: String,
    pub backend: String,
    pub cwd: PathBuf,
    pub success: bool,
    pub result_path: PathBuf,
    pub metadata_path: PathBuf,
    pub created_at: DateTime<Utc>,
}

impl From<&RunRecord> for RunIndexEntry {
    fn from(record: &RunRecord) -> Self {
        Self {
            run_id: record.run_id.clone(),
            session_id: record.session_id.clone(),
            profile: record.profile.clone(),
            backend: record.backend.clone(),
            cwd: record.cwd.clone(),
            success: record.success,
            result_path: record.result_path.clone(),
            metadata_path: record.metadata_path.clone(),
            created_at: record.started_at,
        }
    }
}

pub fn append_index(doc_root: &Path, record: &RunRecord) -> OccResult<()> {
    let path = doc_root.join("index.jsonl");
    append_json_line(&path, &RunIndexEntry::from(record))
}

pub fn list(doc_root: &Path, limit: usize) -> OccResult<Vec<RunIndexEntry>> {
    let path = doc_root.join("index.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path).map_err(|error| {
        OccError::io(
            "config_parse_failed",
            format!("Failed to read '{}'", path.display()),
            error,
        )
    })?;
    let mut entries = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(entry) = serde_json::from_str::<RunIndexEntry>(line) {
            entries.push(entry);
        }
    }
    entries.reverse();
    entries.truncate(limit);
    Ok(entries)
}

pub fn find(doc_root: &Path, run_id: &str) -> OccResult<Option<RunIndexEntry>> {
    Ok(list(doc_root, usize::MAX)?
        .into_iter()
        .find(|entry| entry.run_id == run_id))
}

pub fn append_json_line<T: Serialize>(path: &Path, value: &T) -> OccResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to create '{}'", parent.display()),
                error,
            )
        })?;
    }
    let line = serde_json::to_string(value).map_err(|error| {
        OccError::new(
            "config_parse_failed",
            format!("Failed to serialize JSON: {}", error),
        )
    })?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!("Failed to open '{}'", path.display()),
                error,
            )
        })?;
    use std::io::Write;
    writeln!(file, "{}", line).map_err(|error| {
        OccError::io(
            "doc_root_not_writable",
            format!("Failed to write '{}'", path.display()),
            error,
        )
    })
}
