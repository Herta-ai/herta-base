# JavaScript 扩展运行时设计

> 状态：实施中，尚未达到完整开放标准。2026-09-09 已接入服务器加载、生命周期、开发重载、
> JSON/multipart Record、Auth、Collection 事件、自定义路由和邮件桥接；Windows x64/MSVC 已验证本页验证报告所列范围。
> 2026-09-28 补齐受限 HTTP 出站、cron 调度、应用消息 SSE 和 SDK topic 订阅，并通过对应 Windows 测试。
> 同日接入扩展文件目录/版本/配额账本及 mail.send、http.send 事务 outbox；基本故障与集成用例通过，完整六步及最终发布验收尚未完成。
> 逐项实测及待验收项见 [实施验证](js-runtime-validation.md)。
> 第 1.1 节保留实施前基线，固定业务约定见第 16.4 节。正文与第 16.1 节采用同一套目标方案，
> 第 16.3 节列出开放对应功能前必须完成的验证，不再把推荐方案视为验证已通过。
> Rust 宿主邮件服务、`[mail]` 配置及管理员邮件发送接口见 [邮件发送](mail.md)。JS 门禁和本地 SMTP 邮箱的实际 Message-ID 核对已通过。

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

### 1.1 实施前基线（`4c0fe64`）

| 能力 | 当前实现与本阶段需要补齐的部分 |
| --- | --- |
| 运行时与配置 | Workspace 尚无 `herta_jsvm` 或 `rquickjs` 依赖；`HbConfig` 尚无 `jsvm` 字段，已有 `mail` 字段。`hooks_dir` 目前仅保存路径，不加载脚本。 |
| 邮件 | `herta_core::{Mailer, MailMessage, MailReceipt}`、`herta_mail`、`ApiState.mailer` 和 `POST /api/admin/mail/send` 已实现；复用服务，仅新增 JS 能力门禁和异步桥接，不重写 SMTP。 |
| Record 与事务 | `RecordManager` 已提供授权 CRUD、软删除和敏感字段清理；SDK 3.2.3 已提供事务句柄，但 `DbClient` 尚未接入，具体方案见第 16.1 节。 |
| 查询过滤 | `compile_filter` 支持受限表达式并绑定字面值，尚不接受示例中的 `$name` 外部参数；列表接口目前使用 page/perPage，不是 limit/offset。 |
| Collection | `SchemaManager` 管理 Schema；OpenAPI 刷新、集合文件清理目前由 HTTP handler 完成，直接调用 manager 不会完成这些后置动作。 |
| 实时 | `RealtimeManager` 通过 LIVE SELECT 与快照校验提供集合变更订阅，没有任意 topic 的 publish、用户/角色/连接寻址接口。 |
| 文件 | `herta_storage::Storage` 已有 put_file/head/get/delete/delete_prefix 和本地/S3 适配器，尚无 list/copy/move 或扩展目录配额。 |
| Auth | `AuthService` 的注册、登录、刷新有独立数据库调用；仅在 RecordManager 加 Hook 不会覆盖这些流程。 |
| 上传 | multipart 在上传前调用 `preflight_*_authorized`，之后再次校验并写入；Hook 接入需要拆分预检与最终校验，不能只包装最后一次写入。 |

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
| `herta_core` | `JsvmConfig`、共享 DTO、能力开关、公共错误码和不依赖具体实现的宿主契约 |
| `herta_jsvm` | 脚本发现/编译、QuickJS 池、注册表、事件调度、FFI 与资源限制 |
| `herta_db` | Record/Collection 操作、事务句柄、Schema/API Rules 最终校验 |
| `herta_api` | 请求事件适配、自定义路由快照和 JS Response 到 Salvo Response 的转换 |
| `herta_server` | 启动/停止顺序、热重载、cron runner 与具体服务装配 |
| 现有实时/存储服务 | 复用 `herta_storage::Storage`；应用消息总线及 JS 文件适配器需要新增，不能视为已存在的 trait |

