# 认证与会话

## 默认和动态 Auth Collection

`hb.auth` 对应内置 `_users` 的别名路由 `/api/auth/*`：

```ts
await hb.auth.register({ email, password, profile: { displayName: 'Alice' } })
await hb.auth.login({ email, password })
const current = await hb.auth.me()
```

自定义 Auth Collection 使用绑定客户端：

```ts
const users = hb.auth.forCollection('blog_users')
const session = await users.login({ email, password })
```

Session 会记录 scope，后续刷新自动选择 `/api/auth/refresh`、`/api/auth/{collection}/refresh` 或 `/api/admin/auth/refresh`。在错误 scope 上调用 `me()` 或显式 `refresh()` 会抛出配置错误，防止刷新令牌发往错误端点。

## 自动刷新

- 登录和注册后，SDK 保存 Access/Refresh Token、用户、scope 和本地计算的 `expiresAt`。
- Access Token 距过期不足 30 秒时，请求前自动轮换 Token。
- 服务端返回 `HB_TOKEN_EXPIRED` 时，SDK 刷新并只重试原请求一次。
- 同一客户端实例的并发请求共享一个刷新 Promise，避免重放一次性 Refresh Token。
- 刷新失败会清空 Session，并把错误返回调用方。

可用 `refreshSkewMs` 修改提前刷新窗口。

## 自定义持久化

默认 `MemoryAuthStore` 不会跨页面刷新保存 Token。可实现：

```ts
import type { AuthSession, AuthStore } from '@hb/sdk'

class MyStore implements AuthStore {
  async get(): Promise<AuthSession | null> {
    /* 从安全存储读取 */ return null
  }

  async set(session: AuthSession): Promise<void> {
    /* 保存 */
  }

  async clear(): Promise<void> {
    /* 删除 */
  }
}
```

通过 `authStore` 传入。使用 localStorage 时要评估 XSS 风险；多个标签页或多个进程共享一次性 Refresh Token 时，还必须在存储层实现跨上下文锁，SDK 只保证单实例内的刷新单飞。

## 会话操作

```ts
const session = await hb.auth.getSession()
await hb.auth.setSession(restoredSession)
const unsubscribe = hb.auth.onChange(next => console.log(next))
await hb.auth.logout() // 只清理客户端，不调用服务端撤销端点
unsubscribe()
```
