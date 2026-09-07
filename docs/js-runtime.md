# JavaScript 扩展运行时设计

> 状态：设计草案，尚未实现。2026-09-07 已对照当前代码复核；下文的 JS API 和配置均为
> 目标契约，不能据此认为当前二进制已支持。实现前需要完成第 16.1 节的契约与可行性验证。

## 1. 目标与范围

Phase 3 不再只提供按文件名匹配的记录 Hook，而是提供一个参考 PocketBase
扩展体验的应用级 JavaScript 运行时。扩展脚本可以显式注册事件 Hook、自定义 HTTP
路由和定时任务，并通过受控的 `$app` API 使用数据库、邮件、HTTP、实时消息、文件和日志服务。

本阶段负责运行时、注册机制、FFI 契约和安全边界。Phase 4 的集合 SSE 订阅和 Phase 5 的
本地/S3 存储已经实现，但不等于已经提供 JS 所需的应用消息总线或文件服务。缺少对应宿主
适配器时，相关 JS API 必须返回明确的 `HB_CAPABILITY_UNAVAILABLE` 错误，不能静默成功；
能力未获配置授权时返回 `HB_CAPABILITY_DENIED`。

扩展文件只能由服务器运维者部署，不通过公共 API 接收多租户上传。脚本属于受约束的服务端
业务代码：可以按授权使用系统身份操作业务数据，但不能直接获得宿主进程、凭据或任意网络与
文件系统权限。资源和能力限制既防御恶意脚本，也防止可信脚本的缺陷拖垮服务。

### 1.1 当前实现基线

| 能力 | 当前实现与本阶段需要补齐的部分 |
| --- | --- |
| 运行时与配置 | Workspace 尚无 `herta_jsvm` 或 `rquickjs` 依赖；`HbConfig` 尚无 `jsvm`、`mail` 字段。`hooks_dir` 目前仅保存路径，不加载脚本。 |
| Record 与事务 | `RecordManager` 已提供授权 CRUD、软删除和敏感字段清理；SDK 3.2.3 已提供事务句柄，但 `DbClient` 尚未接入，具体方案见第 16.1 节。 |
| 查询过滤 | `compile_filter` 支持受限表达式并绑定字面值，尚不接受示例中的 `$name` 外部参数；列表接口目前使用 page/perPage，不是 limit/offset。 |
| Collection | `SchemaManager` 管理 Schema；OpenAPI 刷新、集合文件清理目前由 HTTP handler 完成，直接调用 manager 不会完成这些后置动作。 |
| 实时 | `RealtimeManager` 通过 LIVE SELECT 与快照校验提供集合变更订阅，没有任意 topic 的 publish、用户/角色/连接寻址接口。 |
| 文件 | `herta_storage::Storage` 已有 put_file/head/get/delete/delete_prefix 和本地/S3 适配器，尚无 list/copy/move 或扩展目录配额。 |
| Auth | `AuthService` 的注册、登录、刷新有独立数据库调用；仅在 RecordManager 加 Hook 不会覆盖这些流程。 |

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
| `herta_core` | 待新增的 `JsvmConfig`、能力开关、公共错误码和不依赖具体实现的宿主契约 |
| `herta_jsvm` | 脚本发现/编译、QuickJS 池、注册表、事件调度、FFI 与资源限制 |
| `herta_db` | Record/Collection 操作、事务句柄、Schema/API Rules 最终校验 |
| `herta_api` | 请求事件适配、自定义路由快照和 JS Response 到 Salvo Response 的转换 |
| `herta_server` | 启动/停止顺序、热重载、cron runner 与具体服务装配 |
| 现有实时/存储服务 | 复用 `herta_storage::Storage`；应用消息总线及 JS 文件适配器需要新增，不能视为已存在的 trait |

`herta_jsvm` 只依赖宿主 trait，不直接依赖 SMTP、S3 或具体实时协议。可用 mock 服务构建和
测试核心。宿主 trait 若使用 Record/Collection DTO，须先解决共享类型的位置，避免
`herta_core -> herta_db -> herta_core` 或 `herta_db <-> herta_jsvm` 的循环依赖。

## 3. 扩展加载

扩展目录默认为 `hb_hooks/`。运行时递归加载其中的 `*.js` 文件，忽略以 `.` 或 `_`
开头的文件及目录，不跟随符号链接或 Windows junction。文件按以 `/` 分隔的规范化相对
路径字典序加载，因此初始化顺序稳定。

脚本在服务启动时执行，用于验证并建立注册描述；执行上下文中闭包的构建方式见第 13 节。
注册示例：

```javascript
onRecordCreate(async (e) => {
  const title = e.record.get("title")
  if (typeof title !== "string" || !title) {
    throw new BadRequestError("title is required")
  }
  e.record.set("slug", title.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-"))
  await e.next()
}, "posts")

routerAdd("GET", "/api/health/custom", (e) => {
  return e.json(200, { ok: true })
})

cronAdd("cleanup", "0 30 2 * * *", async () => {
  await $app.db.query("DELETE expired_sessions WHERE expires_at < time::now()")
})
```

脚本顶层只允许注册和纯计算，禁止宿主 I/O；顶层 Promise 必须在启动超时内完成。任一脚本
语法错误或注册冲突都使整个候选注册表失效，不能保留失败脚本已注册的部分处理器。
生产模式首次加载失败则启动失败；开发模式有上一成功版本时保留旧版本，没有时也启动失败，
避免跳过鉴权或校验 Hook 后以不完整扩展运行。开发模式可监听文件变化并原子地重建完整注册表。

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
- 正常返回且未调用 `e.next()`：有意终止处理链，不得伪造核心写入成功。请求事件必须提供
  有效响应；持久化事件无响应通道时按操作被拒绝处理，具体返回契约须在接入 CRUD 前确定。
- 同一处理器最多调用一次 `e.next()`；重复调用或处理器结束后的调用属于 Hook 错误。
  宿主必须跟踪尚未完成的下游调用，不能将其作为脱离事件生命周期的后台任务；处理器提前
  返回时如何收束下游并报告已提交/未提交结果，须纳入第 16.1 节验证。