`herta_jsvm` 只依赖宿主 trait，不直接依赖 SMTP、S3 或具体实时协议。可用 mock 服务构建和
测试核心。共享 Record/Collection DTO 与宿主契约迁入 core，db 保留 re-export，具体 Surreal
转换留在 db，见第 16.1.4 节，避免 core/db/jsvm 的循环依赖。

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
  const drafts = await $app.findRecordsByFilter(
    "drafts", "expires_at < $cutoff", "expires_at", 50, 0,
    { cutoff: new Date().toISOString() },
  )
  for (const draft of drafts) await $app.delete(draft)
})
```

清理示例每次最多软删除 50 条业务记录，剩余记录留到后续计划运行；不操作宿主会话表，
也不依赖首版禁止的原生 DML。

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
也可使用 `onRecordCreate(handler, { collections: ["posts"], authMode: "request" })`，
对象形式与名称列表互斥。身份默认值与嵌套继承见第 6.2 节；完整事件规则见第 16.1.2 节。

### 4.2 中间件链语义

每个处理器接收事件对象 `e`。调用 `await e.next()` 执行下一个处理器或核心操作；不调用
`e.next()` 即中断处理链。处理器可以在 `e.next()` 前后执行逻辑：

```javascript
onRecordUpdate(async (e) => {
  $app.logger.debug("updating record", { id: e.record.id })
  e.afterCommit(async (committed) => {
    await $app.realtime.publish(`audit/${committed.collection.name}`, {
      action: "update",
      id: committed.record.id,
    }, { roles: [{ collection: "_admins", role: "admin" }] })
  })
  await e.next()
}, "posts")
```

为避免旧文档中 `return false`、抛异常和隐式返回的歧义，新的统一规则是：

- `e.next()`：继续处理链。
- 正常返回且未调用 `e.next()`：有意终止处理链，不得伪造核心写入成功。请求事件必须提供
  Response 描述，否则返回 `500 HB_HOOK_ERROR`；持久化事件返回 `409 HB_HOOK_ABORTED`
  并回滚。Auth 无 next 同样拒绝；生命周期中断规则见第 16.1.2 节。
- 同一处理器最多调用一次 `e.next()`；重复调用或处理器结束后的调用属于 Hook 错误。
  宿主必须跟踪尚未完成的下游调用，不能将其作为脱离事件生命周期的后台任务；处理器提前
  返回时下游仍未完成则标记 Hook 失败，按第 16.1.2 节由 owner 收束；提交结果不能靠猜测。
- 抛出 `BadRequestError`、`ForbiddenError`、`NotFoundError` 等公开错误：按对应状态返回。
- 抛出普通 `Error`：记录完整堆栈，对外返回 `HB_HOOK_ERROR`。
- 宿主确认该核心操作提交成功后发生的异常不能回滚已提交写入。宿主记录为 post-commit
  failure，保留已确认的核心成功结果；不能只凭调用过 `e.next()` 就吞掉异常。自定义路由中
  某一次 `$app.save()` 成功不代表整个路由已成功，后续异常仍按路由错误返回。

### 4.3 事件对象

所有事件包含：

```typescript
interface BaseEvent {
  readonly name: string
  readonly requestId: string | null
  readonly authMode: "system" | "request"
  context: Record<string, unknown>
  next(): Promise<void>
}
```

请求事件额外包含 `request`、`auth` 和 `response`；单记录事件包含 `record`、
`originalRecord`、`collection`，列表事件不能假定存在单个 `record`；Collection 事件包含
`collection` 和 `originalCollection`。创建时的 `original*` 为 null，其余 `original*`
是只读快照；请求身份只暴露经脱敏的数据，不能包含 `AuthIdentity` 内部的 token_key。

AuthRegister 提供 `e.profile`，允许在 next 前对顶层业务字段赋值或 delete；嵌套值是深拷贝，修改后须重新赋回。
`e.account` 在注册核心执行前为 null，执行后为只读账户视图；登录和刷新从一开始就提供只读账户。
Auth 请求的 json/text/bytes 只包含清理后的注册 profile，登录/刷新为 `{}`；headers/query/params 不复制认证输入。
afterCommit 快照仅含事件名、集合、requestId、只读账户以及注册时的最终 profile，不包含凭据或令牌。

Collection 事件通过 `e.collection.fields = [...]`、`indexes` 或 `rules` 赋回候选，嵌套读取同样返回深拷贝。
管理接口和 `$app.collections.findByName/list` 返回不可伪造的 `version`；save 必须携带读到的版本。
HTTP PATCH 仍接受原有增量字段/索引/规则格式，可额外携带 version 做乐观并发校验。未修改返回原版本。
已确认的数据库事务冲突返回 409；仅无法确认提交结果时进入核对流程。删除后的文件清理记录与 DDL 共用事务，
提交后执行，失败保留账本并阻止同名集合重建，启动时恢复。

已接通 API 的声明和类型测试位于 `packages/types`，运行示例见 [examples/js-runtime](../examples/js-runtime/README.md)。

Record Hook 在 `e.next()` 前的修改会在 Schema 校验和 API Rules 最终检查前写入候选
Record。请求体快照与候选 Record 必须分开保存：已有 Rules 中的 `$request.body` 保持
HTTP 适配层现有解析/规范化后的输入语义，不随 Hook 修改候选值而改变；`$record` 的新旧值
语义与现有 create/update Rules 对齐：create 检查最终候选，update/delete 的行条件检查
写入前记录，不能因 Hook 修改候选而变成另一套授权对象。公开 CRUD 的
核心写入始终使用原请求身份，不能被 Hook 的系统身份提升权限。

**Record 共同事务已接入，完整故障/平台验收仍在进行。** 克隆 `DbClient` 不会创建共同事务；现有
SchemaManager 的单次 `BEGIN ... COMMIT` 查询也不能直接作为跨 JS await 的事务句柄。
整条持久化 Hook 链完成后才提交，嵌套 save/delete 加入同一 owner 的真实事务；链中 `next()` 后的
异常仍会回滚。Mem/SurrealKv 的已通过用例和仍待完成的故障验证分别记录在实施验证报告中。

`e.next()` 仅表示下游处理完成，不天然代表最外层事务已提交。特别是 Hook 内的嵌套
`$app.save()`，其 `e.next()` 返回时外层仍可能失败。HTTP、邮件、实时推送及不可事务化文件
操作只能在宿主确认脱离事务后执行，不能仅根据源码位于 `await e.next()` 后就放行。
持久化事件提供 `e.afterCommit(callback): void`，在最外层提交确认后按顺序接收只读快照；
回滚丢弃，失败只记日志，受原调用剩余预算限制。请求事件没有此方法，显式事务使用
`tx.afterCommit`。可靠副作用使用 `$app.outbox.enqueue` 或 `tx.outbox.enqueue` 在同一事务入队，
独立宿主 worker 投递；afterCommit 本身不提供宕机可靠性。

Hook 接入需同时覆盖 JSON 与 multipart 写入，保留已有上传失败补偿；确认回滚后回收本次
新上传对象，提交结果 unknown 时保留并核对，不能直接按超时删除。具体顺序见第 16.1.2 节。
AuthService 的事件接入单独处理，不能靠 CRUD Hook 间接覆盖。嵌套 save/delete 触发持久化
Hook，不触发 HTTP 请求 Hook；默认深度上限 8，调用栈中同 collection/record 再次写入
返回 `HB_HOOK_RECURSION`，不得另行申请 worker。

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
  `/api/realtime`、`/api/events`、`/api-doc`、`/swagger-ui`、`/webui`、`/web` 及其子路径，允许额外前缀
  的配置也不能覆盖这些路径。检查必须按路径段进行，不能只判断完整字符串是否相等。
- 同一 method 的路由只要可能匹配同一具体路径就视为启动错误，例如 `/api/x/{id}` 与
  `/api/x/{name}` 或 `/api/x/latest`。参数路由也不能匹配到上述保留前缀。
- 路径参数使用 `{name}`；不接受任意正则路径。
- 路由处理器必须返回 `e.json`、`e.rawJson`、`e.text`、`e.html`、`e.file` 或 `e.noContent`
  的结果；`e.file` 为异步，参数仅接受扩展文件逻辑键。
- 请求体按 `HB_MAX_REQUEST_BODY_SIZE` 限制，响应体也受 JS 路由响应上限限制。

`$apis` 第一版提供 `requireAuth()`、`requireAdmin()`、`bodyLimit(bytes)` 和
`rateLimit(options)`。鉴权复用 AuthService；`requireAdmin()` 判断管理员身份，不能仅凭
普通用户的 role 字符串授权。鉴权中间件产生的身份放在 `e.auth`，不能由脚本伪造。
`bodyLimit` 只能缩小全局上限。原生中间件在用户 JS 前执行，`e.json` 使用现有成功 envelope，
原始 JSON 使用 `e.rawJson`，204 使用 `e.noContent`。路由默认 request 模式；显式选项使用
`routerAdd({ method, path, authMode, middleware }, handler)`。完整规则见第 16.1.5 节。

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

`$name` 过滤参数已通过受限 AST 的值参数节点绑定，不将用户值拼接为 SurrealQL。
RecordQuery 支持精确 limit/offset、投影和稳定排序，HTTP 继续支持 page/perPage。
JS delete 沿用 RecordManager 的软删除语义。Record 包装器区分字段未修改与 unset，
保存时只提交候选变更并保护系统字段。

根 Record/Collection/Auth Hook、任务和生命周期默认 system，自定义路由默认 request。
system 可绕过公共 API Rules，仍不能绕过 Schema、系统表保护和事务约束。Hook 可用第 4.1
节的对象选项声明 `authMode: "request"`；没有请求身份时按匿名规则执行，不回退 system。
子操作继承父操作的有效身份，子 Hook 的 system 声明不能提升 request 调用树的权限。
此选项只约束 `$app` 数据调用，内置请求核心操作始终使用原请求身份。任务和生命周期没有
request 模式。需要多次 CRUD 原子提交时使用 `$app.transaction(async (tx) => { ... })`；
事务内 `$app` 与 tx 的数据调用都加入同一事务，禁止嵌套事务，详见第 16.1.1 节。

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

`$app.db.query` 默认关闭；首版开启后也只在有效 system 模式下接受单条、单表、已登记
base Collection 的受限 SELECT。解析为 AST 后转换成同一 RecordQuery 读取模型；排除 Auth
集合、系统表、函数、子查询、图遍历、DML/DDL 和事务控制，不执行原始 SQL。动态值必须绑定，
返回 `[rows]`。这不是 SurrealQL 透传授权；不能证明第 16.1.4 节的边界时保持关闭。

## 7. 定时任务

```javascript
cronAdd("daily-report", "0 0 8 * * *", async () => {
  const records = await $app.findRecordsByFilter("reports", "sent = false")
  // ...
})

