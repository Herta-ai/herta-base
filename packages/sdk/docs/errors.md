# 错误处理

所有 SDK 自身和标准 JSON API 错误都使用 `HertaError`：

```ts
import { isHertaError } from '@hb/sdk'

try {
  await posts.get(id)
}
catch (error) {
  if (!isHertaError(error))
    throw error
  if (error.code === 'HB_RECORD_NOT_FOUND')
    return null
  console.error(error.kind, error.status, error.code, error.details)
}
```

## kind

| kind            | 含义                                     |
| --------------- | ---------------------------------------- |
| `api`           | 服务端返回标准错误 envelope              |
| `network`       | fetch、DNS、连接或流错误                 |
| `timeout`       | SDK 超时中止请求                         |
| `abort`         | 调用方 AbortSignal 主动取消              |
| `protocol`      | 响应不是有效 JSON envelope 或流缺少 body |
| `configuration` | baseUrl、认证 scope 或调用参数配置错误   |

API 错误提供 `status`、`code` 和可选 `details`。`request` 包含 method、path 和最终 URL，便于日志关联，但不会包含 Token。

## 重试边界

普通 HTTP 请求只在明确收到 `HB_TOKEN_EXPIRED` 且存在 Session 时刷新并重试一次。网络错误不会自动重试写请求，以避免服务端已执行但响应丢失造成重复写入。SSE 使用单独的安全重连策略。

常见分支包括 `HB_VALIDATION_ERROR`、`HB_INVALID_FILTER`、`HB_AUTH_REQUIRED`、`HB_TOKEN_EXPIRED`、`HB_FORBIDDEN`、`HB_RECORD_NOT_FOUND`、`HB_RATE_LIMITED` 和文件相关错误码。