- 抛出 `BadRequestError`、`ForbiddenError`、`NotFoundError` 等公开错误：按对应状态返回。
- 抛出普通 `Error`：记录完整堆栈，对外返回 `HB_HOOK_ERROR`。
- 宿主确认该核心操作提交成功后发生的异常不能回滚已提交写入。宿主记录为 post-commit
  failure，保留已确认的核心成功结果；不能只凭调用过 `e.next()` 就吞掉异常。自定义路由中
  某一次 `$app.save()` 成功不代表整个路由已成功，后续异常仍按路由错误返回。

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

请求事件额外包含 `request`、`auth` 和 `response`；单记录事件包含 `record`、
`originalRecord`、`collection`，列表事件不能假定存在单个 `record`；Collection 事件包含
`collection` 和 `originalCollection`。创建时的 `original*` 为 null，其余 `original*`
是只读快照；请求身份只暴露经脱敏的数据，不能包含 `AuthIdentity` 内部的 token_key。

Record Hook 在 `e.next()` 前的修改会在 Schema 校验和 API Rules 最终检查前写入候选
Record。请求体快照与候选 Record 必须分开保存：已有 Rules 中的 `$request.body` 保持
HTTP 适配层现有解析/规范化后的输入语义，不随 Hook 修改候选值而改变；`$record` 的新旧值
语义须与现有 create/update Rules 对齐，不能因 Hook 接入而改变授权对象。公开 CRUD 的
核心写入始终使用原请求身份，不能被 Hook 的系统身份提升权限。

**共同事务尚未接入项目；SDK 能力已确认，集成行为待验证。** 克隆 `DbClient` 不会创建共同事务；现有
SchemaManager 的单次 `BEGIN ... COMMIT` 查询也不能直接作为跨 JS await 的事务句柄。
只有在第 16.1 节的事务验证通过后，才能承诺“核心写入失败时 Hook 写入一起回滚”。

`e.next()` 仅表示下游处理完成，不天然代表最外层事务已提交。特别是 Hook 内的嵌套
`$app.save()`，其 `e.next()` 返回时外层仍可能失败。HTTP、邮件、实时推送及不可事务化文件
操作只能在宿主确认脱离事务后执行，不能仅根据源码位于 `await e.next()` 后就放行。
可靠副作用需要在同一事务中写入 outbox，再由独立任务投递；当前尚无通用 outbox 服务。

Hook 接入需同时覆盖 JSON 与 multipart 写入，保留已有上传失败补偿；Hook 拒绝、超时和
提交失败也必须回收本次新上传对象。AuthService 的事件接入需单独处理，不能靠 CRUD Hook
间接覆盖。嵌套 save/delete 是否再触发 Hook、最大深度以及同记录递归拒绝规则须先固定。

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
- 保留内置 `/_`、`/api/collections`、`/api/auth`、`/api/admin`、`/api/files`、
  `/api/realtime`、`/api-doc`、`/swagger-ui`、`/webui`、`/web` 及其子路径，允许额外前缀
  的配置也不能覆盖这些路径。检查必须按路径段进行，不能只判断完整字符串是否相等。
- 同一 method/path 重复注册视为启动错误；参数改名不能避开冲突，例如 `/api/x/{id}` 和
  `/api/x/{name}` 属于同一模式。参数路由也不能匹配到上述保留路径。
- 路径参数使用 `{name}`；不接受任意正则路径。
- 路由处理器必须返回 `e.json`、`e.text`、`e.html`、`e.file` 或 `e.noContent` 的结果。
- 请求体按 `HB_MAX_REQUEST_BODY_SIZE` 限制，响应体也受 JS 路由响应上限限制。

`$apis` 第一版提供 `requireAuth()`、`requireAdmin()`、`bodyLimit(bytes)` 和
`rateLimit(options)`。鉴权复用 AuthService；`requireAdmin()` 判断管理员身份，不能仅凭
普通用户的 role 字符串授权。鉴权中间件产生的身份放在 `e.auth`，不能由脚本伪造。
`bodyLimit` 只能缩小全局上限。中间件顺序、rateLimit 选项及响应 envelope 见第 16.1 节待定项。

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

`$name` 是拟新增的过滤器参数语法，不是当前 `compile_filter` 已支持的功能。应在既有受限
语法的 AST 中加入参数节点并绑定值，不能把 filter 直接拼接为 SurrealQL；limit/offset 也需
新增明确的数据层接口，不能用页码近似换算任意 offset。JS delete 沿用 RecordManager 的
软删除语义。Record 包装器需区分字段未修改与 unset，不能将完整快照当作 PATCH 回写系统字段。

服务端扩展默认以系统身份运行，因此可以绕过公共 API Rules，但仍不能绕过 Schema、系统表
保护和事务约束。每个注册函数可通过选项声明 `authMode: "request"`，让数据库调用继承请求
身份。两种模式不能在单次调用中隐式切换。此选项约束 `$app` 数据调用，不改变内置请求
核心操作的身份。具体注册签名及嵌套继承规则尚待确定，见第 16.1 节。

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
必须走统一宿主服务，由其调用 `SchemaManager`，并执行 OpenAPI 刷新、文件清理等后置动作；
这些动作目前分散在 HTTP handlers 中，需要抽取供 HTTP 与 JS 共用。当前 Collection 更新
采用新增字段/索引和规则补丁，`save(collection)` 不能直接把完整定义传给现有 patch 接口，
更不能暗含删除字段或迁移已有数据。

`$app.db.query` 是受审计的高级能力：必须使用变量绑定；禁止 `DEFINE USER`、`DEFINE ACCESS`、
`USE NS/DB` 和系统表破坏操作；可通过配置完全关闭。返回值是按语句分组的 JSON 数组。

仅屏蔽上述关键词不足以满足安全边界：直接 DML 会绕过 Rust Record 校验、软删除和 Hook，
SELECT 也可能读取密码哈希或调用带副作用的函数。启用前必须明确允许的 AST、表、字段和函数，
在数据库执行能力层同时禁止越权及出站网络；嵌套语句、动态表名、自定义函数和事务控制都不能
成为绕过路径。不能证明该边界时保持此能力不开放，不能把 raw_query_enabled 当作关闭
Schema/敏感字段保护的授权。是否首版仅提供受限只读查询见第 16.1 节。

