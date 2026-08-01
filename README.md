<div align="center">

# 🚀 HertaBase

**The Next-Generation, Single-Binary Backend-as-a-Service.**

[![Rust](https://img.shields.io/badge/Rust-1.75+-f06292.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org)
[![Salvo](https://img.shields.io/badge/Salvo-Web_Framework-blue.svg?style=for-the-badge)](https://salvo.rs/)
[![SurrealDB](https://img.shields.io/badge/SurrealDB-Database-ff00a0.svg?style=for-the-badge)](https://surrealdb.com/)
[![QuickJS](https://img.shields.io/badge/QuickJS-JS_Runtime-f7df1e.svg?style=for-the-badge&logo=javascript)](https://bellard.org/quickjs/)
[![License: MIT](https://img.shields.io/badge/License-MIT-success.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=for-the-badge)](http://makeapullrequest.com)

> 💡 **HertaBase** 是一个用 Rust 编写的开源 BaaS（后端即服务）。它将 Web 服务器、图/文档数据库、实时订阅引擎、JS 扩展运行时和精美的管理后台，全部打包进**一个极致轻量的独立二进制文件**中。

[快速开始](#-快速开始) • [核心特性](#-核心特性) • [架构设计](#-架构设计) • [编写 Hooks](#-编写扩展-hooks) • [路线图](#-开发路线图-roadmap)

</div>

---

## ✨ 核心特性

HertaBase 旨在提供比肩 PocketBase 的极致开发体验（DX），同时利用 Rust 生态带来**降维打击般的性能与可扩展性**。

- 📦 **All-in-One 单文件部署**：无需配置复杂的环境，一个二进制文件搞定数据库、API 和管理后台。
- 🗄️ **强悍的 SurrealDB 引擎**：原生支持图关系（Graph）、文档（Document）、Schema-less/full 混合模式。支持从本地单文件（RocksDB）无缝迁移至分布式集群（TiKV）。
- ⚡ **极致极速的 JS 运行时**：内置 `rquickjs`，允许你用 JavaScript/TypeScript 编写生命周期 Hook，微秒级冷启动，内存占用极低。
- 🔄 **原生实时订阅 (Realtime)**：基于 SurrealDB 的 `LIVE QUERY` 与 Salvo SSE，数据变更毫秒级推送到客户端。
- 📖 **OpenAPI 自动生成**：当你在后台创建 Collection（表）时，系统自动生成完美的 Swagger/Redoc API 文档。
- 🛡️ **内建 Auth 与权限引擎**：开箱即用的 JWT 鉴权，支持细粒度的 API 访问规则（API Rules）。
- 🎨 **精美的现代化 Admin UI**：基于 SvelteKit 构建，并在编译时通过 `rust-embed` 静态嵌入到二进制中。

---

## 🛠️ 技术栈 (Tech Stack)

| 模块 | 核心技术 | 描述 |
| :--- | :--- | :--- |
| **语言** | 🦀 **Rust** | 内存安全、零成本抽象、无与伦比的并发性能 |
| **HTTP 层** | 🌐 **Salvo** | 极其易用且强大的 Rust Web 框架，原生支持 OpenAPI |
| **数据库** | 🗄️ **SurrealDB** | 下一代多模数据库，BaaS 架构的终极杀器 |
| **JS 引擎** | 🚀 **rquickjs** | 极速轻量的 JavaScript 引擎，完美融入 Rust 异步生态 |
| **管理后台**| 🎨 **SvelteKit + Tailwind** | 极致流畅的 SPA 体验，通过 PNPM Workspace 联合构建 |

---

## 🚀 快速开始

### 1. 下载或编译

你可以直接从 [Releases](#) 下载预编译的二进制文件，或者通过源码编译：

```bash
# 克隆项目 (包含 Cargo & PNPM monorepo)
git clone https://github.com/Herta-ai/hertabase.git
cd hertabase

# 安装前端依赖并构建 UI
pnpm install
pnpm build:ui

# 编译 Rust 核心服务器 (Release 模式)
cargo build --release
```

### 2. 启动服务

```bash
./target/release/hertabase serve
```

🎉 成功！现在你可以访问：

- 管理后台：`http://localhost:8080/_/`
- API 接口：`http://localhost:8080/api/`
- API 文档：`http://localhost:8080/swagger-ui/`

---

## 🧩 编写扩展 Hooks (Serverless JS)

不想编译 Rust？没问题！在运行目录下创建 `pb_hooks/before_create_post.js`，你可以直接拦截并修改 HTTP 请求与数据库操作：

```javascript
// 🔥 享受完美的 TypeScript 代码提示，零配置运行
export default function (context) {
    const { request, user } = context;

    // 1. 验证自定义业务逻辑
    if (request.body.title.includes("spam")) {
        throw new Error("Title contains spam words!");
    }

    // 2. 自动补充字段
    request.body.author_id = user.id;
    request.body.created_at = new Date().toISOString();

    // 3. 调用底层 SurrealDB (返回 Promise)
    $app.db.query("UPDATE metrics SET total_posts += 1");

    return true; // 允许记录创建
}
```

---

## 🏗️ Monorepo 目录结构

本项目采用 `Cargo Workspace` + `PNPM Workspace` 混合架构，完美分离底层与前端：

```text
hertabase/
├── crates/                  # 🦀 Rust 后端微内核 (Cargo Workspace)
│   ├── api/                 # Salvo 路由与中间件
│   ├── core/                # App 上下文与配置
│   ├── db/                  # SurrealDB FFI 封装
│   ├── jsvm/                # rquickjs 运行时与沙盒
│   └── server/              # CLI 入口与前端静态资源嵌入
│
├── frontend/                # 🌐 前端项目 (PNPM Workspace)
│   └── admin-ui/            # 基于 SvelteKit 的管理面板
│
└── packages/types/          # 📦 提供给 JS Hooks 编写者的 .d.ts 类型定义
```

---

## 🗺️ 开发路线图 (Roadmap)

- [ ] **Phase 1**: 基础框架搭建，Salvo 与 SurrealDB 互通。
- [ ] **Phase 2**: 动态 Collection CRUD API 与 OpenAPI 自动生成。
- [ ] **Phase 3**: rquickjs 引擎集成与 HTTP/DB 上下文跨语言映射 (FFI)。
- [ ] **Phase 4**: 基于 SurrealDB Live Query 的实时 SSE 订阅中心。
- [ ] **Phase 5**: SvelteKit 现代化管理后台开发。
- [ ] **Phase 6**: 本地文件与 S3 兼容云存储 (Storage) 模块集成。
- [ ] **Phase 7**: 分布式集群模式支持 (连接外部 TiKV)。

---

## 🤝 参与贡献 (Contributing)

我们非常欢迎任何形式的贡献！无论是一个错别字的修复，还是一个核心模块的重构，请随时提交 PR 或 Issue。

1. Fork 本仓库
2. 创建您的特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交您的更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 打开一个 Pull Request

---

## 📜 许可证 (License)

本项目基于 [MIT License](LICENSE) 开源。

<div align="center">
  <i>构建于 <b>Rust</b> 🦀 之上，为下一代开发者而生。</i><br>
  如果这个项目对你有帮助，请给个 ⭐️ 吧！
</div>