cronRemove("obsolete-job")
```

- 表达式使用含秒的 6 段 cron，支持数字、`*`、逗号列表、闭区间和 `/步长`；不接受宏、英文月份/星期、`?`、`L`、`W`、`#`。星期 0/7 均表示周日，各段按 AND 匹配。时区默认 UTC，可在任务选项中指定 IANA 时区。
- 任务名称全局唯一；重载时用新注册表原子替换旧调度，但仍在运行的旧任务保留原快照，
  同名任务的互斥状态必须跨快照保留，不能因重载获得第二个执行槽。
- 同一任务默认不并发执行；上一次未完成时跳过并记录 `warn`。
- 每次执行有独立超时和关联 ID。重试会重复执行已经成功的部分副作用，不保证 exactly-once；
  默认 retries=0，仅显式 idempotent=true 时允许最多 3 次重试，间隔 1/2/4 秒。
- 单机嵌入模式保证进程内 at-most-one 并发，不承诺宕机补偿。未来集群模式需要数据库租约。

完整注册签名为 `cronAdd(name, expression, handler, { timezone, maxRuntimeMs, retries, idempotent })`，
最后一个参数可省略；handler 接收只读 runId/attemptId/scheduledAt，不使用 next。cronRemove
只在初始化期间删除本候选版本先前注册的任务；运行期禁止修改注册表。DST 与重试互斥规则见第 16.1.6 节。

## 8. 邮件发送

```javascript
await $app.mailer.send({
  from: { address: "noreply@example.com", name: "HertaBase" },
  to: [{ address: user.get("email") }],
  subject: "Welcome",
  text: "Welcome to HertaBase",
  html: "<strong>Welcome to HertaBase</strong>",
  headers: e.requestId === null ? {} : { "X-Event-Id": e.requestId },
})
```

Rust `herta_core::Mailer` trait、`herta_mail` SMTP 实现及上述 JS 绑定已接入，真实邮箱验证仍待完成；
示例用于已脱离事务的回调。复用 `ApiState.mailer` 对应的服务实例，不经管理员 HTTP 接口转发。
JS 入参按现有 `MailMessage` 解码：from 可省略，to/subject 必填，text/html 至少一种非空；
只接受 `X-*` 字符串自定义头，未知字段（含 attachments/cc/bcc）拒绝。JS 不接触 SMTP 凭据。

先检查 `jsvm.mail.enabled`，再检查宿主服务是否可用，然后检查事务状态和入参。未授权为
`HB_CAPABILITY_DENIED`，授权但 driver=disabled 为已有 `HB_CAPABILITY_UNAVAILABLE`，
活动事务内为 `HB_SIDE_EFFECT_IN_TRANSACTION`，三者都不提交 SMTP。JS 限额只能收紧：
收件人数取 JS/宿主上限的较小值，text+html 的 UTF-8 字节数同理；主题、自定义头、发件人
继续由 `MailConfig::prepare` 校验。`max_bridge_bytes` 另行限制完整 FFI 入参，不把正文
字节数称为 MIME 邮件总大小。

Promise 返回现有 `MailReceipt` 的 JSON：`{ messageId, status: "accepted" }`，不含 HTTP
envelope；accepted 只表示 SMTP 接收端确认接收。现有校验、413、`HB_MAIL_SEND_FAILED`
和 `HB_MAIL_TIMEOUT` 错误保留。发送预算取调用剩余时长与 mail.timeout_ms 的较小值；
宿主跟踪在途发送的结果，超时或丢弃等待都不表示未发送，不自动重试。事务内可靠发送需写入
事务 outbox，提交后独立投递；直接发送可放在 afterCommit，但不保证宕机可靠性。

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
白名单项采用精确 origin，如 `https://api.example.com:443`，省略端口按 scheme 的默认端口；
拒绝用户信息、路径（空路径或 `/` 除外）、query、fragment 和通配符。仅配置 HTTPS 不授权
同一主机的 HTTP。每跳校验并固定连接 IP、禁用环境代理，细节见第 16.1.6 节。