## 7. 定时任务

```javascript
cronAdd("daily-report", "0 0 8 * * *", async () => {
  const records = await $app.findRecordsByFilter("reports", "sent = false")
  // ...
})

cronRemove("obsolete-job")
```

- 表达式使用含秒的 6 段 cron，时区默认 UTC，可在任务选项中指定 IANA 时区。
- 任务名称全局唯一；重载时用新注册表原子替换旧调度，但仍在运行的旧任务保留原快照，
  同名任务的互斥状态必须跨快照保留，不能因重载获得第二个执行槽。
- 同一任务默认不并发执行；上一次未完成时跳过并记录 `warn`。
- 每次执行有独立超时和关联 ID。重试会重复执行已经成功的部分副作用，不保证 exactly-once；
  默认重试次数及幂等要求必须在开放 cron 前明确，不能无条件重试任意失败任务。
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

邮件通过待新增的 Rust `Mailer` trait 发送，JS 不接触 SMTP 凭据。收件人数、主题长度、正文大小、
附件总量和允许的发件地址由配置限制。事务内业务应写入 outbox，再由独立任务在事务提交后
发送，避免数据库回滚但邮件已经发出。无需可靠投递的直接发送也必须先确认脱离事务。

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

`publish(topic, data, audience?)` 是待新增的应用消息能力，不能直接桥接现有集合 SSE。
开放此 API 前必须同时定义客户端订阅入口、事件格式、鉴权与断线语义，并实现对应总线。
现有 `/api/realtime/{collection}` 的协议保持兼容；只实现发送端不能算完成功能。topic 必须符合
`[A-Za-z0-9][A-Za-z0-9._/-]{0,127}`，消息大小和发布速率受限。audience 可限定用户、角色或
连接；省略 audience 的全局广播默认禁用。记录 CRUD 的标准实时事件由核心自动产生，JS
只负责业务事件，避免重复推送。用户寻址必须包含 auth collection 与记录 ID，不能用裸 ID
跨集合匹配；角色与连接 audience 的可信来源和授权规则也需要在总线契约中定义。

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
符号链接/junction 逃逸和超限文件。扩展根目录与 Storage 模式必须明确选择，不能失败后
自动切换到另一后端。Storage 模式使用独立扩展前缀，不允许访问记录附件、网页部署或系统目录。
本地写入采用临时文件加原子替换；S3 不能承诺跨键原子 move，应明确为 copy + delete 及其
失败恢复语义。总配额需要并发预留和失败释放，不能只在写入前读取一次目录大小。

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

- 使用 `rquickjs::AsyncRuntime` 和 `AsyncContext`，I/O FFI 映射为 Promise。AsyncRuntime
  不会自动隔离同步 JS；CPU 执行和 Promise job pumping 需在专用 worker 线程运行，避免
  死循环占据 Tokio 请求 worker，并通过 QuickJS interrupt handler 实现中断。
- 每个执行上下文相互隔离；只共享不可变的已编译脚本缓存和 Rust 服务句柄。
- 运行时池有固定上限和有界等待队列，避免请求高峰无限创建 QuickJS 实例。
- 内存、栈、CPU 时间、异步总时长、Promise/宿主 I/O 并发数和返回数据量分别限额。必须
  区分 JS 活跃执行预算与包含等待 I/O 的墙钟超时，不能把单一 Instant 截止时间称为 CPU 用时。
- 超时会停止调度新宿主调用、发出取消请求并丢弃该 JS context；context 不返回池中复用。
  丢弃 Rust future 不证明数据库回滚、HTTP 未送达或邮件未发送。提交结果不确定时必须记录
  不确定状态，禁止当作“未执行”自动重试，并由宿主负责上传补偿和资源释放。
- Hook 注册表使用不可变快照。热重载期间已开始的请求继续使用旧快照，新请求使用新快照。
- 禁止脚本保存请求、Record 或事务对象供下次执行使用。

**闭包与上下文隔离仍需验证。** 启动 context 中的 Function 保留其创建环境，传递函数引用
不会生成独立的请求闭包，也不能将它当作跨 runtime 共享的缓存。建议快照只保存脚本/字节码及注册
描述，每次调用在新 context 重放纯初始化以建立本地闭包，并核对注册描述一致性；这意味着
顶层计算可能重复执行，不能承诺“顶层只执行一次”或支持跨请求全局变量。重放期间禁止 I/O、
动态增删正式注册项和基于时间/随机数改变注册清单。字节码缓存不能替代闭包实例化。

池容量必须按 runtime/worker 的真实内存计费边界计算；共享同一 runtime 的多个 context
不能被误认为各自拥有独立堆配额。队列满、等待超时、嵌套 Hook 重入和 worker 退出时的
处理必须在技术验证中固定，特别要避免 pool_size=1 时内层 save 等待自身占用的执行槽。

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

以上不是当前可生效配置。还需补齐启动超时、排队超时、响应上限、日志限额、宿主调用并发、
递归深度、实时授权/限流、路由前缀及 cron 重试字段，并在配置参考中列出默认值和环境变量
映射。`env_allowlist` 不能仅按敏感关键词过滤：运维者只应列入公开值，且宿主凭据项必须拒绝。

## 16. 实施顺序

### 16.1 实现前必须关闭的问题

**结论：六项都有可行的解决路线。** 其中事务已有当前版本 SDK 的直接支持，其余主要是
宿主服务抽象、执行生命周期及公开接口的取舍。以下为推荐落地方案；新增 API、配置与错误码
均未实现，源码查证也不等于集成测试通过。实施时应将建议同步到前文契约、开发指南和类型包，
不能同时保留相互冲突的两套行为。

#### 1. 事务与提交边界

**问题：** 在当前 SurrealDB 3.2.3、Mem/SurrealKv 后端验证跨多次异步调用的事务方案，
证明 read-your-writes、嵌套写入失败回滚和超时取消行为，不能用多个自动提交查询模拟共同事务。

