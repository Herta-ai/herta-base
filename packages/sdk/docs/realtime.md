# 实时订阅

```ts
const subscription = await hb.collection<Task>("tasks").subscribe({
  filter: "workspace = 'workspaces:one'",
  onStatus: (status) => console.log(status),
  onEvent: (event) => {
    if (event.type === "update") console.log(event.data.record);
  },
  onError: (error) => console.error(error),
});
```

`subscribe()` 在 SSE HTTP 握手成功后返回；401、403、404 等首次握手错误会直接拒绝 Promise。服务端的 `connected` 帧到达后，状态变为 `connected`。

## 事件

- `connected`：包含 `subscriptionId`、Collection 和时间戳。
- `create`、`update`、`delete`：包含事件 ID、动作、完整记录和时间戳。
- `ping`：服务端心跳。
- `error`：服务端以标准错误 envelope 发送的流内错误。

还可在创建后增加监听器：

```ts
const off = subscription.onEvent(handler);
const offStatus = subscription.onStatus(handlerStatus);
off();
offStatus();
subscription.close();
```

## 重连

默认对网络错误、EOF、429 和 5xx 使用带抖动的指数退避：初始 500ms、上限 30 秒、倍率 2、无限尝试。可配置：

```ts
reconnect: {
  enabled: true,
  initialDelayMs: 1_000,
  maxDelayMs: 15_000,
  multiplier: 2,
  jitter: 0.2,
  maxAttempts: 10,
}
```

传 `reconnect: false` 可禁用。403、404、畸形协议和不可恢复认证错误不会重连。Access Token 到期时，SDK 轮换 Session 后立即重连。

## 一致性边界

当前服务端不支持 `Last-Event-ID` 重放。连接断开到重连成功之间的事件可能丢失；需要严格一致性的应用应在 `connected` 状态恢复后重新执行一次列表查询，以服务器当前状态为准。
