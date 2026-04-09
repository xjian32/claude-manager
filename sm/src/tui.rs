use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use crossterm::{event::{self, Event, KeyCode}, execute};
use std::io::stdout;
use session_store::{SqliteSessionStore, SessionStore, SessionFilter};
use std::path::PathBuf;
use directories::ProjectDirs;

pub fn get_db_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "session-manager", "sm") {
        let data_dir = proj_dirs.data_local_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.join("sessions.db")
    } else {
        PathBuf::from("/tmp/sessions.db")
    }
}

struct AppState {
    sessions: Vec<session_store::Session>,
    selected: usize,
    tags: Vec<String>,
    filter_tag: Option<String>,
    search_query: Option<String>,
}

impl AppState {
    fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected: 0,
            tags: Vec::new(),
            filter_tag: None,
            search_query: None,
        }
    }

    fn load_sessions(&mut self) {
        let db_path = get_db_path();
        let store = SqliteSessionStore::new(db_path).ok();
        if let Some(store) = store {
            let filter = SessionFilter {
                tool: None,
                tags: self.filter_tag.as_ref().map(|t| vec![t.clone()]),
                project_path: None,
                query: self.search_query.clone(),
            };
            self.sessions = store.list_sessions(&filter).unwrap_or_default();
            self.tags = store.list_all_tags().unwrap_or_default();
            if self.selected >= self.sessions.len() && !self.sessions.is_empty() {
                self.selected = self.sessions.len() - 1;
            }
        }
    }

    fn copy_resume_cmd(&self) -> Option<String> {
        self.sessions.get(self.selected).map(|s| {
            if s.tool == "claude" {
                format!("claude --resume {}", s.session_id)
            } else {
                format!("opencode -s {}", s.session_id)
            }
        })
    }
}

struct TuiGuard;

impl Drop for TuiGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(stdout(), crossterm::terminal::LeaveAlternateScreen);
    }
}

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TuiGuard;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    execute!(stdout(), crossterm::terminal::EnterAlternateScreen)?;
    crossterm::terminal::enable_raw_mode()?;

    let mut state = AppState::new();
    state.load_sessions();

    let mut should_quit = false;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(3),
                ])
                .split(f.size());

            // Header
            let header = Paragraph::new("Session Manager - Press q to quit, Enter to copy resume command")
                .block(Block::default().borders(Borders::ALL).title("Header"));
            f.render_widget(header, chunks[0]);

            // Main content
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(chunks[1]);

            // Session list
            let items: Vec<ListItem> = state.sessions.iter().enumerate().map(|(i, s)| {
                let proj = s.project_path.as_ref().map(|p| {
                    let name = std::path::Path::new(p)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(p);
                    format!("{}", name)
                }).unwrap_or_else(|| "unknown".to_string());

                let line = if i == state.selected {
                    format!("[{}] {} ({})", s.tool, &s.session_id[..8.min(s.session_id.len())], proj)
                } else {
                    format!("{} {} ({})", s.tool, &s.session_id[..8.min(s.session_id.len())], proj)
                };
                ListItem::new(line)
            }).collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Sessions"))
                .style(Style::default());

            f.render_widget(list, content_chunks[0]);

            // Detail panel
            if let Some(session) = state.sessions.get(state.selected) {
                let detail = format!(
                    "Tool: {}\nSession ID: {}\nProject: {}\nModel: {}\nCreated: {}\nTitle: {}",
                    session.tool,
                    session.session_id,
                    session.project_path.as_ref().unwrap_or(&"none".to_string()),
                    session.model.as_ref().unwrap_or(&"unknown".to_string()),
                    session.created_at,
                    session.title.as_ref().unwrap_or(&"(no title)".to_string()),
                );
                let para = Paragraph::new(detail)
                    .block(Block::default().borders(Borders::ALL).title("Detail"));
                f.render_widget(para, content_chunks[1]);
            }

            // Tags bar
            let tags_display = if state.tags.is_empty() {
                "No tags".to_string()
            } else {
                state.tags.iter()
                    .map(|t| {
                        if state.filter_tag.as_ref() == Some(t) {
                            format!("[{}]", t)
                        } else {
                            t.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let tags_bar = Paragraph::new(format!("Tags: {}", tags_display))
                .block(Block::default().borders(Borders::ALL).title("Tags"));
            f.render_widget(tags_bar, chunks[2]);
        })?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Down => {
                    if state.selected + 1 < state.sessions.len() {
                        state.selected += 1;
                    }
                }
                KeyCode::Up => {
                    if state.selected > 0 {
                        state.selected -= 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(cmd) = state.copy_resume_cmd() {
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            let _ = clipboard.set_text(&cmd);
                            println!("\nCopied: {}\n", cmd);
                        }
                    }
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    should_quit = true;
                }
                KeyCode::Char('r') => {
                    state.load_sessions();
                }
                _ => {}
            }
        }

        if should_quit {
            break;
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    execute!(stdout(), crossterm::terminal::LeaveAlternateScreen)?;
    Ok(())
}
