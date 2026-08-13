# HertaBase 项目架构设计

> 注意：HertaBase 项目目前处于规划阶段（Planning Stage），以下目录结构将在开发过程中逐步创建。

对于兼具 Rust 复杂底层逻辑和前端管理后台（JS/TS）的项目，HertaBase 采用**混合 Monorepo 架构（Cargo Workspace + PNPM Workspace）**。这种架构能将 Rust 生态与前端 Node 生态有效结合。

---

## 一、 整体目录结构设计 (The Big Picture)

项目分为两大部分：**Rust 核心（Cargo 工作区）** 和 **前端/JS 生态（PNPM 工作区）**。

```text
hertabase/
├── .git/
├── .github/                 # CI/CD 流程配置
├── Cargo.toml               # 📦 Cargo Workspace 根配置
├── pnpm-workspace.yaml      # 📦 PNPM Workspace 根配置
├── package.json             # 根 package.json (用于提供全局运行脚本)
├── Justfile / Makefile      # 任务运行器
│
├── crates/                  # 🦀 Rust 后端微内核 (Cargo Workspace)
│   ├── herta_api/           # Salvo 路由、控制器、中间件
│   ├── herta_core/          # App 上下文、配置管理、全局错误处理
│   ├── herta_db/            # SurrealDB 客户端封装、Schema 动态转换
│   ├── herta_jsvm/          # rquickjs 运行时、沙盒、Rust↔JS FFI
│   ├── herta_storage/       # 文件存储抽象适配器 (FS/S3)
│   └── herta_server/        # CLI 入口、前端静态资源嵌入 (rust-embed)
│
├── frontend/                # 🌐 前端项目 (PNPM Workspace)
│   └── admin-ui/            # React/Vite 管理后台 SPA
│
├── packages/                # 📦 JS/TS 包 (PNPM Workspace)
│   ├── @hb/sdk/             # (可选) 面向最终用户的 JS SDK
│   └── @hb/types/           # Hook 编写者的 .d.ts 类型定义
│
└── examples/                # 示例与参考
    ├── hooks/               # Hook 脚本示例
    └── data/
```

`herta_storage` 只负责逻辑 key 安全、对象读写、范围读取与前缀删除；它不理解 Collection Rule 或记录字段。`herta_api` 负责 multipart 编排、文件令牌、记录成员关系校验和失败补偿，`herta_db` 负责 file 字段的单值/数组 schema 与记录校验，`herta_auth` 负责与账户 `token_key` 绑定的文件 JWT。完整边界见 [文件存储与上传](storage.md)。

---

## 二、 核心配置文件编写

### 1. Cargo Workspace 根配置 (`Cargo.toml`)

在根目录通过 Cargo 统一管理 Rust 依赖，避免不同 crate 编译出不同版本的底层依赖，优化编译速度。

```toml
[workspace]
resolver = "2"
members = [
    "crates/*"
]

[workspace.dependencies]
anyhow = "1"
salvo = { version = "0.95", features = ["oapi", "cors", "anyhow", "jwt-auth"] }
surrealdb = { version = "3.2.3", features = ["kv-rocksdb"] }
rquickjs = { version = "0.12.2", features = ["macro"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.53", features = ["full"] }
```

### 2. PNPM Workspace 根配置 (`pnpm-workspace.yaml`)

配置 PNPM 工作区包含前端模块与依赖包模块。

```yaml
packages:
  - 'frontend/*'
  - 'packages/*'
```

### 3. 根级 `package.json` (任务调度中心)

利用根目录的 `package.json`，将跨语言编译命令封装成 npm scripts，以实现前后端协同构建。

```json
{
  "name": "@hb/monorepo",
  "private": true,
  "scripts": {
    "dev:ui": "pnpm --filter @hb/admin-ui dev",
    "dev:server": "cargo run -p herta_server serve",
    "dev": "pnpm dev:ui",
    "build:ui": "pnpm --filter @hb/admin-ui build",
    "build:server": "cargo build --release",
    "build": "pnpm build:ui && pnpm build:server"
  }
}
```

