use scanner_core::{ScannedSession, ToolScanner, ScannerError};
use std::path::PathBuf;

pub struct OpenCodeScanner {
    db_path: PathBuf,
}

impl OpenCodeScanner {
    pub fn new() -> Self {
        let db_path = dirs::home_dir()
            .map(|h| h.join(".local/share/opencode/opencode.db"))
            .unwrap_or_else(|| PathBuf::from("~/.local/share/opencode/opencode.db"));
        Self { db_path }
    }

    pub fn with_db(db_path: PathBuf) -> Self {
        Self { db_path }
    }
}

impl ToolScanner for OpenCodeScanner {
    fn name(&self) -> &str {
        "opencode"
    }

    fn scan(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        let mut sessions = Vec::new();

        if !self.db_path.exists() {
            return Ok(sessions);
        }

        let conn = match rusqlite::Connection::open(&self.db_path) {
            Ok(c) => c,
            Err(_) => return Ok(sessions),
        };

        let mut stmt = match conn.prepare(
            "SELECT id, title, directory, time_created FROM session WHERE time_archived IS NULL ORDER BY time_created DESC"
        ) {
            Ok(s) => s,
            Err(_) => return Ok(sessions),
        };

        let rows = match stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let directory: String = row.get(2)?;
            let time_created: i64 = row.get(3)?;
            Ok((id, title, directory, time_created))
        }) {
            Ok(r) => r,
            Err(_) => return Ok(sessions),
        };

        for row in rows.flatten() {
            let (session_id, title, directory, time_created) = row;
            let created_at = chrono::DateTime::from_timestamp(time_created / 1000, 0)
                .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap())
                .to_rfc3339();

            let metadata = serde_json::to_string(&serde_json::json!({
                "source": "opencode_db",
                "title": title,
            })).ok();

            sessions.push(ScannedSession {
                tool: "opencode".to_string(),
                session_id,
                project_path: Some(directory),
                model: None,
                created_at,
                metadata,
            });
        }

        Ok(sessions)
    }

    fn get_last_message(&self, session_id: &str) -> Result<Option<String>, ScannerError> {
        if !self.db_path.exists() {
            return Ok(None);
        }

        let conn = match rusqlite::Connection::open(&self.db_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let result: Option<String> = conn.query_row(
            "SELECT data FROM message WHERE session_id = ?1 AND data LIKE '%\"role\":\"user\"%' ORDER BY time_created DESC LIMIT 1",
            [session_id],
            |row| {
                let data: String = row.get(0)?;
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                    let summary = json.get("summary")
                        .and_then(|s| s.get("title"))
                        .and_then(|t| t.as_str())
                        .map(String::from);
                    Ok(summary)
                } else {
                    Ok(None)
                }
            },
        ).ok().flatten();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_format() {
        let scanner = OpenCodeScanner::new();
        let result = scanner.scan();
        assert!(result.is_ok());
    }
}
