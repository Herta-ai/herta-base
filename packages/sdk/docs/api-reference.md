# API 参考

## HertaBaseClient

```ts
new HertaBaseClient(options?: HertaBaseClientOptions)
```

- `auth: AuthClient`：默认 `_users` 认证。
- `files: FilesClient`：文件令牌和读取。
- `collection<TRecord,TCreate,TUpdate>(name)`：绑定 Collection。
- `request<T>(path, options)`：请求自定义 JSON envelope 端点。

`HertaBaseSDK` 是 `HertaBaseClient` 的兼容导出别名。

## AuthClient

- `forCollection(name): AuthClient`
- `register<TProfile>(credentials): Promise<AuthSession<TProfile>>`
- `login<TProfile>({email,password}): Promise<AuthSession<TProfile>>`
- `refresh(): Promise<AuthSession>`
- `me<TProfile>(): Promise<ProfiledAuthUser<TProfile>>`
- `getSession()`、`setSession(session)`、`logout()`
- `onChange(listener): unsubscribe`

`Credentials.profile` 会平铺到注册 JSON 中，与服务端 Auth Collection 自定义字段一致。

## CollectionClient

- `list(options?): Promise<Page<TRecord>>`
- `get(id, options?): Promise<TRecord>`
- `create(data, options?): Promise<TRecord>`
- `update(id, data, options?): Promise<TRecord>`
- `delete(id, options?): Promise<TRecord>`
- `createWithFiles(upload, options?): Promise<TRecord>`
- `updateWithFiles(id, upload, options?): Promise<TRecord>`
- `subscribe(options?): Promise<RealtimeSubscription<TRecord>>`

`ListOptions` 支持 `page`、`perPage`、`sort`、`filter`、`expand`、`signal` 和 `timeoutMs`。sort/expand 可传逗号字符串或字符串数组。

## FilesClient

- `issueToken({collection,recordId,field})`
- `buildDownloadUrl(reference, token?)`
- `download(reference, options?): Promise<Response>`
- `head(reference, options?): Promise<Response>`

`FileAccessOptions` 支持 file token、Range、If-None-Match、signal 和 timeout。

## RealtimeSubscription

- `status: connecting | connected | reconnecting | closed`
- `onEvent(listener)`、`onStatus(listener)`、`onError(listener)`
- `close()`

所有监听注册方法返回取消函数。

## HertaBaseAdminClient

从 `@hb/sdk/admin` 导入。除管理员 `auth`、`files`、`collection()` 和 `request()` 外，还提供：

- `collections.list/get/create/update/delete`
- `logs.list(options): Promise<Page<LogEntry>>`
- `webProjects.list/get/deploy/update/delete/versions/rollback`

## HertaError

属性：`kind`、`status?`、`code?`、`details?`、`request?`、标准 Error `cause?`。使用 `isHertaError(value)` 做类型收窄。
