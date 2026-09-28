# 错误代码参考

本文档列出 HertaBase 接口返回的标准化错误代码，供客户端开发者在处理异常逻辑时参考。

## 1. 标准 API 响应格式

遇到错误时，HertaBase 总是返回如下结构的 JSON：

```json
{
  "data": null,
  "meta": null,
  "error": {
    "code": 400,
    "message": "Validation failed.",
    "error": "HB_VALIDATION_ERROR",
    "details": { "field": "email", "reason": "invalid format" }
  }
}
```

## 2. 错误代码分类及完整列表

- **客户端错误 (4xx)**
  - `HB_VALIDATION_ERROR` (400) — 请求体数据验证失败。
  - `HB_INVALID_FILTER` (400) — 查询过滤表达式语法无效。
  - `HB_INVALID_SORT` (400) — 排序参数无效。
  - `HB_AUTH_REQUIRED` (401) — 请求需要认证，但没有提供认证令牌。
  - `HB_UNAUTHORIZED` (401) — 已提供的令牌无效、被篡改、已撤销或类型不匹配。
  - `HB_TOKEN_EXPIRED` (401) — JWT 认证令牌已过期。
  - `HB_FORBIDDEN` (403) — API Rule 拒绝该操作。
  - `HB_ACCOUNT_LOCKED` (423) — 连续登录失败达到阈值，账户处于临时锁定状态。
  - `HB_HOOK_ABORTED` (409) — 持久化/Auth Hook 未调用 next，或当前事务已被标记只能回滚。
  - `HB_HOOK_RECURSION` (409) — 写入调用树递归操作同一记录或超过深度上限。
  - `HB_SIDE_EFFECT_IN_TRANSACTION` (409) — 当前事务中禁止执行邮件等外部副作用。
  - `HB_QUERY_UNSUPPORTED` (400) — SQL 超出受限 SELECT 子集。
  - `HB_CAPABILITY_DENIED` (403) — JS 扩展没有使用某项宿主能力的授权。
  - `HB_OUTBOUND_DENIED` (403) — JS HTTP 请求目标未通过出站安全策略。
  - `HB_FILE_ACCESS_DENIED` (403) — JS 文件路径或操作超出沙盒授权范围。
  - `HB_RECORD_NOT_FOUND` (404) — 记录不存在、已软删除或被 `view` Rule 隐藏。
  - `HB_NOT_FOUND` (404) — 非记录类目标（例如文件或路由）未找到。
  - `HB_COLLECTION_NOT_FOUND` (404) — 指定的数据集合（Collection）不存在。
  - `HB_CONFLICT` (409) — 唯一性、集合版本或已确认回滚的数据库并发冲突。
  - `HB_PAYLOAD_TOO_LARGE` (413) — 上传载荷或请求体超过系统限制。
  - `HB_UNSUPPORTED_MEDIA_TYPE` (415) — 文件 MIME 不匹配、客户端伪造文件引用或目标不是文件字段。
  - `HB_RANGE_NOT_SATISFIABLE` (416) — 文件 Range 非法、包含多个范围或超出对象长度。
  - `HB_RATE_LIMITED` (429) — 请求过于频繁，触发限流。

- **服务端错误 (5xx)**
  - `HB_HOOK_TIMEOUT` (504) — JS Hook 活跃执行预算或调用总时长超限。
  - `HB_JS_BUSY` (503) — worker 队列已满、排队超时或运行时正在关闭。
  - `HB_COMMIT_UNKNOWN` (503) — 提交结果暂时不能确认；details 包含 operationId/checkUrl/checkCredential，通过核对接口查询，不能直接重发业务写入。
  - `HB_HOOK_ERROR` (500) — JS Hook 内部抛出未捕获的异常。
  - `HB_HOOK_OOM` (500) — JS Hook 执行消耗内存超出沙盒限制。
  - `HB_SCRIPT_LOAD_ERROR` (500) — JS 扩展加载、编译或注册失败。
  - `HB_ROUTE_CONFLICT` (500) — JS 自定义路由与已有路由冲突。
  - `HB_CAPABILITY_UNAVAILABLE` (503) — 所需宿主服务未启用或适配器尚未可用，包括 disabled 邮件驱动。
  - `HB_MAIL_SEND_FAILED` (502) — SMTP 连接、TLS、认证或提交失败；接口不返回 SMTP 原始错误或凭据。
  - `HB_MAIL_TIMEOUT` (504) — SMTP 提交超时，接收端是否已经接受邮件可能未知；服务不会自动重试。
  - `HB_HTTP_SEND_FAILED` (502) — JS 出站 HTTP 连接、TLS、传输或 UTF-8 响应失败；不返回底层连接诊断。
  - `HB_HTTP_TIMEOUT` (504) — JS 出站 HTTP 总预算耗尽，远端是否接受可能未知；服务不会自动重试。
  - `HB_DB_ERROR` (500) — 底层 SurrealDB 数据库操作失败。
  - `HB_STORAGE_ERROR` (500) — 文件存储适配器（LocalFS / S3）操作失败。
  - `HB_INTERNAL_ERROR` (500) — 不可预期的系统内部错误。

## 3. HTTP 状态码映射

HertaBase 的标准 HTTP 状态码与具体的 `error` 字符串深度绑定。对于所有以 `HB_` 开头的错误字符串，上层网关和反向代理也会接收到表格中列出的对应 HTTP Status Code。

## 4. 客户端处理建议

在构建基于 `@hb/sdk` 的客户端应用时，建议捕获 HTTP 响应的非 2xx 状态，依据 `error` 字段进行具体的分支处理，同时将 `message` 或 `details` 反馈至用户前端提示。

## 5. 不同上下文中的错误响应

- **REST API**：遵循上述 JSON 规范。
- **SSE (Realtime)**：系统将发送 `event: error`，并将上述 JSON 放入数据体中推送，随后可能会关闭连接。
- **Admin UI**：管理后台捕获异常后将通过弹窗呈现友好的多语言提示。

文件扩展的 `HB_FILE_MOVE_PARTIAL` (409) 在 details 中包含目标 FileItem、`destinationExists: true`、
源 key 以及 `sourceState: present | absent | unknown`。该错误表示 copy 已成功，不能当作完整回滚。
`HB_IDEMPOTENCY_CONFLICT` (409) 表示同一 outbox kind/key 的规范化 payload 不同。
Outbox 回执中的 `HB_OUTBOX_LEASE_EXPIRED` 和 `HB_OUTBOX_LEASE_LOST` 是待核对原因，不表示未发送，
不作为新的 HTTP 错误响应状态。
