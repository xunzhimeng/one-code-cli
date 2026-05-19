use std::fs;
use std::path::PathBuf;

use crate::error::{OccError, OccResult};
use crate::output::{self, Table};
use crate::run_record;

use super::current_doc_root;

pub fn runs_list(config_arg: Option<&PathBuf>, limit: usize) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let mut table = Table::new(&[
        "RUN_ID",
        "SESSION_ID",
        "AGENT_ALIAS",
        "MODEL",
        "EFFORT",
        "SUCCESS",
        "CREATED_AT",
    ]);
    for entry in run_record::list(&doc_root, limit)? {
        table.add_row(vec![
            entry.run_id,
            entry.session_id,
            entry.profile,
            entry.model.unwrap_or_else(|| "-".to_string()),
            entry.effort.unwrap_or_else(|| "-".to_string()),
            entry.success.to_string(),
            entry.created_at.to_string(),
        ]);
    }
    table.print();
    Ok(())
}

pub fn runs_show(config_arg: Option<&PathBuf>, run_id: &str) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let entry = run_record::find(&doc_root, run_id)?.ok_or_else(|| {
        OccError::new("run_not_found", format!("Run '{}' was not found.", run_id))
    })?;
    let text = fs::read_to_string(&entry.metadata_path).map_err(|error| {
        OccError::io(
            "run_not_found",
            format!(
                "Failed to read '{}'",
                output::display_path(&entry.metadata_path)
            ),
            error,
        )
    })?;
    println!("{}", output::display_text(&text));
    Ok(())
}

pub fn runs_open(config_arg: Option<&PathBuf>, run_id: &str, print: bool) -> OccResult<()> {
    let doc_root = current_doc_root(config_arg)?;
    let entry = run_record::find(&doc_root, run_id)?.ok_or_else(|| {
        OccError::new("run_not_found", format!("Run '{}' was not found.", run_id))
    })?;
    println!("{}", output::display_path(&entry.result_path));
    if !print {
        let _ = open::that(&entry.result_path);
    }
    Ok(())
}
