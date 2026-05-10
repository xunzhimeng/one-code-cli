use std::path::PathBuf;

use serde::Serialize;

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
    pub cwd: PathBuf,
    pub result_path: PathBuf,
    pub metadata_path: PathBuf,
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

pub fn print_run_response(mode: OutputMode, response: &RunResponse) -> OccResult<()> {
    match mode {
        OutputMode::Text => {
            println!("success: {}", response.success);
            println!("run_id: {}", response.run_id);
            println!("session_id: {}", response.session_id);
            println!("result_path: {}", response.result_path.display());
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
            println!("{}", text);
        }
        OutputMode::Path => println!("{}", response.result_path.display()),
    }
    Ok(())
}

pub fn print_json_error(error: &OccError) {
    let value = serde_json::json!({
        "success": false,
        "error": {
            "code": error.code(),
            "message": error.message(),
        }
    });
    match serde_json::to_string_pretty(&value) {
        Ok(text) => eprintln!("{}", text),
        Err(_) => eprintln!("{}: {}", error.code(), error.message()),
    }
}