**判断与依据：可以接入现有 SDK，无需更换数据库或自行实现事务引擎。** 已查阅本机锁定版本
源码：`surrealdb-3.2.3/src/method/transaction.rs` 提供 `Transaction<C>` 及 query/commit/cancel；
`src/method/mod.rs` 提供 `Surreal::begin(self)`；`src/engine/local/mod.rs` 按事务 ID 执行查询。
上游 `tests/api_integration/basic.rs::client_side_transactions` 也包含提交、取消及多次写入用例。
缺口在本项目尚未接入，而非该版本 SDK 没有能力；远程 HTTP 不在本次支持范围内。

**推荐方案：**

- 在 `herta_db` 新增 `DbSession` 查询入口，内部区分普通客户端和 `Transaction<Db>`。
  用 `db.inner().clone().begin().await` 创建事务；所有 Record 查询、Rules 检查、关系校验和
  Schema 读取都接受该入口，禁止事务中通过 `inner()` 回退到自动提交客户端。
- 每次顶层 Record 写入建立一个事务，**整条持久化 Hook 链完成后才提交**，包括所有
  `await e.next()` 后的代码。嵌套 save/delete 加入同一事务；任一持久化失败将事务标记为
  rollback-only，即便 JS 捕获异常也不能提交部分结果。首版不提供嵌套事务/savepoint。
- 请求 Hook 包裹这一完整操作，本身不自动成为跨多个 CRUD 的事务。自定义路由/任务若需要
  多记录原子操作，新增 `$app.transaction(async (tx) => { ... })`；tx 提供同样的 Record API。
  活动事务内的 `$app` 数据调用也必须加入该事务，不能绕开 tx 自动提交。一个调用树一次只允许
  一个活动事务，事务内命令串行执行，不承诺并行 save 的执行次序。
- 新增 `e.afterCommit(callback): void`。注册时保存回调及其身份，最外层 commit 确认后才
  按注册顺序执行，参数是已提交事件的只读快照；回滚时丢弃。回调不得持有可用事务句柄，失败
  只记录日志，不更改核心写入成功结果。它是进程内 best-effort 功能，不提供宕机可靠性。
  此接口只出现在持久化事件上；显式事务提供 tx.afterCommit。无活动事务的请求事件不能把
  回调含糊地绑定到“下一次写入”。
- 可靠副作用通过 `$app.outbox.enqueue(kind, payload, { idempotencyKey })` 写入同一事务，
  由宿主独立投递。仅存可序列化数据与宿主已注册的任务类型，不持久化 JS 闭包；投递采用租约、
  有界重试与失败状态，语义为 at-least-once，收件服务仍需按幂等键去重。
- 事务由独立于请求 future 的宿主 owner 管理，状态为 active、committing、committed、
  rolling-back、rolled-back 或 unknown。超时/断连只通知 owner；未提交时显式等待 cancel，
  提交已发出时继续收取结果。不能依赖 Drop 自动回滚，也不能因丢弃 commit future 就宣告失败。
  结果未知时不运行 afterCommit、不自动重放业务操作，并由后台核对提交标记后再做补偿。
  为此，将 operationId 提交标记写入同事务的宿主系统表；仅在提交任务已结束且新的独立查询
  能确认结果时收敛状态，不能在 commit 仍进行中因为暂时查不到标记就判定回滚。
- multipart 新对象的清理也归 owner：确认回滚后删除，确认提交后保留；unknown 时保留并
  待核对，避免删掉实际已提交记录引用的文件。旧文件删除、OpenAPI 刷新都在提交后执行。
  Collection Schema 变更使用单独的顶层事务，首版拒绝在 Record 事务中混入 Schema 变更。

推荐的边界如下；请求 Hook 中独立发起的写入不隐式并入图中的 CRUD 事务：

```text
请求 Hook 前置
  → begin → 持久化 Hook 前置 → 核心写入 → 持久化 Hook 收尾 → commit
  → afterCommit → 请求 Hook 收尾 → HTTP 响应
```

因此，前文“next 后发送邮件”的示例应改为显式注册提交后动作：

```javascript
onRecordCreate(async (e) => {
  e.afterCommit(async (committed) => {
    await $app.mailer.send({
      to: [{ address: committed.record.get("email") }],
      subject: "Welcome",
      text: "Your account is ready.",
    })
  })
  await e.next()
}, "users")
```

**验收重点：** 两个后端分别测试事务内可见/事务外不可见、两次写入一起回滚、嵌套异常被捕获
后仍不能提交、写冲突、提交期间断连，以及提交确认前无外部副作用。还需验证显式事务提交对
现有 LIVE SELECT 的通知行为，保留已有快照校验；不能仅凭上游基本事务用例认为全部通过。

#### 2. 可执行的事件契约

**问题：** 固定各事件字段、无 next 的结果、未等待 next 的处理、authMode 注册签名与
嵌套继承、递归深度，以及生命周期时点；Auth 不得暴露密码、密钥或破坏令牌轮换。

**推荐方案：可以通过统一的事件状态机解决，避免每类 handler 自行解释 next。**

- 保留 `onRecordCreate(handler, ...collectionNames)` 简写；新增互斥的对象形式
  `onRecordCreate(handler, { collections: ["posts"], authMode: "request" })`，不混用两种参数。
  Collection/Auth Hook 使用同样选项；应用生命周期 Hook 不接受 collections 或 request 模式。
- Record/Collection/Auth 的根 Hook 默认 system，自定义路由默认 request，任务和生命周期
  默认 system。路由保留已有简写，并新增 `routerAdd({ method, path, authMode, middleware }, handler)`。
  这是对前文“所有注册默认 system”的建议修订；尚未发布 JS API，不需要维护这条草案的兼容性。
- 请求身份和数据库执行身份分别保存。公开 CRUD 核心操作永远使用原请求身份；嵌套 `$app`
  调用继承父操作的有效身份，子 Hook 的 system 声明不能提升一个 request 调用树的权限。
  system 表示允许使用的权限上限；父调用限制为 request 时，子调用也按 request 检查 Rules。
  将有效模式写入只读事件字段和日志，不使用可被并发分支覆盖的全局身份变量。
