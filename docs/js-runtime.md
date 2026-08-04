# JavaScript 扩展运行时设计

## 1. 目标与范围

Phase 3 不再只提供按文件名匹配的记录 Hook，而是提供一个参考 PocketBase
扩展体验的应用级 JavaScript 运行时。扩展脚本可以显式注册事件 Hook、自定义 HTTP
路由和定时任务，并通过受控的 `$app` API 使用数据库、邮件、HTTP、实时消息、文件和日志服务。

本阶段负责运行时、注册机制、FFI 契约和安全边界。实时订阅传输层与文件存储适配器仍分别由
Phase 4 和 Phase 5 实现；在对应服务未启用时，相关 JS API 必须返回明确的
`HB_CAPABILITY_UNAVAILABLE` 错误，不能静默成功。

扩展文件只能由服务器运维者部署，不通过公共 API 接收多租户上传。脚本属于受约束的服务端
业务代码：可以按授权使用系统身份操作业务数据，但不能直接获得宿主进程、凭据或任意网络与
文件系统权限。资源和能力限制既防御恶意脚本，也防止可信脚本的缺陷拖垮服务。

## 2. 设计原则

- **显式注册**：脚本通过注册函数声明行为，不依赖文件名推断业务语义。
- **一个应用上下文**：Hook、路由和任务共享同一套 `$app` 服务接口和错误模型。
- **默认拒绝能力**：网络、邮件、实时广播和文件写入按配置授予，不暴露 Node.js API。
- **异步优先**：所有可能执行 I/O 的接口返回 Promise，禁止阻塞 Tokio worker。
- **记录优先 API**：常用操作使用 Record/Collection API；原生 SurrealQL 仅作为高级接口。
- **可组合 Hook**：同一事件允许多个处理器，按确定顺序执行，并支持中断处理链。
- **可观测**：每次调用携带脚本、事件、请求或任务 ID，错误与耗时进入结构化日志。

### 2.1 模块边界

| 模块 | 职责 |
| --- | --- |
| `herta_core` | `JsvmConfig`、能力开关、公共错误码和应用服务 trait |
| `herta_jsvm` | 脚本发现/编译、QuickJS 池、注册表、事件调度、FFI 与资源限制 |
| `herta_db` | Record/Collection 操作、事务句柄、Schema/API Rules 最终校验 |
| `herta_api` | 请求事件适配、自定义路由快照和 JS Response 到 Salvo Response 的转换 |
| `herta_server` | 启动/停止顺序、热重载、cron runner 与具体服务装配 |
| Phase 4/5 服务 | 分别实现 `RealtimeBus` 与 `Storage` trait |

`herta_jsvm` 只依赖宿主 trait，不直接依赖 SMTP、S3 或具体实时协议。这样未启用后续阶段时
仍可构建和测试 Phase 3 核心，后续适配器也不会扩大 QuickJS FFI 表面。

## 3. 扩展加载

扩展目录默认为 `hb_hooks/`。运行时递归加载其中的 `*.js` 文件，忽略以 `.` 或 `_`
开头的文件和符号链接。文件按规范化相对路径的字典序加载，因此初始化顺序稳定。

脚本在服务启动时执行一次，用于完成注册：

```javascript
onRecordCreate(async (e) => {
  if (!e.record.get("title")) {
    throw new BadRequestError("title is required")
  }
  e.record.set("slug", slugify(e.record.get("title")))
  await e.next()
}, "posts")

routerAdd("GET", "/api/health/custom", (e) => {
  return e.json(200, { ok: true })
})

cronAdd("cleanup", "0 30 2 * * *", async () => {
  await $app.db.query("DELETE expired_sessions WHERE expires_at < time::now()")
})
```

脚本顶层只允许注册和纯计算。顶层 Promise 必须在启动超时内完成。任一脚本语法错误或注册
冲突会使生产模式启动失败；开发模式记录错误并跳过该脚本。开发模式可监听文件变化并原子地
重建完整注册表，重载失败时继续使用上一版本。

不提供 CommonJS、Node.js 内置模块或任意 npm 包加载。第一版支持普通脚本；ES Module 和
预构建 TypeScript 可在保持本契约不变的前提下增加。

## 4. Event Hooks

### 4.1 注册函数

记录和集合事件使用以下全局注册函数：

| 分类 | 注册函数 |
| --- | --- |
| Record 持久化 | `onRecordCreate`, `onRecordUpdate`, `onRecordDelete` |
| Record API 请求 | `onRecordListRequest`, `onRecordViewRequest`, `onRecordCreateRequest`, `onRecordUpdateRequest`, `onRecordDeleteRequest` |
| Collection | `onCollectionCreate`, `onCollectionUpdate`, `onCollectionDelete` |
| Auth | `onAuthLogin`, `onAuthRegister`, `onTokenRefresh` |
| 应用 | `onBootstrap`, `onServe`, `onShutdown` |

