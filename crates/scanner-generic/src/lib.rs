use scanner_core::{ScannedSession, ToolScanner, ScannerError};
use std::fs;
use std::path::PathBuf;

pub struct GenericScanner {
    name: String,
    scan_dir: PathBuf,
    pattern: String,
}

impl GenericScanner {
    pub fn new(name: &str, path: &str, pattern: &str) -> Self {
        let scan_dir = if path.starts_with('~') {
            dirs::home_dir()
                .map(|h| h.join(path.trim_start_matches('~').trim_start_matches('/')))
                .unwrap_or_else(|| PathBuf::from(path))
        } else {
            PathBuf::from(path)
        };

        Self {
            name: name.to_string(),
            scan_dir,
            pattern: pattern.to_string(),
        }
    }

    fn matches_pattern(path: &std::path::Path, pattern: &str) -> bool {
        if pattern.is_empty() || pattern == "*" || pattern == "*.*" {
            return true;
        }

        if pattern.starts_with("*.") {
            // Extension match: *.json matches test.json, foo.bar.json
            let ext = &pattern[2..];
            if ext.is_empty() {
                return true;
            }
            return path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e == ext)
                .unwrap_or(false);
        }

        if pattern.contains('*') {
            // General glob: convert to regex-like matching
            let path_str = path.to_string_lossy();
            let mut pattern_chars = pattern.chars().peekable();
            let path_chars = path_str.chars().peekable();

            let mut pattern_idx = 0;
            let mut stars = Vec::new();

            for (i, c) in pattern.chars().enumerate() {
                if c == '*' {
                    stars.push(i);
                }
            }

            if stars.is_empty() {
                return false;
            }

            // Match from last star onwards
            let last_star = *stars.last().unwrap();
            let prefix = &pattern[..stars[0]];
            let suffix = &pattern[last_star + 1..];

            // Check prefix
            if !prefix.is_empty() && !path_str.starts_with(prefix) {
                return false;
            }

            // Check suffix
            if !suffix.is_empty() && !path_str.ends_with(suffix) {
                return false;
            }

            // Check that content between stars exists
            if stars.len() == 1 {
                // *foo - anything followed by foo
                let middle = &pattern[stars[0] + 1..];
                if !middle.is_empty() && !path_str.contains(middle) {
                    return false;
                }
            }

            true
        } else {
            // No wildcards, do exact extension match
            path.extension()
                .and_then(|e| e.to_str())
                .map(|e| pattern == e || pattern == format!(".{}", e))
                .unwrap_or(false)
        }
    }
}

impl ToolScanner for GenericScanner {
    fn name(&self) -> &str {
        &self.name
    }

    fn scan(&self) -> Result<Vec<ScannedSession>, ScannerError> {
        let mut sessions = Vec::new();

        if !self.scan_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.scan_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if !Self::matches_pattern(&path, &self.pattern) {
                continue;
            }

            let session_id = path.file_stem()
                .and_then(|s| s.to_str())
                .map(String::from)
                .unwrap_or_else(|| "unknown".to_string());

            let content = fs::read_to_string(&path).ok();
            let (project_path, created_at, model, metadata) = if let Some(ref c) = content {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(c) {
                    let project_path = json.get("cwd")
                        .or_else(|| json.get("directory"))
                        .or_else(|| json.get("path"))
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let created_at = json.get("startedAt")
                        .or_else(|| json.get("createdAt"))
                        .or_else(|| json.get("timestamp"))
                        .and_then(|v| {
                            if let Some(ms) = v.as_i64() {
                                let secs = ms / 1000;
                                let nsecs = ((ms % 1000) * 1_000_000) as u32;
                                chrono::DateTime::from_timestamp(secs, nsecs)
                                    .map(|dt| dt.to_rfc3339())
                            } else if let Some(s) = v.as_str() {
                                chrono::DateTime::parse_from_rfc3339(s)
                                    .ok()
                                    .map(|dt| dt.to_rfc3339())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                    let model = json.get("model")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    let metadata = Some(c.clone());

                    (project_path, created_at, model, metadata)
                } else {
                    (None, chrono::Utc::now().to_rfc3339(), None, None)
                }
            } else {
                (None, chrono::Utc::now().to_rfc3339(), None, None)
            };

            sessions.push(ScannedSession {
                tool: self.name.clone(),
                session_id,
                project_path,
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
    fn test_pattern_matching() {
        assert!(GenericScanner::matches_pattern(
            std::path::Path::new("test.json"),
            "json"
        ));
        assert!(GenericScanner::matches_pattern(
            std::path::Path::new("test.json"),
            "*.json"
        ));
        assert!(GenericScanner::matches_pattern(
            std::path::Path::new("test.json"),
            "*.json"
        ));
        assert!(!GenericScanner::matches_pattern(
            std::path::Path::new("test.txt"),
            "*.json"
        ));
    }
}
