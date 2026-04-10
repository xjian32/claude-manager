use scanner_core::{ScannedSession, ToolScanner, ScannerError};
use std::fs;
use std::path::PathBuf;

pub struct ClaudeCodeScanner {
    session_dir: PathBuf,
}

impl ClaudeCodeScanner {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        Self {
            session_dir: home.join(".claude_code/sessions"),
        }
    }

    pub fn with_path(path: &str) -> Self {
        let path_buf = if path.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path.trim_start_matches('~').trim_start_matches('/')))
                .unwrap_or_else(|| PathBuf::from(path))
        } else {
            PathBuf::from(path)
        };
        Self { session_dir: path_buf }
    }
}

impl ToolScanner for ClaudeCodeScanner {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn scan(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        let mut sessions = Vec::new();
        if !self.session_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.session_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;

            let session_id = json.get("sessionId")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(String::from)
                    .unwrap_or_default());

            let cwd = json.get("cwd")
                .and_then(|v| v.as_str())
                .map(String::from);

            let created_at = json.get("createdAt")
                .and_then(|v| v.as_i64())
                .map(|ms| {
                    let secs = ms / 1000;
                    let nsecs = ((ms % 1000) * 1_000_000) as u32;
                    chrono::DateTime::from_timestamp(secs, nsecs)
                        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
                        .to_rfc3339()
                })
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let model = json.get("model")
                .and_then(|v| v.as_str())
                .map(String::from);

            let metadata = serde_json::to_string(&json).ok();

            sessions.push(ScannedSession {
                tool: "claude-code".to_string(),
                session_id,
                project_path: cwd,
                model,
                created_at,
                metadata,
            });
        }

        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_format() {
        let scanner = ClaudeCodeScanner::new();
        let result = scanner.scan();
        assert!(result.is_ok());
    }
}
