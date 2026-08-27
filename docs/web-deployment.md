# 网页部署与静态托管

本文档定义 HertaBase 托管用户前端构建产物的行为。网页部署与静态托管已实现，包含管理员
上传、原子发布、版本备份/回滚、动态路由和静态响应缓存控制。内置 Admin UI 仍独立实现并
通过 `/webui/` 提供，不使用本文的 `/web/{project}/` 路由或数据目录。

## 1. 项目模型

每个网页项目对应数据目录下的一个独立目录。默认数据目录为 `./hb_data`，因此项目 `xxx`
的默认落盘位置为：

```text
./hb_data/web/xxx/
```

`xxx` 同时是压缩包内的根目录名和默认项目名。项目名是一个 URL 路径段，解码后最长 64 个
字符。为保证 URL 和文件系统安全，名称不能包含 `/`、`\`、`?`、`#`、NUL、绝对路径或
`.`、`..` 路径段；其他 URL 字符在请求中按标准百分号编码。

项目的默认访问路由为 `/web/xxx/`。访问不带尾斜杠的 `/web/xxx` 时，服务端返回
`308 Permanent Redirect` 到 `/web/xxx/`。该路由下的所有子路径都由同一项目处理，例如
`/web/xxx/assets/app.js`。

## 2. Admin API

网页项目仅能由管理员通过 Admin API 管理，Admin UI 使用同一组接口。普通用户凭据返回
`403 HB_FORBIDDEN`；缺少管理员凭据返回 `401 HB_AUTH_REQUIRED`，已提供但无效的凭据返回 `401 HB_UNAUTHORIZED`。

目标接口如下：

| 方法 | 路径 | 用途 |
| --- | --- | --- |
| `GET` | `/_/web-projects` | 列出项目及数据库中的路由设置 |
| `POST` | `/_/web-projects` | 以 `multipart/form-data` 上传并部署项目压缩包 |
| `GET` | `/_/web-projects/{project}` | 查看项目配置和当前部署状态 |
| `PATCH` | `/_/web-projects/{project}` | 修改别名、SPA fallback、缓存及 404 设置 |
| `DELETE` | `/_/web-projects/{project}` | 删除当前目录和数据库配置，并立即撤销路由 |
| `GET` | `/_/web-projects/{project}/versions` | 从备份目录读取可回滚版本 |
| `POST` | `/_/web-projects/{project}/rollback` | 将指定备份版本恢复为当前版本 |

创建或更新使用同一个上传接口。`POST /_/web-projects` 的 multipart 字段为：

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `archive` | 是 | ZIP、`tar.gz` 或 7z 压缩包；项目名取包内唯一根目录名 |
| `alias` | 否 | `/web/` 开头的路由别名 |
| `spaFallback` | 否 | 是否启用 SPA fallback，默认 `true` |
| `cacheControl` | 否 | 项目静态响应的 `Cache-Control` 策略 |
| `notFound` | 否 | 项目目录内自定义 404 文件的相对路径 |

`PATCH /_/web-projects/{project}` 使用 JSON 请求体修改 `alias`、`spaFallback`、
`cacheControl` 和 `notFound`，不接受项目改名或文件内容。回滚接口请求体为：

```json
{ "version": "YYYY-MM-DD-HH-mm-ss" }
```

所有非流式管理接口使用 HertaBase 标准 JSON envelope。压缩格式应根据内容识别，不能只信任
客户端文件名或 `Content-Type`。

上传接口接受 ZIP、`tar.gz` 和 7z。压缩包文件大小上限默认为 100 MB；超过上限时返回
`413 HB_PAYLOAD_TOO_LARGE`。本阶段不限制压缩比、文件数量、单文件大小或解压后的总大小，
部署者需要自行监控 `HB_DATA_DIR` 所在文件系统的剩余空间。该 multipart 上传端点使用独立的
`HB_WEB_MAX_ARCHIVE_SIZE`，不受普通 JSON API 的 `server.max_body_size` 默认值限制。

## 3. 上传包结构

压缩包根层级必须且只能包含一个非空目录，不能直接包含文件；该目录名即项目名 `xxx`。
项目根目录不强制包含 `index.html`。

有效结构：

```text
site.zip
└── xxx/
    ├── index.html
    ├── assets/
    └── favicon.ico
```

以下结构无效，因为文件直接位于压缩包根目录：

```text
site.zip
├── index.html
└── assets/
```

包含多个根目录、根目录之外的额外文件或只有空目录的压缩包也必须拒绝。ZIP、`tar.gz` 和
7z 使用完全相同的结构检查。

## 4. 安全解压与发布

解压器必须拒绝以下压缩包条目，并拒绝整个上传：

- 软链接和硬链接。
- 包含 `..`、绝对路径、盘符、UNC 路径或其他路径穿越形式的条目。
- 解码并规范化后会写出项目临时目录的条目。

