# 文件存储与上传

本文档定义 Phase 5 的记录绑定文件模型、上传协议、访问控制、存储后端和一致性策略。文件不是独立资源；每个文件必须属于一个 Collection 记录的 `file` 字段。

Phase 5 不包含 `$app.files` JS API，也不包含 Phase 6 的网页项目部署与静态托管。

## 1. 存储模型

`herta_storage` 提供异步 `Storage` trait：

- `put_file(key, source)`：从临时文件流式写入对象。
- `head(key)`：返回长度、ETag 和最后修改时间。
- `get(key, range)`：流式读取完整对象或一个字节范围。
- `delete(key)`：幂等删除对象。
- `delete_prefix(prefix)`：删除集合前缀下的对象。

LocalFS 根目录固定为 `HB_DATA_DIR/storage`。S3 bucket 必须保持私有，所有下载都由 HertaBase 代理。逻辑 key 使用：

```text
records/{collection}/{record_id}/{field}/{server_filename}
```

逻辑 key 拒绝绝对路径、空段、`.`、`..`、反斜杠、NUL 和路径穿越。客户端文件名不会直接成为逻辑 key；服务端使用 UUIDv7 和经过校验的扩展名生成引用。

## 2. file 字段

```json
{
  "name": "attachments",
  "type": "file",
  "required": false,
  "options": {
    "maxSelect": 3,
    "maxSize": 5242880,
    "mimeTypes": ["image/png", "image/jpeg"],
    "extensions": ["png", "jpg", "jpeg"]
  }
}
```

- `maxSelect` 默认 `1`，范围 `1..=100`。`1` 在记录中存储字符串；大于 `1` 存储字符串数组。
- `maxSize` 是该字段的单文件字节上限；实际限制取它与 `HB_STORAGE_MAX_FILE_SIZE` 的较小值。
- `mimeTypes` 和 `extensions` 是非空字符串数组。扩展名不含点，只允许 ASCII 字母和数字。
- `required` 字段不能被清空；可选单文件清空为 `null`，可选多文件清空为 `[]`。
- REST JSON 请求不能提交非空文件引用，避免伪造已存储文件名。

不自动迁移旧版本中任意路径形式的 file 值。启用 Phase 5 前应确认生产数据中不存在 legacy file 路径。

## 3. 记录上传协议

原有 `application/json` CRUD 保持不变。创建和更新记录还接受 `multipart/form-data`：

- 唯一允许的普通 part 是可选的 `data`，内容必须是一个 JSON 对象。
- 文件 part 名称必须等于 Collection 中的 file 字段名。
- 多文件字段通过重复同名 part 上传；数量不能超过 `maxSelect`。
- 所有 part 完成解析并通过字段、数量、大小、MIME、扩展名、记录校验和 Collection Rule 预检后，才开始写最终对象。

示例：

```bash
curl -X POST http://localhost:8080/api/collections/posts/records \
  -H "Authorization: Bearer $TOKEN" \
  -F 'data={"title":"Hello"};type=application/json' \
  -F 'cover=@cover.png;type=image/png' \
  -F 'attachments=@one.pdf;type=application/pdf' \
  -F 'attachments=@two.pdf;type=application/pdf'
```

PATCH 规则：

- 字段缺席：保留原文件。
- `null` 或 `[]`：清空该字段，并按单值/数组类型归一化。
- 上传一个或多个同字段 part：整体替换旧值。
- 上传和清空标记同时出现时，上传值优先。

## 4. 文件令牌与下载

已认证用户先调用：

```http
POST /api/files/token
Authorization: Bearer <access-token>
Content-Type: application/json

{"collection":"posts","recordId":"...","field":"cover"}
```

服务端执行记录 `view` rule，确认字段是 file 字段且当前含文件，然后签发短期 JWT。令牌绑定集合、记录、字段、账户和 `token_key`；账户凭据轮换后立即失效。

文件读取地址：

```http
GET|HEAD /api/files/{collection}/{recordId}/{field}/{filename}
Authorization: Bearer <access-token>
```

原生媒体元素不能设置 Authorization 时，可使用 `?token=<file-token>`。两者同时存在时 Authorization 优先，错误的 Authorization 不会回退到查询令牌。服务端始终确认 `filename` 仍属于对应记录字段。

读取支持：

- GET 和 HEAD。
- 单个 `Range: bytes=...`，成功返回 206；多 Range 或越界返回 416。
- `ETag`、`If-None-Match` 和 304。
- `Content-Length`、`Content-Type`、`Content-Disposition`、`Accept-Ranges`。
- `Cache-Control: private, max-age=0, must-revalidate` 和 `X-Content-Type-Options: nosniff`。
- HTML、SVG、JavaScript、CSS、Wasm 等主动内容强制使用 `attachment` 下载。

## 5. 配置

```toml
[storage]
type = "local"                 # local | s3
max_file_size = 10485760       # 10 MiB，单文件全局上限
file_token_ttl_seconds = 300

[storage.s3]
endpoint = "https://s3.example.com"
bucket = "hertabase-files"
region = "us-east-1"
prefix = "hertabase"
force_path_style = true
allow_http = false
```

S3 凭据只从 `HB_S3_ACCESS_KEY`、`HB_S3_SECRET_KEY` 和可选的 `HB_S3_SESSION_TOKEN` 读取，不接受 TOML 明文凭据。HTTP endpoint 只允许在 `server.dev_mode=true` 且 `allow_http=true` 时使用。

完整环境变量见 [配置参考](configuration.md)。

## 6. 一致性与生命周期

1. Salvo 将 multipart 文件流式写入受请求大小限制的临时文件。
2. HertaBase 完成全部元数据、记录和权限预检。
3. LocalFS/S3 从临时文件分块写入最终对象；失败会中止 multipart 写入。
4. 数据库写入失败时删除本次新对象；补偿失败记录结构化 warning。
5. 数据库成功后，替换或清空产生的旧对象采用 best-effort 删除。

记录软删除保留文件。Collection 删除成功后执行 `records/{collection}` 前缀清理；失败不会回滚数据库删除，而是保留孤儿对象并记录日志，等待后续 GC 能力处理。

## 7. 错误

- `HB_PAYLOAD_TOO_LARGE` (413)：请求体或文件超过限制。
- `HB_UNSUPPORTED_MEDIA_TYPE` (415)：MIME、扩展名或上传目标字段不允许。
- `HB_RANGE_NOT_SATISFIABLE` (416)：Range 非法、为多范围或越界。
- `HB_STORAGE_ERROR` (500)：LocalFS/S3 操作失败；生产响应隐藏底层路径和云端细节。

## 8. 运维注意事项

- LocalFS 部署必须将数据库、`auth/jwt-secret` 与 `storage/` 一起备份。
- S3 bucket 不应配置公开读策略；代理层负责规则校验和响应头。
- `HB_MAX_REQUEST_BODY_SIZE` 是整个 multipart 请求上限，多个文件上传时应配置为高于期望总大小。
- S3 兼容服务可用 MinIO 验证；未配置测试 endpoint 时只运行 builder 和本地/内存测试。
