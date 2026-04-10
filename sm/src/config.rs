use serde::Deserialize;
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub scanner: ScannerConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ScannerConfig {
    #[serde(default)]
    pub claude: ScannerEntry,
    #[serde(default)]
    pub opencode: ScannerEntry,
    #[serde(default)]
    pub claude_code: ScannerEntry,
    #[serde(default)]
    pub cursor: ScannerEntry,
    #[serde(default)]
    pub windsurf: ScannerEntry,
    #[serde(default)]
    pub generic: GenericScannerConfig,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ScannerEntry {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub path: Option<String>,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GenericScannerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub sources: Vec<GenericSource>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GenericSource {
    pub name: String,
    pub path: String,
    pub pattern: String,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        ScannerConfig {
            claude: ScannerEntry {
                enabled: true,
                path: None,
            },
            opencode: ScannerEntry {
                enabled: true,
                path: None,
            },
            claude_code: ScannerEntry {
                enabled: false,
                path: None,
            },
            cursor: ScannerEntry {
                enabled: false,
                path: None,
            },
            windsurf: ScannerEntry {
                enabled: false,
                path: None,
            },
            generic: GenericScannerConfig {
                enabled: false,
                sources: Vec::new(),
            },
        }
    }
}

pub fn get_config_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "session-manager", "sm") {
        proj_dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".config/session-manager")
    }
}

pub fn get_default_path(tool: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    match tool {
        "claude" => Some(format!("{}/.claude/sessions", home)),
        "opencode" => Some(format!("{}/.local/share/opencode/opencode.db", home)),
        "claude_code" => Some(format!("{}/.claude_code/sessions", home)),
        "cursor" => Some(format!("{}/.cursor.chat/data", home)),
        "windsurf" => Some(format!("{}/.windsurf/history", home)),
        _ => None,
    }
}

pub fn load_config() -> Config {
    let config_path = get_config_dir().join("config.toml");

    if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(content) => {
                match toml::from_str(&content) {
                    Ok(config) => {
                        println!("Loaded config from {}", config_path.display());
                        return config;
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to parse config: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to read config: {}", e);
            }
        }
    } else {
        println!("No config file found at {}, using defaults", config_path.display());
    }

    Config {
        scanner: ScannerConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.scanner.claude.enabled);
        assert!(!config.scanner.claude_code.enabled);
    }

    #[test]
    fn test_get_default_path() {
        std::env::set_var("HOME", "/home/user");
        assert_eq!(
            get_default_path("claude"),
            Some("/home/user/.claude/sessions".to_string())
        );
    }
}
