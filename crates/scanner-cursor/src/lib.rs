use scanner_core::{ScannedSession, ToolScanner, ScannerError};
use std::path::PathBuf;

pub struct CursorScanner {
    data_dir: PathBuf,
}

impl CursorScanner {
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        Self {
            data_dir: home.join(".cursor.chat/data"),
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
        Self { data_dir: path_buf }
    }

    fn scan_sqlite(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        let mut sessions = Vec::new();
        let db_path = self.data_dir.join("cursor.chat.db");

        if !db_path.exists() {
            return Ok(sessions);
        }

        let conn = match rusqlite::Connection::open(&db_path) {
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
                "source": "cursor_db",
                "title": title,
            })).ok();

            sessions.push(ScannedSession {
                tool: "cursor".to_string(),
                session_id,
                project_path: Some(directory),
                model: None,
                created_at,
                metadata,
            });
        }

        Ok(sessions)
    }
}

impl ToolScanner for CursorScanner {
    fn name(&self) -> &str {
        "cursor"
    }

    fn scan(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        self.scan_sqlite()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_format() {
        let scanner = CursorScanner::new();
        let result = scanner.scan();
        assert!(result.is_ok());
    }
}
