# Session Manager

统一管理多个 AI 编程工具的历史 Session，支持 Claude、Claude Code、Cursor、Windsurf、OpenCode 以及自定义目录。

## 功能特性

- **多工具支持** — 自动扫描并统一管理多个 AI 工具的 Session
- **CLI 命令** — `scan`、`list`、`search`、`tag`、`title` 命令行操作
- **TUI 界面** — 交互式终端界面，支持浏览、搜索、编辑标签和标题
- **Web UI** — 浏览器访问的图形化界面 (`sm web`)
- **定时任务** — 支持安装 launchd 定时扫描 Hook
- **跨工具搜索** — 支持按 Session ID、目录名称、标签搜索
- **目录合并** — 同一目录下的 Session 可折叠/展开显示

## 支持的工具

| 工具 | 默认路径 | 扫描方式 |
|------|----------|----------|
| Claude | `~/.claude/sessions/` | JSON 文件 |
| Claude Code | `~/.claude_code/sessions/` | JSON 文件 |
| Cursor | `~/.cursor.chat/data/` | SQLite 数据库 |
| Windsurf | `~/.windsurf/history/` | JSON 文件 |
| OpenCode | `~/.local/share/opencode/opencode.db` | SQLite 数据库 |
| Generic | 用户自定义 | Glob pattern |

## 安装

```bash
cargo build --release
cp target/release/sm /usr/local/bin/sm
```

## 配置

配置文件位于: `~/.config/session-manager/config.toml`

从示例文件复制并修改:

```bash
cp config.toml.example ~/.config/session-manager/config.toml
```

### 配置项说明

```toml
[scanner.claude]
enabled = true                    # 是否启用
path = "~/.claude/sessions"      # 可选，使用默认路径

[scanner.opencode]
enabled = true
path = "~/.local/share/opencode"

[scanner.claude-code]
enabled = false                   # 默认关闭
path = "~/.claude_code/sessions"

[scanner.cursor]
enabled = false
path = "~/.cursor.chat/data"

[scanner.windsurf]
enabled = false
path = "~/.windsurf/history"

# 通用扫描器 - 扫描任意目录
[scanner.generic]
enabled = false
[[scanner.generic.sources]]
name = "my-sessions"             # 扫描器名称
path = "/path/to/sessions"       # 目录路径
pattern = "*.json"               # 文件匹配模式
```

## CLI 命令

```bash
# 扫描所有 session
sm scan

# 列出所有 session
sm list

# 按工具过滤
sm list --tool claude

# 按标签过滤
sm list --tag work

# 搜索 session
sm search "project-name"

# 管理标签
sm tag --action add <session_id> --value <tag>
sm tag --action remove <session_id> --value <tag>
sm tag --action list <session_id>

# 修改标题
sm title <session_id> <new_title>

# 启动 Web UI (默认 http://localhost:8080)
sm web
sm web --port 3000

# 安装定时扫描 (macOS)
sm install-hook

# 交互式 TUI
sm tui
```

## TUI 快捷键

| 按键 | 功能 |
|------|------|
| `↑` / `↓` | 上/下选择 session |
| `Enter` | 复制 resume 命令到剪贴板 |
| `/` 或 `Ctrl+F` | 进入搜索模式 |
| `t` | 编辑选中 session 的标题 |
| `r` | 重置搜索和标签过滤 |
| `q` | 退出 |

**搜索模式:**
- 输入搜索内容后按 `Enter` 确认
- 按 `Esc` 取消搜索
- 搜索支持 Session ID、目录名称、标签

**标题编辑模式:**
- 输入新标题后按 `Enter` 保存
- 按 `Esc` 取消编辑

## Web UI

启动后访问 http://localhost:8080

- 浏览器浏览 Session 列表
- 点击 Session 卡片复制 resume 命令
- 搜索和过滤功能
- 标签管理

## 数据存储

- SQLite 数据库: `~/Library/Application Support/com.session-manager.sm/sessions.db`

## 架构

```
session-manager/
├── Cargo.toml                  # Workspace root
├── config.toml.example         # 配置示例
├── crates/
│   ├── session-store/          # SQLite 数据层 + Store trait
│   ├── scanner-core/            # ToolScanner trait 定义
│   ├── scanner-claude/          # Claude scanner 实现
│   ├── scanner-claude-code/     # Claude Code scanner
│   ├── scanner-cursor/          # Cursor scanner
│   ├── scanner-windsurf/        # Windsurf scanner
│   ├── scanner-opencode/        # OpenCode scanner
│   └── scanner-generic/         # 通用目录 scanner
└── sm/
    ├── main.rs                 # CLI 入口 + 命令处理
    ├── tui.rs                  # TUI 界面
    └── web.rs                  # Web UI 服务
```

## 开发

```bash
# 构建所有 crate
cargo build --workspace

# 运行测试
cargo test --workspace

# 运行特定 crate 测试
cargo test -p scanner-claude
```