- `e.next()` 只能调用一次，宿主跟踪其下游结果。持久化链未调用 next 则返回
  `409 HB_HOOK_ABORTED` 并回滚；请求链未调用 next 时必须返回 Response 描述，否则返回
  `500 HB_HOOK_ERROR`。重复调用、处理器已经结束后的调用也报 Hook 错误。
- 若处理器返回时下游仍未完成，标记 Hook 失败，由 owner 收束所有命令；尚未提交的事务
  回滚。若请求 Hook 的下游已进入 committing，则按真实提交结果处理；已确认提交的内置
  CRUD 保留成功结果并记录违规，不能再声称回滚。无需猜测源码是否写了 await：已完成的
  下游按完成处理。提交前必须同时满足持久化整链成功、无未完成命令、事务未被标记回滚。
- 单记录事件提供候选 record、originalRecord、collection；列表提供 query 和 records
  结果，不虚构单条 record。create 的 originalRecord 为 null；update 用旧记录加 dirty patch
  构建候选；delete 的记录只读。核心写入后 e.record 转为只读，避免修改已生成结果却没有落库。
  context 只在本次调用树共享，original/身份/提交状态都不能由 JS 改写。
- 嵌套写入仍触发持久化 Hook，不再次触发 HTTP 请求 Hook；默认深度上限 8。对调用栈中已经
  存在的同一 collection/record 再次写入立即报 `HB_HOOK_RECURSION`，新记录在进入 Hook 前
  分配 UUID。额外限制一次调用的宿主命令总数，阻止不断创建新 ID 绕过同记录检测。
- Auth Hook 放在 AuthService 的业务边界：注册可修改经清理的 profile；登录 Hook 在
  密码验证通过后、令牌签发前运行；refresh Hook 在令牌与账户验证后、轮换前运行。密码哈希、
  token_key、JWT 和 refresh token 不进入事件或 afterCommit 快照。账户创建及刷新令牌落库
  纳入事务；密码错误计数、重放检测等安全状态按现有 AuthService 语义独立维护，不能被 Hook
  否决而撤销。管理员身份始终按 AuthIdentity 判断。
- bootstrap 在宿主服务装配与脚本校验后、监听端口前运行；serve 在 listener 已绑定但尚未
  接收请求前运行，失败则关闭 listener 并终止启动。shutdown 在停止接收请求与新任务、完成
  有界排空后运行，宿主服务最后关闭；热重载不重复触发这三个进程生命周期事件。
  生命周期链末端是空操作，处理器也需要 next；bootstrap/serve 中断则启动失败，shutdown
  中断或失败只记日志，不能阻止 Rust 资源清理。Auth 链无 next 按核心操作被拒绝处理。

**验收重点：** 事件调用顺序、无 next/双 next/提前返回、request 模式嵌套提权、同记录递归、
Auth 注册回滚及刷新令牌重放，以及启动 Hook 失败时端口不对外提供服务。

#### 3. 运行时验证

**问题：** 固定闭包重建、worker/runtime 关系、资源计费与重入方式，并验证中断、OOM 和
Windows/部署平台上的行为。

**推荐方案：固定 worker 数量，每次根调用创建独立 runtime/context；优先保证隔离正确。**

- worker 使用专用线程及本地异步执行器。一次根调用占一个 worker，创建一对
  AsyncRuntime/AsyncContext，完成后整体销毁，正常结束也不复用 JS 堆。pool_size 限制并行
  根调用数量；Rust I/O 通过有界通道交给宿主 Tokio runtime，JS 对象不跨线程传递。
- 快照存不可变脚本内容、内容哈希和注册描述。先缓存源码；确认所选 rquickjs 版本的字节码
  行为后再增加编译缓存，禁止从不可信磁盘加载 QuickJS 字节码。每次调用按快照重放纯初始化，
  建立本地闭包并核对注册描述；不得读取热重载后磁盘上的新源码来执行旧快照。
- 嵌套 Hook 复用当前调用的 runtime、快照、预算与事务，通过当前 JS 调度器续接，不再次
  申请 worker。Rust 数据适配器只执行带调用令牌的宿主命令，不能持有 QuickJS 锁再同步等待
  重入同一 context。事务命令串行，非事务 I/O 允许有上限的并发。
- memory_limit 对应整个调用 runtime 的堆；stack_limit 配合 QuickJS 栈检查。Rust 端
  请求缓存、查询结果、序列化临时数据与通道也必须单独限量，不能认为 JS 堆限制会覆盖它们。
  排队项只持有请求描述，获得执行槽及缓冲预算前不读取完整正文；运行期对累计宿主缓冲按字节
  预留，超限拒绝分配。宿主原生函数不得 panic、无限分配或同步执行无界工作。
- 将 execution_timeout_ms 明确定义为**累计 JS 活跃执行片段的墙钟预算**，不宣称是操作系统
  CPU 用时；在 eval、回调和 Promise job 的执行边界累计，等待宿主 I/O 时暂停累计。
  QuickJS interrupt 检查本片段剩余预算及根调用取消状态；async_timeout_ms 覆盖调用的实际
  总时长。纯 Promise 链仍受执行时间与内存限制，首版不承诺无法精确计量的“所有 Promise 个数”。
- 队列满或排队超时返回 `503 HB_JS_BUSY`；开始执行后才计算执行预算，排队有独立上限。
  取消后禁止新 FFI，交由事务 owner 清理，runtime 销毁；任务重试也必须创建新 runtime。
  afterCommit 使用原调用剩余预算，预算耗尽时记失败；可靠投递依赖 outbox，不依赖该回调必达。
- QuickJS 在进程内运行可限制脚本能力，但不能提供抵御引擎内存安全漏洞的进程级隔离。
  对当前仅运维部署脚本的范围，采用上述模型；未来若接收不可信租户脚本，新增独立子进程
  与 OS 资源约束，不能仅扩大当前线程池来宣称满足相同隔离要求。

**验收重点：** pool_size=1 嵌套 save 不死锁、连续请求全局变量不共享、纯 JS/Promise
死循环被中断、长 I/O 不误耗 JS 执行预算、OOM 后下一调用可正常完成，以及热重载期间旧
请求仍使用旧闭包。本机尚无 rquickjs 依赖，这些属于后续需要实际运行的技术用例。

#### 4. 数据库边界

