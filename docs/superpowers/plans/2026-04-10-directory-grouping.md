# Directory Grouping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 TUI 和 Web UI 添加目录分组折叠功能,相同目录下的 session 合并显示,用户可展开查看详情。

**Architecture:** TUI 添加 `SessionGroup` 结构体,按目录分组 sessions; Web UI 前端计算分组并添加展开/折叠交互。两者共享相同的分组逻辑(按 project_path 分组)。

**Tech Stack:** Rust (TUI), TypeScript/HTML/CSS (Web UI), Axum (API)

---

## File Mapping

### TUI 修改
- **Modify:** `sm/src/tui.rs` — 添加 SessionGroup 结构体,修改 load_sessions, 修改渲染和键盘事件处理

### Web UI 修改
- **Modify:** `sm/src/web.rs` — 返回分组数据(可选,前端分组也行)
- **Modify:** HTML 内联 — 添加分组渲染和交互逻辑

---

## Task 1: TUI - 添加 SessionGroup 数据结构

**Files:**
- Modify: `sm/src/tui.rs:24-36`

- [ ] **Step 1: 在 AppState 前添加 SessionGroup 结构体**

```rust
struct SessionGroup {
    path: String,
    dir_name: String,
    tool_counts: (usize, usize),
    latest_time: String,
    sessions: Vec<session_store::Session>,
    is_expanded: bool,
    selected_child: usize,
}
```

- [ ] **Step 2: 修改 AppState 将 sessions 替换为 groups**

```rust
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
```

- [ ] **Step 3: 修改 new() 初始化**

```rust
fn new() -> Self {
    Self {
        groups: Vec::new(),
        selected: 0,
        // ... 其他字段保持不变
    }
}
```

---

## Task 2: TUI - 实现分组逻辑

**Files:**
- Modify: `sm/src/tui.rs:55-67`

- [ ] **Step 1: 修改 load_sessions 函数实现分组**

在 `impl AppState` 中,修改 `load_sessions`:

```rust
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
```

---

## Task 3: TUI - 修改渲染逻辑

**Files:**
- Modify: `sm/src/tui.rs:210-231`

- [ ] **Step 1: 修改列表渲染以支持分组显示**

替换原来的 `items` 构造逻辑:

```rust
let mut items = Vec::new();
for (i, group) in state.groups.iter().enumerate() {
    let is_selected = i == state.selected && state.groups[i].selected_child == 0;
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
            let child_selected = i == state.selected && j == state.groups[i].selected_child;
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
```

---

## Task 4: TUI - 修改键盘事件处理

**Files:**
- Modify: `sm/src/tui.rs:295-335`

- [ ] **Step 1: 修改 Down 键处理**

替换原来的 `KeyCode::Down` 分支:

```rust
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
```

- [ ] **Step 2: 修改 Up 键处理**

```rust
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
```

- [ ] **Step 3: 修改 Enter 键处理**

```rust
KeyCode::Enter => {
    let group = &state.groups[state.selected];
    if group.is_expanded && group.selected_child > 0 {
        // 在子项上,复制 resume
        let session = &group.sessions[group.selected_child];
        let cmd = if session.tool == "claude" {
            format!("claude --resume {}", session.session_id)
        } else {
            format!("opencode -s {}", session.session_id)
        };
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&cmd);
            println!("\nCopied: {}\n", cmd);
        }
    } else {
        // 在目录行上,展开/折叠
        state.groups[state.selected].is_expanded = !state.groups[state.selected].is_expanded;
        if state.groups[state.selected].is_expanded {
            state.groups[state.selected].selected_child = 0;
        }
    }
}
```

- [ ] **Step 4: 修改 copy_resume_cmd 函数**

```rust
fn copy_resume_cmd(&self) -> Option<String> {
    let group = self.groups.get(self.selected)?;
    let session = group.sessions.get(group.selected_child)?;
    Some(if session.tool == "claude" {
        format!("claude --resume {}", session.session_id)
    } else {
        format!("opencode -s {}", session.session_id)
    })
}
```

- [ ] **Step 5: 修改 get_last_message 函数**

```rust
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
```

- [ ] **Step 6: 修改 submit_search 和 reset_search**

```rust
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
```

---

## Task 5: TUI - 修改 Detail 面板渲染

**Files:**
- Modify: `sm/src/tui.rs:233-260`

- [ ] **Step 1: 更新 Detail 面板使用正确的 session 引用**

替换原来的 Detail 渲染:

```rust
if let Some(group) = state.groups.get(state.selected) {
    let session = &group.sessions[group.selected_child];
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
    // ...
}
```

---

## Task 6: Web UI - 添加分组渲染逻辑

**Files:**
- Modify: `sm/src/web.rs` HTML 部分

**关键修复:** 必须维护一个持久化的 `groups` 数组来保存展开状态,不能每次渲染都重新计算分组。

- [ ] **Step 1: 添加全局状态变量**

在 `<script>` 标签开头添加:

```javascript
let sessions = [];
let groups = [];  // 持久化分组,保存展开状态
let selectedGroup = 0;
let selectedChild = 0;  // 0 = 目录行, >0 = 子项索引+1
```

