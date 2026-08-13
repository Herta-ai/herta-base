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
  - `HB_AUTH_REQUIRED` (401) — 缺少认证令牌或令牌无效。
  - `HB_TOKEN_EXPIRED` (401) — JWT 认证令牌已过期。
  - `HB_FORBIDDEN` (403) — API Rule 拒绝该操作。
  - `HB_ACCOUNT_LOCKED` (423) — 连续登录失败达到阈值，账户处于临时锁定状态。
  - `HB_HOOK_DENIED` (403) — JS Event Hook 在调用 `e.next()` 前明确拒绝或终止操作。
  - `HB_CAPABILITY_DENIED` (403) — JS 扩展没有使用某项宿主能力的授权。
  - `HB_OUTBOUND_DENIED` (403) — JS HTTP 请求目标未通过出站安全策略。
  - `HB_FILE_ACCESS_DENIED` (403) — JS 文件路径或操作超出沙盒授权范围。
  - `HB_NOT_FOUND` (404) — 请求的具体记录或目标未找到。
  - `HB_COLLECTION_NOT_FOUND` (404) — 指定的数据集合（Collection）不存在。
  - `HB_CONFLICT` (409) — 唯一性约束冲突。
  - `HB_PAYLOAD_TOO_LARGE` (413) — 上传载荷或请求体超过系统限制。
  - `HB_UNSUPPORTED_MEDIA_TYPE` (415) — 文件 MIME、扩展名或目标字段不允许。
  - `HB_RANGE_NOT_SATISFIABLE` (416) — 文件 Range 非法、包含多个范围或超出对象长度。
  - `HB_RATE_LIMITED` (429) — 请求过于频繁，触发限流。

- **服务端错误 (5xx)**
  - `HB_HOOK_TIMEOUT` (500) — JS Hook 执行时间超出安全限制。
  - `HB_HOOK_ERROR` (500) — JS Hook 内部抛出未捕获的异常。
  - `HB_HOOK_OOM` (500) — JS Hook 执行消耗内存超出沙盒限制。
  - `HB_SCRIPT_LOAD_ERROR` (500) — JS 扩展加载、编译或注册失败。
  - `HB_ROUTE_CONFLICT` (500) — JS 自定义路由与已有路由冲突。
  - `HB_CAPABILITY_UNAVAILABLE` (503) — JS API 依赖的宿主服务尚未启用。
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

## 项目阶段 (Roadmap)

- Phase 1: 基础架构与动态 ORM — Salvo + SurrealDB 互通，动态 Collection CRUD，数据类型转换层，OpenAPI 自动生成
- Phase 2: 鉴权与权限引擎 — 用户系统（_users/_admins 表），JWT 签发与中间件，API Rules 动态规则引擎
- Phase 3: JS 扩展运行时 — rquickjs AsyncRuntime 集成，Rust→JS FFI 映射，生命周期 Hook 挂载与执行
- Phase 4: 实时订阅引擎 — 基于 SurrealDB LIVE SELECT + Salvo SSE 的数据变更推送
- Phase 5: 文件存储模块 — 抽象 Storage trait，LocalFS + S3 兼容云存储适配器
- Phase 6: 管理后台与单体打包 — React Admin UI 开发，rust-embed 静态嵌入，Salvo 静态文件服务
- Phase 7: 生产加固与分布式 — CLI 工具 (clap)，结构化日志 (tracing)，RocksDB→TiKV 集群支持
