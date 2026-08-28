# 管理端 SDK

管理能力位于独立子入口，不会进入普通客户端入口：

```ts
import { HertaBaseAdminClient } from "@hb/sdk/admin";

const admin = new HertaBaseAdminClient({ baseUrl });
await admin.auth.login({ email: adminEmail, password: adminPassword });
```

Admin Session 与用户 Session 应使用不同的 `AuthStore` 实例。

## Collection Schema

```ts
await admin.collections.create({
  name: "posts",
  type: "base",
  schema_mode: "strict",
  fields: [{ name: "title", type: "text", required: true }],
  rules: { list: true, view: true, create: null, update: null, delete: null },
});

await admin.collections.update("posts", {
  fields: [{ name: "published", type: "bool", required: true }],
});
```

服务端 PATCH 只允许增加字段和索引，但可整体替换 rules。管理端仍可通过 `admin.collection<T>('posts')` 操作记录。

## 日志

```ts
const errors = await admin.logs.list({
  level: "error",
  logType: "request",
  statusCode: 500,
  perPage: 100,
});
```

日志列表返回 `Page<LogEntry>`，字段保持服务端 snake_case。

## 网页项目

```ts
const project = await admin.webProjects.deploy({
  archive: { blob: archiveBlob, filename: "site.zip" },
  alias: "/web/docs",
  spaFallback: true,
});

const versions = await admin.webProjects.versions(project.name);
await admin.webProjects.rollback(project.name, versions[0]!);
```

部署支持 ZIP、tar.gz 和 7z；包内必须只有一个根目录。`update` 只修改路由配置，不上传文件。