Record Hook 的最后一组参数是可选 Collection 名称过滤器：

```javascript
onRecordCreate(sendWelcome, "users", "members")
```

不传过滤器表示监听所有 Collection。过滤器必须是精确名称，第一版不接受正则表达式。

### 4.2 中间件链语义

每个处理器接收事件对象 `e`。调用 `await e.next()` 执行下一个处理器或核心操作；不调用
`e.next()` 即中断处理链。处理器可以在 `e.next()` 前后执行逻辑：

```javascript
onRecordUpdateRequest(async (e) => {
  $app.logger.debug("updating record", { id: e.record.id })
  await e.next()
  await $app.realtime.publish(`audit/${e.collection.name}`, {
    action: "update",
    id: e.record.id,
  })
}, "posts")
```

为避免旧文档中 `return false`、抛异常和隐式返回的歧义，新的统一规则是：

- `e.next()`：继续处理链。
- 正常返回且未调用 `e.next()`：有意终止处理链。
- 抛出 `BadRequestError`、`ForbiddenError`、`NotFoundError` 等公开错误：按对应状态返回。
- 抛出普通 `Error`：记录完整堆栈，对外返回 `HB_HOOK_ERROR`。
- 核心操作成功后发生的异常不能回滚已提交写入。宿主记录为 post-commit failure，不把成功
  响应改成失败；需要可靠投递的外部副作用应使用数据库 outbox。

### 4.3 事件对象

所有事件包含：

```typescript
interface BaseEvent {
  name: string
  requestId: string | null
  context: Record<string, unknown>
  next(): Promise<void>
}
```

请求事件额外包含 `request`、`auth` 和 `response`；Record 事件包含 `record`、
`originalRecord`、`collection`；Collection 事件包含 `collection` 和
`originalCollection`。`original*` 是只读快照。

Record Hook 在 `e.next()` 前的修改会在 Schema 校验和 API Rules 最终检查前写入候选
Record。数据库事务内 Hook 必须使用同一个事务句柄，确保核心写入失败时 Hook 写入一起回滚。
外部 HTTP、邮件、实时推送和不可事务化文件操作禁止在事务内直接执行，应放在
`await e.next()` 之后或 outbox 任务中。

## 5. 自定义路由

```javascript
routerAdd("POST", "/api/reports/{id}", async (e) => {
  const input = await e.request.json()
  const report = await $app.findRecordById("reports", e.request.pathValue("id"))
  return e.json(200, { report, options: input })
}, $apis.requireAuth())
```

`routerAdd(method, path, handler, ...middleware)` 在启动期注册路由。规则如下：

- 自定义路由必须以 `/api/` 开头；只有显式配置后才允许注册到其他前缀。
- 禁止覆盖内置 `/_/`、`/api/collections`、`/api/auth`、文档和实时连接路由。
- 同一 method/path 重复注册视为启动错误。
- 路径参数使用 `{name}`；不接受任意正则路径。
- 路由处理器必须返回 `e.json`、`e.text`、`e.html`、`e.file` 或 `e.noContent` 的结果。
- 请求体按 `HB_MAX_REQUEST_BODY_SIZE` 限制，响应体也受 JS 路由响应上限限制。

`$apis` 第一版提供 `requireAuth()`、`requireAdmin()`、`bodyLimit(bytes)` 和
`rateLimit(options)`。鉴权中间件产生的身份放在 `e.auth`，不能由脚本伪造。

## 6. 数据库 API

### 6.1 Record

Record 是带 Collection 元数据的可变包装器，而不是无约束 JSON：

```typescript
interface RecordModel {
  readonly id: string
  readonly collectionName: string
  get(field: string): unknown
  set(field: string, value: unknown): void
  unset(field: string): void
  original(field: string): unknown
  isNew(): boolean
  toJSON(): Record<string, unknown>
}
```

系统字段和 auth 敏感字段继续由 Rust 数据层保护，`set()` 不能绕过 Schema、关系、密码或
API Rules 校验。

### 6.2 常用操作

```javascript
const record = await $app.findRecordById("posts", id)
const rows = await $app.findRecordsByFilter(
  "posts",
  "status = $status",
  "-created_at",
  50,
  0,
  { status: "published" },
)

const draft = $app.newRecord("posts", { title: "Draft" })
await $app.save(draft)
await $app.delete(draft)
```

