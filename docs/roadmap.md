# HertaBase 开发路线图

HertaBase 项目的开发生命周期分为 7 个核心阶段（Phases），按照敏捷迭代的方式推进，从底层基础设施逐步扩展到上层应用框架及企业级能力。

---

## 🗺️ 核心开发路线图

### Phase 1: 基础架构与动态 ORM — *预计 3-4 周*

**目标：** 实现系统骨架，打通 Salvo 与 SurrealDB，实现动态集合（Collection）的增删改查。

* **Step 1: 核心上下文抽象**。设计应用级别的 Context，封装 SurrealDB 客户端（采用 `surrealdb::engine::local::SurrealKv` 作为持久化内嵌引擎，`Mem` 用于测试）。
* **Step 2: 动态路由与 CRUD**。在 Salvo 中注册通用的 RESTful 路由（如 `GET /api/collections/{name}/records`）。
* **Step 3: 数据类型转换层**。开发适配层，将客户端请求的 `serde_json::Value` 安全地映射为 SurrealDB 的 `surrealdb::sql::Value`。
* **Step 4: OpenAPI 自动生成**。集成 Salvo 的 `oapi` 模块，实现数据库 Collection 的 OpenAPI（Swagger/Redoc）文档自动生成。

### Phase 2: 鉴权与权限引擎 — *预计 3 周*

**目标：** 构建完善的用户系统和细粒度的 API 访问控制引擎。

* **Step 1: 基础用户模型**。在 SurrealDB 中初始化系统级的 `_users` 和 `_admins` 表结构。
* **Step 2: JWT 签发与中间件**。实现登录鉴权接口以签发 JWT，并编写 Salvo 的 Auth 中间件解析 Token，将用户信息注入请求上下文。
* **Step 3: 动态规则引擎 (API Rules)**。
  * 解析访问规则表达式（如 `@request.auth.id = user_id`）。
  * 将规则动态编译为 SurrealDB 的查询条件（如 `WHERE` 子句），复用底层数据库的评估性能。

### Phase 3: JS 扩展运行时 — *预计 4-6 周*

**目标：** 集成 `rquickjs`，实现参考 PocketBase 开发体验的应用级 JavaScript 扩展环境。
完整契约见 [JavaScript 扩展运行时设计](js-runtime.md)。

* **Step 1: 运行时与注册表**。配置 `rquickjs::AsyncRuntime`、隔离的 `AsyncContext`、资源限制、脚本发现、编译缓存和原子热重载；扩展通过注册函数声明行为，不再通过文件名推断 Hook。
* **Step 2: Event Hooks**。支持 Record、Collection、Auth 和应用生命周期事件，采用 `e.next()` 中间件链语义，并明确调用前的事务内逻辑与调用后的提交后逻辑边界。
* **Step 3: FFI 边界映射 (Rust -> JS)**。
  * 提供 Record/Collection 优先的数据库 API，并保留受限且可关闭的 `$app.db.query` 高级接口。
  * 提供五级结构化日志、受限环境变量和统一错误类型。
  * 将 HTTP Request/Response 映射给 JS，支持 `routerAdd()` 注册自定义路由。
* **Step 4: 后台与外部服务**。支持 `cronAdd()` 定时任务、受 SSRF 策略保护的 HTTP 请求和基于 Mailer trait 的邮件发送。
* **Step 5: 跨阶段能力桥接**。定义 `$app.realtime` 和 `$app.files` 契约；分别对接 Phase 4 实时总线和 Phase 5 Storage，在服务未启用时明确返回能力不可用错误。

### Phase 4: 实时订阅引擎 — *预计 2-3 周*

**目标：** 基于系统原生机制实现数据变更的实时推送。

* **Step 1: 原生实时查询**。利用 SurrealDB 的 `LIVE SELECT` 功能捕获底层数据变更流。
* **Step 2: SSE 转发**。建立客户端与 Salvo 的 Server-Sent Events (SSE) 或 WebSocket 连接，将数据变更流低延迟分发到对应订阅的客户端。

### Phase 5: 文件存储模块 — *预计 2-3 周*

**目标：** 提供标准化的文件上传与存储能力。

* **Step 1: 抽象存储接口**。定义通用的 `Storage` trait 以规范文件操作协议。
* **Step 2: 存储适配器**。实现基于本地文件系统的 `LocalFS` 适配器（持久化到本地数据目录）以及兼容 `S3` 的云存储适配器。