**问题：** 固定参数化过滤、任意 offset、Record dirty/unset 及原生查询范围，避免绕过
Schema、系统表、敏感字段和宿主网络策略。

**推荐方案：业务写入统一走 Record 服务，原生接口首版只接受可编译到同一读取模型的 SELECT。**

- 给现有 filter AST 增加参数节点，参数只能作为值，不能替代字段、表名或操作符。缺失参数、
  非 JSON 值、保留内部绑定名都在执行前拒绝；不做字符串替换。关系值继续按 Schema 转成
  RecordId，字符串中的引号或 SQL 片段始终只是值。
- 新增内部 `RecordQuery { fields, filter, bindings, sort, limit, offset }`；fields 为可选投影，
  HTTP 默认使用既有完整记录视图。HTTP page/perPage
  无损转换到它，JS 直接传 limit/offset。默认 limit=30、offset=0，limit 范围 1..500；
  offset 为非负安全整数并做底层范围检查。默认 `-created_at`，排序末尾补 id 保证同时间戳
  下结果稳定；它仍是 offset 分页，不承诺并发写入期间跨页快照稳定。
- Record 保存三部分数据：只读原值、候选值和 dirty/unset 集合。get/original 返回深拷贝，
  修改嵌套对象后需要显式 set；set(null) 沿用现有字段清空语义，unset 明确表示删除字段，
  undefined 不作为可写值。save 只提交变化，提交前验证最终候选和 dirty patch，防止删掉必填项。
  newRecord 只创建候选和 UUID，不同步查库；save 时加载 Schema。事务内首次创建成功后
  后续 save 按 update 处理，回滚时相关句柄失效。
- 系统字段及 Auth 保护字段校验统一下沉 Rust 服务，Auth 集合无论 Schema 模式都不能通过
  save 写入密码哈希、token_key 或提升角色；普通 base Collection 的业务字段仍按自身 Schema
  处理。首版 save 拒绝密码写入，若需要修改密码应在 AuthService 新增专用操作并处理令牌失效。
  所有 JS 读取路径，包括 original 和序列化，均使用相同脱敏规则。普通 save 不接受伪造的
  记录附件引用。
- raw_query_enabled 默认 false；开启后，`$app.db.query` 仅在有效 system 模式下接受
  **一条单表 SELECT**。允许显式字段/受控的星号、既有 filter 子集、排序、LIMIT/START；
  只允许已登记的 base Collection，排除 Auth 集合、系统表、动态目标、子查询、图遍历、
  函数、聚合、DDL/DML 和事务控制。解析后转换成 RecordQuery 再生成 SQL，不直接执行原文。
  星号展开为允许字段，并强制加软删除条件、行数/字节/超时上限，返回 `[rows]` 保持按语句分组。
- 数据库实例显式禁用出站网络与内置脚本能力，作为第二层限制；这些是实例级配置，须保留
  核心现有查询需要的内置函数和实时能力，不能用全函数禁用导致 Auth/时间戳逻辑失效。
  不在允许子集的 SQL 返回 `400 HB_QUERY_UNSUPPORTED`，request 模式调用 raw API 返回
  `403 HB_CAPABILITY_DENIED`。清理任务改用参数化查找加软删除，不能继续使用前文 DELETE 示例。
- collections.save 比较原始定义与候选，仅允许新增字段/索引、修改规则；拒绝改名、改类型、
  删除/修改已有字段和隐式数据迁移。无变化返回原定义；并发 Schema 已变化时返回 409，避免
  覆盖别人更新。统一 CollectionService 管理事务、提交后 OpenAPI 刷新与存储清理，HTTP/JS
  复用该入口，消除分散在 handler 中的行为差异。
- 共享 JS 事件 DTO、权限描述和 HostServices/EventDispatcher 契约放入 herta_core；不包含
  QuickJS 或 Surreal 类型。Record/Collection Schema DTO 从 herta_db 移到 core，并在 db
  保留 re-export，Surreal 转换留在 db。db/jsvm 只依赖 core，由 server 装配具体服务和事件
  分发器，避免循环依赖。现有 DTO 上依赖 Surreal 的 inherent methods 改为 db 内的转换函数
  或扩展 trait，不能对迁出的类型继续添加跨 crate 固有方法；用不含 dispatcher 的服务句柄
  执行纯 DB 命令，避免 Arc 强引用环。

**验收重点：** 参数注入、关系绑定、offset=1 等非整页偏移、required unset、嵌套值修改、
Auth 敏感字段所有读取路径，以及 raw SELECT 中隐藏的函数/子查询。任何拒绝必须发生在数据库
执行之前，不能靠查询后过滤输出补救读取越权。

#### 5. HTTP 与 SDK 契约

**问题：** 固定请求/响应、中间件、路由匹配和 HEAD/OPTIONS 行为，并让 SDK 能正确解析结果。

**推荐方案：业务 JSON 默认遵循现有 envelope，显式保留原始响应接口。**

- `e.json(status, data, meta?)` 仅用于有正文的 2xx 成功响应，输出
  `{ data, meta: meta ?? null, error: null }`；业务失败抛公开 Error，宿主生成既有错误 envelope。
  新增 `e.rawJson(status, value)` 处理 webhook 等原始 JSON 场景；text/html/file 同样是原始响应。
  SDK `hb.request()` 继续只解析 envelope；原始响应使用 fetch，无需改变现有 SDK 的默认语义。
  204 通过 e.noContent 返回，SDK 将 204 映射为 undefined，禁止把空正文当作无效 JSON。
- 请求对象提供只读 method、path、pathValue、query、header 及受限的 json()/text()/bytes()。
  宿主只缓存一份有界正文；重复读取复用缓存，格式错误映射 400、超限 413。Auth 事件使用
  专门清理后的请求视图，不能通过 request.json 再取回被事件字段隐藏的密码/令牌。
- 宿主先完成路径解析、可选身份解析和全局上限检查；注册的 `$apis` 原生中间件在任何用户 JS
  前按声明顺序执行，其后执行 JS 中间件，再执行 handler。requireAuth 允许已认证用户和
  管理员；requireAdmin 只接受管理员身份。无 Bearer 为匿名，错误 Bearer 返回 401。
  bodyLimit 取全局与路由限额的较小值，脚本不能放宽限制。
