# HertaBase JavaScript 扩展开发指南

> Phase 3 尚在开发，本指南定义目标 API，不代表当前二进制已经实现这些接口。运行时架构、
> 安全边界与实施顺序见 [JavaScript 扩展运行时设计](js-runtime.md)。2026-09-08 已统一目标
> 契约；尚未通过的事务、闭包隔离和跨服务开放门槛见该文档第 16.3 节，示例不是可运行承诺。

## 1. 扩展文件

将普通 JavaScript 文件放入 `hb_hooks/`。服务启动时会按相对路径字典序加载全部 `*.js`
并建立不可变注册表。文件名只用于排序和错误定位，不再使用
`before_create_posts.js` 之类的命名推断 Hook。

推荐按领域组织文件：

```text
hb_hooks/
├── 00-bootstrap.js
├── posts.js
├── routes/
│   └── reports.js
└── jobs/
    └── cleanup.js
```

首次加载时，语法错误、重复路由或重复任务名会阻止服务启动。开发模式重载失败时保留上一次
成功加载的完整注册表，并输出带脚本位置的错误；不跳过个别失败脚本或保留其部分注册项。
顶层仅用于注册与纯计算，不得执行宿主 I/O，也不能依赖顶层只执行一次或全局变量跨请求存活。

## 2. Event Hooks

扩展使用全局注册函数声明 Hook。处理器接收事件对象 `e`，通过 `await e.next()` 继续后续
Hook 和核心操作：

```javascript
onRecordCreate(async (e) => {
  const title = e.record.get("title")
  if (!title) {
    throw new BadRequestError("title is required")
  }

  e.record.set("slug", title.toLowerCase().replaceAll(/[^a-z0-9]+/g, "-"))
  await e.next()
}, "posts")
```

同一事件可以注册多个处理器，按脚本加载顺序和脚本内注册顺序执行。正常返回且没有调用
`e.next()` 表示有意终止处理链，不代表写入成功；请求事件必须提供有效响应，持久化事件的
拒绝结果为 `409 HB_HOOK_ABORTED` 并回滚。每个处理器最多调用一次 next，必须等待其完成。不要再使用旧版设计
中的 `return false` 或全局 `context`。

常用注册函数包括：

- Record 持久化：`onRecordCreate`、`onRecordUpdate`、`onRecordDelete`。
- Record API：`onRecordListRequest`、`onRecordViewRequest`、
  `onRecordCreateRequest`、`onRecordUpdateRequest`、`onRecordDeleteRequest`。
- Collection：`onCollectionCreate`、`onCollectionUpdate`、`onCollectionDelete`。
- Auth：`onAuthLogin`、`onAuthRegister`、`onTokenRefresh`。
- 应用：`onBootstrap`、`onServe`、`onShutdown`。

Record Hook 注册函数末尾可传一个或多个 Collection 精确名称。不传表示监听所有
Collection。

`await e.next()` 之前的代码运行在核心操作前，可以修改候选 Record 或拒绝操作；之后的代码
只在下游成功时运行。**整条持久化链完成后才提交**，`next()` 后的代码仍在事务内；邮件、HTTP
和实时通知使用 `e.afterCommit(callback)`，宿主确认提交后才执行。已确认提交后的回调失败不能回滚数据，
宿主将其记录为 post-commit failure；需要可靠投递时应使用同事务 outbox 与独立投递任务。
当前数据层尚无供 Hook 连续调用的事务句柄或通用 outbox 服务。

## 3. Record 与数据库操作

事件中的 `e.record` 是 Record 包装器：

```javascript
const title = e.record.get("title")
e.record.set("published", true)
e.record.unset("draftNote")

$app.logger.debug("record changed", {
  id: e.record.id,
  originalTitle: e.record.original("title"),
  record: e.record.toJSON(),
})
```

常用数据库操作使用 `$app` 的 Record API：

