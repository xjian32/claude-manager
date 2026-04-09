use scanner_core::{ScannedSession, ToolScanner, ScannerError};
use std::fs;
use std::path::PathBuf;

pub struct ClaudeScanner {
    session_dir: PathBuf,
    projects_dir: PathBuf,
}

impl ClaudeScanner {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        Self {
            session_dir: home.join(".claude/sessions"),
            projects_dir: home.join(".claude/projects"),
        }
    }

    pub fn with_dir(session_dir: PathBuf) -> Self {
        Self {
            session_dir: session_dir.clone(),
            projects_dir: session_dir.parent().unwrap_or(&session_dir).join("projects"),
        }
    }

    fn scan_sessions_dir(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        let mut sessions = Vec::new();
        if !self.session_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.session_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Only process .json files (skip compaction-log.txt, .tmp files, etc.)
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;

            let session_id = json.get("sessionId")
                .and_then(|v| v.as_str())
                .map(String::from);

            let session_id = match session_id {
                Some(id) => id,
                None => continue,
            };

            let cwd = json.get("cwd")
                .and_then(|v| v.as_str())
                .map(String::from);

            let started_at = json.get("startedAt")
                .and_then(|v| v.as_i64())
                .map(|ms| {
                    let secs = ms / 1000;
                    let nsecs = ((ms % 1000) * 1_000_000) as u32;
                    chrono::DateTime::from_timestamp(secs, nsecs)
                        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
                        .to_rfc3339()
                })
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let _kind = json.get("kind")
                .and_then(|v| v.as_str())
                .map(String::from);

            let _pid = json.get("pid")
                .and_then(|v| v.as_i64())
                .map(|v| v.to_string());

            let metadata = serde_json::to_string(&json).ok();

            sessions.push(ScannedSession {
                tool: "claude".to_string(),
                session_id,
                project_path: cwd,
                model: None,
                created_at: started_at,
                metadata,
            });
        }

        Ok(sessions)
    }

    fn scan_projects_index(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        let mut sessions = Vec::new();
        if !self.projects_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.projects_dir)? {
            let entry = entry?;
            let path = entry.path();

            let index_path = path.join("sessions-index.json");
            if !index_path.exists() {
                continue;
            }

            let content = fs::read_to_string(&index_path)?;
            let index: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let entries = match index.get("entries").and_then(|v| v.as_array()) {
                Some(e) => e,
                None => continue,
            };

            for entry in entries {
                let session_id = match entry.get("sessionId").and_then(|v| v.as_str()) {
                    Some(id) => id.to_string(),
                    None => continue,
                };

                let project_path = entry.get("projectPath")
                    .or_else(|| entry.get("originalPath"))
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let created_at = entry.get("created")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                let summary = entry.get("summary")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let message_count = entry.get("messageCount")
                    .and_then(|v| v.as_i64());

                let metadata = serde_json::to_string(&serde_json::json!({
                    "source": "sessions-index",
                    "summary": summary,
                    "messageCount": message_count,
                    "fullPath": entry.get("fullPath").and_then(|v| v.as_str()),
                    "modified": entry.get("modified").and_then(|v| v.as_str()),
                })).ok();

                sessions.push(ScannedSession {
                    tool: "claude".to_string(),
                    session_id,
                    project_path,
                    model: None,
                    created_at,
                    metadata,
                });
            }
        }

        Ok(sessions)
    }
}

impl ToolScanner for ClaudeScanner {
    fn name(&self) -> &str {
        "claude"
    }

    fn scan(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        let mut sessions = self.scan_sessions_dir()?;
        sessions.extend(self.scan_projects_index()?);
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        sessions.dedup_by(|a, b| a.session_id == b.session_id);
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_format() {
        let scanner = ClaudeScanner::new();
        let result = scanner.scan();
        assert!(result.is_ok());
    }
}
