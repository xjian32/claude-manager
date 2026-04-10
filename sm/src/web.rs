use axum::{
    extract::Query,
    routing::{get, post},
    Router, Json,
};
use session_store::{SqliteSessionStore, SessionStore, SessionFilter, SessionUpdate};
use std::net::SocketAddr;
use std::path::PathBuf;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub tool: String,
    pub session_id: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub model: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct SessionDetail {
    pub id: String,
    pub tool: String,
    pub session_id: String,
    pub project_path: Option<String>,
    pub title: Option<String>,
    pub created_at: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub claude: usize,
    pub opencode: usize,
    pub total: usize,
}

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub tool: Option<String>,
    pub tag: Option<String>,
    pub query: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TitleUpdate {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct TagUpdate {
    pub session_id: String,
    pub action: String,
    pub value: String,
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

fn format_beijing_time(utc_str: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(utc_str) {
        let beijing = dt.with_timezone(&chrono_tz::Asia::Shanghai);
        beijing.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        utc_str.to_string()
    }
}

async fn list_sessions(Query(params): Query<ListParams>) -> Json<Vec<Session>> {
    let db_path = get_db_path();
    let store = match SqliteSessionStore::new(db_path) {
        Ok(s) => s,
        Err(_) => return Json(vec![]),
    };

    let tags = params.tag.as_ref().map(|t| vec![t.clone()]);
    let filter = SessionFilter {
        tool: params.tool,
        tags,
        project_path: None,
        query: params.query,
    };

    let sessions = store.list_sessions(&filter).unwrap_or_default();
    let result: Vec<Session> = sessions
        .into_iter()
        .map(|s| Session {
            id: s.id,
            tool: s.tool,
            session_id: s.session_id,
            project_path: s.project_path,
            title: s.title,
            model: s.model,
            created_at: s.created_at,
        })
        .collect();

    Json(result)
}

async fn get_stats(Query(params): Query<ListParams>) -> Json<Stats> {
    let db_path = get_db_path();
    let store = match SqliteSessionStore::new(db_path) {
        Ok(s) => s,
        Err(_) => return Json(Stats { claude: 0, opencode: 0, total: 0 }),
    };

    let tags = params.tag.as_ref().map(|t| vec![t.clone()]);
    let filter = SessionFilter {
        tool: None,
        tags,
        project_path: None,
        query: params.query.clone(),
    };

    let sessions = store.list_sessions(&filter).unwrap_or_default();
    let claude = sessions.iter().filter(|s| s.tool == "claude").count();
    let opencode = sessions.iter().filter(|s| s.tool == "opencode").count();

    Json(Stats {
        claude,
        opencode,
        total: claude + opencode,
    })
}

async fn get_session_detail(Query(params): Query<ListParams>) -> Json<Option<SessionDetail>> {
    let db_path = get_db_path();
    let store = match SqliteSessionStore::new(db_path) {
        Ok(s) => s,
        Err(_) => return Json(None),
    };

    let tags = params.tag.as_ref().map(|t| vec![t.clone()]);
    let filter = SessionFilter {
        tool: params.tool,
        tags,
        project_path: None,
        query: params.query,
    };

    let sessions = store.list_sessions(&filter).unwrap_or_default();
    if let Some(s) = sessions.first() {
        let tags = store.get_tags(&s.session_id).unwrap_or_default();
        Json(Some(SessionDetail {
            id: s.id.clone(),
            tool: s.tool.clone(),
            session_id: s.session_id.clone(),
            project_path: s.project_path.clone(),
            title: s.title.clone(),
            created_at: format_beijing_time(&s.created_at),
            tags,
        }))
    } else {
        Json(None)
    }
}

async fn get_all_tags() -> Json<Vec<String>> {
    let db_path = get_db_path();
    let store = match SqliteSessionStore::new(db_path) {
        Ok(s) => s,
        Err(_) => return Json(vec![]),
    };

    let tags = store.list_all_tags().unwrap_or_default();
    Json(tags)
}

async fn update_title(Json(payload): Json<TitleUpdate>) -> Json<bool> {
    let db_path = get_db_path();
    let mut store = match SqliteSessionStore::new(db_path) {
        Ok(s) => s,
        Err(_) => return Json(false),
    };

    let result = store.update_session(&payload.id, &SessionUpdate {
        title: Some(payload.title),
        project_path: None,
        metadata: None,
    });

    Json(result.is_ok())
}

async fn update_tag(Json(payload): Json<TagUpdate>) -> Json<bool> {
    let db_path = get_db_path();
    let mut store = match SqliteSessionStore::new(db_path) {
        Ok(s) => s,
        Err(_) => return Json(false),
    };

    let result = match payload.action.as_str() {
        "add" => store.add_tag(&payload.session_id, &payload.value),
        "remove" => store.remove_tag(&payload.session_id, &payload.value),
        _ => return Json(false),
    };

    Json(result.is_ok())
}

async fn get_resume_command(Query(params): Query<ListParams>) -> Json<Option<String>> {
    let db_path = get_db_path();
    let store = match SqliteSessionStore::new(db_path) {
        Ok(s) => s,
        Err(_) => return Json(None),
    };

    let session_id = match params.session_id {
        Some(id) => id,
        None => return Json(None),
    };

    let sessions = store.list_sessions(&SessionFilter::default()).unwrap_or_default();
    let session = sessions.into_iter().find(|s| s.session_id == session_id);

    let cmd = session.map(|s| {
        if s.tool == "claude" {
            format!("claude --resume {}", s.session_id)
        } else {
            format!("opencode -s {}", s.session_id)
        }
    });

    Json(cmd)
}

async fn index() -> axum::response::Html<&'static str> {
    axum::response::Html(HTML)
}

#[tokio::main]
pub async fn run_server() {
    let app = Router::new()
        .route("/api/sessions", get(list_sessions))
        .route("/api/stats", get(get_stats))
        .route("/api/session-detail", get(get_session_detail))
        .route("/api/tags", get(get_all_tags))
        .route("/api/title", post(update_title))
        .route("/api/tag", post(update_tag))
        .route("/api/resume", get(get_resume_command))
        .route("/", get(index));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("Session Manager Web UI: http://127.0.0.1:8080");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

static HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Session Manager</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }

        :root {
            --bg: #0a0a0f;
            --surface: #12121a;
            --surface-hover: #1a1a24;
            --border: #2a2a3a;
            --text: #e4e4e7;
            --text-muted: #71717a;
            --accent: #10b981;
            --accent-dim: rgba(16, 185, 129, 0.15);
            --claude: #d97706;
            --opencode: #6366f1;
        }

        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            background: var(--bg);
            color: var(--text);
            min-height: 100vh;
        }

        .container {
            display: flex;
            flex-direction: column;
            height: 100vh;
        }

        /* Header */
        header {
            background: var(--surface);
            border-bottom: 1px solid var(--border);
            padding: 16px 24px;
            display: flex;
            align-items: center;
            gap: 24px;
        }

        .logo {
            font-size: 18px;
            font-weight: 600;
            color: var(--text);
            display: flex;
            align-items: center;
            gap: 8px;
        }

        .logo-icon {
            width: 28px;
            height: 28px;
            background: linear-gradient(135deg, var(--accent), #059669);
            border-radius: 6px;
        }

        .search-box {
            flex: 1;
            max-width: 400px;
            position: relative;
        }

        .search-box input {
            width: 100%;
            background: var(--bg);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 10px 16px;
            color: var(--text);
            font-size: 14px;
            outline: none;
            transition: border-color 0.2s, box-shadow 0.2s;
        }

        .search-box input:focus {
            border-color: var(--accent);
            box-shadow: 0 0 0 3px var(--accent-dim);
        }

        .stats {
            display: flex;
            gap: 16px;
            font-size: 13px;
        }

        .stat {
            display: flex;
            align-items: center;
            gap: 6px;
        }

        .stat-dot {
            width: 8px;
            height: 8px;
            border-radius: 50%;
        }

        .stat-dot.claude { background: var(--claude); }
        .stat-dot.opencode { background: var(--opencode); }
        .stat-dot.total { background: var(--accent); }

        .stat-value { color: var(--text); font-weight: 500; }
        .stat-label { color: var(--text-muted); }

        /* Main Content */
        main {
            display: flex;
            flex: 1;
            overflow: hidden;
        }

        /* Session List */
        .session-list {
            width: 55%;
            border-right: 1px solid var(--border);
            overflow-y: auto;
        }

        .session-item {
            padding: 14px 20px;
            border-bottom: 1px solid var(--border);
            cursor: pointer;
            transition: background 0.15s;
            display: flex;
            align-items: center;
            gap: 12px;
        }

        .session-item:hover {
            background: var(--surface-hover);
        }

        .session-item.selected {
            background: var(--accent-dim);
            border-left: 3px solid var(--accent);
            padding-left: 17px;
        }

        .session-tool {
            font-size: 11px;
            font-weight: 600;
            padding: 3px 8px;
            border-radius: 4px;
            text-transform: uppercase;
        }

        .session-tool.claude {
            background: rgba(217, 119, 6, 0.2);
            color: var(--claude);
        }

        .session-tool.opencode {
            background: rgba(99, 102, 241, 0.2);
            color: var(--opencode);
        }

        .session-id {
            font-family: 'SF Mono', Monaco, monospace;
            font-size: 12px;
            color: var(--text-muted);
        }

        .session-project {
            font-size: 13px;
            color: var(--text);
            flex: 1;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }

        .session-title {
            font-size: 12px;
            color: var(--text-muted);
            margin-left: auto;
        }

        /* Detail Panel */
        .detail {
            width: 45%;
            padding: 24px;
            overflow-y: auto;
            background: var(--surface);
        }

        .detail-empty {
            height: 100%;
            display: flex;
            align-items: center;
            justify-content: center;
            color: var(--text-muted);
            font-size: 14px;
        }

        .detail-header {
            margin-bottom: 24px;
        }

        .detail-title {
            font-size: 16px;
            font-weight: 500;
            margin-bottom: 4px;
        }

        .detail-tool {
            display: inline-block;
            font-size: 11px;
            font-weight: 600;
            padding: 3px 8px;
            border-radius: 4px;
            text-transform: uppercase;
        }

        .detail-row {
            margin-bottom: 16px;
        }

        .detail-label {
            font-size: 11px;
            color: var(--text-muted);
            text-transform: uppercase;
            letter-spacing: 0.5px;
            margin-bottom: 4px;
        }

        .detail-value {
            font-size: 14px;
            color: var(--text);
            word-break: break-all;
        }

        .detail-value.mono {
            font-family: 'SF Mono', Monaco, monospace;
            font-size: 12px;
        }

        .detail-actions {
            display: flex;
            gap: 12px;
            margin-top: 24px;
            padding-top: 24px;
            border-top: 1px solid var(--border);
        }

        .btn {
            padding: 10px 16px;
            border-radius: 8px;
            font-size: 13px;
            font-weight: 500;
            cursor: pointer;
            border: none;
            transition: all 0.15s;
        }

        .btn-primary {
            background: var(--accent);
            color: #fff;
        }

        .btn-primary:hover {
            background: #059669;
        }

        .btn-secondary {
            background: var(--surface-hover);
            color: var(--text);
            border: 1px solid var(--border);
        }

        .btn-secondary:hover {
            background: var(--border);
        }

        .btn-danger {
            background: rgba(239, 68, 68, 0.1);
            color: #ef4444;
            border: 1px solid rgba(239, 68, 68, 0.2);
        }

        .btn-danger:hover {
            background: rgba(239, 68, 68, 0.2);
        }

        .input-group {
            margin-top: 16px;
        }

        .input-group input {
            width: 100%;
            background: var(--bg);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 10px 16px;
            color: var(--text);
            font-size: 14px;
            outline: none;
        }

        .input-group input:focus {
            border-color: var(--accent);
        }

        .input-group-row {
            display: flex;
            gap: 8px;
        }

        .input-group-row input {
            flex: 1;
        }

        /* Tags */
        .tags-section {
            padding: 16px 24px;
            background: var(--surface);
            border-top: 1px solid var(--border);
        }

        .tags-label {
            font-size: 11px;
            color: var(--text-muted);
            text-transform: uppercase;
            letter-spacing: 0.5px;
            margin-bottom: 12px;
        }

        .tags-list {
            display: flex;
            flex-wrap: wrap;
            gap: 8px;
        }

        .tag {
            padding: 6px 12px;
            background: var(--bg);
            border: 1px solid var(--border);
            border-radius: 6px;
            font-size: 12px;
            color: var(--text-muted);
            cursor: pointer;
            transition: all 0.15s;
        }

        .tag:hover {
            border-color: var(--accent);
            color: var(--accent);
        }

        .tag.active {
            background: var(--accent-dim);
            border-color: var(--accent);
            color: var(--accent);
        }

        .tag-reset {
            padding: 6px 12px;
            background: transparent;
            border: 1px dashed var(--border);
            border-radius: 6px;
            font-size: 12px;
            color: var(--text-muted);
            cursor: pointer;
        }

        .tag-reset:hover {
            border-color: var(--text-muted);
            color: var(--text);
        }

        /* Toast */
        .toast {
            position: fixed;
            bottom: 80px;
            left: 50%;
            transform: translateX(-50%) translateY(100px);
            background: var(--surface);
            border: 1px solid var(--border);
            padding: 12px 24px;
            border-radius: 8px;
            font-size: 14px;
            opacity: 0;
            transition: all 0.3s;
            z-index: 100;
        }

        .toast.show {
            transform: translateX(-50%) translateY(0);
            opacity: 1;
        }

        /* Edit Title Modal */
        .modal-overlay {
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0, 0, 0, 0.7);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 200;
            opacity: 0;
            pointer-events: none;
            transition: opacity 0.2s;
        }

        .modal-overlay.show {
            opacity: 1;
            pointer-events: auto;
        }

        .modal {
            background: var(--surface);
            border: 1px solid var(--border);
            border-radius: 12px;
            padding: 24px;
            width: 400px;
            max-width: 90vw;
        }

        .modal-title {
            font-size: 16px;
            font-weight: 600;
            margin-bottom: 16px;
        }

        .modal input {
            width: 100%;
            background: var(--bg);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 12px 16px;
            color: var(--text);
            font-size: 14px;
            outline: none;
            margin-bottom: 16px;
        }

        .modal input:focus {
            border-color: var(--accent);
        }

        .modal-actions {
            display: flex;
            gap: 12px;
            justify-content: flex-end;
        }

        /* Add Tag Modal */
        .tag-input-group {
            display: flex;
            gap: 8px;
            margin-bottom: 16px;
        }

        .tag-input-group input {
            flex: 1;
            margin-bottom: 0 !important;
        }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <div class="logo">
                <div class="logo-icon"></div>
                Session Manager
            </div>
            <div class="search-box">
                <input type="text" id="search" placeholder="搜索 Session ID、目录、标签...">
            </div>
            <div class="stats">
                <div class="stat">
                    <div class="stat-dot claude"></div>
                    <span class="stat-value" id="stat-claude">0</span>
                    <span class="stat-label">Claude</span>
                </div>
                <div class="stat">
                    <div class="stat-dot opencode"></div>
                    <span class="stat-value" id="stat-opencode">0</span>
                    <span class="stat-label">OpenCode</span>
                </div>
                <div class="stat">
                    <div class="stat-dot total"></div>
                    <span class="stat-value" id="stat-total">0</span>
                    <span class="stat-label">Total</span>
                </div>
            </div>
        </header>

        <main>
            <div class="session-list" id="session-list"></div>
            <div class="detail" id="detail">
                <div class="detail-empty">选择一个 Session 查看详情</div>
            </div>
        </main>

        <div class="tags-section">
            <div class="tags-label">标签筛选</div>
            <div class="tags-list" id="tags-list"></div>
        </div>
    </div>

    <div class="toast" id="toast"></div>

    <div class="modal-overlay" id="title-modal">
        <div class="modal">
            <div class="modal-title">编辑标题</div>
            <input type="text" id="title-input" placeholder="输入新标题...">
            <div class="modal-actions">
                <button class="btn btn-secondary" onclick="closeTitleModal()">取消</button>
                <button class="btn btn-primary" onclick="saveTitle()">保存</button>
            </div>
        </div>
    </div>

    <div class="modal-overlay" id="tag-modal">
        <div class="modal">
            <div class="modal-title">管理标签</div>
            <div class="tag-input-group">
                <input type="text" id="tag-input" placeholder="输入标签...">
            </div>
            <div id="current-tags" style="margin-bottom: 16px;"></div>
            <div class="modal-actions">
                <button class="btn btn-secondary" onclick="closeTagModal()">关闭</button>
                <button class="btn btn-primary" onclick="addTag()">添加标签</button>
            </div>
        </div>
    </div>

    <script>
        let sessions = [];
        let selectedSession = null;
        let filterTag = null;
        let searchTimeout = null;

        const searchInput = document.getElementById('search');
        const sessionList = document.getElementById('session-list');
        const detailPanel = document.getElementById('detail');
        const tagsList = document.getElementById('tags-list');

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
            const stats = await statsRes.json();
            const tags = await tagsRes.json();

            document.getElementById('stat-claude').textContent = stats.claude;
            document.getElementById('stat-opencode').textContent = stats.opencode;
            document.getElementById('stat-total').textContent = stats.total;

            renderSessions();
            renderTags(tags);
        }

        function renderSessions() {
            if (sessions.length === 0) {
                sessionList.innerHTML = '<div style="padding: 40px; text-align: center; color: var(--text-muted);">没有找到 Session</div>';
                return;
            }

            sessionList.innerHTML = sessions.map(s => {
                const projectName = s.project_path ? s.project_path.split('/').pop() : 'unknown';
                const isSelected = selectedSession && selectedSession.session_id === s.session_id;
                return `
                    <div class="session-item ${isSelected ? 'selected' : ''}" onclick="selectSession('${s.session_id}')">
                        <span class="session-tool ${s.tool}">${s.tool}</span>
                        <span class="session-id">${s.session_id.substring(0, 8)}</span>
                        <span class="session-project">${projectName}</span>
                        ${s.title ? `<span class="session-title">${s.title}</span>` : ''}
                    </div>
                `;
            }).join('');
        }

        function renderTags(tags) {
            tagsList.innerHTML = `
                <div class="tag-reset" onclick="resetFilter()">重置</div>
                ${tags.map(t => `
                    <div class="tag ${filterTag === t ? 'active' : ''}" onclick="filterByTag('${t}')">${t}</div>
                `).join('')}
            `;
        }

        async function selectSession(sessionId) {
            selectedSession = sessions.find(s => s.session_id === sessionId);
            await renderDetail();
        }

        async function renderDetail() {
            if (!selectedSession) {
                detailPanel.innerHTML = '<div class="detail-empty">选择一个 Session 查看详情</div>';
                return;
            }

            const params = new URLSearchParams();
            if (filterTag) params.set('tag', filterTag);
            if (searchInput.value) params.set('query', searchInput.value);

            const res = await fetch('/api/session-detail?' + params);
            const detail = await res.json();

            if (!detail) {
                detailPanel.innerHTML = '<div class="detail-empty">Session 未找到</div>';
                return;
            }

            const resumeCmd = detail.tool === 'claude'
                ? `claude --resume ${detail.session_id}`
                : `opencode -s ${detail.session_id}`;

            detailPanel.innerHTML = `
                <div class="detail-header">
                    <div class="detail-title">${detail.title || '无标题'}</div>
                    <span class="session-tool ${detail.tool}" style="margin-top: 8px; display: inline-block;">${detail.tool}</span>
                </div>

                <div class="detail-row">
                    <div class="detail-label">Session ID</div>
                    <div class="detail-value mono">${detail.session_id}</div>
                </div>

                <div class="detail-row">
                    <div class="detail-label">项目路径</div>
                    <div class="detail-value">${detail.project_path || '无'}</div>
                </div>

                <div class="detail-row">
                    <div class="detail-label">创建时间</div>
                    <div class="detail-value">${detail.created_at}</div>
                </div>

                <div class="detail-row">
                    <div class="detail-label">标签</div>
                    <div class="detail-value">${detail.tags.length > 0 ? detail.tags.join(', ') : '无'}</div>
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

        async function copyResume() {
            if (!selectedSession) return;

            const res = await fetch('/api/resume?session_id=' + encodeURIComponent(selectedSession.session_id));
            const data = await res.json();

            if (data) {
                await navigator.clipboard.writeText(data);
                showToast('已复制: ' + data);
            }
        }

        function showToast(msg) {
            const toast = document.getElementById('toast');
            toast.textContent = msg;
            toast.classList.add('show');
            setTimeout(() => toast.classList.remove('show'), 2000);
        }

        function filterByTag(tag) {
            filterTag = filterTag === tag ? null : tag;
            fetchSessions();
        }

        function resetFilter() {
            filterTag = null;
            searchInput.value = '';
            fetchSessions();
        }

        function openTitleModal() {
            if (!selectedSession) return;
            document.getElementById('title-input').value = selectedSession.title || '';
            document.getElementById('title-modal').classList.add('show');
            document.getElementById('title-input').focus();
        }

        function closeTitleModal() {
            document.getElementById('title-modal').classList.remove('show');
        }

        async function saveTitle() {
            if (!selectedSession) return;

            const title = document.getElementById('title-input').value;
            const res = await fetch('/api/title', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ id: selectedSession.id, title })
            });

            if (await res.json()) {
                closeTitleModal();
                showToast('标题已保存');
                fetchSessions();
                renderDetail();
            }
        }

        function openTagModal() {
            if (!selectedSession) return;
            loadCurrentTags();
            document.getElementById('tag-modal').classList.add('show');
            document.getElementById('tag-input').focus();
        }

        function closeTagModal() {
            document.getElementById('tag-modal').classList.remove('show');
        }

        async function loadCurrentTags() {
            const res = await fetch('/api/session-detail?' + new URLSearchParams({ query: searchInput.value }));
            const detail = await res.json();

            if (detail && detail.tags) {
                document.getElementById('current-tags').innerHTML = detail.tags.map(t => `
                    <span class="tag active" style="display: inline-flex; align-items: center; gap: 6px;">
                        ${t}
                        <span style="cursor: pointer; opacity: 0.6;" onclick="removeTag('${t}')">×</span>
                    </span>
                `).join('');
            }
        }

        async function addTag() {
            if (!selectedSession) return;

            const input = document.getElementById('tag-input');
            const tag = input.value.trim();
            if (!tag) return;

            const res = await fetch('/api/tag', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ session_id: selectedSession.session_id, action: 'add', value: tag })
            });

            if (await res.json()) {
                input.value = '';
                showToast('标签已添加');
                loadCurrentTags();
                fetchSessions();
            }
        }

        async function removeTag(tag) {
            if (!selectedSession) return;

            const res = await fetch('/api/tag', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ session_id: selectedSession.session_id, action: 'remove', value: tag })
            });

            if (await res.json()) {
                showToast('标签已移除');
                loadCurrentTags();
                fetchSessions();
            }
        }

        // Search with debounce
        searchInput.addEventListener('input', () => {
            clearTimeout(searchTimeout);
            searchTimeout = setTimeout(fetchSessions, 300);
        });

        // Enter to copy resume
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && document.activeElement === searchInput) {
                if (sessions.length > 0) {
                    selectSession(sessions[0].session_id);
                }
            }
        });

        // Close modals on escape
        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') {
                closeTitleModal();
                closeTagModal();
            }
        });

        // Initial load
        fetchSessions();
    </script>
</body>
</html>"#;

pub fn main() {
    run_server()
}
