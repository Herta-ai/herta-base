# Collection 与记录

## 类型绑定

```ts
interface Post {
  id: string
  title: string
  author: string
  tags: string[]
  expand?: { author?: { id: string, email: string } }
}

type CreatePost = Omit<Post, 'id' | 'expand'>
type UpdatePost = Partial<Pick<Post, 'title' | 'tags'>>

const posts = hb.collection<Post, CreatePost, UpdatePost>('posts')
```

## CRUD

```ts
const created = await posts.create({ title: 'Hello', author: userId, tags: [] })
const one = await posts.get(created.id, { expand: 'author' })
const updated = await posts.update(created.id, { title: 'Updated' })
const deletedRecord = await posts.delete(created.id)
```

服务端 DELETE 返回被软删除的记录。之后的 get、update、delete 会得到 `HB_RECORD_NOT_FOUND`。

ID 可以是裸 key，也可以是完整 `collection:key`。SDK 始终对整个路径段编码；完整 ID 的 Collection 与当前客户端不一致时，服务端返回 404。

## 列表、过滤和展开

```ts
const page = await posts.list({
  page: 1,
  perPage: 30,
  sort: ['-created_at', 'title'],
  filter: 'published = true AND author = \'users:one\'',
  expand: ['author', 'comments.user'],
})
```

`Page<T>` 包含 `items`、`total`、`page`、`perPage`。`filter` 是服务端受限且参数化的表达式，不是任意 SurrealQL。Relation 过滤必须使用完整目标 ID；多值 Relation 使用 `CONTAINS`。

`expand` 最多 10 条路径、每条最多 3 层。原 Relation ID 保持不变，展开结果写入记录的 `expand` 字段。

## 请求控制

各 CRUD 方法接受 `signal` 和 `timeoutMs`。需要调用尚未封装的标准 JSON 端点时，可使用：

```ts
const value = await hb.request<MyType>('/api/custom/path', {
  method: 'POST',
  body: { input: true },
})
```

`request()` 只适用于 HertaBase JSON envelope，不用于文件或其他二进制响应。
