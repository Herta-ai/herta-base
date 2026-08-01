这是一个极具商业潜力和技术深度的宏大工程！将 **Salvo（灵活且自带 OpenAPI） + SurrealDB（降维打击的图/文档/实时数据库） + rquickjs（极速轻量 JS 运行时）** 结合，你打造的不仅是“下一个 PocketBase”，而是一个**天然支持分布式扩容、原生具备图查询能力、且性能极高**的下一代企业级 BaaS。

为了让这个项目能够在生产环境稳定运行，我为你规划了一份详细的 **从零到商业化发布的开发路线图 (Roadmap)**，并深度剖析了开发过程中**必须避开的“深坑”**。

---

### 🗺️ 核心开发路线图 (Roadmap)

建议将整个工程分为 6 个阶段（Milestones），采用敏捷迭代的方式推进。

#### Phase 1: 基础设施与动态 ORM (MVP 核心) - *预计 3-4 周*

**目标：** 实现系统骨架，打通 Salvo 与 SurrealDB，实现动态集合（Collection）的增删改查。

* **Step 1: 核心上下文抽象**。设计 `App` 结构体，封装 SurrealDB 的连接池（使用 `surrealdb::engine::local::RocksDb` 作为内嵌单文件引擎）。
* **Step 2: 动态路由与 CRUD**。在 Salvo 中注册通用的 RESTful 路由（如 `GET /api/collections/{name}/records`）。
* **Step 3: 数据类型转换层**。这是重点，由于 Rust 是静态类型，你需要编写适配器，将客户端请求的 `serde_json::Value` 安全地转换为 SurrealDB 的 `surrealdb::sql::Value`。
* **Step 4: OpenAPI 自动生成**。利用 Salvo 的 `oapi` 宏，实现：当用户在数据库建表时，后端动态生成对应 Collection 的 Swagger UI。

#### Phase 2: 鉴权体系与安全规则 (Auth & Rules) - *预计 3 周*

**目标：** 构建完善的用户系统和 API 访问控制（类似 PocketBase 的 API Rules）。

* **Step 1: 基础用户模型**。在 SurrealDB 中初始化系统级的 `_users` 和 `_admins` 表。
* **Step 2: JWT 签发与 Salvo 中间件**。实现登录接口，签发 JWT，并编写 Salvo 的 Auth 中间件解析 Token，将用户信息注入请求上下文。
* **Step 3: 动态规则引擎 (API Rules)**。
  * 解析类似 `@request.auth.id = user_id` 的字符串规则。
  * *杀手锏*：不要自己写解析器！直接将规则拼接成 SurrealDB 的 `WHERE` 子句（例如：`SELECT * FROM posts WHERE (user_id = $auth_id)`），让 SurrealDB 强大的原生引擎去评估权限，性能极高。

#### Phase 3: 扩展运行时 (JS Engine Integration) - *预计 4 周*

**目标：** 引入 `rquickjs`，实现类似 `pb_hooks` 的无服务器函数功能。

* **Step 1: rquickjs 环境初始化**。配置 `AsyncRuntime` 和 `AsyncContext`。
* **Step 2: FFI 边界映射 (Rust -> JS)**。
  * 利用 `rquickjs::class` 宏，将 HTTP Request（Header/Body）和 Response 暴露给 JS。
  * 暴露 `$app.db.query(sql, binds)` 方法给 JS，使其能直接操作数据库。
* **Step 3: 生命周期 Hook 挂载**。在 Salvo 的 Handler 中，在执行 DB 操作前后，拦截并执行 `before_create.js` / `after_create.js`。

#### Phase 4: 实时推送与文件存储 (Realtime & Storage) - *预计 3 周*

**目标：** 实现数据变更的实时订阅，以及本地/云端文件上传。

* **Step 1: 极简实时引擎**。
  * 利用 SurrealDB 原生的 `LIVE SELECT` 功能。
  * 当客户端通过 WebSocket/SSE 连接到 Salvo 时，开启一个 Live Query，将 SurrealDB 吐出的 Stream 直接转发给 Salvo 的 SSE 响应流。（*这比 PocketBase 自己维护事件总线要简单且强大得多*）。
* **Step 2: 抽象存储接口**。定义 `Storage` trait，实现 `LocalFS`（存入本地 `data/storage` 目录）和 `S3`（对接 AWS/MinIO，使用 `aws-sdk-s3`）。

#### Phase 5: 管理后台与单体打包 (Admin UI & Build) - *预计 4 周*

**目标：** 开发可视化后台，并打包为单二进制文件。

* **Step 1: 前端 Admin UI 开发**。使用 SvelteKit 或 Vue3 开发极简、现代的后台管理面板。
* **Step 2: 静态资源嵌入**。前端 build 出静态文件后，使用 Rust 的 `rust-embed` 宏，将 HTML/JS/CSS 编译进 Rust 的二进制可执行文件中。
* **Step 3: Salvo 静态文件服务**。配置 Salvo 路由，当访问 `/` 或 `/_/` 时，从内存中提供前端 UI 文件。