`$app.findFirstRecordByFilter`、`findRecordById`、`findRecordsByFilter`、`newRecord`、
`save` 和 `delete` 是主要业务接口。所有过滤变量必须绑定，不能字符串拼接用户输入。

服务端扩展默认以系统身份运行，因此可以绕过公共 API Rules，但仍不能绕过 Schema、系统表
保护和事务约束。每个注册函数可通过选项声明 `authMode: "request"`，让数据库调用继承请求
身份。两种模式不能在单次调用中隐式切换。

### 6.3 Collection 和原生查询

```javascript
const collection = await $app.collections.findByName("posts")
await $app.collections.save(collection)

const result = await $app.db.query(
  "SELECT * FROM posts WHERE author_id = $author LIMIT 20",
  { author: e.auth.id },
)
```

`$app.collections` 提供 `findByName`、`list`、`create`、`save`、`delete`。变更 Collection
必须走 `SchemaManager`，以便刷新 OpenAPI、Hook 索引和验证缓存。

`$app.db.query` 是受审计的高级能力：必须使用变量绑定；禁止 `DEFINE USER`、`DEFINE ACCESS`、
`USE NS/DB` 和系统表破坏操作；可通过配置完全关闭。返回值是按语句分组的 JSON 数组。

## 7. 定时任务

```javascript
cronAdd("daily-report", "0 0 8 * * *", async () => {
  const records = await $app.findRecordsByFilter("reports", "sent = false")
  // ...
})

cronRemove("obsolete-job")
```

- 表达式使用含秒的 6 段 cron，时区默认 UTC，可在任务选项中指定 IANA 时区。
- 任务名称全局唯一；重载时用新注册表原子替换旧调度。
- 同一任务默认不并发执行；上一次未完成时跳过并记录 `warn`。
- 每次执行有独立超时和关联 ID。失败按指数退避重试，最大次数可配置。
- 单机嵌入模式保证进程内 at-most-one 并发，不承诺宕机补偿。未来集群模式需要数据库租约。

## 8. 邮件发送

```javascript
await $app.mailer.send({
  from: { address: "noreply@example.com", name: "HertaBase" },
  to: [{ address: user.get("email") }],
  subject: "Welcome",
  text: "Welcome to HertaBase",
  html: "<strong>Welcome to HertaBase</strong>",
  headers: { "X-Event-Id": e.requestId },
})
```

邮件通过 Rust `Mailer` trait 发送，JS 不接触 SMTP 凭据。收件人数、主题长度、正文大小、
附件总量和允许的发件地址由配置限制。事务内业务应写入 outbox，并在 `e.next()` 成功后或由
任务发送，避免数据库回滚但邮件已经发出。

## 9. HTTP 请求

```javascript
const response = await $app.http.send({
  method: "POST",
  url: "https://api.example.com/events",
  headers: { "content-type": "application/json" },
  body: JSON.stringify({ id: e.record.id }),
  timeoutMs: 3000,
})

if (!response.ok) throw new Error(`upstream returned ${response.status}`)
const payload = response.json()
```

不暴露原生 `fetch`。`$app.http.send` 强制执行 scheme/host/port 白名单、DNS 解析后地址校验、
重定向次数、连接超时、总超时以及请求/响应大小限制。默认拒绝 localhost、私网、链路本地、
云元数据地址和非 HTTP(S) 协议；每次重定向都重新校验目标，防止 SSRF 与 DNS rebinding。

## 10. 实时事件

```javascript
await $app.realtime.publish("reports/ready", {
  reportId: report.id,
}, { userIds: [e.auth.id] })
```

`publish(topic, data, audience?)` 向 Phase 4 的实时总线发布应用事件。topic 必须符合
`[A-Za-z0-9][A-Za-z0-9._/-]{0,127}`，消息大小和发布速率受限。audience 可限定用户、角色或
连接；省略 audience 的全局广播默认禁用。记录 CRUD 的标准实时事件由核心自动产生，JS
只负责业务事件，避免重复推送。

## 11. 文件操作

```javascript
await $app.files.write("exports/report.json", JSON.stringify(report), {
  contentType: "application/json",
})
const content = await $app.files.readText("exports/report.json")
const entries = await $app.files.list("exports")
await $app.files.remove("exports/report.json")
```

JS 只能访问配置的扩展文件根目录或 Phase 5 Storage 逻辑键，不能访问任意宿主路径。
`readBytes`/`write` 使用字节或字符串，另提供 `readText`、`exists`、`stat`、`list`、`copy`、
`move` 和 `remove`。所有路径在规范化后必须仍位于根目录内；拒绝绝对路径、`..`、设备路径、
符号链接逃逸和超限文件。写入采用临时文件加原子替换，并受总配额限制。

