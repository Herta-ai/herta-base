# HertaBase JavaScript 扩展开发指南

> Phase 3 尚在开发，本指南定义目标 API，不代表当前二进制已经实现这些接口。运行时架构、
> 安全边界与实施顺序见 [JavaScript 扩展运行时设计](js-runtime.md)。

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

生产模式下，语法错误、重复路由或重复任务名会阻止服务启动。开发模式会保留上一次成功加载
的注册表，并输出带脚本位置的错误。

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
`e.next()` 表示有意终止处理链。不要再使用旧版设计中的 `return false` 或全局 `context`。

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
只在下游成功时运行，适合发起邮件、HTTP 和实时通知。提交后失败不能回滚数据，宿主会将其
记录为 post-commit failure；需要可靠投递时应使用数据库 outbox。

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

Collection 管理通过 `$app.collections.findByName/list/create/save/delete` 完成，以确保
Schema、OpenAPI 和缓存同步刷新。

原生查询仅用于高级场景，并可能被管理员关闭：

```javascript
const result = await $app.db.query(
  "SELECT * FROM posts WHERE status = $status LIMIT $limit",
  { status: "published", limit: 10 },
)
```

所有动态值必须使用变量绑定。服务端扩展默认使用受审计的系统身份，但仍不能绕过 Schema、
系统表保护或事务限制。

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

路径参数使用 `{name}`。自定义路由不能覆盖 HertaBase 内置 API。可用响应包括 `e.json()`、
`e.text()`、`e.html()`、`e.file()` 和 `e.noContent()`；可用中间件包括
`$apis.requireAuth()`、`requireAdmin()`、`bodyLimit()` 和 `rateLimit()`。

## 5. 定时任务

```javascript
cronAdd("remove-expired-drafts", "0 0 3 * * *", async () => {
  await $app.db.query(
    "DELETE drafts WHERE expiresAt < time::now()",
  )
})
```

cron 使用含秒的 6 段表达式，默认时区为 UTC。任务名称全局唯一，同一任务默认禁止重叠
执行。可用 `cronRemove(name)` 删除先前注册的任务。

## 6. 邮件、HTTP 与实时事件

这些调用会产生不可事务化的外部副作用，应放在 `await e.next()` 之后或定时任务中。

```javascript
onRecordCreate(async (e) => {
  await e.next()

  await $app.mailer.send({
    to: [{ address: e.record.get("email") }],
    subject: "Welcome",
    text: "Your account is ready.",
  })

  const response = await $app.http.send({
    method: "POST",
    url: "https://api.example.com/users",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ id: e.record.id }),
    timeoutMs: 3000,
  })
  if (!response.ok) {
    throw new Error(`upstream returned ${response.status}`)
  }

  await $app.realtime.publish("users/created", {
    id: e.record.id,
  }, { roles: ["admin"] })
}, "users")
```

HTTP 目标必须在 allowlist 中，并会经过私网地址、重定向、超时和大小检查。邮件凭据不会暴露
给 JS。实时能力依赖 Phase 4；服务未启用时返回 `HB_CAPABILITY_UNAVAILABLE`。

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
`move` 和 `remove`。路径是扩展文件根目录或 Storage 内的逻辑键，不是宿主绝对路径；`..`、
设备路径和符号链接逃逸会被拒绝。文件能力依赖配置，Storage 集成依赖 Phase 5。

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

每次执行受到内存、栈、同步 CPU 时间、异步总时长、日志数和返回数据大小限制。运行时不提供
Node.js 的 `process`、`fs`、`net`、`child_process`、原生 `fetch` 或任意动态模块加载。
环境变量只能通过 `$app.env(name)` 读取白名单项，SMTP、数据库和签名密钥永不进入白名单。

## 10. TypeScript 类型

`@hb/types` 将提供注册函数、事件、Record 和 `$app` 的类型声明：

```javascript
/// <reference types="@hb/types" />
```

类型包只发布服务端已经实现并通过测试的 API，版本与 HertaBase minor 版本对齐。