#### Phase 6: 商业级加固 (Production Ready) - *持续进行*

**目标：** 性能压测、安全限制、日志追踪。

* **CLI 工具**：集成 `clap`，提供 `start`、`superuser`、`migrate` 等命令。
* **监控与日志**：集成 `tracing`，输出结构化日志。
* **多引擎支持**：通过改配置，允许 SurrealDB 从本地 `RocksDB` 切换到分布式 `TiKV` 集群（商业版核心卖点）。

---

### ⚠️ 核心注意点与“深坑”排雷指南

做这个架构，有几个致命的技术挑战需要提前规划，否则后期重构成本极大：

#### 1. rquickjs 与 Tokio 异步运行时的冲突 (The Async Pitfall)

* **坑在哪里**：Rust 的 Tokio 是多线程异步，而 JS 引擎通常是单线程同步的。如果你在 JS Hook 里调用了异步的 Rust 函数（比如查数据库），处理不当极容易导致 Tokio 的 Worker 线程被阻塞（Block），甚至引发死锁。
* **解决方案**：
  * 必须使用 `rquickjs::AsyncRuntime` 和 `rquickjs::AsyncContext`。
  * 所有暴露给 JS 的耗时操作（如 DB 查询、发 HTTP 请求）必须返回 `Promise`。
  * 在 Rust 侧执行 JS 时，要将其包裹在 `tokio::task::spawn_blocking` 甚至专门的 JS 线程池中，避免抢占 Salvo 的网络 I/O 线程。

#### 2. 恶意 JS 代码的沙盒隔离 (Security & Limits)

* **坑在哪里**：用户在后台写了一段死循环 `while(true){}` 或是分配了巨大内存的 JS 代码，直接把你的 BaaS 宿主进程搞崩溃。
* **解决方案**：QuickJS 是为嵌入式设计的，天然支持限制。
  * **内存限制**：在初始化 JS Runtime 时，严格调用 `rt.set_max_stack_size()` 和 `rt.set_memory_limit()`，比如限制每个请求的 JS 只能用 10MB 内存。
  * **执行时间限制**：利用 QuickJS 的 `set_interrupt_handler`。在 Rust 侧维护一个超时计时器（比如 50ms），一旦超时，Interrupt Handler 返回 `true`，强制打断 JS 执行并抛出超时异常。

#### 3. 动态数据结构的序列化性能损耗 (Serialization Overhead)

* **坑在哪里**：整个数据流是 `HTTP Request (JSON) -> Salvo (Rust Struct) -> JS Engine (JS Object) -> SurrealDB (Surreal Value)`。频繁的反序列化/序列化会榨干 CPU，导致并发性能急剧下降。
* **解决方案**：
  * 不要在各个层之间反复转换 JSON 字符串。
  * 尽可能实现 `serde::Serialize` / `Deserialize` 到 `rquickjs::IntoJs` / `FromJs` 的零拷贝转换。
  * SurrealDB 官方库自带了与 `serde` 的良好集成，好好利用这一层，将 Salvo 提取的 JSON 直接 `bind` 到 SurrealQL 中，只有在触发 JS Hook 时才转换为 JS 对象。

#### 4. 二进制体积膨胀 (Binary Size)

* **坑在哪里**：PocketBase 只有 20MB 左右。但 Rust 引入 RocksDB（C++ 编译）、QuickJS（C 编译）、V8（如果选错引擎）、外加完整 Web 前端，最终打出来的二进制包可能轻松突破 100MB 甚至 200MB。
* **解决方案**：
  * Release 编译时开启 `lto = "fat"` 和 `opt-level = "z"`。
  * 使用 `strip = true` 剔除调试符号。
  * SurrealDB 的 features 配置一定要极简，如果只做单机，只开启 `kv-rocksdb`，去掉不需要的存储引擎（如 fdb, indb）依赖。

#### 5. 商业化产品形态思考：本地单体 vs 分布式集群

* 这是你超越 PocketBase 的核心卖点。
* 在架构设计时，数据库连接层的初始化一定要设计成**工厂模式**。
* 开源免费版：默认使用 `surrealdb::engine::local::RocksDb`，单文件部署。
* 企业/云托管版：允许在 `.env` 中配置 `DB_URI=ws://tikv-cluster...`，代码逻辑不用改一行，直接变为高可用的水平扩展架构。

### 结语

这个技术选型（Salvo + SurrealDB + rquickjs）绝对是目前 Rust 后端生态里的 **“王炸组合”**。它不仅弥补了 PocketBase 无法水平扩展、查询语法简单的短板，还能享受到 Rust 极致的安全性和并发性能。
建议你先花 1-2 周时间，写一个最小可行性 Demo，仅仅测试 **Salvo 接收请求 -> 触发一段 rquickjs 脚本 -> 调用 SurrealDB 插入数据** 这条链路。一旦这条主干打通，整个项目的成功率将高达 90%！祝你好运，非常期待能在开源社区看到这个项目！
