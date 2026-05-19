use std::io::{self, Write};
use std::path::{Path, PathBuf};

use colored::Colorize;
use serde::{Serialize, Serializer};

use crate::cli::OutputMode;
use crate::error::{OccError, OccResult};

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct RunResponse {
    pub success: bool,
    pub run_id: String,
    pub session_id: String,
    #[serde(rename = "agent")]
    pub profile: String,
    #[serde(rename = "cli")]
    pub backend: String,
    pub model: Option<String>,
    pub model_source: String,
    pub effort: Option<String>,
    pub effort_source: String,
    #[serde(serialize_with = "serialize_display_path")]
    pub cwd: PathBuf,
    #[serde(serialize_with = "serialize_display_path")]
    pub result_path: PathBuf,
    #[serde(serialize_with = "serialize_display_path")]
    pub metadata_path: PathBuf,
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Serialize)]
pub struct BatchRunError {
    pub agent: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub success: bool,
    pub batch_id: String,
    pub runs: Vec<RunResponse>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<BatchRunError>,
}

pub fn display_text(value: &str) -> String {
    value
        .replace(r"\\\\?\\\UNC\\\\", r"\\\\")
        .replace(r"\\?\UNC\\", r"\\")
        .replace(r"\\\\?\\\\", "")
        .replace(r"\\?\\", "")
}

pub fn display_path(path: &Path) -> String {
    display_text(&path.display().to_string())
}

fn serialize_display_path<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&display_path(path))
}

pub fn print_run_response(mode: OutputMode, response: &RunResponse) -> OccResult<()> {
    match mode {
        OutputMode::Text => {
            println!(
                "{} {}",
                "success:".bold(),
                if response.success {
                    response.success.to_string().green().to_string()
                } else {
                    response.success.to_string().red().to_string()
                }
            );
            println!("{} {}", "run_id:".bold(), response.run_id);
            println!("{} {}", "session_id:".bold(), response.session_id);
            println!("{} {}", "agent:".bold(), response.profile);
            println!("{} {}", "cli:".bold(), response.backend);
            println!("{} {}", "model_source:".bold(), response.model_source);
            if let Some(model) = &response.model {
                println!("{} {}", "model:".bold(), model);
            }
            println!("{} {}", "effort_source:".bold(), response.effort_source);
            if let Some(effort) = &response.effort {
                println!("{} {}", "effort:".bold(), effort);
            }
            println!("{} {}", "cwd:".bold(), display_path(&response.cwd).dimmed());
            println!(
                "{} {}",
                "result_path:".bold(),
                display_path(&response.result_path).cyan()
            );
            if let Some(exit_code) = response.exit_code {
                println!("{} {}", "exit_code:".bold(), exit_code);
            }
            if let Some(error) = &response.error {
                println!(
                    "{} {}: {}",
                    "error:".red().bold(),
                    error.code,
                    error.message
                );
            }
        }
        OutputMode::Json => {
            let text = serde_json::to_string_pretty(response).map_err(|error| {
                OccError::new(
                    "serialization_failed",
                    format!("Failed to serialize JSON output: {}", error),
                )
            })?;
            println!("{}", display_text(&text));
        }
        OutputMode::Path => println!("{}", display_path(&response.result_path)),
    }
    Ok(())
}

pub fn print_batch_response(mode: OutputMode, response: &BatchResponse) -> OccResult<()> {
    match mode {
        OutputMode::Json => {
            let text = serde_json::to_string_pretty(response).map_err(|error| {
                OccError::new(
                    "serialization_failed",
                    format!("Failed to serialize batch JSON output: {}", error),
                )
            })?;
            println!("{}", display_text(&text));
        }
        OutputMode::Path => {
            for run in &response.runs {
                println!("{}\t{}", run.profile, display_path(&run.result_path));
            }
        }
        OutputMode::Text => {
            println!(
                "{} {}",
                "success:".bold(),
                if response.success {
                    response.success.to_string().green().to_string()
                } else {
                    response.success.to_string().red().to_string()
                }
            );
            println!("{} {}", "batch_id:".bold(), response.batch_id);
            let mut table = Table::new(&["AGENT", "CLI", "SUCCESS", "RUN_ID", "RESULT"]);
            for run in &response.runs {
                table.add_row(vec![
                    run.profile.clone(),
                    run.backend.clone(),
                    run.success.to_string(),
                    run.run_id.clone(),
                    display_path(&run.result_path),
                ]);
            }
            table.print();
            for error in &response.errors {
                eprintln!(
                    "{} {}: {}",
                    format!("{}:", error.agent).red().bold(),
                    error.code,
                    display_text(&error.message)
                );
            }
        }
    }
    Ok(())
}

pub fn print_json_error(error: &OccError) {
    let value = serde_json::json!({
        "success": false,
        "error": {
            "code": error.code(),
            "message": display_text(error.message()),
        }
    });
    match serde_json::to_string_pretty(&value) {
        Ok(text) => eprintln!("{}", text),
        Err(_) => eprintln!("{}: {}", error.code(), display_text(error.message())),
    }
}

// ── Color helpers ──

pub fn section_title(title: &str) -> String {
    title.bold().to_string()
}

// ── Table formatting ──

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(headers: &[&str]) -> Self {
        Self {
            headers: headers.iter().map(|h| h.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Vec<String>) {
        self.rows.push(row);
    }

    pub fn print(&self) {
        let col_count = self.headers.len();
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.len()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_count {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        let header_line: String = self
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                if i + 1 < col_count {
                    format!("{:<width$}", h, width = widths[i] + 2)
                } else {
                    h.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("");
        let mut output = String::new();
        output.push_str(&header_line);
        output.push('\n');

        for row in &self.rows {
            let line: String = row
                .iter()
                .enumerate()
                .map(|(i, cell)| {
                    if i + 1 < col_count {
                        format!(
                            "{:<width$}",
                            cell,
                            width = widths.get(i).copied().unwrap_or(0) + 2
                        )
                    } else {
                        cell.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("");
            output.push_str(&line);
            output.push('\n');
        }
        output.push('\n');
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(output.as_bytes());
        let _ = stdout.flush();
    }
}
