use scanner_core::{ScannedSession, ToolScanner, ScannerError};
use std::fs;
use std::path::PathBuf;

pub struct OpenCodeScanner {
    session_dir: PathBuf,
}

impl OpenCodeScanner {
    pub fn new() -> Self {
        let session_dir = dirs::home_dir()
            .map(|h| h.join(".opencode/sessions"))
            .unwrap_or_else(|| PathBuf::from("~/.opencode/sessions"));
        Self { session_dir }
    }

    pub fn with_dir(session_dir: PathBuf) -> Self {
        Self { session_dir }
    }
}

impl ToolScanner for OpenCodeScanner {
    fn name(&self) -> &str {
        "opencode"
    }

    fn scan(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        let mut sessions = Vec::new();

        if !self.session_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.session_dir)? {
            let entry = entry?;
            let path = entry.path();

            // OpenCode sessions might be in a different format
            // Try JSON first
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        let session_id = json.get("sessionId")
                            .or_else(|| json.get("id"))
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        if let Some(session_id) = session_id {
                            let cwd = json.get("cwd")
                                .or_else(|| json.get("project_path"))
                                .and_then(|v| v.as_str())
                                .map(String::from);

                            let created_at = json.get("createdAt")
                                .or_else(|| json.get("startedAt"))
                                .and_then(|v| v.as_i64())
                                .map(|ms| {
                                    let secs = ms / 1000;
                                    let nsecs = ((ms % 1000) * 1_000_000) as u32;
                                    chrono::DateTime::from_timestamp(secs, nsecs)
                                        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
                                        .to_rfc3339()
                                })
                                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                            let metadata = serde_json::to_string(&json).ok();

                            sessions.push(ScannedSession {
                                tool: "opencode".to_string(),
                                session_id,
                                project_path: cwd,
                                model: None,
                                created_at,
                                metadata,
                            });
                        }
                    }
                }
            } else if path.is_dir() {
                // Directory-based session storage
                let session_id = path.file_name()
                    .and_then(|s| s.to_str())
                    .map(String::from);

                if let Some(session_id) = session_id {
                    sessions.push(ScannedSession {
                        tool: "opencode".to_string(),
                        session_id,
                        project_path: None,
                        model: None,
                        created_at: chrono::Utc::now().to_rfc3339(),
                        metadata: None,
                    });
                }
            }
        }

        Ok(sessions)
    }
}