- [ ] **Step 2: 添加分组计算函数(只用于初始化)**

```javascript
function computeGroups(sessions) {
    const map = {};
    for (const s of sessions) {
        const path = s.project_path || 'unknown';
        if (!map[path]) {
            map[path] = [];
        }
        map[path].push(s);
    }

    return Object.entries(map).map(([path, sess]) => {
        sess.sort((a, b) => b.created_at.localeCompare(a.created_at));
        const dirName = path.split('/').pop() || path;
        const claude = sess.filter(s => s.tool === 'claude').length;
        const opencode = sess.filter(s => s.tool === 'opencode').length;
        const latestTime = sess.length > 0 ? formatBeijingTime(sess[0].created_at) : '';
        return { path, dirName, claude, opencode, latestTime, sessions: sess, isExpanded: false };
    }).sort((a, b) => b.latestTime.localeCompare(a.latestTime));
}
```

- [ ] **Step 3: 修改 fetchSessions 函数**

```javascript
async function fetchSessions() {
    const params = new URLSearchParams();
    if (searchInput.value) params.set('query', searchInput.value);
    if (filterTag) params.set('tag', filterTag);

    const [sessionsRes, statsRes, tagsRes] = await Promise.all([
        fetch('/api/sessions?' + params),
        fetch('/api/stats?' + params),
        fetch('/api/tags')
    ]);

    sessions = await sessionsRes.json();
    groups = computeGroups(sessions);  // 重新计算但保持展开状态
    const stats = await statsRes.json();
    const tags = await tagsRes.json();

    // 重置选择状态
    selectedGroup = 0;
    selectedChild = 0;

    // 更新统计
    document.getElementById('stat-claude').textContent = stats.claude;
    document.getElementById('stat-opencode').textContent = stats.opencode;
    document.getElementById('stat-total').textContent = stats.total;

    renderSessions();
    renderTags(tags);
    renderDetail();
}
```

- [ ] **Step 4: 添加 selectGroup 函数**

```javascript
function selectGroup(gi, si) {
    const group = groups[gi];
    if (!group) return;

    // si === 0 表示点击目录行
    if (si === 0) {
        if (group.isExpanded) {
            // 已展开,折叠
            group.isExpanded = false;
        } else {
            // 未展开,展开
            group.isExpanded = true;
        }
        selectedGroup = gi;
        selectedChild = 0;
    } else {
        // 点击子项
        const sessionIndex = si - 1;  // si=1 对应 sessions[0]
        if (sessionIndex >= 0 && sessionIndex < group.sessions.length) {
            selectedGroup = gi;
            selectedChild = si;
        }
    }

    renderSessions();
    renderDetail();
}
```

- [ ] **Step 5: 修改 renderSessions 函数**

```javascript
function renderSessions() {
    if (groups.length === 0) {
        sessionList.innerHTML = '<div style="padding: 40px; text-align: center; color: var(--text-muted);">没有找到 Session</div>';
        return;
    }

    sessionList.innerHTML = groups.map((g, gi) => {
        const isGroupSelected = selectedGroup === gi && selectedChild === 0;
        const expandIcon = g.isExpanded ? '[-]' : '[+]';
        const counts = `Claude:${g.claude}, OpenCode:${g.opencode}`;

        let html = '';
        if (g.isExpanded) {
            // 目录行
            html += `<div class="session-item ${isGroupSelected ? 'selected' : ''}" onclick="selectGroup(${gi}, 0)">
                <span class="expand-icon">${expandIcon}</span>
                <span class="session-project">${g.dirName}</span>
                <span style="color: var(--text-muted); font-size: 12px;">(${counts})</span>
                <span style="color: var(--text-muted); font-size: 11px; margin-left: auto;">latest: ${g.latestTime}</span>
            </div>`;
            // 子项
            g.sessions.forEach((s, si) => {
                const childSelected = selectedGroup === gi && selectedChild === si + 1;
                const title = s.title || '';
                html += `<div class="session-item child ${childSelected ? 'selected' : ''}" onclick="selectGroup(${gi}, ${si + 1})">
                    <span style="width: 20px;"></span>
                    <span class="session-tool ${s.tool}">${s.tool}</span>
                    <span class="session-id">${s.session_id.substring(0, 8)}</span>
                    <span class="session-project">${title}</span>
                    <span class="session-time">${formatBeijingTime(s.created_at)}</span>
                </div>`;
            });
        } else {
            html = `<div class="session-item ${isGroupSelected ? 'selected' : ''}" onclick="selectGroup(${gi}, 0)">
                <span class="expand-icon">${expandIcon}</span>
                <span class="session-project">${g.dirName}</span>
                <span style="color: var(--text-muted); font-size: 12px;">(${counts})</span>
                <span style="color: var(--text-muted); font-size: 11px; margin-left: auto;">latest: ${g.latestTime}</span>
            </div>`;
        }
        return html;
    }).join('');
}
```

- [ ] **Step 6: 修改 renderDetail 函数**

