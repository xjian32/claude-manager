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

fn format_beijing_time(utc_str: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(utc_str) {
        let beijing = dt.with_timezone(&chrono_tz::Asia::Shanghai);
        beijing.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        utc_str.to_string()
    }
}

pub fn get_db_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "session-manager", "sm") {
        let data_dir = proj_dirs.data_local_dir();
        std::fs::create_dir_all(data_dir).ok();
        data_dir.join("sessions.db")
    } else {
        PathBuf::from("/tmp/sessions.db")
    }
}

struct SessionGroup {
    path: String,
    dir_name: String,
    tool_counts: (usize, usize),
    latest_time: String,
    sessions: Vec<session_store::Session>,
    is_expanded: bool,
    selected_child: usize,
}

struct AppState {
    groups: Vec<SessionGroup>,
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
            groups: Vec::new(),
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
            let sessions = store.list_sessions(&filter).unwrap_or_default();
            self.groups = Self::group_sessions(sessions);
            self.tags = store.list_all_tags().unwrap_or_default();
            if self.selected >= self.groups.len() {
                self.selected = self.groups.len().saturating_sub(1);
            }
        }
    }

    fn group_sessions(sessions: Vec<session_store::Session>) -> Vec<SessionGroup> {
        use std::collections::HashMap;
        let mut map: HashMap<String, Vec<session_store::Session>> = HashMap::new();

        for s in sessions {
            let path = s.project_path.clone().unwrap_or_else(|| "unknown".to_string());
            map.entry(path).or_insert_with(Vec::new).push(s);
        }

        let mut groups: Vec<SessionGroup> = map.into_iter().map(|(path, mut sessions)| {
            sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            let dir_name = std::path::Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&path)
                .to_string();
            let claude = sessions.iter().filter(|s| s.tool == "claude").count();
            let opencode = sessions.iter().filter(|s| s.tool == "opencode").count();
            let latest_time = sessions.first()
                .map(|s| format_beijing_time(&s.created_at))
                .unwrap_or_default();
            SessionGroup {
                path,
                dir_name,
                tool_counts: (claude, opencode),
                latest_time,
                sessions,
                is_expanded: false,
                selected_child: 0,
            }
        }).collect();

        groups.sort_by(|a, b| b.latest_time.cmp(&a.latest_time));
        groups
    }

    fn copy_resume_cmd(&self) -> Option<String> {
        let group = self.groups.get(self.selected)?;
        let session = group.sessions.get(group.selected_child)?;
        Some(if session.tool == "claude" {
            format!("claude --resume {}", session.session_id)
        } else {
            format!("opencode -s {}", session.session_id)
        })
    }

    fn get_last_message(&self) -> Option<String> {
        let group = self.groups.get(self.selected)?;
        let session = group.sessions.get(group.selected_child)?;
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
        // 折叠所有分组
        for group in &mut self.groups {
            group.is_expanded = false;
            group.selected_child = 0;
        }
    }

    fn reset_search(&mut self) {
        self.search_query = None;
        self.search_active = false;
        self.search_buffer.clear();
        self.filter_tag = None;
        self.load_sessions();
        for group in &mut self.groups {
            group.is_expanded = false;
            group.selected_child = 0;
        }
    }

    fn submit_title_edit(&mut self) {
        if !self.title_edit_buffer.is_empty() {
            if let Some(group) = self.groups.get(self.selected) {
                if let Some(session) = group.sessions.get(group.selected_child) {
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
        }
        self.title_edit_active = false;
        self.title_edit_buffer.clear();
    }

    fn cancel_title_edit(&mut self) {
        self.title_edit_active = false;
        self.title_edit_buffer.clear();
    }

    fn start_title_edit(&mut self) {
        if let Some(group) = self.groups.get(self.selected) {
            if let Some(session) = group.sessions.get(group.selected_child) {
                self.title_edit_buffer = session.title.clone().unwrap_or_default();
                self.title_edit_active = true;
            }
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
                let claude: usize = state.groups.iter().map(|g| g.tool_counts.0).sum();
                let opencode: usize = state.groups.iter().map(|g| g.tool_counts.1).sum();
                let total: usize = state.groups.iter().map(|g| g.sessions.len()).sum();
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
            let mut items = Vec::new();
            for (i, group) in state.groups.iter().enumerate() {
                let is_selected = i == state.selected && group.selected_child == 0;
                let indicator = if group.is_expanded { "[-]" } else { "[+]" };
                let counts = format!("Claude:{}, OpenCode:{}", group.tool_counts.0, group.tool_counts.1);

                if group.is_expanded {
                    // 目录行
                    let line = if is_selected {
                        format!("{} {} ({}) - latest: {}", indicator, &group.dir_name, counts, group.latest_time)
                    } else {
                        format!("{} {} ({}) - latest: {}", indicator, &group.dir_name, counts, group.latest_time)
                    };
                    items.push(ListItem::new(line));

                    // 子项
                    for (j, s) in group.sessions.iter().enumerate() {
                        let _child_selected = i == state.selected && j == group.selected_child;
                        let prefix = "  ";
                        let title = s.title.as_ref().map(|t| t.as_str()).unwrap_or("");
                        let line = format!("{} [{}] {} - {} - {}",
                            prefix, s.tool, &s.session_id[..8.min(s.session_id.len())], title, format_beijing_time(&s.created_at));
                        items.push(ListItem::new(line));
                    }
                } else {
                    let line = format!("{} {} ({}) - latest: {}", indicator, &group.dir_name, counts, group.latest_time);
                    items.push(ListItem::new(line));
                }
            }

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Sessions"))
                .style(Style::default());

            f.render_stateful_widget(list, content_chunks[0], &mut list_state);

            // Detail panel
            if let Some(group) = state.groups.get(state.selected) {
                if let Some(session) = group.sessions.get(group.selected_child) {
                    let last_msg = state.get_last_message();
                    let last_msg_line = last_msg
                        .map(|m| format!("Last: {}", m))
                        .unwrap_or_else(|| "Last: (none)".to_string());

                    let detail = format!(
                        "Tool: {}\nSession ID: {}\nProject: {}\nCreated: {}\nTitle: {}\n{}",
                        session.tool,
                        session.session_id,
                        session.project_path.as_ref().unwrap_or(&"none".to_string()),
                        format_beijing_time(&session.created_at),
                        session.title.as_ref().unwrap_or(&"(no title)".to_string()),
                        last_msg_line,
                    );
                    let para = Paragraph::new(detail)
                        .block(Block::default().borders(Borders::ALL).title("Detail"));
                    f.render_widget(para, content_chunks[1]);
                }
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
                        if state.groups.is_empty() { continue; }

                        let group = &mut state.groups[state.selected];
                        if group.is_expanded {
                            if group.selected_child < group.sessions.len() - 1 {
                                group.selected_child += 1;
                            } else if state.selected < state.groups.len() - 1 {
                                state.selected += 1;
                                state.groups[state.selected].selected_child = 0;
                                state.groups[state.selected].is_expanded = false;
                            }
                        } else {
                            if state.selected < state.groups.len() - 1 {
                                state.selected += 1;
                            }
                        }
                        list_state.select(Some(state.selected));
                    }
                    KeyCode::Up => {
                        if state.groups.is_empty() { continue; }

                        let group = &mut state.groups[state.selected];
                        if group.is_expanded {
                            if group.selected_child > 0 {
                                group.selected_child -= 1;
                            } else if state.selected > 0 {
                                state.selected -= 1;
                                let prev_group = &state.groups[state.selected];
                                state.groups[state.selected].selected_child = prev_group.sessions.len().saturating_sub(1);
                            }
                        } else {
                            if state.selected > 0 {
                                state.selected -= 1;
                            }
                        }
                        list_state.select(Some(state.selected));
                    }
                    KeyCode::Enter => {
                        if state.groups.is_empty() { continue; }
                        let group = &state.groups[state.selected];
                        if group.is_expanded && group.selected_child > 0 {
                            // 在子项上,复制 resume
                            if let Some(cmd) = state.copy_resume_cmd() {
                                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                                    let _ = clipboard.set_text(&cmd);
                                    println!("\nCopied: {}\n", cmd);
                                }
                            }
                        } else {
                            // 在目录行上,展开/折叠
                            state.groups[state.selected].is_expanded = !state.groups[state.selected].is_expanded;
                            if state.groups[state.selected].is_expanded {
                                state.groups[state.selected].selected_child = 0;
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
                        if !state.groups.is_empty() {
                            list_state.select(Some(state.selected.min(state.groups.len().saturating_sub(1))));
                        }
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
