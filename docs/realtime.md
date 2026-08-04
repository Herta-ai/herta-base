# 实时订阅协议

HertaBase Phase 4 使用 SurrealDB `LIVE SELECT` 与 Salvo Server-Sent Events (SSE)
推送集合变更。本阶段只支持服务器到客户端的 SSE，不支持 WebSocket、事件持久化或
`Last-Event-ID` 重放，也不会在连接时发送初始记录快照。

## 订阅端点

```http
GET /api/realtime/{collection}?filter=...&token=...
Accept: text/event-stream
Authorization: Bearer <access-token>
```

`filter` 使用记录列表接口的过滤语法。JWT 可放在 `Authorization` Header 或 `token`
查询参数中；两者同时存在时 Header 优先。生产环境推荐 Header，避免令牌进入 URL、代理日志
和浏览器历史。匿名连接不设置到期计时器。

建立 SSE 前会完成集合、过滤器、权限、令牌和连接限额检查。失败时返回标准 JSON 错误和
对应 HTTP 状态码；成功时返回 `200 OK`、`Content-Type: text/event-stream`、
`Cache-Control: no-cache` 和 `X-Accel-Buffering: no`。

## 权限与过滤

实时订阅使用集合的 `view` API Rule，而不是 `list` Rule。管理员绕过规则；匿名用户只能
连接允许公开查看的集合。行级规则和客户端 `filter` 会编译到同一个参数绑定的
`LIVE SELECT ... WHERE` 中。

过滤遵循 SurrealDB 原生 live query 语义：记录更新后不再匹配 `view` 或 `filter` 时，
服务端不会人为补发 `delete`。Auth 集合事件会移除 `password_hash`、`token_key`、
`failed_attempts` 和 `locked_until`。

## 事件格式

连接建立后的第一帧：

```text
event:connected
data:{"subscriptionId":"...","collection":"posts","timestamp":"2026-08-04T12:00:00.000Z"}
```

创建和更新事件包含完整的清理后记录。每个数据事件的 SSE `id` 是 UUIDv7，但当前不能用于重放：

```text
event:create
data:{"id":"posts:...","action":"create","record":{"id":"posts:..."},"timestamp":"..."}
id:019...
```

原生 `DELETE` 和设置 `deleted_at` 的软删除都会映射为 `delete`，仅返回记录 ID：

```text
event:delete
data:{"id":"posts:...","action":"delete","record":{"id":"posts:..."},"timestamp":"..."}
id:019...
```

空闲连接按配置周期发送实际的 `ping` 事件：

```text
event:ping
data:{"timestamp":"..."}
```

运行期数据库错误和 JWT 到期会发送标准错误 envelope 的 `error` 事件，然后关闭连接。
JWT 到期错误码为 `HB_TOKEN_EXPIRED`。

## 生命周期与限制

客户端断开、令牌到期、数据库流结束或发生错误时，服务端会丢弃 live stream，SurrealDB
随即清理查询，同时释放全局和单 IP 连接配额。

```toml
[realtime]
max_connections = 1000
max_connections_per_ip = 20
heartbeat_seconds = 30
```

达到任一上限返回 `429 HB_RATE_LIMITED`。部署者仍需按预期并发量配置操作系统文件描述符、
反向代理读取超时和代理缓冲策略。

## 客户端示例

浏览器原生 `EventSource` 不能设置 Authorization Header，因此下例使用查询参数。可设置 Header
的客户端应优先使用 Header。

```javascript
const source = new EventSource(`/api/realtime/posts?token=${encodeURIComponent(token)}`);

source.addEventListener("create", (event) => {
  const payload = JSON.parse(event.data);
  console.log(payload.record);
});

source.addEventListener("error", () => {
  source.close();
});
```

客户端应使用指数退避重连。由于没有事件持久化与重放，断线期间的变更需要通过普通列表接口重新同步。
