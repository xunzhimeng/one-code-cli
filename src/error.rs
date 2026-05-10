use thiserror::Error;

pub type OccResult<T> = Result<T, OccError>;

#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct OccError {
    code: &'static str,
    message: String,
}

impl OccError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn io(code: &'static str, action: impl Into<String>, error: std::io::Error) -> Self {
        Self::new(code, format!("{}: {}", action.into(), error))
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
