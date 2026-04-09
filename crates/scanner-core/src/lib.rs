use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScannerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("scan failed: {0}")]
    ScanFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedSession {
    pub tool: String,
    pub session_id: String,
    pub project_path: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
    pub metadata: Option<String>,
}

pub trait ToolScanner: Send + Sync {
    fn name(&self) -> &str;
    fn scan(&self) -> Result<Vec<ScannedSession>, ScannerError>;
    fn get_last_message(&self, _session_id: &str) -> Result<Option<String>, ScannerError> {
        Ok(None)
    }
}