当前桥接由 `herta_http` 实现，仅接受 GET/HEAD/POST/PUT/PATCH/DELETE/OPTIONS；GET/HEAD 不带正文。
body 为 UTF-8 字符串，响应为 `{ status, ok, headers, body, json() }`；非 2xx 仍返回响应，`json()` 同步解析，
无效 JSON 抛出 `BadRequestError`。响应不自动解压，无效 UTF-8 返回 `HB_HTTP_SEND_FAILED`。
Host、Content-Length、Transfer-Encoding 等传输头由宿主管理，CRLF、重复大小写头拒绝；跨源仅保留
Accept、Accept-Language、Accept-Encoding、Content-Type。301/302 的 POST 及 303 的非 HEAD 改为 GET，307/308 保留方法和正文。
整个 DNS/连接/重定向/读取过程共用剩余总预算；请求、响应、URL、头数量/字节和 DNS 地址数量均有上限。
`HB_HTTP_SEND_FAILED` 为 502，`HB_HTTP_TIMEOUT` 为 504，超限为 413；请求均不自动重试，超时可能已经产生远端副作用。

## 10. 实时事件

```javascript
await $app.realtime.publish("reports/ready", {
  reportId: report.id,
}, { users: [{ collection: e.auth.collection, id: e.auth.id }] })
```

`publish(topic, data, audience)` 已接入独立的应用消息总线。
示例要求 e.auth 非匿名；客户端使用新增 `GET /api/events?topic=...` 与独立的
`hb.realtime.subscribe(topic, options)`，必须登录，一个连接订阅一个精确 topic，无断线重放。
现有 `/api/realtime/{collection}` 的协议保持兼容；只实现发送端不能算完成功能。topic 必须符合
`[A-Za-z0-9][A-Za-z0-9._/-]{0,127}`，消息大小和发布速率受限。audience 必填且只接受一种
非空目标列表：`users: [{ collection, id }]`、`roles: [{ collection, role }]` 或
`connections: [connectionId]`；省略返回 `HB_CAPABILITY_DENIED`，混用/空列表为校验错误，
首版不开放全局广播。管理员角色使用 `_admins` 命名空间。记录 CRUD 的标准实时事件由核心
自动产生，JS 只负责业务事件。投递校验与慢消费者处理见第 16.1.6 节。
返回 `{ queued, dropped }`，分别表示入队及因撤销/过期/慢消费关闭的连接数；它不是客户端处理确认。
队列中的消息在出队前再次校验当前身份和 audience；不匹配的消息丢弃，撤销和到期会关闭连接。

## 11. 文件操作

```javascript
await $app.files.write("exports/report.json", JSON.stringify(report), {
  contentType: "application/json",
})
const content = await $app.files.readText("exports/report.json")
const entries = await $app.files.list("exports")
await $app.files.remove("exports/report.json")
```

首版统一复用当前配置的 Storage，逻辑键相对于 `jsvm.files.prefix`（默认 `extensions`），
不提供独立本地 root 模式或后端自动回退。不能访问任意宿主路径、记录附件、网页部署或系统目录。
异步 `readBytes` 返回 Uint8Array，write 接受 Uint8Array 或字符串，另提供 `readText`、
`exists`、`stat`、`list`、`copy`、`move` 和 `remove`。拒绝绝对路径、`..`、设备路径、
符号链接/junction 逃逸和超限文件；本地访问必须以受控目录句柄防止路径替换竞态。
list 使用有界分页。move 为 copy + delete，删除失败必须报告目标已存在的可恢复错误；
总配额按第 16.1.6 节的账本预留和恢复，不承诺 Storage 跨键原子性。

`list(prefix, {limit, cursor})` 返回 `{items, nextCursor}`，默认 100、最多 500。
items 包含 `key/size/contentType/version/updatedAt`；prefix 是字面前缀，可使用 `exports/`。
cursor 绑定命名空间和 prefix，采用 keyset 分页，不冻结目录快照。
每次 write/copy 分配新物理版本，目录切换与旧版本待清理标记共同提交。清理失败保留旧占用，
服务器启动按每批 100 条账本恢复；PUT/COPY 失败且对象仍未出现时保守保留预留，不能仅凭一次 HEAD 404 释放配额。
`HB_FILE_MOVE_PARTIAL` 的 details 返回 destination 元数据、`destinationExists: true` 和
`sourceState: present|absent|unknown`。后续恢复可能完成源删除，调用方应重新查询状态。

### 11.1 事务 outbox

```javascript
await $app.transaction(async tx => {
  await tx.save(tx.newRecord('audit', { message: 'queued welcome' }))
  await tx.outbox.enqueue('mail.send', {
    to: [{ address: 'reader@example.com' }], subject: 'Welcome', text: 'Hello',
  }, { idempotencyKey: 'welcome-001' })
})
```

仅支持 `mail.send` 与 `http.send`，payload 复用相应 DTO；没有活动事务时 enqueue 自行建立事务。
幂等键限 1–128 个 ASCII 字母、数字或 `-_.:`，作用域为应用内 `(kind, key)`。
相同规范化 payload 复用已有任务，不同 payload 返回 `409 HB_IDEMPOTENCY_CONFLICT`；事务中的失败仍标记 rollback-only。
回执只含 jobId、kind、state、attempts、毫秒时间戳、errorCode 和宿主 result，不含 payload 或凭据。

worker 在 serve 成功后启动，按 60 秒租约、20 秒续租投递；可确定安全的失败最多重试三次，
延迟 1/2/4 秒。`leased` 尚未发送，过期可重新排队；`sending` 过期进入 unknown。
SMTP 错误/超时不自动重发。HTTP 的明确未发送临时失败可重试；不确定结果只有目标 origin
列在 `jsvm.outbox.idempotent_origins` 才可重试，此时宿主设置稳定 `Idempotency-Key` 并禁止跳转。
收到 HTTP 响应即记录 accepted 和 status，accepted 不表示业务成功；SMTP accepted 也不表示最终送达。

管理员 `GET /api/admin/outbox?limit=100&cursor=...`、`GET /api/admin/outbox/{id}` 查询回执；
`POST /api/admin/outbox/{id}/resolve` 接受 `{resolution: "accepted"|"not_sent", note: "核对依据"}`。
仅 unknown 可核对；not_sent 要求运维确认原尝试已停止且未被接收，之后重新排队。核对保留管理员身份和说明。
accepted/failed 默认保留七天；pending/unknown 不自动清除。shutdown 停止接收新投递并有界排空，
未收敛发送保留持久化租约，不能当作未发送自动重试。

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
- 固定专用 worker 数量，每次根调用创建独立 runtime/context，正常结束也整体销毁；
  只共享不可变源码快照和 Rust 服务句柄，字节码缓存属于后续可选优化。
