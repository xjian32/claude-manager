use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use crossterm::{event::{self, Event, KeyCode}, execute};
use std::io::stdout;
use std::path::PathBuf;
use directories::ProjectDirs;
use session_store::{SqliteSessionStore, SessionStore, SessionFilter};

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
    search_active: bool,
    search_buffer: String,
    title_edit_active: bool,
    title_edit_buffer: String,
    claude_scanner: scanner_claude::ClaudeScanner,
    opencode_scanner: scanner_opencode::OpenCodeScanner,
}

impl AppState {
    fn new() -> Self {
        Self {
            sessions: Vec::new(),
            selected: 0,
            tags: Vec::new(),
            filter_tag: None,
            search_query: None,
            search_active: false,
            search_buffer: String::new(),
            title_edit_active: false,
            title_edit_buffer: String::new(),
            claude_scanner: scanner_claude::ClaudeScanner::new(),
            opencode_scanner: scanner_opencode::OpenCodeScanner::new(),
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
            if self.selected >= self.sessions.len() {
                self.selected = self.sessions.len().saturating_sub(1);
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

    fn get_last_message(&self) -> Option<String> {
        let session = self.sessions.get(self.selected)?;
        let scanner: &dyn scanner_core::ToolScanner = if session.tool == "claude" {
            &self.claude_scanner as &dyn scanner_core::ToolScanner
        } else {
            &self.opencode_scanner as &dyn scanner_core::ToolScanner
        };
        scanner.get_last_message(&session.session_id).ok().flatten()
    }

    fn submit_search(&mut self) {
        let q = self.search_buffer.trim().to_string();
        self.search_query = if q.is_empty() { None } else { Some(q) };
        self.search_active = false;
        self.search_buffer.clear();
        self.load_sessions();
    }

    fn reset_search(&mut self) {
        self.search_query = None;
        self.search_active = false;
        self.search_buffer.clear();
        self.filter_tag = None;
        self.load_sessions();
    }

    fn submit_title_edit(&mut self) {
        if !self.title_edit_buffer.is_empty() {
            if let Some(session) = self.sessions.get(self.selected) {
                let db_path = get_db_path();
                if let Ok(mut store) = SqliteSessionStore::new(db_path) {
                    let _ = store.update_session(&session.id, &session_store::SessionUpdate {
                        title: Some(self.title_edit_buffer.clone()),
                        project_path: None,
                        metadata: None,
                    });
                    self.load_sessions();
                }
            }
        }
        self.title_edit_active = false;
        self.title_edit_buffer.clear();
    }

    fn cancel_title_edit(&mut self) {
        self.title_edit_active = false;
        self.title_edit_buffer.clear();
    }

    fn start_title_edit(&mut self) {
        if let Some(session) = self.sessions.get(self.selected) {
            self.title_edit_buffer = session.title.clone().unwrap_or_default();
            self.title_edit_active = true;
        }
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

    let mut list_state = ListState::default();
    list_state.select(Some(0));

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

            // Header / Search bar
            let counts = {
                let claude: usize = state.sessions.iter().filter(|s| s.tool == "claude").count();
                let opencode: usize = state.sessions.iter().filter(|s| s.tool == "opencode").count();
                let total = state.sessions.len();
                format!("Claude: {}, OpenCode: {}, Total: {}", claude, opencode, total)
            };
            let header_text = if state.title_edit_active {
                format!("Title: {}_ (Enter=save, Esc=cancel) [{}]", state.title_edit_buffer, counts)
            } else if state.search_active {
                format!("Search: {}_ (Enter=confirm, Esc=cancel) [{}]", state.search_buffer, counts)
            } else {
                format!("Session Manager [{}] - / or Ctrl+F to search, t to edit title, r to reset, q to quit, Enter to copy resume", counts)
            };
            let header = Paragraph::new(header_text)
                .block(Block::default().borders(Borders::ALL).title("Header"));
            f.render_widget(header, chunks[0]);

            // Main content
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(chunks[1]);

            // Session list (stateful for scrolling)
            let items: Vec<ListItem> = state.sessions.iter().enumerate().map(|(i, s)| {
                let proj = s.project_path.as_ref().map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(p)
                        .to_string()
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

            f.render_stateful_widget(list, content_chunks[0], &mut list_state);

            // Detail panel
            if let Some(session) = state.sessions.get(state.selected) {
                let last_msg = state.get_last_message();
                let last_msg_line = last_msg
                    .map(|m| format!("Last: {}", m))
                    .unwrap_or_else(|| "Last: (none)".to_string());

                let detail = format!(
                    "Tool: {}\nSession ID: {}\nProject: {}\nModel: {}\nCreated: {}\nTitle: {}\n{}",
                    session.tool,
                    session.session_id,
                    session.project_path.as_ref().unwrap_or(&"none".to_string()),
                    session.model.as_ref().unwrap_or(&"unknown".to_string()),
                    session.created_at,
                    session.title.as_ref().unwrap_or(&"(no title)".to_string()),
                    last_msg_line,
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
            if state.title_edit_active {
                match key.code {
                    KeyCode::Enter => {
                        state.submit_title_edit();
                    }
                    KeyCode::Esc => {
                        state.cancel_title_edit();
                    }
                    KeyCode::Backspace => {
                        state.title_edit_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        state.title_edit_buffer.push(c);
                    }
                    _ => {}
                }
            } else if state.search_active {
                match key.code {
                    KeyCode::Enter => {
                        state.submit_search();
                    }
                    KeyCode::Esc => {
                        state.search_active = false;
                        state.search_buffer.clear();
                    }
                    KeyCode::Backspace => {
                        state.search_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        state.search_buffer.push(c);
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Down => {
                        if state.selected + 1 < state.sessions.len() {
                            state.selected += 1;
                            list_state.select(Some(state.selected));
                        }
                    }
                    KeyCode::Up => {
                        if state.selected > 0 {
                            state.selected -= 1;
                            list_state.select(Some(state.selected));
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
                    KeyCode::Char('/') | KeyCode::Char('f')
                        if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                        state.search_active = true;
                        state.search_buffer.clear();
                    }
                    KeyCode::Char('t') => {
                        state.start_title_edit();
                    }
                    KeyCode::Char('q') => {
                        should_quit = true;
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        if state.search_active {
                            state.reset_search();
                        } else {
                            state.load_sessions();
                        }
                        list_state.select(Some(state.selected.min(state.sessions.len().saturating_sub(1))));
                    }
                    _ => {}
                }
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
