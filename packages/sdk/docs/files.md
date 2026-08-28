# 文件字段

HertaBase 文件与记录字段绑定，没有独立的通用上传端点。普通 JSON 不能提交非空文件引用；创建或更新文件必须使用 multipart 方法。

## 上传

```ts
const tasks = hb.collection<Task, CreateTask, UpdateTask>('tasks')

const created = await tasks.createWithFiles({
  data: { title: 'Design review', status: 'todo' },
  files: {
    cover: new File(['image'], 'cover.png', { type: 'image/png' }),
    attachments: [
      new File(['brief'], 'brief.pdf', { type: 'application/pdf' }),
      { blob: new Blob(['notes']), filename: 'notes.txt' },
    ],
  },
})
```

`files` 的键必须等于 Collection 中的 file 字段名。浏览器可直接传 `File`；Node.js 或只有 `Blob` 时建议显式提供 filename。

## 替换、追加和清空

上传到 PATCH 时默认整体替换该文件字段：

```ts
await tasks.updateWithFiles(id, {
  files: { attachments: replacementFiles },
})
```

多文件字段可追加：

```ts
await tasks.updateWithFiles(
  id,
  { files: { attachments: newFiles } },
  { appendFiles: ['attachments'] },
)
```

清空单文件字段使用普通 `update(id, { cover: null })`，清空多文件字段使用空数组。不能在同一次请求中既清空又追加同一字段。

## 文件令牌和 URL

```ts
const issued = await hb.files.issueToken({
  collection: 'tasks',
  recordId: id,
  field: 'attachments',
})

const url = hb.files.buildDownloadUrl(
  { collection: 'tasks', recordId: id, field: 'attachments', filename },
  issued.token,
)
image.src = url
```

文件令牌是短期、字段级令牌，适合 `<img>` 等不能方便添加 Authorization header 的场景。

## 流式下载与 HEAD

```ts
const response = await hb.files.download(reference, { range: 'bytes=0-1023' })
const metadata = await hb.files.head(reference, { ifNoneMatch: previousEtag })
```

这两个方法返回原始 `Response`，调用方负责读取 body 并处理 206、304、ETag、Content-Type 和 Content-Disposition。
