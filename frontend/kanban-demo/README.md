# HertaKanban (`kanban-demo`)

基于 **React 19 + Vite 8 + TanStack 全家桶 (Router, Store, Table, Query) + Tailwind CSS + shadcn/ui** 构建的现代敏捷看板与协同系统前端，100% 深度集成 **HertaBase** 官方 TypeScript SDK (`@hb/sdk`) 与实时 SSE 总线。

---

## 🌟 核心特性

- 🎨 **Linear 级视觉质感与主题切换**：
  - 沉浸式暗黑/明亮双主题（`Light` / `Dark` / `System`），无缝平滑切换并持久化。
  - 基于 Tailwind CSS 与 Radix UI 原语构建的 shadcn/ui 组件系统。
- 📋 **敏捷看板与拖拽协同 (@dnd-kit)**：
  - 4 大状态泳道：`待处理 (Todo)`、`进行中 (In Progress)`、`评审中 (In Review)`、`已完成 (Done)`。
  - 支持流畅的跨泳道卡片拖拽与同泳道连续权重排序，带乐观更新（Optimistic UI）。
  - 移动至 `已完成` 时触发全屏庆祝五彩纸屑动画 (`canvas-confetti`)。
- 📊 **专业任务表格视图 (@tanstack/react-table)**：
  - 强大的表格视图，支持多列升降序排序、分页、状态行内快速切换。
  - 优先级/执行人/我的任务多维组合筛选与全文关键字即时检索。
- ⚡ **实时 SSE 协同同步总线**：
  - 监听 `/api/realtime/kb_tasks?filter=workspace = '...'` 实时事件。
  - 多端同屏操作时毫秒级自动同步新增、移动、修改与删除事件，顶部带实时连接状态指示灯。
- 📎 **多任务附件与 Token 鉴权下载**：
  - 支持多附件上传、格式与数量校验（PNG, JPG, PDF, ZIP，最多 5 个）。
  - 支持调用 `/api/files/token` 获取短期访问 Token 进行安全下载与预览。
- 💬 **任务评论与讨论流**：
  - 任务详情对话框内置讨论区，支持即时发表评论与作者权限删除。
- 🛡️ **细粒度 RBAC 权限与多角色快速切换**：
  - 严格对齐 `kb_workspaces` 与 `kb_tasks` API Rules（仅负责人/成员可写，私密卡片隔离）。
  - 顶部栏支持一键切换预设角色：
    - 👑 **项目负责人 (Owner)**
    - 🚀 **主要开发 (Assignee)**
    - 👥 **团队成员 (Member)**
    - 🚫 **外部访客 (Outsider - 用于验证 403/404 越权拦截)**
- 🚀 **智能初始化探测与一键建表向导**：
  - 纯 100% 真实 API 对接（无任何 Mock 假数据）。
  - 启动时自动探测后端集合是否就绪，若未建表则自动引导至 `/setup` 一键自动建表并灌入演示数据！

---

## 🛠️ 数据集合结构 (严格对齐 HertaBase)

1. **`kb_users`** (`auth`): 团队成员账号体系。
2. **`kb_workspaces`** (`base`): 工作区，含 `owner` (`relation<kb_users>`), `members` (`relation<kb_users>[]`)。
3. **`kb_tasks`** (`base`): 敏捷任务卡片，含 `title`, `description`, `priority`, `status`, `workspace`, `assignees`, `attachments`, `order`, `is_private`。
4. **`kb_comments`** (`base`): 任务讨论流，含 `task`, `author`, `content`。

---

## 🚀 快速启动

### 1. 启动 HertaBase 后端服务

从项目根目录启动（内存模式或持久化模式）：

```bash
# 启动内存数据库服务端 (默认端口 8080)
cargo run -p herta_server -- serve --db-engine memory
```

### 2. 启动前端开发服务器

从项目根目录运行：

```bash
# 启动看板前端 (默认端口 5175，自动反向代理 /api 到 8080)
pnpm dev:kanban
```

或直接进入 `frontend/kanban-demo` 目录：

```bash
cd frontend/kanban-demo
pnpm dev
```

在浏览器打开 `http://localhost:5175`，首次打开将自动进入初始化向导，点击 **“一键初始化看板与演示数据”** 即可直接体验！

### 3. 生产构建

```bash
pnpm build:kanban
```

构建产物将输出至 `frontend/kanban-demo/dist`，可直接配合 HertaBase Web 静态托管功能一键部署。
