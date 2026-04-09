mod tui;

use clap::Parser;
use session_store::{SqliteSessionStore, SessionStore, SessionFilter, SessionUpdate};
use scanner_claude::ClaudeScanner;
use scanner_opencode::OpenCodeScanner;
use scanner_core::ToolScanner;
use std::path::PathBuf;
use directories::ProjectDirs;
use tracing::error;

#[derive(Parser)]
#[command(name = "sm")]
#[command(about = "Session Manager - manage Claude/OpenCode sessions")]
enum Cli {
    Scan {
        #[arg(short, long)]
        verbose: bool,
    },
    List {
        #[arg(long)]
        tool: Option<String>,
        #[arg(short = 't', long)]
        tag: Option<String>,
        #[arg(short, long)]
        query: Option<String>,
    },
    Search {
        query: String,
    },
    InstallHook {
        #[arg(long)]
        dry_run: bool,
    },
    Tag {
        #[arg(short, long)]
        action: String,
        session_id: String,
        #[arg(short, long)]
        value: Option<String>,
    },
    Title {
        session_id: String,
        title: String,
    },
    Tui,
}

fn get_db_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "session-manager", "sm") {
        let data_dir = proj_dirs.data_local_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.join("sessions.db")
    } else {
        PathBuf::from("/tmp/sessions.db")
    }
}

fn run_scan(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path();
    let mut store = SqliteSessionStore::new(db_path)?;

    let scanners: Vec<(&str, Box<dyn ToolScanner>)> = vec![
        ("claude", Box::new(ClaudeScanner::new()) as Box<dyn ToolScanner>),
        ("opencode", Box::new(OpenCodeScanner::new()) as Box<dyn ToolScanner>),
    ];

    for (name, scanner) in scanners {
        if verbose {
            println!("Scanning {}...", name);
        }
        match scanner.scan() {
            Ok(sessions) => {
                for session in sessions {
                    if verbose {
                        println!("  Found: {} ({})", session.session_id, session.project_path.as_ref().unwrap_or(&"no path".to_string()));
                    }
                    store.upsert_scanned(&session)?;
                }
                println!("{}: {} sessions", name, store.list_sessions(&SessionFilter { tool: Some(name.to_string()), ..Default::default() })?.len());
            }
            Err(e) => {
                error!("Scanner {} failed: {}", name, e);
            }
        }
    }

    println!("Scan complete.");
    Ok(())
}

fn run_list(tool: Option<String>, tag: Option<String>, query: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path();
    let store = SqliteSessionStore::new(db_path)?;

    let tags = tag.map(|t| vec![t]);

    let filter = SessionFilter {
        tool,
        tags,
        project_path: None,
        query,
    };

    let sessions = store.list_sessions(&filter)?;

    if sessions.is_empty() {
        println!("No sessions found.");
        return Ok(());
    }

    println!("{:<36} {:<10} {:<40} {}", "ID", "TOOL", "PROJECT", "CREATED");
    println!("{}", "-".repeat(100));
    for s in sessions {
        let proj = s.project_path.as_ref().map(|p| {
            if p.len() > 40 { format!("...{}", &p[p.len()-37..]) } else { p.clone() }
        }).unwrap_or_default();
        println!("{:<36} {:<10} {:<40} {}", &s.session_id[..36.min(s.session_id.len())], s.tool, proj, &s.created_at[..10]);
    }

    Ok(())
}

fn run_tag(action: &str, session_id: &str, value: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path();
    let mut store = SqliteSessionStore::new(db_path)?;

    match action {
        "add" => {
            let tag = value.ok_or("Tag value required: sm tag add <session_id> --value <tag>")?;
            store.add_tag(session_id, tag)?;
            println!("Added tag '{}' to session {}", tag, &session_id[..36.min(session_id.len())]);
        }
        "remove" => {
            let tag = value.ok_or("Tag value required: sm tag remove <session_id> --value <tag>")?;
            store.remove_tag(session_id, tag)?;
            println!("Removed tag '{}' from session {}", tag, &session_id[..36.min(session_id.len())]);
        }
        "list" => {
            let tags = store.get_tags(session_id)?;
            if tags.is_empty() {
                println!("No tags for session {}", &session_id[..36.min(session_id.len())]);
            } else {
                println!("Tags for session {}: {}", &session_id[..36.min(session_id.len())], tags.join(", "));
            }
        }
        _ => {
            println!("Unknown action: {}. Use add, remove, or list.", action);
        }
    }
    Ok(())
}

fn run_title(session_id: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path();
    let mut store = SqliteSessionStore::new(db_path)?;

    // Find session by session_id
    let sessions = store.list_sessions(&SessionFilter {
        tool: None,
        tags: None,
        project_path: None,
        query: None,
    })?;

    let session = sessions.into_iter().find(|s| s.session_id == session_id);
    if let Some(session) = session {
        store.update_session(&session.id, &SessionUpdate {
            title: Some(title.to_string()),
            project_path: None,
            metadata: None,
        })?;
        println!("Updated title to '{}' for session {}", title, &session_id[..36.min(session_id.len())]);
    } else {
        println!("Session not found: {}", session_id);
    }
    Ok(())
}

fn run_install_hook(dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.session-manager.scan</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/sm</string>
        <string>scan</string>
    </array>
    <key>StartInterval</key>
    <integer>900</integer>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardErrorPath</key>
    <string>/tmp/sm-scan.err</string>
    <key>StandardOutPath</key>
    <string>/tmp/sm-scan.out</string>
</dict>
</plist>"#;

    if dry_run {
        println!("{}", plist);
    } else {
        let dest = std::path::Path::new(&std::env::var("HOME").unwrap_or_default())
            .join("Library/LaunchAgents/com.session-manager.scan.plist");
        std::fs::write(&dest, plist)?;
        println!("Installed to {}. Load with: launchctl load {}", dest.display(), dest.display());
    }

    Ok(())
}

fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli {
        Cli::Scan { verbose } => {
            if let Err(e) = run_scan(verbose) {
                error!("Scan failed: {}", e);
                std::process::exit(1);
            }
        }
        Cli::List { tool, tag, query } => {
            if let Err(e) = run_list(tool, tag, query) {
                error!("List failed: {}", e);
                std::process::exit(1);
            }
        }
        Cli::Search { query } => {
            if let Err(e) = run_list(None, None, Some(query)) {
                error!("Search failed: {}", e);
                std::process::exit(1);
            }
        }
        Cli::InstallHook { dry_run } => {
            if let Err(e) = run_install_hook(dry_run) {
                error!("Install hook failed: {}", e);
                std::process::exit(1);
            }
        }
        Cli::Tag { action, session_id, value } => {
            if let Err(e) = run_tag(&action, &session_id, value.as_deref()) {
                error!("Tag command failed: {}", e);
                std::process::exit(1);
            }
        }
        Cli::Title { session_id, title } => {
            if let Err(e) = run_title(&session_id, &title) {
                error!("Title command failed: {}", e);
                std::process::exit(1);
            }
        }
        Cli::Tui => {
            if let Err(e) = tui::run_tui() {
                error!("TUI error: {}", e);
                std::process::exit(1);
            }
        }
    }
}