- `rateLimit({ limit, windowMs, key: "ip" | "auth" })` 采用进程内滑动窗口；auth key 使用
  collection/id，匿名回退到连接 IP，默认 key=ip。限流键包含稳定的路由 method/规范化路径，
  重载不清零；过期条目回收、键数有界，容量耗尽拒绝新键而非淘汰活跃键来绕过限流。
  不直接信任客户端传入的 X-Forwarded-For。
- 路由和请求共用路径规范化：只解码一次，拒绝点路径段、重复分隔符和编码的斜杠/反斜杠，
  非根路径统一去掉尾斜杠。比较路由的匹配集合，而非仅比较字符串；首版同 method 的两条
  自定义路由只要可能匹配同一具体路径就拒绝注册，参数名称变化不能避开检查。
  与内置保留前缀存在交集的参数路由也拒绝，且将新增 `/api/events` 纳入保留范围。
- 显式 HEAD 优先，否则回退 GET 并由宿主移除正文；GET handler 不应产生写入副作用。
  OPTIONS 由宿主根据快照生成 Allow，脚本不得注册覆盖；跨域策略仍由宿主统一执行。
  路由快照在请求开始时取得，完成前不切换到新快照。
- e.file 只接受扩展文件逻辑键，由宿主 FileService 查找和读取，返回 Promise<Response>；
  调用方式为 `return await e.file(key)`，绝不把字符串当成本地绝对路径传给 Salvo。
  JSON 在发布响应前完成有界序列化，文件也受响应字节上限约束。禁止脚本设置 hop-by-hop
  headers 或伪造 Content-Length；响应大小按 UTF-8/实际字节计算。

例如 `return e.json(200, { ok: true })` 可直接由
`await hb.request<{ ok: boolean }>("/api/health/custom")` 得到 `{ ok: true }`。

**验收重点：** envelope/204/raw 三种路径、普通用户伪造 admin role、bodyLimit 放宽尝试、
重叠参数路由、编码路径绕过、热重载路由一致性及文件逻辑键越权。

#### 6. 跨服务与配置

**问题：** 为应用 topic 补齐订阅端，固定文件后端、配额恢复和非原子 move；明确 cron 的
时区、重试、互斥及遗漏配置。

**推荐方案：复用已有基础设施，新增的每项能力都形成完整的发送/接收或写入/恢复闭环。**

**实时消息：** 新增宿主 RealtimeBus 与 `GET /api/events?topic=...`，保持原集合 SSE 不变。
首版一个连接订阅一个精确 topic，必须登录；沿用现有连接/IP 配额、心跳、过期关闭及清理机制。
返回 connected/message/ping/error 事件，message 为 `{ id, topic, data, timestamp }`，
不保证断线重放。SDK 新增独立的 `hb.realtime.subscribe(topic, options)`，不复用记录 CRUD
的事件模型。每个连接的发送队列有界，慢消费者收到错误后断开，不阻塞发布者。

audience 首版必须显式提供，且一次仅接受 users、roles、connections 中的一种：users 使用
`{ collection, id }`；roles 使用 `{ collection, role }`，管理员角色归 `_admins`；connections
使用宿主签发的连接 ID。宿主在投递时核对连接的 topic、当前身份、到期与目标匹配，JS 不能
自行提交一个“已验证身份”。省略 audience 返回能力拒绝，首版不开放全局广播。建议将前文
userIds/裸 roles 示例改为此结构，避免多 Auth Collection 间 ID/角色撞名。publish 的结果只
表示已进入符合条件连接的队列，不表示客户端已处理；需要可靠消费的业务另用持久化消息设计。

**文件：** 首版统一复用当前配置的 Storage，以 `extensions/` 作为专用前缀；不再另建独立
的任意本地 root 模式。将第 15 节 root 改为受校验的 prefix，禁止与 records/ 等宿主保留
前缀重叠；本地和 S3 使用相同逻辑键、元数据及配额语义。为 Storage 补充有界分页 list 和
copy；move 明确为 copy 成功后 delete，删除失败返回包含目标已存在信息的可恢复错误。

建立宿主专用文件目录与操作账本，记录逻辑键、大小、版本及 pending 操作。写入/copy 前在
数据库事务中预留**完整新对象大小**，同键串行，旧对象清理成功后释放旧占用；因此覆盖大文件
也需要临时配额。失败或重启时根据操作账本核对对象，不能不确定是否存在就释放配额。分页
扫描修复孤儿对象，修复完成前保守保留占用。配额指扩展服务管理的对象与预留，不包括 S3
供应商保留的历史版本；启用桶版本控制时由部署策略负责历史版本生命周期。

本地访问采用目录句柄约束并拒绝符号链接/reparse point，打开和使用必须是同一受控对象，
避免 canonicalize 后替换路径的竞态。异步 readBytes 返回 Uint8Array，write 接受字符串或
Uint8Array，所有列表、读取和复制均有大小上限。文件上传到记录仍使用现有 multipart 流程。

**cron：** 使用严格的 6 段表达式，时区采用 IANA 数据，默认 UTC。DST 不存在的本地时刻
跳过，重复时刻只取较早的一次；不补跑停机期间错过的时间点。任务名的互斥状态由宿主持有，
独立于注册表版本；重载不立即补跑，在途运行及其重试保持旧快照，下一计划运行使用新快照。

默认 retries=0；任务显式声明幂等并设置重试次数后才允许重试，最多 3 次，延迟依次为
1/2/4 秒。同一计划运行的 runId/幂等键不变，attemptId 每次变化；重试等待也占用该任务名
的互斥权，期间到达的新计划点跳过。提交 unknown 必须先核对，不直接重试。每次尝试新建
JS runtime，默认最长 30 秒；任务配置只能缩小此上限，它替代普通调用的 5 秒总时限，所有
子调用再取当前剩余时间与能力自身超时的较小值。shutdown 先停止新任务和重试，再有界排空。
注册签名补为 `cronAdd(name, expression, handler, { timezone, maxRuntimeMs, retries, idempotent })`；
handler 接收含 runId、attemptId、scheduledAt 的只读任务上下文，不使用 next。idempotent
声明本身不会消除重复副作用，任务仍需向支持去重的接收端传递稳定幂等键；普通 SMTP 不保证去重。