### Phase 6: 管理后台与单体打包 — *预计 4 周*

**目标：** 开发可视化数据管理面板并整合应用分发形态。

* **Step 1: Admin UI 开发**。使用 Vue 开发单页应用（SPA）形式的管理后台。
* **Step 2: 静态资源嵌入**。利用 `rust-embed` 宏在编译期将构建后的前端 HTML/JS/CSS 文件静态嵌入到 Rust 二进制程序中。
* **Step 3: 静态文件路由**。配置 Salvo 处理面板路径路由，从内存中直接提供 UI 静态资源。

### Phase 7: 生产加固与分布式 — *持续进行*

**目标：** 提升系统的企业级生产可用性和水平扩展能力。

* **Step 1: CLI 工具集成**。通过 `clap` 提供多级命令结构（如 `serve`, `superuser`, `migrate`）。
* **Step 2: 可观测性与日志**。集成 `tracing` 框架，输出符合标准的结构化日志，以供监控系统分析。
* **Step 3: 分布式集群支持**。实现存储层的无缝切换能力，使 SurrealDB 从本地 `RocksDB` 引擎支持平滑迁移至连接外部的分布式 `TiKV` 集群。

---

## ⚠️ 技术风险与应对策略

项目开发涉及跨语言边界和并发运行时的处理，以下技术风险需在设计阶段严格规避：

### 1. rquickjs 与 Tokio 异步运行时的冲突

* **风险**：Tokio 采用多线程异步模型，而 JS 引擎是单线程同步的。在 JS Hook 中调用阻塞型的 Rust 函数可能导致 Tokio Worker 线程耗尽或引发死锁。
* **策略**：
  * 必须使用 `rquickjs::AsyncRuntime` 与 `rquickjs::AsyncContext`。
  * 将所有耗时 I/O 操作（数据库操作、外部请求）以 `Promise` 形式返回。
  * 隔离运行环境，将 JS 执行调度至 `tokio::task::spawn_blocking` 甚至独立线程池内。

### 2. JS 沙盒安全限制与网络控制

* **风险**：恶意或缺陷 JS 脚本可能造成内存溢出、死循环，甚至发起内部网络探测攻击。
* **策略**：
  * **内存与执行控制**：启动 JS 运行时前配置 `rt.set_max_stack_size()` 和 `rt.set_memory_limit()`；利用中断处理（Interrupt Handler）实施微秒级超时强杀。
  * **网络访问控制**：在沙盒环境中默认禁止全局网络访问，如需对外请求，需在 `$app` 接口下提供封装好的受限请求方法（检查域名白名单等），避免 SSRF 漏洞。

### 3. API Rules 与 SQL 注入防护

* **风险**：动态规则引擎解析用户自定义规则时，如处理不当，易受到注入攻击，危及数据隔离边界。
* **策略**：
  * 拒绝直接在查询中进行字符串拼接。
  * 在转换层将规则参数化，严格使用 SurrealQL 的绑定变量（如 `WHERE user_id = $id`），配合完备的输入验证（Input Validation）模块清理非预期符号。

### 4. 身份验证与 JWT 密钥管理

* **风险**：硬编码 JWT 密钥或长效 Token 未能妥善处理轮换会导致持久性安全风险。
* **策略**：
  * 要求在部署时注入随机的 `JWT_SECRET`，并提供内部管理机制进行定期密钥轮换（Key Rotation）。
  * 在鉴权流程中建立撤销列表（Revocation List）机制或在用户发生密码变更、权限降级时自动使相关 Token 失效。

### 5. 跨层级数据转换开销 (Serialization Overhead)

* **风险**：数据在 `HTTP (JSON) -> Salvo (Rust) -> rquickjs (JS) -> SurrealDB (Value)` 中的反复序列化会导致严重的 CPU 损耗及性能降级。
* **策略**：
  * 避免无关业务的多重 JSON 字符串转换。
  * 优化 `serde::Serialize / Deserialize` 到 `rquickjs` 间的零拷贝流程。
  * 利用 SurrealDB 对 `serde` 的原生支持进行直接绑定，仅在明确需要触发 JS Hook 时才将必须字段转化为 JS 对象。
