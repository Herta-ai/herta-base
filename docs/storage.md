# 文件存储与上传

本文档定义记录绑定文件、上传协议、访问控制、存储后端和扩展逻辑文件的一致性策略。记录附件必须属于 Collection 记录的 `file` 字段；JS 扩展文件使用第 9 节的独立逻辑目录。

网页项目部署与静态托管另见对应文档。

## 1. 存储模型

`herta_storage` 提供异步 `Storage` trait：

- `put_file(key, source)`：从临时文件流式写入对象。
- `head(key)`：返回长度、ETag 和最后修改时间。
- `get(key, range)`：流式读取完整对象或一个字节范围。
- `delete(key)`：幂等删除对象。
- `delete_prefix(prefix)`：删除集合前缀下的对象。
- `put_bytes(key, bytes)`：写入已经在宿主额度内的字节缓冲。
- `list(prefix, limit)`：内部恢复用的有界列举，1..10000 条；超出上限报错，不返回截断的不完整结果。
- `list_page(prefix, limit, cursor)`：返回 `{ items, next_cursor }`，1..10000 条，按完整物理键排序；cursor 绑定目录前缀，删除上一页对象后仍可续页。不提供并发写入期间的目录快照。
- `copy(source, destination, max_bytes)`：受大小限制的对象复制。

物理分页只保留下一页及一条探测项；本地逐项遍历受控目录，S3 使用流式列表并处理服务端续页。
底层列表不保证排序，因此每页可能扫描剩余对象，内存有界但扫描成本随对象数量增长。
本地前缀清理每批枚举 256 个名称后再删除，可清理超过 10,000 项的单目录；不会跟随链接。

本地读写通过受控目录与文件句柄完成，逐级 no-follow，拒绝 junction/reparse point 和符号链接。
写入先写同一受控目录下的临时文件，同步内容后重命名；读取保持已打开的文件句柄，目录名被替换不会改变读取目标。

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
- `PATCH ...?appendFiles=attachments,images`：仅对列出的多文件字段追加；旧引用在前，新引用在后，组合数量仍不能超过 `maxSelect`。未列出的字段继续整体替换。
- `appendFiles` 拒绝未知、重复、非文件、单文件或本次没有上传 part 的字段；同一字段不能在一次请求中同时清空和追加。
- 扩展名不在 `extensions` 白名单时返回 `400 HB_VALIDATION_ERROR`；声明/推断 MIME 不匹配仍返回 `415 HB_UNSUPPORTED_MEDIA_TYPE`。

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

记录软删除保留文件。Collection 删除成功后执行 `records/{collection}` 前缀清理；失败保留持久化后置动作账本，启动时重试，完成前禁止重建同名集合。上传补偿依据事务已确认的提交/回滚状态执行，unknown 时保留对象待核对。

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

## 9. JavaScript 扩展文件

`$app.files` 使用当前 Storage 的专用 `extensions/` 前缀（可配置）。逻辑目录保存在数据库，
write/copy 每次生成不可变对象版本。覆盖先预留完整新对象大小；旧对象确认删除后才释放旧配额。
读取、元数据、默认 100/最大 500 条的 keyset 分页、copy/move/remove 以及 `e.file(key)` 已接入。
本地存储逐级使用受控目录句柄，拒绝符号链接和 Windows junction/reparse point。

启动恢复也分页扫描当前扩展前缀下的物理对象。只有服务生成的 UUID v7 对象名及 `.hb-{UUID v7}.tmp`
临时文件参与孤儿回收；版本账本或逻辑目录仍引用的对象、其他前缀及非服务对象均保留。
扫描与写入共用宿主操作锁，先持久化孤儿大小为待清理预留，再删除；失败继续计入配额并在重启后重试。
自定义 Storage 适配器未实现 `list_page` 时仍恢复操作账本，但不能执行物理孤儿扫描。

请求正文、`readBytes`、二进制 `write` 与 `e.file` 通过独立字节缓冲跨越 worker 边界。缓冲先预留额度，
取消 JS 等待者不会释放在途原生调用持有的字节。读回 JS 时复制到 QuickJS 管理的内存，继续受 JS 堆限制；
返回的数组与源视图、请求缓存和存储对象互不共享可变内存。

文件写入不参与 Record 数据库事务，活动事务内返回 `HB_SIDE_EFFECT_IN_TRANSACTION`；可在
afterCommit 中执行。move 为 copy 后 delete；删除失败返回 `HB_FILE_MOVE_PARTIAL`，包含已创建目标
和源对象的已确认状态。待清理对象仍计入配额，服务器启动时恢复操作账本；结果不确定且对象不存在时
继续保留预留，不以一次 HEAD 404 判定请求未送达。详细契约和实测范围见 [运行时设计](js-runtime.md#11-文件操作)。

管理员使用 `/api/admin/file-operations` 的 list/get/resolve 接口核对当前配置前缀内的未知写入。
确认后端已拒绝写入且没有仍可能创建对象的请求后，提交 `{ "resolution": "not_written", "note": "核对依据" }`。
服务仍要求对象当前不存在；对象存在、元数据不可用或版本已生效时拒绝释放。配额释放与核对回执
在同一数据库事务中持久化，重复请求返回原回执。回执保留管理员身份、时间和说明，不自动删除。