- pool_size 限制并行根调用，有界队列限制等待调用。嵌套 Hook 在原 worker/runtime 续接，
  复用身份、事务与剩余预算，不再次申请执行槽。
- 内存、栈、累计 JS 活跃片段墙钟时间、异步总时长、宿主 I/O 数量和返回数据量分别限额。
  等待宿主 I/O 不消耗活跃执行预算；不宣称它是 OS CPU 用时，也不承诺计量全部 Promise 数量。
- 超时会停止调度新宿主调用、发出取消请求并丢弃该 JS context；context 不返回池中复用。
  丢弃 Rust future 不证明数据库回滚、HTTP 未送达或邮件未发送。提交结果不确定时必须记录
  不确定状态，禁止当作“未执行”自动重试，并由宿主负责上传补偿和资源释放。
- Hook 注册表使用不可变快照。热重载期间已开始的请求继续使用旧快照，新请求使用新快照。
- 禁止脚本保存请求、Record 或事务对象供下次执行使用。

**闭包与上下文隔离已有 Windows 回归用例。** 启动 context 中的 Function 保留其创建环境，传递函数引用
不会生成独立的请求闭包，也不能将它当作跨 runtime 共享的缓存。快照只保存源码、哈希及注册
描述，每次调用在新 runtime/context 重放纯初始化以建立本地闭包，并核对注册描述一致性；这意味着
顶层计算可能重复执行，不能承诺“顶层只执行一次”或支持跨请求全局变量。重放期间禁止 I/O、
动态增删正式注册项和基于时间/随机数改变注册清单。字节码缓存不能替代闭包实例化。

池容量必须按 runtime/worker 的真实内存计费边界计算；共享同一 runtime 的多个 context
不能被误认为各自拥有独立堆配额。队列满、等待超时、嵌套 Hook 重入和 worker 退出时的
处理必须在技术验证中固定，特别要避免 pool_size=1 时内层 save 等待自身占用的执行槽。

## 14. 错误模型

宿主错误在 JS 中表现为带 `code`、`message`、`details` 的 Error 子类。下列错误码已接入共享
错误模型和 JS 桥接，`HB_CAPABILITY_UNAVAILABLE` 使用通用服务不可用文案。
复用既有错误时保持现有 HTTP 状态，不把邮件错误统一折叠成 Hook 错误。

| 错误码 | 含义 |
| --- | --- |
| `HB_SCRIPT_LOAD_ERROR` | 启动/重载诊断；扩展加载或注册失败 |
| `HB_HOOK_ERROR` | 500；Hook 未处理异常 |
| `HB_HOOK_TIMEOUT` | 504；调用执行超时 |
| `HB_HOOK_OOM` | 500；调用超过 JS 堆内存限制 |
| `HB_ROUTE_CONFLICT` | 启动/重载诊断；自定义路由冲突 |
| `HB_CAPABILITY_DENIED` | 403；当前调用未获能力授权 |
| `HB_CAPABILITY_UNAVAILABLE` | 503；依赖服务尚未启用 |
| `HB_OUTBOUND_DENIED` | 403；HTTP 目标未通过安全策略 |
| `HB_HTTP_SEND_FAILED` | 502；出站连接、TLS、传输或 UTF-8 响应失败 |
| `HB_HTTP_TIMEOUT` | 504；出站总预算耗尽，远端结果可能不确定 |
| `HB_FILE_ACCESS_DENIED` | 403；文件路径或操作越权 |
| `HB_HOOK_ABORTED` | 409；持久化/Auth 链未调用 next，核心操作拒绝 |
| `HB_HOOK_RECURSION` | 409；同记录递归或嵌套深度超限 |
| `HB_JS_BUSY` | 503；执行队列已满或排队超时 |
| `HB_QUERY_UNSUPPORTED` | 400；查询超出受限 SELECT 子集 |
| `HB_SIDE_EFFECT_IN_TRANSACTION` | 409；活动事务内直接执行外部副作用 |

公开宿主错误的 code/status 来自不可伪造的桥接标记；脚本给普通 Error 设置同名 code 属性
不能冒充提交状态或宿主错误。公开业务错误通过已提供的 Error 子类构造。

生产环境对外隐藏普通异常堆栈；完整异常、脚本位置和 cause chain 只进入受控日志。

## 15. 配置模型

以下列出目标默认值；空白名单意味着没有目标获准。`jsvm.enabled` 默认 false，升级不会
自动执行磁盘上已有脚本；开启后 cron.enabled 默认 true，外部能力仍需分别授权。

```toml
[jsvm]
enabled = false
memory_limit_mb = 16
stack_limit_kb = 512
execution_timeout_ms = 100
async_timeout_ms = 5000
startup_timeout_ms = 10000
queue_timeout_ms = 1000
shutdown_timeout_ms = 30000
pool_size = 4
queue_capacity = 128
max_response_bytes = 4194304
max_bridge_bytes = 4194304
max_host_buffer_bytes = 16777216
max_pending_host_calls = 32
max_host_calls = 256
max_hook_depth = 8
max_logs = 100
max_log_bytes = 65536
route_prefixes = ["/api/"]
raw_query_enabled = false
env_allowlist = []

[jsvm.http]
enabled = false
allowlist = [] # 例如 ["https://api.example.com:443"]
max_redirects = 3
max_request_bytes = 1048576
max_response_bytes = 4194304
connect_timeout_ms = 3000
timeout_ms = 5000

[jsvm.files]
enabled = false
prefix = "extensions"
quota_bytes = 104857600
max_file_bytes = 10485760

[jsvm.mail]
enabled = false
max_recipients = 20
max_body_bytes = 1048576

[jsvm.realtime]
enabled = false
max_message_bytes = 65536
publish_per_second = 100
max_audience = 100
connection_queue_capacity = 64
connection_queue_bytes = 262144

[jsvm.cron]
enabled = true
timezone = "UTC"
max_runtime_ms = 30000
retries = 0
max_retries = 3

[jsvm.outbox]
enabled = false
lease_seconds = 60
renew_seconds = 20
max_retries = 3
retention_days = 7
concurrency = 4
max_jobs = 10000
idempotent_origins = []

[mail]
driver = "disabled"
from_address = "noreply@example.com"
from_name = "HertaBase"
```

