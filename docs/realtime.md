# 实时订阅协议

本文档详细说明 HertaBase 的实时数据推送系统（Phase 4），阐述连接协议、消息格式以及权限控制。

## 1. 概览

HertaBase 采用基于 SurrealDB 原生 `LIVE SELECT` 功能结合 Server-Sent Events (SSE) 的方式，为客户端提供低延迟的实时数据变更推送服务。

## 2. SSE 订阅端点

客户端连接端点：
`GET /api/realtime/{collection}`

## 3. 连接协议

- 客户端发起 `GET` 请求，并携带头信息 `Accept: text/event-stream`。
- 身份认证可通过传递 `Authorization` 头（JWT）或在查询参数中提供 `?token=` 进行。
- 服务端响应 HTTP 200，并保持长连接以进行事件流推送。

## 4. 事件类型与消息格式

推送事件包含以下几种标准类型：

- `event: connected` — 成功建立连接，提供订阅 ID 确认。
- `event: create` — 新记录被创建，数据体包含完整的记录内容。
- `event: update` — 记录被更新，数据体包含更新后的记录内容。
- `event: delete` — 记录被删除，数据体包含被删除的记录 ID。
- `event: error` — 订阅过程发生错误（如权限不足、内部错误）。
- `event: ping` — 服务端心跳保持包（默认每 30 秒发送一次）。

## 5. 消息数据格式示例

所有数据体均采用 JSON 格式序列化：

```text
event: create
data: {"id": "xxx", "action": "create", "record": {...}, "timestamp": "2026-08-02T19:00:00Z"}
```

## 6. 数据过滤

客户端可通过在 URL 查询参数中提供过滤条件，对特定记录或子集进行精准订阅，减少无关数据的推送带宽损耗。

## 7. 鉴权与授权引擎整合

- 所有的实时订阅请求均严格遵守 API Rules 动态规则引擎。
- 只有当前用户拥有 `view` 权限的记录变更，才会被服务端推送到该客户端。

## 8. 重连策略

- 推荐客户端实现指数退避（Exponential Backoff）重连机制以减轻服务端雪崩压力。
- 计划支持 `Last-Event-ID` 标头以实现断线后事件的精确恢复。

## 9. 限流与连接限制

系统管理员可在 `hertabase.toml` 中配置文件描述符限制及单 IP 最大 SSE 连接数，防止资源滥用。

## 10. 客户端实现示例 (JavaScript)

```javascript
const eventSource = new EventSource('https://api.example.com/api/realtime/posts?token=YOUR_JWT');

eventSource.addEventListener('create', (e) => {
    const payload = JSON.parse(e.data);
    console.log('New post created:', payload.record);
});

eventSource.addEventListener('error', (e) => {
    console.error('SSE Error:', e);
});
```

## 11. 为什么选择 SSE 而不是 WebSocket？

SSE 相比 WebSocket 更加轻量，专注于“服务器到客户端”的单向推送，完美契合数据库变更通知的需求。SSE 天然通过 HTTP 协议传输，不仅无需处理复杂的握手及协议升级，也更容易配置于 Nginx/Caddy 等反向代理之后，实现简单的水平扩展。

## 项目阶段 (Roadmap)

- Phase 1: 基础架构与动态 ORM — Salvo + SurrealDB 互通，动态 Collection CRUD，数据类型转换层，OpenAPI 自动生成
- Phase 2: 鉴权与权限引擎 — 用户系统（_users/_admins 表），JWT 签发与中间件，API Rules 动态规则引擎
- Phase 3: JS 扩展运行时 — rquickjs AsyncRuntime 集成，Rust→JS FFI 映射，生命周期 Hook 挂载与执行
- Phase 4: 实时订阅引擎 — 基于 SurrealDB LIVE SELECT + Salvo SSE 的数据变更推送
- Phase 5: 文件存储模块 — 抽象 Storage trait，LocalFS + S3 兼容云存储适配器
- Phase 6: 管理后台与单体打包 — Vue Admin UI 开发，rust-embed 静态嵌入，Salvo 静态文件服务
- Phase 7: 生产加固与分布式 — CLI 工具 (clap)，结构化日志 (tracing)，RocksDB→TiKV 集群支持
