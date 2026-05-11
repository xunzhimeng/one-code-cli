use std::path::{Path, PathBuf};

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
    pub profile: String,
    pub backend: String,
    pub model: Option<String>,
    pub model_source: String,
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

pub fn display_text(value: &str) -> String {
    value
        .replace(r"\\\\?\\UNC\\", r"\\\\")
        .replace(r"\\?\UNC\", r"\\")
        .replace(r"\\\\?\\", "")
        .replace(r"\\?\", "")
}

pub fn display_path(path: &Path) -> String {
    display_text(&path.display().to_string())
}

fn serialize_display_path<S>(path: &PathBuf, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&display_path(path))
}

pub fn print_run_response(mode: OutputMode, response: &RunResponse) -> OccResult<()> {
    match mode {
        OutputMode::Text => {
            println!("success: {}", response.success);
            println!("run_id: {}", response.run_id);
            println!("session_id: {}", response.session_id);
            println!("model_source: {}", response.model_source);
            if let Some(model) = &response.model {
                println!("model: {}", model);
            }
            println!("result_path: {}", display_path(&response.result_path));
            if let Some(error) = &response.error {
                println!("error: {}: {}", error.code, error.message);
            }
        }
        OutputMode::Json => {
            let text = serde_json::to_string_pretty(response).map_err(|error| {
                OccError::new(
                    "config_parse_failed",
                    format!("Failed to serialize JSON output: {}", error),
                )
            })?;
            println!("{}", display_text(&text));
        }
        OutputMode::Path => println!("{}", display_path(&response.result_path)),
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
