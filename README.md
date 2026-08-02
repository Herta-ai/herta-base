<div align="center">

# 🚀 HertaBase

**The Next-Generation, Single-Binary Backend-as-a-Service.**

[![Rust](https://img.shields.io/badge/Rust-1.97.1-f06292.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![Salvo](https://img.shields.io/badge/Salvo-Web_Framework-blue.svg?style=for-the-badge)](https://salvo.rs/)
[![SurrealDB](https://img.shields.io/badge/SurrealDB-Database-ff00a0.svg?style=for-the-badge)](https://surrealdb.com/)
[![QuickJS](https://img.shields.io/badge/QuickJS-JS_Runtime-f7df1e.svg?style=for-the-badge&logo=javascript)](https://bellard.org/quickjs/)
[![License: MIT](https://img.shields.io/badge/License-MIT-success.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=for-the-badge)](http://makeapullrequest.com)
[![Status](https://img.shields.io/badge/Status-Pre--Alpha-orange.svg?style=for-the-badge)](#)

> 💡 **HertaBase** 是一个用 Rust 编写的开源 BaaS（后端即服务）。它将 Web 服务器、图/文档数据库、实时订阅引擎、JS 扩展运行时和精美的管理后台，全部打包进**一个极致轻量的独立二进制文件**中。

**⚠️ 注意：本项目目前处于 Pre-Alpha，Phase 1 后端正在实现。**

[快速开始](#-快速开始) • [核心特性](#-核心特性) • [架构设计](#-架构设计) • [编写 Hooks](#-编写扩展-hooks) • [路线图](#-开发路线图-roadmap)

</div>

---

## ✨ 核心特性

HertaBase 旨在提供极致的开发体验（DX），同时利用 Rust 生态带来高性能与可扩展性。

- 📦 **All-in-One 单文件部署**：无需配置复杂的环境，一个二进制文件包含数据库、API 和管理后台。
- 🗄️ **强悍的 SurrealDB 引擎**：原生支持图关系（Graph）、文档（Document）、Schema-less/full 混合模式。支持从本地单文件（RocksDB）无缝迁移至分布式集群（TiKV）。
- ⚡ **极速 JS 运行时**：内置 `rquickjs`，允许使用 JavaScript/TypeScript 编写生命周期 Hook，微秒级冷启动，内存占用极低。
- 🔄 **原生实时订阅 (Realtime)**：基于 SurrealDB 的 `LIVE SELECT` 与 Salvo SSE，数据变更推送到客户端。
- 📖 **OpenAPI 自动生成**：当在后台创建 Collection（表）时，系统自动生成 Swagger/Redoc API 文档。
- 🛡️ **内建 Auth 与权限引擎**：开箱即用的 JWT 鉴权，支持细粒度的 API 访问规则（API Rules）。
- 🎨 **现代化 Admin UI**：基于 SvelteKit 构建，并在编译时通过 `rust-embed` 静态嵌入到二进制中。

---

## 🛠️ 技术栈 (Tech Stack)

| 模块 | 核心技术 | 描述 |
| :--- | :--- | :--- |
| **语言** | 🦀 **Rust** | 内存安全、零成本抽象、无与伦比的并发性能 |
| **HTTP 层** | 🌐 **Salvo** | 易用且强大的 Rust Web 框架，原生支持 OpenAPI |
| **数据库** | 🗄️ **SurrealDB** | 多模数据库，BaaS 架构的核心组件 |
| **JS 引擎** | 🚀 **rquickjs** | 轻量的 JavaScript 引擎，融入 Rust 异步生态 |
| **管理后台**| 🎨 **SvelteKit + Tailwind** | SPA 体验，通过 PNPM Workspace 联合构建 |

---

## 🚀 快速开始

### 1. 下载或编译

可以直接从 Releases 下载预编译的二进制文件，或者通过源码编译：

```bash
# 克隆项目 (包含 Cargo & PNPM monorepo)
git clone https://github.com/Herta-ai/hertabase.git
cd hertabase

# 编译 Rust 核心服务器 (Release 模式；Windows 需提供 CMake)
cargo build --release
```

### 2. 启动服务

```bash
./target/release/hertabase serve
```

成功启动后可以访问：

- 管理后台：`http://localhost:8080/_/`
- API 接口：`http://localhost:8080/api/`
- API 文档：`http://localhost:8080/swagger-ui/`

---

## 🧩 编写扩展 Hooks (Serverless JS)

在运行目录下创建 `hb_hooks/{lifecycle}_{collection}.js` 形式的 Hook 脚本（如 `hb_hooks/before_create_posts.js`），即可拦截并修改 HTTP 请求与数据库操作。详见 [Hook 开发指南](docs/hooks-guide.md)。

```javascript
// 享受 TypeScript 代码提示 (@hb/types)
export default async function (context) {
    const { request, user } = context;

    // 1. 验证自定义业务逻辑
    if (request.body.title.includes("spam")) {
        throw new Error("Title contains spam words!");
    }

    // 2. 自动补充字段
    request.body.author_id = user.id;
    request.body.created_at = new Date().toISOString();

    // 3. 调用底层 SurrealDB (使用 await 等待 Promise 返回)
    await $app.db.query("UPDATE metrics SET total_posts += 1");

    return true; // 允许记录创建
}
```

---

## 🏗️ Monorepo 目录结构

本项目采用 `Cargo Workspace` + `PNPM Workspace` 混合架构，分离底层与前端：

```text
hertabase/
├── crates/                  # 🦀 Rust 后端微内核 (Cargo Workspace)
│   ├── herta_api/           # Salvo 路由、控制器、中间件
│   ├── herta_core/          # App 上下文、配置管理、全局错误处理
│   ├── herta_db/            # SurrealDB 客户端封装、Schema 动态转换
│   ├── herta_jsvm/          # rquickjs 运行时、沙盒、Rust↔JS FFI
│   ├── herta_storage/       # 文件存储抽象适配器 (FS/S3)
│   └── herta_server/        # CLI 入口、前端静态资源嵌入 (rust-embed)
│
├── frontend/                # 🌐 前端项目 (PNPM Workspace)
│   └── admin-ui/            # SvelteKit 管理后台 SPA
│
└── packages/                # 📦 JS/TS 包 (PNPM Workspace)
    ├── @hb/sdk/             # 面向最终用户的 JS SDK
    └── @hb/types/           # Hook 编写者的 .d.ts 类型定义
```

---

## 🗺️ 开发路线图 (Roadmap)

项目分为 7 个主要阶段推进，详细内容请参考 [开发路线图](docs/roadmap.md)。

| 阶段 | 核心目标 |
| :--- | :--- |
| **Phase 1** | 基础架构与动态 ORM (Salvo + SurrealDB) |
| **Phase 2** | 鉴权与权限引擎 (JWT, API Rules) |
| **Phase 3** | JS 扩展运行时 (rquickjs 集成) |
| **Phase 4** | 实时订阅引擎 (SurrealDB LIVE SELECT + SSE) |
| **Phase 5** | 文件存储模块 (LocalFS + S3) |
| **Phase 6** | 管理后台与单体打包 (SvelteKit, rust-embed) |
| **Phase 7** | 生产加固与分布式 (CLI, 结构化日志, TiKV 集群) |

---

## 🤝 参与贡献 (Contributing)

欢迎任何形式的贡献。请随时提交 PR 或 Issue。

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

---

## 📜 许可证 (License)

本项目基于 [MIT License](LICENSE) 开源。
