# 快速开始

## 安装

```bash
npm install @hb/sdk
# 或 pnpm add @hb/sdk
```

```ts
import { HertaBaseClient } from '@hb/sdk'

const hb = new HertaBaseClient({
  baseUrl: 'https://api.example.com',
})
```

`baseUrl` 默认是 `http://localhost:8080`。SDK 会去除结尾斜杠，但不会自动添加 `/api`。

## 登录并读取记录

```ts
await hb.auth.forCollection('app_users').login({
  email: 'user@example.com',
  password: 'correct password',
})

interface Task {
  id: string
  title: string
  status: 'todo' | 'done'
  owner: string
}

type CreateTask = Omit<Task, 'id'>
type UpdateTask = Partial<Pick<Task, 'title' | 'status'>>

const tasks = hb.collection<Task, CreateTask, UpdateTask>('tasks')
const created = await tasks.create({
  title: 'Read SDK guide',
  status: 'todo',
  owner: 'app_users:user-id',
})

const page = await tasks.list({ page: 1, perPage: 20, sort: '-created_at' })
console.log(page.items, page.total)
```

泛型只在编译期约束业务代码，不会替代服务端 Schema 校验和 API Rules。

## 取消请求

```ts
const controller = new AbortController()
const pending = tasks.list({ signal: controller.signal })
controller.abort()
await pending
```

请求失败会抛出 `HertaError`，详见[错误处理](errors.md)。完整示例见 [examples/quickstart.ts](examples/quickstart.ts)。