高压缩比本身不作为拒绝条件，除 100 MB 压缩包上限外不设置其他归档资源限制。

新项目应先解压到 `HB_DATA_DIR/web` 下的临时目录，通过全部校验后再以同文件系统内的原子
重命名发布到 `HB_DATA_DIR/web/xxx`。上传失败时不得留下可访问的半成品目录或数据库配置。

## 5. 新建与更新

目标目录不存在时，本次上传创建新项目。目标目录已存在时，本次上传视为更新，不再返回冲突：

1. 完整校验并解压新压缩包到临时目录。
2. 将当前 `HB_DATA_DIR/web/xxx` 复制到版本备份目录。
3. 将新版本发布到 `HB_DATA_DIR/web/xxx`。
4. 更新数据库中的项目配置和当前部署信息。

若备份、发布或数据库更新失败，服务端必须避免暴露半成品，并尽可能恢复更新前的当前目录及
配置。更新时间戳采用服务端本地时间，目录格式为：

```text
./hb_data/web_backup/xxx/YYYY-MM-DD-HH-mm-ss/
```

备份目录内直接保存该版本的项目文件。版本历史不写入数据库；版本列表通过读取
`HB_DATA_DIR/web_backup/xxx` 的合法时间戳子目录生成。同一项目在同一秒内不能生成两个同名
备份，发生时间戳冲突时本次更新应返回冲突，不能覆盖已有备份。

## 6. 项目配置与路由

项目名、可选别名、SPA fallback、缓存策略和自定义 404 设置存入数据库。文件系统目录不是
路由配置的唯一来源。

项目可以设置路由别名，但别名必须以 `/web/` 开头，并且是规范化的绝对 URL 路径，例如：

```text
/web/docs
/web/customer-portal
```

别名只改变 HTTP 访问路由，不改变项目落盘目录。别名必须唯一，不能覆盖 HertaBase 内置路由，
也不能与其他项目的默认路由或别名冲突。别名根路径同样执行无尾斜杠到有尾斜杠的 `308`
重定向。

服务端收到项目删除请求后，删除 `HB_DATA_DIR/web/xxx` 和对应数据库配置，并立即撤销默认路由
及别名。删除当前项目不自动删除 `HB_DATA_DIR/web_backup/xxx`，以保留版本历史；备份清理策略
需要另行管理。删除项目不会删除数据库 Collection、Auth 用户、文件存储对象或其他应用数据。

## 7. SPA History Fallback 与 404

每个项目默认开启类似 Nginx `try_files` 的 history fallback：

1. 请求路径对应项目内的普通文件时，返回该文件。
2. 请求路径对应目录中的索引文件时，返回该索引文件。
3. 路径不存在且项目根目录存在 `index.html` 时，返回该文件并由前端路由接管。
4. 路径不存在且项目缺少 `index.html` 时，返回项目的自定义 404；未配置时返回系统默认 404。

项目可以关闭 SPA fallback。关闭后，不存在的文件或目录直接返回项目自定义 404，未配置时
返回系统默认 404，不再回退到 `index.html`。自定义 404 文件必须位于当前项目目录内，其相对
路径保存在项目数据库配置中。

fallback 和自定义 404 必须限制在当前项目目录内，不能访问其他项目、版本备份或 HertaBase
内置资源。404 响应始终使用 HTTP `404` 状态码，即使响应体来自项目文件。

## 8. HTTP 静态文件行为

- 为普通文件生成 ETag，并处理 `If-None-Match`，匹配时返回 `304 Not Modified`。
- 支持项目级缓存策略，并使用数据库配置生成相应的 `Cache-Control` 响应头。
- 支持字节 Range 请求；合法单范围请求返回 `206 Partial Content`，不可满足的范围返回 `416`。
- 不检测或优先提供预压缩的 `.br`、`.gz` 文件，也不提供预压缩配置。
- MIME 类型根据实际文件扩展名确定，未知类型使用安全的二进制默认值。

## 9. 版本历史与回滚

版本接口只读取 `HB_DATA_DIR/web_backup/xxx`，不依赖数据库版本记录。回滚请求必须指定目录名
格式的版本，例如 `YYYY-MM-DD-HH-mm-ss`。服务端验证该版本确实位于目标项目的备份目录后：

1. 删除当前 `HB_DATA_DIR/web/xxx` 目录。
2. 将指定的备份目录复制到 `HB_DATA_DIR/web/xxx`。
3. 保持现有数据库路由配置不变，并立即继续通过原默认路由和别名提供服务。

回滚不能接受任意文件系统路径，且不能修改或删除被选中的备份目录。复制失败时必须返回错误，
不得将不完整目录作为成功回滚结果。由于版本历史不入库，手动删除备份目录后，该版本会自动
从版本列表消失。