`jsvm` 环境变量将字段路径去掉 `jsvm.`，点替换为 `_`、全大写并加 `HB_JS_`：
例如 `jsvm.mail.max_body_bytes` 对应 `HB_JS_MAIL_MAX_BODY_BYTES`。所有字段一一映射，
环境变量覆盖 TOML；布尔值使用 true/false，列表使用 JSON 数组字符串。未知 jsvm 字段、
非法范围和不存在的 IANA 时区拒绝启动。密钥只进入 Rust 服务配置，不能通过 `$app.env()` 读取。

`mail`、`mail.smtp` 已实现，完整默认值见 [配置参考](configuration.md)。保留既有
`HB_MAIL_*` / `HB_SMTP_*` 名称及 `HB_MAIL_ALLOWED_FROM_ADDRESSES` 的逗号列表格式；
JS 的列表新规则不改变现有邮件配置。当前 MailConfig/SmtpConfig 没有 deny_unknown_fields，
不能声称已拒绝未知邮件配置；本阶段严格校验新增 jsvm 字段，邮件配置兼容性变更单独处理。

以上 `jsvm` 部分不是当前可生效配置。配额与时间均作用于整个根调用，嵌套 Hook 不重置；
max_pending_host_calls 是在途数量，max_host_calls/max_logs/max_log_bytes 是累计数量。
max_host_buffer_bytes 是同时持有的宿主缓冲预留上限，释放后可复用；完整序列化临时缓冲也要计入。
响应上限和桥接上限分别限制每次响应与单次 FFI 入/出参，Uint8Array 按实际字节计费。
memory_limit_mb/stack_limit_kb 分别按 MiB/KiB 换算，其他 *_bytes 为字节，*_ms 为毫秒。
startup_timeout_ms 限制完整候选验证，单次重放还受根调用预算；cron 的 max_runtime_ms
替代 async_timeout_ms，所有子 I/O 取剩余预算与自身上限的较小值。读取 10 MiB 文件仍需满足
默认 4 MiB 桥接上限；max_file_bytes 是存储上限，不保证该大小可在单次 JS 调用中传输。
`env_allowlist` 只应列入公开值，宿主凭据项即便在列表中也拒绝。enabled=false 不加载脚本。

## 16. 实施顺序

### 16.1 目标方案与验证要求

以下六项是采用的实施方案，正文与开发指南同步遵循。事务、宿主服务、执行生命周期、配置
和错误码已接入；具体测试及剩余缺口见第 16.3 节及 [验证记录](js-runtime-validation.md)。
源码查证不等于集成测试通过，只有实际落地部分才能进入类型包。

#### 1. 事务与提交边界

**问题：** 在当前 SurrealDB 3.2.3、Mem/SurrealKv 后端验证跨多次异步调用的事务方案，
证明 read-your-writes、嵌套写入失败回滚和超时取消行为，不能用多个自动提交查询模拟共同事务。

**判断与依据：可以接入现有 SDK，无需更换数据库或自行实现事务引擎。** 已查阅本机锁定版本
源码：`surrealdb-3.2.3/src/method/transaction.rs` 提供 `Transaction<C>` 及 query/commit/cancel；
`src/method/mod.rs` 提供 `Surreal::begin(self)`；`src/engine/local/mod.rs` 按事务 ID 执行查询。
上游 `tests/api_integration/basic.rs::client_side_transactions` 也包含提交、取消及多次写入用例。
本项目现已通过 DbSession 和独立 TransactionOwner 接入上述句柄；远程 HTTP 数据库连接不在本次支持范围内。

**采用方案：**

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
  有界重试与待核对状态；它不承诺无条件 at-least-once，结果不确定且接收端不保证幂等时停在 unknown。
- 事务由独立于请求 future 的宿主 owner 管理，状态为 active、committing、committed、
  rolling-back、rolled-back 或 unknown。超时/断连只通知 owner；未提交时显式等待 cancel，
  提交已发出时继续收取结果。不能依赖 Drop 自动回滚，也不能因丢弃 commit future 就宣告失败。
  结果未知时不运行 afterCommit、不自动重放业务操作，并由后台核对提交标记后再做补偿。
  为此，将 operationId 提交标记写入同事务的宿主系统表；仅在提交任务已结束且新的独立查询
  能确认结果时收敛状态，不能在 commit 仍进行中因为暂时查不到标记就判定回滚。
- multipart 新对象的清理也归 owner：确认回滚后删除，确认提交后保留；unknown 时保留并
  待核对，避免删掉实际已提交记录引用的文件。旧文件删除、OpenAPI 刷新都在提交后执行。
  Collection Schema 变更使用单独的顶层事务，首版拒绝在 Record 事务中混入 Schema 变更。

事务边界如下；请求 Hook 中独立发起的写入不隐式并入图中的 CRUD 事务：

```text
请求 Hook 前置
  → begin → 持久化 Hook 前置 → 核心写入 → 持久化 Hook 收尾 → commit
  → afterCommit → 请求 Hook 收尾 → HTTP 响应
```

持久化链内发送邮件必须显式注册提交后动作，例如业务订阅记录创建后发信：

```javascript
onRecordCreate(async (e) => {
  e.afterCommit(async (committed) => {
    await $app.mailer.send({
      to: [{ address: committed.record.get("email") }],
      subject: "Welcome",
      text: "Your subscription is ready.",
    })
  })
  await e.next()
}, "newsletter_subscriptions")
```

**验收重点：** 两个后端分别测试事务内可见/事务外不可见、两次写入一起回滚、嵌套异常被捕获
后仍不能提交、写冲突、提交期间断连，以及提交确认前无外部副作用。还需验证显式事务提交对
现有 LIVE SELECT 的通知行为，保留已有快照校验；不能仅凭上游基本事务用例认为全部通过。

#### 2. 可执行的事件契约

**问题：** 固定各事件字段、无 next 的结果、未等待 next 的处理、authMode 注册签名与
嵌套继承、递归深度，以及生命周期时点；Auth 不得暴露密码、密钥或破坏令牌轮换。

**采用方案：统一事件状态机，避免每类 handler 自行解释 next。**

- 保留 `onRecordCreate(handler, ...collectionNames)` 简写；新增互斥的对象形式
  `onRecordCreate(handler, { collections: ["posts"], authMode: "request" })`，不混用两种参数。
  Collection/Auth Hook 使用同样选项；应用生命周期 Hook 不接受 collections 或 request 模式。