---

## 三、 架构设计亮点与技术解析

### 1. Crates 模块拆分优势

- **编译速度**：`rquickjs`（包含 C 编译）和 `surrealdb`（包含 RocksDB C++ 编译）构建耗时较长。拆分后，`herta_jsvm` 和 `herta_db` 若无变动即可复用缓存，Salvo 的路由层（`herta_api`）可实现快速编译。
- **依赖隔离**：前端后台打包（`rust-embed`）仅在 `herta_server` crate 中进行，不污染底层 `herta_core` 模块。

### 2. 前端打包与 Rust 静态嵌入流程

通过 `rust-embed` 将 React/Vite 前端产物打包进 Rust 二进制文件，并由 Salvo 在
`/webui/` 下提供静态资源和 SPA history fallback。

**构建流程：**

1. 执行 `pnpm build:ui`，在 `frontend/admin-ui/dist` 生成静态 HTML/JS/CSS。
2. 在 `crates/herta_server` 中，引入 `rust-embed` 库。
3. 代码实现如下：

   ```rust
   // crates/herta_server/src/ui.rs
   use rust_embed::Embed;

   #[derive(Embed)]
   #[folder = "../../frontend/admin-ui/dist/"]
   struct AdminUiAssets;
   ```

4. **构建顺序**：使用根目录 `pnpm build` 或 `just build`，确保先执行 UI 构建，再执行
   Cargo release 构建。直接执行 `cargo build -p herta_server` 前必须已有 `dist` 目录。

### 3. `@hb/types` 包的作用

HertaBase 允许在 `hb_hooks` 目录下编写应用级 JS 扩展。脚本在启动期通过
`onRecordCreate()`、`routerAdd()`、`cronAdd()` 等函数显式注册行为，运行时通过
`$app` 使用数据库、邮件、受限 HTTP、实时、文件和日志服务。完整契约见
[JavaScript 扩展运行时设计](js-runtime.md)，`@hb/types` 提供编写时的代码提示。

- 在 `packages/@hb/types` 中维护 `index.d.ts`，定义 FFI 暴露给 JS 的 API：

    ```typescript
    // packages/@hb/types/index.d.ts
    declare global {
      const $app: App;
      const $apis: ApiMiddlewareFactory;
      function onRecordCreate(
        handler: (event: RecordEvent) => void | Promise<void>,
        ...collections: string[]
      ): void;
      function routerAdd(
        method: string,
        path: string,
        handler: (event: RequestEvent) => unknown,
        ...middleware: ApiMiddleware[]
      ): void;
      function cronAdd(name: string, expression: string, handler: () => unknown): void;
    }
    ```

- 开发者在编写扩展时引用该包即可获得类型提示，实际运行时由 Rust 的 `rquickjs` 提供注册函数、事件对象和 `$app`。类型包只声明服务端已经实现的能力。

### 4. 开发环境的热更新联动

- **前端热更新**：`admin-ui` 使用 Vite 启动监听（默认入口 `http://localhost:5173/webui/`）。
- **后端热更新**：使用 `cargo-watch` 监听 Rust 服务（默认 `http://localhost:8080`）。
- **API 代理**：在 `admin-ui` 的 `vite.config.ts` 中配置 proxy，将对 `/api/*` 的请求代理到 Rust 服务器。该模式保障了前后端分离的热更新体验，最终 Release 阶段再合并发布。

### 总结

Cargo + PNPM 混合工作区架构，实现了 Rust 底层服务与前端 Node 生态的解耦。

1. **开发者体验 (DX)**：利用 Vite 极速开发前端，通过 cargo-watch 极速调试后端。
2. **架构扩展性**：各模块独立，便于未来替换或升级单独组件。
3. **分发优雅**：应用可被打包为单一二进制文件 `hertabase`，实现开箱即用。
