# Session Manager - 目录合并功能设计

## 概述

为 TUI 和 Web UI 添加目录分组折叠功能,相同目录下的 session 合并显示,用户可展开查看详情。

## 设计决策

### 数据结构

```rust
struct SessionGroup {
    path: String,                    // 目录路径(完整路径)
    dir_name: String,                // 目录名(用于显示)
    tool_counts: (usize, usize),     // (claude数量, opencode数量)
    latest_time: String,            // 最新创建时间(格式化: 北京时间)
    sessions: Vec<Session>,          // 按创建时间倒序排列
    is_expanded: bool,
    selected_child: usize,          // 展开时选中的子项索引
}
```

### 状态管理

TUI AppState 变化:
```rust
// Before
sessions: Vec<Session>

// After
groups: Vec<SessionGroup>
selected: usize  // 当前选中的是 group 索引 (展开时指向 group)
```

Web UI 状态:
```typescript
interface State {
    groups: SessionGroup[];
    expandedPaths: Set<string>;  // 已展开的目录路径
    selectedGroup: number;
    selectedChild: number | null;  // null 表示选中的是目录行
}
```

## 渲染规格

### TUI 渲染

**折叠状态行:**
```
[+] /path/to/project (Claude:3, OpenCode:2) - latest: 2026-04-10 19:30:45
```
- `[+]` 展开指示符
- 目录路径
- 各工具数量
- 最新创建时间

**展开状态行 (目录头):**
```
[-] /path/to/project (Claude:3, OpenCode:2) - latest: 2026-04-10 19:30:45
```
- `[-]` 折叠指示符

**展开状态行 (子项):**
```
  [Claude] abc12345 - MyTitle - 2026-04-10 19:30:45
```
- 两个空格缩进
- 工具图标
- Session ID 前8位
- 标题(如有)
- 创建时间

### Web UI 渲染

**折叠状态:**
```html
<div class="group collapsed" onclick="toggleGroup(0)">
    <span class="expand-icon">▶</span>
    <span class="path">/path/to/project</span>
    <span class="counts">[Claude: 3, OpenCode: 2]</span>
    <span class="latest">latest: 2026-04-10 19:30:45</span>
</div>
```

**展开状态:**
```html
<div class="group expanded" onclick="toggleGroup(0)">
    <span class="expand-icon">▼</span>
    <span class="path">/path/to/project</span>
    ...
</div>
<div class="group-sessions">
    <div class="session-item">...</div>
    <div class="session-item">...</div>
</div>
```

## 交互规则

| 操作 | TUI 按键 | Web UI 操作 |
|------|----------|-------------|
| 展开目录 | Enter | 点击目录行 |
| 折叠目录 | Enter (在目录行上) | 点击已展开目录 |
| 展开时向上 | ↑ (在第一个子项上) | 移动到目录行 |
| 展开时向下 | ↓ (在最后一个子项上) | - |
| 选择子项复制 | Enter | 点击复制按钮 |
| 导航 | ↑/↓ 在目录行/子项间移动 | 鼠标悬停 |

## 搜索和过滤

- 搜索时按目录路径+session_id+标签匹配
- 过滤标签时只显示包含该标签的目录(及其所有 session)
- 搜索/过滤后所有分组默认折叠

## 统计显示

Header 统计逻辑不变,但计算方式调整为:
- 遍历所有 group,累加 `tool_counts`
- Total = 所有 session 数量

## 实现计划

### TUI 修改
1. `tui.rs` - 添加 `SessionGroup` 结构体
2. `load_sessions` 返回 `Vec<SessionGroup>`
3. 修改渲染逻辑处理折叠/展开
4. 修改键盘事件处理导航

### Web UI 修改
1. API 返回分组数据或前端计算分组
2. 添加 CSS 样式 (collapsed/expanded 状态)
3. 添加 `toggleGroup` 和 `selectGroup` JavaScript 函数
4. 修改 Enter 键事件处理

### API 变更 (可选)

前端分组计算更简单,但如果需要后端支持可添加:
```
GET /api/sessions?grouped=true
```

## 验证步骤

1. 运行 `cargo build --bin sm` 和 `cargo build --bin sm-web`
2. 启动 TUI: 列表按目录分组显示,Enter 展开/折叠
3. 启动 Web UI: 点击目录行展开/折叠
4. 搜索和标签过滤正常工作
5. 统计数字正确
