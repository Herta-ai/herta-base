# 教程：Blog 客户端

本例使用 `blog_users`、`blog_posts` 和 `blog_comments`，与仓库 Blog 集成场景一致。

## 定义类型并登录

```ts
interface BlogPost {
  id: string
  title: string
  content: string
  author: string
  is_published: boolean
  expand?: { author?: BlogUser }
}

interface BlogUser {
  id: string
  email: string
  displayName: string
}

await hb.auth.forCollection('blog_users').login({ email, password })
const posts = hb.collection<BlogPost, Omit<BlogPost, 'id' | 'expand'>, Partial<BlogPost>>(
  'blog_posts',
)
```

## 创建和读取

```ts
const me = await hb.auth.forCollection('blog_users').me<{
  displayName: string
}>()

await posts.create({
  title: 'HertaBase SDK',
  content: 'Typed clients for dynamic collections.',
  author: me.id,
  is_published: true,
})

const publicPosts = await posts.list({
  filter: 'is_published = true',
  expand: 'author',
  sort: '-created_at',
})
```

API Rules 决定可见记录。列表为空不一定表示数据库为空，也可能是当前身份没有访问权；对无权查看的单条记录，服务端通常返回 `HB_RECORD_NOT_FOUND`，避免泄漏记录存在性。

## 评论 Relation

```ts
await hb.collection('blog_comments').create({
  post: post.id,
  author: me.id,
  content: 'First comment',
})
```

Relation 必须使用服务端返回的完整 ID，不要自行截取 key。可编译版本见 [../examples/blog.ts](../examples/blog.ts)。