- Record/Collection/Auth 的根 Hook 默认 system，自定义路由默认 request，任务和生命周期
  默认 system。路由保留已有简写，并新增 `routerAdd({ method, path, authMode, middleware }, handler)`。
  默认值与第 6.2 节一致，不存在所有入口默认 system 的另一套规则。
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
- JSON/multipart 先做身份解析、正文/文件大小及类型检查、附件引用防伪；multipart 生成候选
  附件引用和固定的 request.body 快照后进入与 JSON 相同的 Hook 链。当前 `preflight_*`
  同时校验必填字段和 Rules，不能原样放在 Hook 前，否则补填必填字段会提前失败。
  将其拆为输入安全检查与 Hook 后的最终 Schema/Rules 校验；权限相关读取仍使用原请求身份。
  最终校验与写入都在共同事务中执行，校验成功后由宿主上传并写记录，持久化 Hook 只执行一次。
  上传是宿主核心操作的一部分，不授予 JS 在事务内直接写文件的能力。跟踪上传引用的归属，
  Hook 不能伪造或改指向其他记录的附件；被候选舍弃的新上传对象也必须清理。
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
JSON/multipart 补填必填字段与 Rules 输入一致性、上传后拒绝/超时的 owner 清理、Auth 注册
回滚及刷新令牌重放，以及启动 Hook 失败时端口不对外提供服务。

#### 3. 运行时验证

**问题：** 固定闭包重建、worker/runtime 关系、资源计费与重入方式，并验证中断、OOM 和
Windows/部署平台上的行为。

**采用方案：固定 worker 数量，每次根调用创建独立 runtime/context。**

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
  排队项只持有请求描述，获得执行槽及缓冲预算前不读取完整正文；运行期对宿主缓冲按字节
  预留，限制同时持有的字节数，释放后归还，超限拒绝分配。宿主原生函数不得 panic、
  无限分配或同步执行无界工作。
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

**采用方案：业务写入统一走 Record 服务，原生接口首版只接受可编译到同一读取模型的 SELECT。**

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
  `403 HB_CAPABILITY_DENIED`。清理任务使用参数化查找加软删除，见第 3 节。
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

**采用方案：业务 JSON 默认遵循现有 envelope，显式保留原始响应接口。**

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

**采用方案：复用已有基础设施，新增能力同时实现发送/接收或写入/恢复。**

**实时消息：** 新增宿主 RealtimeBus 与 `GET /api/events?topic=...`，保持原集合 SSE 不变。
首版一个连接订阅一个精确 topic，必须登录；沿用现有连接/IP 配额、心跳、过期关闭及清理机制。
返回 connected/message/ping/error 事件，message 为 `{ id, topic, data, timestamp }`，
不保证断线重放。SDK 新增独立的 `hb.realtime.subscribe(topic, options)`，不复用记录 CRUD
的事件模型。每个连接的发送队列有界，慢消费者断开，不阻塞发布者；队列已满时 error 事件
只能尽力发送，客户端必须能处理没有末尾 error 的断连。

audience 首版必须显式提供，且一次仅接受 users、roles、connections 中的一种：users 使用
`{ collection, id }`；roles 使用 `{ collection, role }`，管理员角色归 `_admins`；connections
使用宿主签发的连接 ID。宿主在投递时核对连接的 topic、当前身份、到期与目标匹配，JS 不能
自行提交一个“已验证身份”。省略 audience 返回能力拒绝，首版不开放全局广播；按第 10 节
使用带 collection 的结构，避免多 Auth Collection 间 ID/角色撞名。publish 的结果只
表示已进入符合条件连接的队列，不表示客户端已处理；需要可靠消费的业务另用持久化消息设计。

**文件：** 首版统一复用当前配置的 Storage，以 `extensions/` 作为专用前缀；不再另建独立
的任意本地 root 模式。第 15 节 prefix 须校验，禁止与 records/ 等宿主保留
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
Mailer 复用第 8 节已有 Rust 实现，其生产 TLS、收件人/正文/头字段及 CRLF 校验无需重写。
两类发送及实时/文件写入都检查宿主事务状态，事务内返回 `HB_SIDE_EFFECT_IN_TRANSACTION`；
outbox.enqueue 只落库，可在事务内调用。outbox 发送也必须经过相同的能力与目标校验。

默认配置及环境变量映射集中在第 15 节。权限开关先于适配器可用性检查：未授权报 DENIED，
已授权但无服务报 UNAVAILABLE；第 8 节明确了与已有邮件配置的兼容边界。
配置上限是部署者可调整的初值，压力测试后再优化，不作为未经测试的吞吐量承诺。

**验收重点：** 多 Auth Collection 的 audience 隔离、慢消费者、JWT 过期；并发覆盖写入、
重启后配额恢复、copy 成功/delete 失败；DST 跳时/重复时刻、重载期间任务不重叠及有副作用的
重试幂等；HTTP 重定向、DNS rebinding 和事务内外部调用拒绝。

**落地顺序：** 先做事务适配和 JS worker 的独立技术验证，再完成 Record/事件纵向链路，
随后加入路由与 SDK、Collection/Auth，最后加入 outbox、外部服务、应用消息与任务。每一批
通过自身验收再扩展；全部实现前保持设计草案状态，类型包只收录实际落地部分。

### 16.2 分步交付

1. 先完成 Mem/SurrealKv 事务适配与 QuickJS worker 的独立技术验证；再建立 `herta_jsvm`
   crate、配置、错误类型、受限脚本发现、源码快照和原子注册表。字节码缓存不作为首版前置条件。
2. 完成共享 DTO/宿主契约、日志与 Record 持久化链，接入 JSON/multipart CRUD 与事务 owner。
3. 完成自定义路由、请求/响应桥接与 auth 中间件，再接入 CollectionService/AuthService 事件。
4. 完成调度器、受限 HTTP client，以及对已有 Mailer 的 JS 桥接；外部副作用只能在宿主确认
   脱离事务后执行，可靠投递采用新增事务 outbox 服务，不重复实现邮件驱动或管理员发送接口。
5. 复用现有 Storage 并补齐扩展文件适配器；新增应用消息总线与客户端订阅桥接，保持现有
   集合 SSE 协议兼容。
6. 增加 `@hb/types`、示例、热重载、配额压力测试和沙盒安全测试。

