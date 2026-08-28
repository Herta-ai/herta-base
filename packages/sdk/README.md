# @hb/sdk

HertaBase 官方 TypeScript SDK。它使用标准 `fetch`，支持现代浏览器与 Node.js 20+，没有运行时依赖。

```bash
pnpm add @hb/sdk
```

```ts
import { HertaBaseClient, isHertaError } from '@hb/sdk'

interface Post {
  id: string
  title: string
  published: boolean
}

const hb = new HertaBaseClient({ baseUrl: 'http://localhost:8080' })

try {
  await hb.auth.forCollection('users').login({
    email: 'reader@example.com',
    password: 'correct password',
  })
  const posts = await hb.collection<Post>('posts').list({
    filter: 'published = true',
    sort: '-created_at',
  })
  console.log(posts.items)
}
catch (error) {
  if (isHertaError(error))
    console.error(error.code, error.message)
}
```

管理端能力使用独立入口：

```ts
import { HertaBaseAdminClient } from '@hb/sdk/admin'

const admin = new HertaBaseAdminClient({ baseUrl: 'http://localhost:8080' })
```

完整指南见 [docs/README.md](docs/README.md)，建议从[快速开始](docs/getting-started.md)阅读。还可以直接查看 [Blog 教程](docs/tutorials/blog.md)、[Kanban 教程](docs/tutorials/kanban.md)与 [Node.js 教程](docs/tutorials/node-service.md)。

## 运行环境

- Node.js 20 或更高版本。
- 支持 `fetch`、`FormData`、`Blob`、Web Streams 和 `AbortController` 的现代浏览器。
- ESM 和 CommonJS 均可使用。

## 许可证

MIT
