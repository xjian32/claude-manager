# Session Manager

CLI + TUI 工具,用于管理 Claude 和 OpenCode 的历史 Session。

## 功能特性

- **自动扫描** — 扫描 Claude 和 OpenCode 的 Session 数据并存储到 SQLite
- **CLI 命令** — `scan`、`list`、`search`、`tag`、`title` 命令行操作
- **TUI 界面** — 交互式终端界面,支持浏览、搜索、编辑标签和标题
- **定时任务** — 支持安装 launchd 定时扫描 Hook
- **跨工具搜索** — 支持按 Session ID、目录名称、标签搜索

## 安装

```bash
cargo build --release
cp target/release/sm /usr/local/bin/sm
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

## 数据存储

- SQLite 数据库: `~/Library/Application Support/com.session-manager.sm/sessions.db`
- Session 数据来源:
  - Claude: `~/.claude/sessions/`
  - OpenCode: `~/.local/share/opencode/opencode.db`

## 架构

```
session-manager/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── session-store/          # SQLite 数据层
│   ├── scanner-core/            # ToolScanner trait
│   ├── scanner-claude/          # Claude scanner
│   └── scanner-opencode/        # OpenCode scanner
└── sm/
    ├── main.rs                 # CLI 入口
    └── tui.rs                  # TUI 界面
```
