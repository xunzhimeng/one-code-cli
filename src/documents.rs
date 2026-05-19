use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::backend::CommandPlan;
use crate::config::EnvMode;
use crate::error::{OccError, OccResult};
use crate::output;
use crate::run_record::RunRecord;

#[derive(Debug, Clone)]
pub struct RunPaths {
    pub prompt_md: PathBuf,
    pub result_md: PathBuf,
    pub stdout_log: PathBuf,
    pub stderr_log: PathBuf,
    pub events_jsonl: PathBuf,
    pub command_json: PathBuf,
    pub run_toml: PathBuf,
    pub artifacts_dir: PathBuf,
}

impl RunPaths {
    pub fn new(doc_root: &Path, run_id: &str) -> Self {
        let run_dir = doc_root.join("runs").join(run_id);
        Self {
            prompt_md: run_dir.join("prompt.md"),
            result_md: run_dir.join("result.md"),
            stdout_log: run_dir.join("stdout.log"),
            stderr_log: run_dir.join("stderr.log"),
            events_jsonl: run_dir.join("events.jsonl"),
            command_json: run_dir.join("command.json"),
            run_toml: run_dir.join("run.toml"),
            artifacts_dir: run_dir.join("artifacts"),
        }
    }

    pub fn create_dirs(&self) -> OccResult<()> {
        fs::create_dir_all(&self.artifacts_dir).map_err(|error| {
            OccError::io(
                "doc_root_not_writable",
                format!(
                    "Failed to create '{}'",
                    output::display_path(&self.artifacts_dir)
                ),
                error,
            )
        })
    }
}

#[derive(Debug, Serialize)]
pub struct CommandMetadata<'a> {
    pub executable: &'a Path,
    pub args: &'a [String],
    pub cwd: &'a Path,
    pub env_keys: Vec<&'a String>,
    pub env_mode: EnvMode,
    pub env_allowlist: &'a [String],
    pub env_removed: &'a [String],
    pub prompt_via_stdin: bool,
    pub prompt_file: Option<&'a PathBuf>,
    pub prompt_transport: crate::config::PromptVia,
    pub timeout: Option<&'a str>,
    pub model: Option<&'a str>,
    pub model_source: &'a str,
    pub effort: Option<&'a str>,
    pub effort_source: &'a str,
}

pub fn write_run_files(
    paths: &RunPaths,
    prompt: Option<&str>,
    stdout: &str,
    stderr: &str,
    plan: &CommandPlan,
    record: &RunRecord,
) -> OccResult<()> {
    paths.create_dirs()?;
    fs::write(&paths.prompt_md, prompt.unwrap_or(""))
        .map_err(|error| write_error(&paths.prompt_md, error))?;
    if !paths.stdout_log.exists() {
        fs::write(&paths.stdout_log, stdout)
            .map_err(|error| write_error(&paths.stdout_log, error))?;
    }
    if !paths.stderr_log.exists() {
        fs::write(&paths.stderr_log, stderr)
            .map_err(|error| write_error(&paths.stderr_log, error))?;
    }
    fs::write(&paths.events_jsonl, event_line(record)?)
        .map_err(|error| write_error(&paths.events_jsonl, error))?;

    let command_metadata = CommandMetadata {
        executable: &plan.executable,
        args: &plan.args,
        cwd: &plan.cwd,
        env_keys: plan.env.keys().collect(),
        env_mode: plan.env_mode,
        env_allowlist: &plan.env_allowlist,
        env_removed: &plan.env_remove,
        prompt_via_stdin: plan.prompt_stdin.is_some(),
        prompt_file: plan.prompt_file.as_ref(),
        prompt_transport: plan.prompt_transport,
        timeout: record.timeout.as_deref(),
        model: record.model.as_deref(),
        model_source: &record.model_source,
        effort: record.effort.as_deref(),
        effort_source: &record.effort_source,
    };
    let command_json = serde_json::to_string_pretty(&command_metadata).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize command JSON: {}", error),
        )
    })?;
    fs::write(&paths.command_json, command_json)
        .map_err(|error| write_error(&paths.command_json, error))?;

    let run_toml = toml::to_string_pretty(record).map_err(|error| {
        OccError::new(
            "serialization_failed",
            format!("Failed to serialize run TOML: {}", error),
        )
    })?;
    fs::write(&paths.run_toml, run_toml).map_err(|error| write_error(&paths.run_toml, error))?;
    fs::write(&paths.result_md, result_markdown(record, stdout, stderr))
        .map_err(|error| write_error(&paths.result_md, error))?;
    Ok(())
}

pub fn result_markdown(record: &RunRecord, stdout: &str, stderr: &str) -> String {
    let output = if stdout.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    format!(
        "# One Code CLI Run Result\n\n## Summary\n\n{}\n\n## Run\n\n- Run ID: {}\n- Session ID: {}\n- Agent: {}\n- CLI: {}\n- Model: {}\n- Model Source: {}\n- Effort: {}\n- Effort Source: {}\n- Working Directory: {}\n- Interactive: {}\n- Success: {}\n- Exit Code: {}\n- Started At: {}\n- Finished At: {}\n\n## Prompt\n\nSee `prompt.md`.\n\n## Output\n\n{}\n\n## Logs\n\n- stdout: `stdout.log`\n- stderr: `stderr.log`\n- events: `events.jsonl`\n",
        first_non_empty_line(output).unwrap_or("No output."),
        record.run_id,
        record.session_id,
        record.profile,
        record.backend,
        record.model.as_deref().unwrap_or(""),
        record.model_source,
        record.effort.as_deref().unwrap_or(""),
        record.effort_source,
        output::display_path(&record.cwd),
        record.interactive,
        record.success,
        record
            .exit_code
            .map(|value| value.to_string())
            .unwrap_or_default(),
        record.started_at,
        record.finished_at,
        fenced(output),
    )
}

fn event_line(record: &RunRecord) -> OccResult<String> {
    let value = serde_json::json!({
        "event": "run_finished",
        "run_id": record.run_id,
        "session_id": record.session_id,
        "success": record.success,
        "exit_code": record.exit_code,
        "created_at": record.finished_at,
    });
    serde_json::to_string(&value)
        .map(|line| format!("{}\n", line))
        .map_err(|error| {
            OccError::new(
                "serialization_failed",
                format!("Failed to serialize event JSON: {}", error),
            )
        })
}

fn fenced(value: &str) -> String {
    format!("```text\n{}\n```", value.trim_end())
}

fn first_non_empty_line(value: &str) -> Option<&str> {
    value.lines().find(|line| !line.trim().is_empty())
}

fn write_error(path: &Path, error: std::io::Error) -> OccError {
    OccError::io(
        "doc_root_not_writable",
        format!("Failed to write '{}'", output::display_path(path)),
        error,
    )
}