每一步覆盖与其相关的正常路径、超时、OOM、权限拒绝、重载并发和宿主服务失败测试。只有服务端实际
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

### 16.3 验证范围与剩余门槛

以下区分已经执行的 Windows 用例与仍需完成的验收。完整统一入口的结果单独记录在
[验证记录](js-runtime-validation.md)，未执行的 Linux/macOS 不计入通过项。

| 开放范围 | 已有通过用例 | 剩余门槛 |
| --- | --- | --- |
| CRUD Hook | Mem/SurrealKv 事务内可见性、异常回滚、冲突、LIVE 提交通知；真实引擎 commit 内取消/等待者丢弃；SurrealKv 提交入口与完成边界强制终止后核对；JSON/multipart 一次 Hook、补填、Rules 原始输入；上传/Collection 清理重试与进程恢复 | Windows 统一回归通过；未模拟断电或 WAL 扇区损坏 |
| JS worker | 固定 rquickjs 0.11.0；同步/Promise 循环、OOM、栈溢出、重放、pool_size=1 重入、关闭、快照配额；有界宿主通道、finish 有界等待；请求/文件二进制通道、取消后的缓冲保留、JS 堆计量 | 宿主适配器内部与 JSON 值树的完整临时分配计量；读取正文前的执行槽/缓冲接纳 |
| HTTP/Auth | 注册双链、密码一次处理、凭据隔离、可变性、嵌套权限、HEAD/OPTIONS、SDK envelope/204、提交 unknown 核对；两个后端的签发/撤销冲突、Schema 迁移；真实 Windows 进程生命周期 | 本轮 Windows 统一回归通过 |
| 邮件 JS 桥接 | 拒绝路径 send=0、回滚不发信、afterCommit、限额/超时不重试、本地 SMTP 收件及 Message-ID 核对 | 本轮 Windows 统一回归通过 |
| raw SELECT / HTTP | AST 白名单、参数/投影/敏感字段；逐跳固定 IP、TLS 原主机/降级拒绝、流限额、代理隔离；outbox 丢失响应/稳定幂等键的本地真实接收端联合测试 | 本轮 Windows 统一回归通过 |
| 文件 | 逻辑/物理分页、部分 move DTO、完整覆盖配额、SurrealKv 崩溃恢复、Windows junction、S3 协议故障；管理员核对释放；孤儿扫描与失败删除的配额账本；单目录超过 10,000 项清理 | 实际 S3 部署验收未执行 |
| 消息 / cron / outbox | 消息身份撤销/过期/慢连接；六段 AND 与 DST；outbox 共同回滚、幂等冲突、租约/重试/保留、管理员核对、HTTP 接收端只执行一次；Windows 进程停服 | 本轮 Windows 统一回归通过 |

outbox 与 cron 不得把 SMTP accepted 解释为最终送达，也不得把发送超时解释为可以安全重发；
具体去重与重试方案采用第 16.4 节。所有未开放能力返回明确错误，类型包不提前声明。

### 16.4 已固定的实施契约

以下约定已确定，替代前文中对应的待决选项；实现及测试完成状态单独记录，不据此推定功能已开放。

| 项目 | 固定约定 |
| --- | --- |
| 注册链 | `AuthRegister → RecordCreate → 核心创建` 共用事务。密码只由 AuthService 处理一次，密码、凭据和令牌不进入事件、请求读取接口或提交快照。 |
| 可变性 | Record/Collection create、update 的候选只在 next 前可写；delete 及核心执行后的视图只读。AuthRegister 只可修改清理后的 profile；登录、刷新账户视图只读。嵌套值返回深拷贝，保留的事件引用不会重新变为可写。 |
| 事件结束 | 请求事件可返回 Response；无 next 且无 Response 为 500。持久化/Auth 无 next 为 409 并回滚。Record/Collection/Auth 事务事件允许 afterCommit，请求事件不提供。 |
| 提交不确定 | `503 HB_COMMIT_UNKNOWN`，返回 operationId、checkUrl 和随机 checkCredential。`GET /api/operations/{id}` 凭原身份的 Bearer token 或 `X-HB-Operation-Credential` 请求头仅返回状态，绝不返回记录或 Auth 令牌。核对凭证不放入 URL。提交标记与业务写入同事务；持久化 owner 日志负责重启核对。前缀 `/api/operations` 保留。 |
| 文件分页 | `list(prefix, {limit,cursor}) → {items,nextCursor}`，默认 100，最大 500。条目含 key、size、contentType、version、updatedAt。 |
| 文件移动 | copy 后 delete。部分失败返回 `409 HB_FILE_MOVE_PARTIAL`，明确目标已存在和源对象的已确认状态。覆盖写入预留完整新对象大小。 |
| 消息订阅 | connected 含 connectionId/topic/timestamp；message 使用第 10 节 DTO。options 为 onEvent/onStatus/onError/reconnect/signal，无断线重放。publish 返回入队及丢弃连接数量。 |
| cron 日历 | 严格六段；秒、分、时、日、月、星期全部按 AND 匹配。DST 不存在时刻跳过，重复时刻只运行较早一次。任务名互斥跨快照保留，重试等待也持有互斥权。 |
| outbox 幂等 | 首批任务为 `mail.send`、`http.send`，复用对应请求 DTO；作用域为应用内 `(kind,idempotencyKey)`。同键同 payload 返回既有任务，不同 payload 返回 409。入队和投递均校验能力与目标。 |
| 不确定投递 | SMTP 和未获接收端幂等保证的 HTTP 进入 unknown 待核对，不自动重发。HTTP 只对运维配置的幂等 origin 允许不确定结果重试。 |
| outbox 运维 | 默认关闭；租约 60 秒，每 20 秒续租。明确可安全重试的失败最多重试 3 次，间隔 1/2/4 秒。管理员 list/get/resolve；unknown 由运维核对为已接受或明确未发送后处理。终态保留 7 天，待处理和 unknown 不自动清除。 |
| 快照额度 | 默认最多 128 个脚本、单文件 1 MiB、总源码 8 MiB、1024 注册项、4 个存活快照（含候选）。额度不足保留当前版本，旧快照释放后才可重载。 |
| 配置和平台 | jsvm.enabled 默认关闭，外部能力分别授权；新增字段严格校验并映射 HB_JS_*。启用而目录不存在时启动失败，明确存在的空目录允许。Windows x64/MSVC 实测验收，Linux/macOS 仅在实际执行后记为通过。 |
