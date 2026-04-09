use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub tool: String,
    pub session_id: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
    pub token_count: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFilter {
    pub tool: Option<String>,
    pub tags: Option<Vec<String>>,
    pub project_path: Option<String>,
    pub query: Option<String>,
}

impl Default for SessionFilter {
    fn default() -> Self {
        Self {
            tool: None,
            tags: None,
            project_path: None,
            query: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUpdate {
    pub title: Option<String>,
    pub project_path: Option<String>,
    pub metadata: Option<String>,
}

// ScannedSession is re-exported from scanner-core to avoid duplication
pub use scanner_core::ScannedSession;