```javascript
async function renderDetail() {
    if (selectedGroup >= groups.length) {
        detailPanel.innerHTML = '<div class="detail-empty">选择一个 Session 查看详情</div>';
        return;
    }

    const group = groups[selectedGroup];
    if (selectedChild === 0) {
        // 选中目录行,显示第一个 session 的详情
        if (!group.sessions.length) {
            detailPanel.innerHTML = '<div class="detail-empty">该目录没有 Session</div>';
            return;
        }
        const session = group.sessions[0];
        renderDetailContent(session);
        return;
    }

    const sessionIndex = selectedChild - 1;
    if (sessionIndex >= group.sessions.length) {
        detailPanel.innerHTML = '<div class="detail-empty">Session 未找到</div>';
        return;
    }

    const session = group.sessions[sessionIndex];
    renderDetailContent(session);
}

function renderDetailContent(session) {
    const resumeCmd = session.tool === 'claude'
        ? `claude --resume ${session.session_id}`
        : `opencode -s ${session.session_id}`;

    detailPanel.innerHTML = `
        <div class="detail-header">
            <div class="detail-title">${session.title || '无标题'}</div>
            <span class="session-tool ${session.tool}" style="margin-top: 8px; display: inline-block;">${session.tool}</span>
        </div>
        <div class="detail-row">
            <div class="detail-label">Session ID</div>
            <div class="detail-value mono">${session.session_id}</div>
        </div>
        <div class="detail-row">
            <div class="detail-label">项目路径</div>
            <div class="detail-value">${session.project_path || '无'}</div>
        </div>
        <div class="detail-row">
            <div class="detail-label">创建时间</div>
            <div class="detail-value">${formatBeijingTime(session.created_at)}</div>
        </div>
        <div class="detail-actions">
            <button class="btn btn-primary" onclick="copyResume()">复制 Resume 命令</button>
            <button class="btn btn-secondary" onclick="openTitleModal()">编辑标题</button>
            <button class="btn btn-secondary" onclick="openTagModal()">管理标签</button>
        </div>
        <div class="input-group" style="margin-top: 16px;">
            <div class="detail-label">Resume 命令</div>
            <div class="detail-value mono" style="background: var(--bg); padding: 12px; border-radius: 8px; margin-top: 8px;">${resumeCmd}</div>
        </div>
    `;
}
```

- [ ] **Step 7: 添加 formatBeijingTime 和 formatBeijingTimeUTC 函数**

```javascript
function formatBeijingTime(utcStr) {
    try {
        const dt = new Date(utcStr);
        return dt.toLocaleString('zh-CN', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' }).replace(/\//g, '-');
    } catch {
        return utcStr;
    }
}
```

- [ ] **Step 8: 添加 CSS 样式**

在 `<style>` 中添加:

```css
.session-item.child {
    padding-left: 40px;
    background: var(--surface);
}

.session-item.child:hover {
    background: var(--surface-hover);
}

.expand-icon {
    font-family: monospace;
    margin-right: 8px;
    color: var(--accent);
}

.session-time {
    font-size: 11px;
    color: var(--text-muted);
    margin-left: auto;
}
```

- [ ] **Step 9: 修改 copyResume 函数**

```javascript
async function copyResume() {
    if (selectedGroup >= groups.length) return;

    const group = groups[selectedGroup];
    let session;
    if (selectedChild === 0) {
        session = group.sessions[0];
    } else {
        const idx = selectedChild - 1;
        if (idx >= group.sessions.length) return;
        session = group.sessions[idx];
    }

    const cmd = session.tool === 'claude'
        ? `claude --resume ${session.session_id}`
        : `opencode -s ${session.session_id}`;

    await navigator.clipboard.writeText(cmd);
    showToast('已复制: ' + cmd);
}
```

- [ ] **Step 10: 添加 Enter 键处理**

在现有的 keydown 监听器中添加:

```javascript
// Enter 键复制 resume
if (e.key === 'Enter' && document.activeElement === document.body) {
    e.preventDefault();
    copyResume();
}
```

---

## Task 7: 构建和测试

- [ ] **Step 1: 构建 TUI**

Run: `cargo build --bin sm`
Expected: 编译成功,无错误

- [ ] **Step 2: 构建 Web UI**

Run: `cargo build --bin sm-web`
Expected: 编译成功,无错误

- [ ] **Step 3: 测试 TUI**

Run: `cargo run --bin sm`
Expected:
- 列表按目录分组显示
- Enter 展开/折叠目录
- 上下键正确导航
- Detail 面板正确显示选中项

- [ ] **Step 4: 测试 Web UI**

Run: `cargo run --bin sm-web`
访问 http://127.0.0.1:8080
Expected:
- 点击目录行展开/折叠
- 统计数字正确
- 搜索和标签过滤正常工作

---

## Task 8: 提交

- [ ] **Commit TUI changes**

```bash
git add sm/src/tui.rs && git commit -m "feat: add directory grouping with expand/collapse in TUI"
```

- [ ] **Commit Web UI changes**

```bash
git add sm/src/web.rs && git commit -m "feat: add directory grouping with expand/collapse in Web UI"
```