```javascript
const author = await $app.findRecordById("users", e.record.get("authorId"))
const posts = await $app.findRecordsByFilter(
  "posts",
  "authorId = $author AND published = $published",
  "-created_at",
  20,
  0,
  { author: author.id, published: true },
)

const audit = $app.newRecord("audit_logs", {
  action: "post.create",
  recordId: e.record.id,
})
await $app.save(audit)
```

带 `$name` 的过滤器、任意 limit/offset 和 Record 包装器是待新增接口。delete 沿用当前
RecordManager 的软删除语义。Collection 管理通过统一宿主服务完成，除 SchemaManager
外还必须调用 OpenAPI 刷新与集合文件清理；仅直接调用 manager 不足以完成这些动作。

原生查询默认关闭，首版仅允许有效 system 模式对 base Collection 执行单条受限 SELECT：

```javascript
const result = await $app.db.query(
  "SELECT * FROM posts WHERE status = $status LIMIT $limit",
  { status: "published", limit: 10 },
)
```

所有动态值必须使用变量绑定。根 Record/Collection/Auth Hook、任务和生命周期默认 system，
自定义路由默认 request；system 仍不能绕过 Schema、系统表保护或事务限制。子 Hook 不能
提升 request 调用树的权限。显式模式使用 `onRecordCreate(handler, { collections: ["posts"],
authMode: "request" })` 或 `routerAdd({ method, path, authMode, middleware }, handler)`。
raw SELECT 解析后转入同一读取模型，禁止 Auth/系统表、函数、子查询和 DML/DDL，返回 `[rows]`。

## 4. 自定义路由

```javascript
routerAdd("POST", "/api/reports/{id}/run", async (e) => {
  const input = await e.request.json()
  const report = await $app.findRecordById(
    "reports",
    e.request.pathValue("id"),
  )

  return e.json(202, {
    reportId: report.id,
    format: input.format ?? "pdf",
  })
}, $apis.requireAuth(), $apis.bodyLimit(64 * 1024))
```

路径参数使用 `{name}`。自定义路由不能覆盖 HertaBase 内置 API。可用响应包括 `e.json()`、`e.rawJson()`、
`e.text()`、`e.html()`、`e.file()` 和 `e.noContent()`；可用中间件包括
`$apis.requireAuth()`、`requireAdmin()`、`bodyLimit()` 和 `rateLimit()`。
`e.json` 返回现有成功 envelope，业务错误抛公开 Error；原始 JSON 使用 e.rawJson。
文件响应写作 `return await e.file(key)`，仅接受扩展逻辑键；204 用 e.noContent，SDK 返回 undefined。

## 5. 定时任务

```javascript
cronAdd("remove-expired-drafts", "0 0 3 * * *", async () => {
  const drafts = await $app.findRecordsByFilter(
    "drafts", "expiresAt < $cutoff", "expiresAt", 50, 0,
    { cutoff: new Date().toISOString() },
  )
  for (const draft of drafts) await $app.delete(draft)
})
```

cron 使用含秒的 6 段表达式，默认时区为 UTC。任务名称全局唯一，同一任务默认禁止重叠
执行，包括重载前后同名任务。初始化期间可用 `cronRemove(name)` 删除当前候选版本先前注册的
任务，运行期不修改注册表。上例每次最多软删除 50 条，剩余留到后续计划运行。默认不重试；
第 4 个参数可指定 timezone/maxRuntimeMs/retries/idempotent，仅声明幂等时允许最多 3 次重试。

## 6. 邮件、HTTP 与实时事件

Rust 宿主 SMTP 服务和管理员邮件发送接口已实现，见 [邮件发送](mail.md)。本节 `$app.mailer.send`
及 Hook 示例仍依赖尚未实现的 JS 运行时，当前不能直接执行。

这些调用会产生不可事务化的外部副作用。下例监听业务订阅记录，用 afterCommit 在最外层
事务提交确认后执行；直接写在持久化 Hook 的 `next()` 后仍会被拒绝。afterCommit 是
进程内 best-effort；可靠发送使用待实现的事务 outbox。Auth 注册接入另见运行时设计。

