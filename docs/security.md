# HertaBase 安全模型 (Security Model)

## 1. 安全设计原则

HertaBase 致力于打造安全的后端基础设施，其架构遵循以下核心原则：

* **纵深防御 (Defense in Depth)**：在网关、路由、业务逻辑层与数据库层设置多重安全校验。
* **最小权限 (Least Privilege)**：所有模块（如 JS Sandbox）与系统操作默认剥夺非必要权限。
* **默认安全 (Secure by Default)**：新创建的 Collection 的 API Rules 默认值为 `null`（即仅管理员可访问），绝不默认开放。

## 2. JS 扩展运行时沙盒安全 (JS Sandbox Security)

作为 **Phase 3** 的核心特性，HertaBase 使用 `rquickjs` 引入了 JavaScript 运行时。为防止恶意 Hook 脚本破坏宿主，采取了极其严格的沙盒隔离：

* **内存限制**：严格限制每个 V8/QuickJS 实例的内存，默认为 **16MB**（可配置）。
* **执行超时控制**：利用 QuickJS 中断处理器 (Interrupt Handler) 限制 CPU 时间，默认最大执行时间为 **100ms**。
* **栈深度限制 (Stack Size)**：防止无限递归导致的 Stack Overflow 崩溃。
* **网络访问 (Network Access)**：默认完全禁用 `fetch` 等网络调用；可通过显式的域名白名单 (Allowlist) 开启。
* **文件系统隔离 (Filesystem)**：不向脚本暴露宿主文件系统 API。可选的 `$app.files` 只能访问配置的扩展文件根目录或 Storage 逻辑键，并强制执行路径规范化、符号链接逃逸检查、单文件上限和总配额。
* **环境变量限制**：无法读取宿主 OS 的任何环境变量。如需使用环境配置，必须在系统后台添加并仅可通过 `$app.env()` 获取。
* **上下文隔离**：每次 Hook 执行均在完全独立且短暂的上下文中运行，杜绝状态污染。
* **出站请求限制**：不暴露原生 `fetch`。`$app.http.send` 对 scheme、域名、端口、DNS 解析结果和每次重定向重新校验，默认拒绝 localhost、私网、链路本地与云元数据地址，并限制超时和响应大小。
* **能力授权**：邮件、HTTP、实时广播、文件写入和原生查询分别授权；未启用时返回明确错误，不允许脚本探测宿主服务或凭据。

## 3. 数据库与数据访问安全

* **API Rules 强制执行**：在请求下发至 SurrealDB 之前，HertaBase 会将 API Rules 编译并与查询合并。
* **防御 SurrealQL 注入**：框架层面强制所有查询必须使用参数化绑定 (`$variables`)，彻底根除 SQL/SurrealQL 注入风险。
* **系统表隔离**：`_admins`、`_collections` 等底层 Schema 配置表禁止通过公有 REST API 访问，仅可通过 Admin 内部接口或服务端 Rust/JS SDK 交互。
* **行级安全 (RLS)**：通过动态计算记录字段与 `$auth` 上下文匹配度，在 SurrealDB 层实现不可绕过的行级数据隔离。

## 4. 传输层安全 (Transport Security)

* **TLS / HTTPS**：生产环境下强制建议置于反向代理（如 Nginx/Caddy）之后以启用 TLS。
* **CORS 策略**：默认采用严格模式，要求显式配置 `Allowed Origins`；开发模式可放宽。
* **安全响应头 (Security Headers)**：Salvo 中间件自动注入：
  * `X-Content-Type-Options: nosniff`
  * `X-Frame-Options: DENY`
  * 严格的内容安全策略 (CSP) 与 HSTS (HTTP Strict Transport Security)。

## 5. 鉴权层安全 (Authentication Security)

* **密码散列**：使用抗 GPU 破解的 **Argon2id** 算法进行哈希。
* **JWT 安全**：当前使用 HS256 和至少 32 字节的密钥；Access Token 默认 15 分钟，Refresh Token 默认 7 天。
* **凭据撤销**：刷新令牌 (Refresh Token) 采用轮转机制，一旦发现重复使用即视为失窃，自动级联撤销所有相关 Token。
* **异常拦截**：所有敏感鉴权端点包含速率限制与多次失败后的账户临时锁定策略。

## 6. 输入校验 (Input Validation)

* **负载限制**：严格限制 HTTP Request Body Size，防范 OOM 与带宽耗尽攻击。
* **文件上传限制**：Phase 5 在写最终对象前校验 part 名称、数量、单文件大小、声明 MIME、扩展名、记录 schema 和 Collection Rule；JSON 请求不能伪造非空文件引用。
* **路径隔离**：存储 key 拒绝绝对路径、空段、`.`、`..`、反斜杠和 NUL。LocalFS 被限制在 `HB_DATA_DIR/storage`，S3 bucket 保持私有。
* **文件令牌**：短期令牌只绑定一个集合、记录和字段，并绑定账户 `token_key`。Authorization 请求头优先于查询令牌，凭据轮换会立即撤销文件令牌。
* **下载加固**：下载重新校验记录字段成员关系，默认私有缓存并发送 `nosniff`；HTML、SVG、脚本、CSS 和 Wasm 等主动内容强制下载。
* **请求净化**：路由层面对所有 URL Path Params 与 Query Params 进行强类型转换与非法字符清洗。

## 7. 日志与审计 (Logging and Auditing)

* 依赖 Rust 的 `tracing` crate 输出结构化日志。
* 服务端日志默认将 `info` 及以上级别持久化到 `_logs`；HTTP 请求日志默认开启但可单独关闭。
* HTTP 日志仅保存请求元数据（方法、路径、状态、身份、referer、连接 IP、user-agent），不保存请求头、请求体或响应体，避免凭据和业务数据进入审计表。
* 关键安全事件（如管理员登录失败、高频 API Rules 鉴权拒绝、沙盒内存溢出/超时违规）将被标记为警告/错误级别并独立持久化，供事后审计分析。

## 8. 路线图与限制

当前版本尚未内置 WAF 功能；分布式环境下的全局速率限制暂依赖于外部网关组件。

## 9. 漏洞披露政策

*(Placeholder: 请将安全漏洞提交至 <wyatex@qq.com>，在问题修复前请勿公开披露。)*