## 12. 日志

```javascript
$app.logger.trace("raw payload", { payload })
$app.logger.debug("matched rule", { rule: "published" })
$app.logger.info("report generated", { reportId })
$app.logger.warn("upstream is slow", { elapsedMs })
$app.logger.error("delivery failed", { error: String(error) })
```

支持 `trace`、`debug`、`info`、`warn`、`error` 五个级别。第二个参数必须是可序列化对象。
宿主自动附加 `source=jsvm`、脚本相对路径、事件名、请求/任务 ID 和执行耗时。日志 API 对
键数量、字符串长度和每次执行条数限流，并递归脱敏 token、authorization、password、secret
等字段。

## 13. 运行时与并发模型

- 使用 `rquickjs::AsyncRuntime` 和 `AsyncContext`，I/O FFI 映射为 Promise。
- 每个执行上下文相互隔离；只共享不可变的已编译脚本缓存和 Rust 服务句柄。
- 运行时池有固定上限和有界等待队列，避免请求高峰无限创建 QuickJS 实例。
- 内存、栈、CPU 时间、异步总时长、Promise 数和返回数据量分别限额。
- 超时会取消 Rust future 并丢弃该 JS context；context 不返回池中复用。
- Hook 注册表使用不可变快照。热重载期间已开始的请求继续使用旧快照，新请求使用新快照。
- 禁止脚本保存请求、Record 或事务对象供下次执行使用。

## 14. 错误模型

宿主错误在 JS 中表现为带 `code`、`message`、`details` 的 Error 子类。新增公共错误码：

| 错误码 | 含义 |
| --- | --- |
| `HB_SCRIPT_LOAD_ERROR` | 扩展加载或注册失败 |
| `HB_HOOK_ERROR` | Hook 未处理异常 |
| `HB_HOOK_TIMEOUT` | Hook 超时 |
| `HB_HOOK_OOM` | Hook 超过内存限制 |
| `HB_ROUTE_CONFLICT` | 自定义路由冲突 |
| `HB_CAPABILITY_DENIED` | 当前脚本未获能力授权 |
| `HB_CAPABILITY_UNAVAILABLE` | 依赖服务尚未启用 |
| `HB_OUTBOUND_DENIED` | HTTP 目标未通过安全策略 |
| `HB_FILE_ACCESS_DENIED` | 文件路径或操作越权 |

生产环境对外隐藏普通异常堆栈；完整异常、脚本位置和 cause chain 只进入受控日志。

## 15. 配置模型

```toml
[jsvm]
enabled = true
memory_limit_mb = 16
stack_limit_kb = 512
execution_timeout_ms = 100
async_timeout_ms = 5000
pool_size = 4
queue_capacity = 128
raw_query_enabled = false
env_allowlist = ["PUBLIC_APP_URL"]

[jsvm.http]
enabled = false
allowlist = ["api.example.com:443"]
max_redirects = 3
max_request_bytes = 1048576
max_response_bytes = 4194304
timeout_ms = 5000

[jsvm.files]
enabled = false
root = "./hb_data/js-files"
quota_bytes = 104857600
max_file_bytes = 10485760

[jsvm.mail]
enabled = false
max_recipients = 20
max_message_bytes = 1048576

[jsvm.cron]
enabled = true
timezone = "UTC"
max_runtime_ms = 30000

[mail]
driver = "smtp"
from_address = "noreply@example.com"
from_name = "HertaBase"

[mail.smtp]
host = "smtp.example.com"
port = 587
username = ""
password = ""
tls = "starttls"
```

环境变量使用对应的 `HB_JS_*` 前缀。密钥只进入 Rust 服务配置，不可通过 `$app.env()` 读取。

## 16. 实施顺序

1. 建立 `herta_jsvm` crate、配置、错误类型、脚本发现、编译缓存与原子注册表。
2. 完成日志、纯事件 Hook 和 Record/Collection FFI，并接入 CRUD 与 SchemaManager。
3. 完成自定义路由和请求/响应桥接，再接入 auth 中间件。
4. 完成调度器、HTTP client 和 Mailer trait；外部副作用统一放在 `e.next()` 后或采用 outbox 模式。
5. 对接 Phase 4 RealtimeBus 与 Phase 5 Storage trait。
6. 增加 `@hb/types`、示例、热重载、配额压力测试和沙盒安全测试。

每一步必须包含正常路径、超时、OOM、权限拒绝、重载并发和宿主服务失败测试。只有服务端实际
实现并通过测试的 API 才能进入 `@hb/types`，避免类型声明承诺不存在的能力。