```javascript
onRecordCreate(async (e) => {
  e.afterCommit(async (committed) => {
    const receipt = await $app.mailer.send({
      to: [{ address: committed.record.get("email") }],
      subject: "Welcome",
      text: "Your subscription is ready.",
    })
    $app.logger.info("mail accepted", { messageId: receipt.messageId })

    const response = await $app.http.send({
      method: "POST",
      url: "https://api.example.com/subscriptions",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ id: committed.record.id }),
      timeoutMs: 3000,
    })
    if (!response.ok) {
      throw new Error(`upstream returned ${response.status}`)
    }

    await $app.realtime.publish("subscriptions/created", {
      id: committed.record.id,
    }, { roles: [{ collection: "_admins", role: "admin" }] })
  })
  await e.next()
}, "newsletter_subscriptions")
```

HTTP 目标必须在精确 origin allowlist（如 `https://api.example.com:443`）中，并经过私网
地址、重定向、超时和大小检查。邮件凭据不会暴露给 JS，accepted 仅表示 SMTP 接收，不保证
最终送达，超时不自动重试。Phase 4 已有集合变更 SSE，但上述应用 topic、audience 和
`/api/events` 订阅入口尚未实现；audience 必填，用户/角色必须带 collection。运行时开放后，
缺少应用消息适配器时返回 `HB_CAPABILITY_UNAVAILABLE`；配置未授权时返回
`HB_CAPABILITY_DENIED`。

## 7. 文件操作

```javascript
const key = `exports/${e.record.id}.json`
await $app.files.write(key, JSON.stringify(e.record.toJSON()), {
  contentType: "application/json",
})

const text = await $app.files.readText(key)
const metadata = await $app.files.stat(key)
$app.logger.info("export ready", { key, bytes: metadata.size })
```

可用操作包括 `readBytes`、`readText`、`write`、`exists`、`stat`、`list`、`copy`、
`move` 和 `remove`。路径相对于当前 Storage 的扩展前缀（默认 `extensions`），不是宿主绝对路径；`..`、
设备路径和符号链接/junction 逃逸会被拒绝。Phase 5 的本地/S3 Storage 已实现，扩展文件
适配器、独立前缀和配额仍待新增；不允许 JS 直接操作记录附件或网页部署目录。S3 move
为 copy + delete，删除失败须处理目标已存在的部分成功结果。写入操作也必须在业务事务外执行。

## 8. 日志

```javascript
$app.logger.trace("payload received", { size: 120 })
$app.logger.debug("rule matched", { rule: "owner" })
$app.logger.info("report generated", { reportId: "r1" })
$app.logger.warn("upstream is slow", { elapsedMs: 1800 })
$app.logger.error("delivery failed", { code: "ECONNRESET" })
```

支持 `trace`、`debug`、`info`、`warn`、`error`。第二个参数必须是可序列化对象。宿主会添加
脚本、事件、请求/任务 ID，并对密码、token、authorization、secret 等字段脱敏。

## 9. 错误与安全限制

业务拒绝应抛出公开错误，例如 `BadRequestError`、`ForbiddenError` 或 `NotFoundError`。普通
`Error` 被视为扩展故障，生产环境不会把堆栈返回客户端。

每次根调用使用独立 runtime，受内存、栈、累计 JS 活跃片段墙钟时间、异步总时长、日志数
和返回数据大小限制；嵌套 Hook 复用剩余预算。运行时不提供
Node.js 的 `process`、`fs`、`net`、`child_process`、原生 `fetch` 或任意动态模块加载。
环境变量只能通过 `$app.env(name)` 读取白名单项，SMTP、数据库和签名密钥永不进入白名单。

## 10. TypeScript 类型

`@hb/types` 将提供注册函数、事件、Record 和 `$app` 的类型声明：

```javascript
/// <reference types="@hb/types" />
```

类型包只发布服务端已经实现并通过测试的 API，版本与 HertaBase minor 版本对齐。
