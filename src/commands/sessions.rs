use std::path::PathBuf;

use crate::error::{OccError, OccResult};
use crate::output::{self, Table};
use crate::session;

use super::{current_cwd, current_doc_root};

pub fn sessions_list(config_arg: Option<&PathBuf>, limit: usize) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let mut table = Table::new(&["SESSION_ID", "AGENT_ALIAS", "CLI_TYPE", "CWD", "UPDATED_AT"]);
    for entry in session::list(&doc_root, limit)? {
        table.add_row(vec![
            entry.session_id,
            entry.profile,
            entry.backend,
            output::display_path(&entry.cwd),
            entry.updated_at.to_string(),
        ]);
    }
    table.print();
    Ok(())
}

pub fn sessions_show(config_arg: Option<&PathBuf>, session_id: &str) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let session = session::load_by_id(&doc_root, session_id)?;
    let text = toml::to_string_pretty(&session).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize session: {}", error),
        )
    })?;
    println!("{}", output::display_text(&text));
    Ok(())
}

pub fn sessions_latest(
    config_arg: Option<&PathBuf>,
    profile: Option<String>,
    backend: Option<String>,
    cwd: Option<PathBuf>,
) -> OccResult<()> {
    let base_cwd = current_cwd()?;
    let doc_root = current_doc_root(config_arg)?;
    let cwd = cwd
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                base_cwd.join(path)
            }
        })
        .map(|path| path.canonicalize().unwrap_or(path));
    let entry = session::latest(
        &doc_root,
        profile.as_deref(),
        backend.as_deref(),
        cwd.as_deref(),
    )?
    .ok_or_else(|| OccError::new("session_not_found", "No matching session was found."))?;
    println!(
        "{}\t{}\t{}\t{}\t{}",
        entry.session_id,
        entry.profile,
        entry.backend,
        output::display_path(&entry.cwd),
        entry.updated_at
    );
    Ok(())
}

pub fn sessions_migrate(config_arg: Option<&PathBuf>) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let count = session::migrate_legacy(&doc_root)?;
    if count == 0 {
        println!("No legacy sessions found to migrate.");
    } else {
        println!("Migrated {} session(s) from JSONL to SQLite.", count);
    }
    Ok(())
}