**HTTP、邮件与 outbox：** 出站 HTTP 使用独立的受限 client，关闭自动重定向与环境代理；
每跳解析并校验所有候选 IP，只连接已验证地址，TLS SNI/证书验证仍使用原主机名，不能校验
DNS 后让 client 再自由解析。跳转跨源不转发 Authorization/Cookie，响应以有界流读取。
Mailer 只消费 Rust 内的 SMTP 配置，生产模式要求 TLS，限制收件人、正文与头字段并拒绝 CRLF
注入。两类发送及实时/文件写入都检查宿主事务状态，事务内返回 `HB_SIDE_EFFECT_IN_TRANSACTION`；
outbox.enqueue 只落库，可在事务内调用。outbox 发送也必须经过相同的能力与目标校验。

**建议补齐的默认配置：** 保留前文已有值，增加以下项目；单位和作用域必须写入配置参考。

| 配置 | 建议默认值与含义 |
| --- | --- |
| jsvm.startup_timeout_ms | 10000；完整候选版本验证时限，重放也受调用预算限制 |
| jsvm.queue_timeout_ms | 1000；独立于执行时限 |
| jsvm.max_response_bytes / max_bridge_bytes | 各 4194304；路由响应及单次 FFI 入/出参上限 |
| jsvm.max_host_buffer_bytes | 16777216；每根调用累计宿主缓冲预算，另计 JS 堆 |
| jsvm.max_pending_host_calls / max_host_calls | 32 / 256；每根调用在途数量和总数量 |
| jsvm.max_hook_depth | 8；嵌套写入深度 |
| jsvm.max_logs / max_log_bytes | 100 / 65536；每根调用日志条数和累计序列化字节 |
| jsvm.route_prefixes | ["/api/"]；额外前缀仍不能覆盖保留路由 |
| jsvm.shutdown_timeout_ms | 30000；JS 侧排空上限，数据库提交/补偿仍由 owner 收束 |
| jsvm.realtime.enabled | false；与现有集合 SSE 开关/限额区分 |
| jsvm.realtime.max_message_bytes / publish_per_second | 65536 / 100；单消息大小、应用级每秒发布上限 |
| jsvm.realtime.max_audience / connection_queue_capacity | 100 / 64；单次目标数、每连接队列条数 |
| jsvm.realtime.connection_queue_bytes | 262144；每连接待发消息累计字节，与条数同时限制 |
| jsvm.files.prefix | "extensions"；替代独立 root，配额沿用前文 100 MiB/单文件 10 MiB |
| jsvm.cron.retries / max_retries | 0 / 3；默认不重试及任务可配置上限 |
| mail.driver | "disabled"；发送还需要 jsvm.mail.enabled 授权 |

所有 HB_JS_* 变量与字段一一映射，环境变量覆盖 TOML；列表使用 JSON 数组字符串，避免
逗号转义歧义。拒绝未知 jsvm/mail 配置字段、非法范围和不存在的 IANA 时区；enabled=false
不加载脚本。权限开关先于适配器可用性检查：未授权报 DENIED，已授权但无服务报 UNAVAILABLE。
配置上限是部署者可调整的初值，压力测试后再优化，不作为未经测试的吞吐量承诺。

**验收重点：** 多 Auth Collection 的 audience 隔离、慢消费者、JWT 过期；并发覆盖写入、
重启后配额恢复、copy 成功/delete 失败；DST 跳时/重复时刻、重载期间任务不重叠及有副作用的
重试幂等；HTTP 重定向、DNS rebinding 和事务内外部调用拒绝。

**建议落地顺序：** 先做事务适配和 JS worker 的独立技术验证，再完成 Record/事件纵向链路，
随后加入路由与 SDK、Collection/Auth，最后加入 outbox、外部服务、应用消息与任务。每一批
通过自身验收再扩展；全部实现前保持设计草案状态，类型包只收录实际落地部分。

### 16.2 分步交付

1. 建立 `herta_jsvm` crate、配置、错误类型、脚本发现、编译缓存与原子注册表。
2. 完成日志、纯事件 Hook 和 Record/Collection FFI，并接入 CRUD 与 SchemaManager。
3. 完成自定义路由和请求/响应桥接，再接入 auth 中间件。
4. 完成调度器、HTTP client 和 Mailer trait；外部副作用只能在宿主确认脱离事务后执行，
   可靠投递采用事务 outbox 模式。
5. 复用现有 Storage 并补齐扩展文件适配器；新增应用消息总线与客户端订阅桥接，保持现有
   集合 SSE 协议兼容。
6. 增加 `@hb/types`、示例、热重载、配额压力测试和沙盒安全测试。

每一步必须包含正常路径、超时、OOM、权限拒绝、重载并发和宿主服务失败测试。只有服务端实际
实现并通过测试的 API 才能进入 `@hb/types`，避免类型声明承诺不存在的能力。

必须补充以下与当前实现相关的回归场景：

- Hook 修改候选记录后仍执行 Schema/API Rules；JSON/multipart 保持同样规则，拒绝和超时
  后新上传对象被回收；Auth 密码哈希、角色保护与 refresh token 轮换不被绕过。
- 双重/未等待 next、嵌套 save、内层成功外层失败、提交后异常、自定义路由部分写入后失败，
  分别验证返回值、最终数据库状态和副作用次数；pool_size=1 不死锁。
- 隔离 context 的闭包可调用但状态不跨请求泄漏；语法错误、半途注册失败和重载失败保留
  完整旧快照；同名旧 cron 尚未结束时新快照不启动第二次执行。
- 参数化 filter 的引号/注入载荷、关系字段、缺失绑定、任意 offset，以及原生查询的
  嵌套 SQL/系统表/函数/网络越权；未经允许的输入在执行前拒绝。
- 自定义参数路由与 admin/files/realtime/web 等保留路径冲突、响应 envelope 的 SDK
  兼容性；文件并发配额、Windows junction、S3 copy 成功 delete 失败均有明确结果。
