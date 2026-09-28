# HertaBase 配置参考 (Configuration Reference)

## 1. 配置加载优先级

HertaBase (`hertabase` 二进制文件) 采用分层配置系统，优先级从高到低依次为：

1. **CLI 命令行参数** (优先级最高，覆盖所有其他配置)
2. **环境变量** (Environment Variables)
3. **配置文件** (`hertabase.toml` 或指定配置)
4. **系统默认值** (内置缺省选项)

## 2. CLI 命令与标志位

HertaBase 通过基于 `clap` 构筑的 CLI 工具进行管理：

* `hertabase serve` — 启动核心服务器
    * `--host`, `-H`：绑定地址 (默认: `0.0.0.0`)
    * `--port`, `-p`：监听端口 (默认: `8080`)
    * `--data-dir`：数据存储主目录 (默认: `./hb_data`)
    * `--hooks-dir`：JS Hook 脚本存放目录 (默认: `./hb_hooks`)
    * `--dev`：启动开发者模式 (自动放宽 CORS 限制，开启调试日志，禁用某些生产缓存)
* `hertabase superuser create` — 交互式创建 Admin 账户
* `hertabase superuser list` — 列出当前系统中的管理员列表
* `hertabase migrate` — 扫描并运行挂起的数据库 Schema 迁移
* `hertabase version` — 打印 HertaBase 版本与构建信息

## 3. 环境变量 (Environment Variables)

系统支持通过 `HB_` 前缀的环境变量控制所有核心行为：

**网络与路径**

* `HB_HOST`, `HB_PORT`：监听地址与端口。
* `HB_DATA_DIR`, `HB_HOOKS_DIR`：数据与扩展脚本目录。

**鉴权配置**

* `HB_JWT_SECRET`：至少 32 字节的 JWT 签名密钥。未设置时自动生成并原子写入 `HB_DATA_DIR/auth/jwt-secret`。
* `HB_BOOTSTRAP_ADMIN_EMAIL`, `HB_BOOTSTRAP_ADMIN_PASSWORD`：仅当 `_admins` 为空时创建首个管理员；必须同时提供，密码至少 12
  个字符。
* `HB_AUTH_ACCESS_TOKEN_TTL_SECONDS`：Access Token 存活秒数，默认 `900`。
* `HB_AUTH_REFRESH_TOKEN_TTL_SECONDS`：Refresh Token 存活秒数，默认 `604800`。
* `HB_AUTH_LOCKOUT_THRESHOLD`, `HB_AUTH_LOCKOUT_SECONDS`：登录失败锁定阈值和时长，默认 `5` 次、`900` 秒。
* `HB_AUTH_REGISTER_RATE_LIMIT_PER_MINUTE`：单 IP 注册限流，默认 `5`。
* `HB_AUTH_LOGIN_RATE_LIMIT_PER_MINUTE`：单 IP 登录限流，默认 `10`。
* `HB_AUTH_REFRESH_RATE_LIMIT_PER_MINUTE`：单 IP 刷新限流，默认 `30`。

**实时订阅配置**

* `HB_REALTIME_MAX_CONNECTIONS`：全局 SSE 连接上限，默认 `1000`。
* `HB_REALTIME_MAX_CONNECTIONS_PER_IP`：单 IP SSE 连接上限，默认 `20`。
* `HB_REALTIME_HEARTBEAT_SECONDS`：SSE 心跳间隔，默认 `30` 秒。
* `HB_REALTIME_RECONCILIATION_SECONDS`：实时订阅一致性校验间隔，默认 `30` 秒。

以上四个值必须大于零。操作系统文件描述符上限仍需在部署环境中单独配置。

**网页部署配置（Phase 6）**

* `HB_WEB_MAX_ARCHIVE_SIZE`：网页项目压缩包大小上限，默认 `104857600` 字节（100 MiB）；
  该值独立于普通 API 的 `HB_MAX_REQUEST_BODY_SIZE`。

网页项目文件和版本备份均位于 `HB_DATA_DIR` 下；别名、SPA fallback、缓存及 404 设置存入数据库。

**数据库连接**

* `HB_DB_ENGINE`：Phase 1 可选值为 `surrealkv`（默认，持久化）或 `memory`（仅供测试）。TiKV 集群支持留到 Phase 7。

**日志与安全**

* `HB_LOG_LEVEL`：日志级别 `trace`, `debug`, `info`, `warn`, `error`。
* `HB_LOG_FORMAT`：日志输出格式，可选 `json` (机器友好) 或 `pretty` (人眼友好)。
* `HB_LOG_SERVER_PERSIST_ENABLED`：是否将服务端日志写入 `_logs`，默认 `true`。
* `HB_LOG_SERVER_PERSIST_LEVEL`：服务端日志入库最低级别，默认 `info`；支持 `trace`, `debug`, `info`, `warn`, `error`。
* `HB_LOG_HTTP_PERSIST_ENABLED`：是否将 HTTP 请求元数据写入 `_logs`，默认 `true`。
* `HB_CORS_ORIGINS`：逗号分隔的 CORS 允许来源列表。
* `HB_MAX_REQUEST_BODY_SIZE`：全局最大请求体大小限制。

**文件存储 (Phase 5)**

* `HB_STORAGE_TYPE`：`local`（默认）或 `s3`。
* `HB_STORAGE_MAX_FILE_SIZE`：全局单文件字节上限，默认 `10485760`。
* `HB_STORAGE_FILE_TOKEN_TTL_SECONDS`：文件令牌有效期，默认 `300`，范围 1 到 86400 秒。
* `HB_S3_ENDPOINT`, `HB_S3_BUCKET`, `HB_S3_REGION`, `HB_S3_PREFIX`：S3 endpoint、bucket、region 和对象前缀。
* `HB_S3_FORCE_PATH_STYLE`, `HB_S3_ALLOW_HTTP`：兼容 S3 服务的 path-style 与开发环境 HTTP 开关。
* `HB_S3_ACCESS_KEY`, `HB_S3_SECRET_KEY`, `HB_S3_SESSION_TOKEN`：S3 凭据，仅允许通过环境变量提供。

生产模式拒绝 HTTP S3 endpoint。LocalFS 固定写入 `HB_DATA_DIR/storage`。完整说明见 [文件存储与上传](storage.md)。

**邮件服务（已实现）**

* `HB_MAIL_DRIVER`：邮件驱动，可选 `disabled`（默认）或 `smtp`。
* `HB_MAIL_FROM_ADDRESS`, `HB_MAIL_FROM_NAME`：默认 `noreply@example.com`、`HertaBase`。
* `HB_MAIL_ALLOWED_FROM_ADDRESSES`：逗号分隔的额外允许发件地址；默认仅允许默认发件地址。
* `HB_MAIL_MAX_RECIPIENTS`：单封收件人数上限，默认 `50`。
* `HB_MAIL_MAX_SUBJECT_BYTES`：主题 UTF-8 字节上限，默认 `998`。
* `HB_MAIL_MAX_BODY_BYTES`：text 与 html 合计 UTF-8 字节上限，默认 `1048576`。
* `HB_MAIL_MAX_HEADER_BYTES`：自定义邮件头名称和值合计 UTF-8 字节上限，默认 `8192`；仅允许 `X-*` 头。
* `HB_MAIL_TIMEOUT_MS`：一次 SMTP 提交的总超时，默认 `10000` 毫秒，不自动重试。
* `HB_SMTP_HOST`, `HB_SMTP_PORT`, `HB_SMTP_USERNAME`, `HB_SMTP_PASSWORD`：SMTP 连接信息，默认端口 `587`；用户名和密码须同时提供，均为空时不认证。
* `HB_SMTP_TLS`：`required` 为连接时直接 TLS，`starttls`（默认）为强制 STARTTLS；两者验证证书且不降级。`none` 仅在 `server.dev_mode=true` 或 CLI `--dev` 下允许。

以上配置对应 `[mail]` / `[mail.smtp]` 同名字段，环境变量覆盖 TOML，CLI 覆盖后统一校验；所有限制和超时必须大于零。
SMTP 凭据可从 TOML 或环境变量读取，不进入配置序列化、调试输出和 API 响应。推荐使用环境变量注入凭据。
管理员可调用 `POST /api/admin/mail/send`。JS 邮件调用已接入，还需 `HB_JS_MAIL_ENABLED` 单独授权，
SMTP 凭据永远不会暴露给 `$app.env()`。完整使用与测试步骤见 [邮件发送](mail.md)。

**JS Sandbox（已接入配置；完整能力仍在实施）**

`HbConfig.jsvm` 和以下 `HB_JS_*` 环境变量已接入严格校验。`HB_JS_ENABLED=true` 后从
`HB_HOOKS_DIR` / `--hooks-dir` 加载脚本；目录不存在或首次加载失败会终止启动，存在的空目录允许启动。
Record/Auth/Collection、生命周期、自定义路由、数据库、邮件、受限 HTTP、应用消息和 cron 已接线。
扩展文件和 outbox 已接入；两者默认关闭。outbox 的邮件和 HTTP 任务还分别受对应能力开关与目标策略约束。
数组环境变量使用 JSON 数组，布尔值使用 true/false；字段默认值见 [JavaScript 扩展运行时设计](js-runtime.md)
第 15 节，实测范围见 [验证报告](js-runtime-validation.md)。

* `HB_JS_ENABLED`：是否启用 JS 扩展运行时，默认 false。
* `HB_JS_MEMORY_LIMIT_MB`, `HB_JS_STACK_LIMIT_KB`：整个调用 runtime 的堆/栈上限，默认 16 MiB/512 KiB。
* `HB_JS_EXECUTION_TIMEOUT_MS`：累计 JS 活跃执行片段的墙钟预算，默认 100 毫秒；等待宿主 I/O 时暂停累计，不是 OS CPU 时间。
* `HB_JS_ASYNC_TIMEOUT_MS`：包含数据库和外部 I/O 的根调用总时长，默认 5000 毫秒。
* `HB_JS_STARTUP_TIMEOUT_MS`, `HB_JS_QUEUE_TIMEOUT_MS`, `HB_JS_SHUTDOWN_TIMEOUT_MS`：候选验证、排队和 JS 排空时限，默认 10000/1000/30000 毫秒。
* `HB_JS_POOL_SIZE`, `HB_JS_QUEUE_CAPACITY`：并行根调用数和等待队列容量，默认 4/128；每次新建 runtime，嵌套 Hook 不重复申请槽。
* `HB_JS_MAX_RESPONSE_BYTES`, `HB_JS_MAX_BRIDGE_BYTES`：单个路由响应、单次 FFI 入/出参上限，各 4194304 字节。
* `HB_JS_MAX_HOST_BUFFER_BYTES`：每根调用同时持有的宿主缓冲预留，默认 16777216 字节，释放后归还。
* `HB_JS_MAX_PENDING_HOST_CALLS`, `HB_JS_MAX_HOST_CALLS`：每根调用在途/累计宿主调用数，默认 32/256。
* `HB_JS_MAX_HOOK_DEPTH`, `HB_JS_MAX_LOGS`, `HB_JS_MAX_LOG_BYTES`：嵌套深度、累计日志条数/字节数，默认 8/100/65536。
* `HB_JS_ROUTE_PREFIXES`：可注册的路由前缀，默认 `["/api/"]`，不能覆盖内置保留路径。
* `HB_JS_RAW_QUERY_ENABLED`：是否允许受限 `$app.db.query`，默认 false；开启也只允许 system 模式单表只读查询。
* `HB_JS_ENV_ALLOWLIST`：`$app.env()` 可读取的非敏感环境变量名列表，默认空。
* `HB_JS_HTTP_ENABLED`, `HB_JS_HTTP_ALLOWLIST`：出站 HTTP 开关和精确 origin 白名单，默认 false/空。
* `HB_JS_HTTP_CONNECT_TIMEOUT_MS`, `HB_JS_HTTP_TIMEOUT_MS`：连接/总超时，默认 3000/5000 毫秒，再受调用剩余预算限制。
* `HB_JS_HTTP_MAX_REDIRECTS`, `HB_JS_HTTP_MAX_REQUEST_BYTES`, `HB_JS_HTTP_MAX_RESPONSE_BYTES`：重定向次数、请求/响应字节上限，默认 3/1048576/4194304。
* `HB_JS_FILES_ENABLED`, `HB_JS_FILES_PREFIX`：文件操作开关和当前 Storage 内的扩展前缀，默认 false/`extensions`，不提供独立本地 root。
* `HB_JS_FILES_QUOTA_BYTES`, `HB_JS_FILES_MAX_FILE_BYTES`：文件配额和单文件上限，默认 104857600/10485760 字节；仍受 FFI/响应上限限制。
* `HB_JS_MAIL_ENABLED`, `HB_JS_MAIL_MAX_RECIPIENTS`, `HB_JS_MAIL_MAX_BODY_BYTES`：邮件授权、收件人数和 text+html 字节上限，默认 false/20/1048576；JS 上限只能收紧已有 MailConfig。
* `HB_JS_REALTIME_ENABLED`, `HB_JS_REALTIME_MAX_MESSAGE_BYTES`, `HB_JS_REALTIME_PUBLISH_PER_SECOND`：应用消息授权、单消息字节数和每秒发布数，默认 false/65536/100；独立于现有集合 SSE。
* `HB_JS_REALTIME_MAX_AUDIENCE`, `HB_JS_REALTIME_CONNECTION_QUEUE_CAPACITY`, `HB_JS_REALTIME_CONNECTION_QUEUE_BYTES`：单次目标数、单连接队列条数/字节数，默认 100/64/262144。
* `HB_JS_CRON_ENABLED`, `HB_JS_CRON_TIMEZONE`, `HB_JS_CRON_MAX_RUNTIME_MS`：cron 开关、默认 IANA 时区和每次尝试总时限，默认 true/UTC/30000；仅在 JS 开启时生效，替代普通调用总时限。
* `HB_JS_CRON_RETRIES`, `HB_JS_CRON_MAX_RETRIES`：默认重试次数及任务可配置上限，默认 0/3，需任务显式声明幂等。

新增环境变量统一由 `jsvm.*` 字段路径转成 `HB_JS_*`，嵌套点改为下划线。JS 列表采用 JSON
数组字符串，布尔值为 true/false，环境变量覆盖 TOML，未知 jsvm 字段和非法值拒绝启动。
这不改变已有 `HB_MAIL_ALLOWED_FROM_ADDRESSES` 的逗号列表格式；现有邮件配置未启用未知字段拒绝。
`HB_JS_HTTP_ALLOWLIST` 示例为 `["https://api.example.com:443"]`，只授权该 scheme/host/port，
不能放开私网和云元数据地址。SMTP、JWT、数据库和对象存储密钥不得加入 `HB_JS_ENV_ALLOWLIST`。

## 4. 配置文件示例 (hertabase.toml)

```toml
# hertabase.toml - 核心配置文件

[server]
host = "127.0.0.1"
port = 8080
dev_mode = false
max_body_size = 10485760 # 10MB

[paths]
data_dir = "./hb_data"
hooks_dir = "./hb_hooks"

[database]
engine = "surrealkv"

[log]
level = "info"
format = "pretty"
server_persist_enabled = true
server_persist_level = "info"
http_persist_enabled = true

[auth]
access_token_ttl_seconds = 900
refresh_token_ttl_seconds = 604800
lockout_threshold = 5
lockout_seconds = 900
register_rate_limit_per_minute = 5
login_rate_limit_per_minute = 10
refresh_rate_limit_per_minute = 30

[realtime]
max_connections = 1000
max_connections_per_ip = 20
heartbeat_seconds = 30
reconciliation_seconds = 30

[web]
max_archive_size = 104857600

[security.cors]
origins = ["https://my-app.com", "https://admin.herta.ai"]

# jsvm 及外部能力默认关闭；逐项启用所需适配器。
[jsvm]
enabled = false
memory_limit_mb = 16
stack_limit_kb = 512
execution_timeout_ms = 100
async_timeout_ms = 5000
pool_size = 4
queue_capacity = 128
raw_query_enabled = false
env_allowlist = []

[jsvm.http]
enabled = false
allowlist = [] # 例如 ["https://api.github.com:443"]
max_redirects = 3
max_request_bytes = 1048576
max_response_bytes = 4194304
connect_timeout_ms = 3000
timeout_ms = 5000

[jsvm.files]
enabled = false
prefix = "extensions"
quota_bytes = 104857600
max_file_bytes = 10485760

[jsvm.mail]
enabled = false
max_recipients = 20
max_body_bytes = 1048576

[jsvm.cron]
enabled = true
timezone = "UTC"
max_runtime_ms = 30000
retries = 0
max_retries = 3

[jsvm.outbox]
enabled = false
lease_seconds = 60
renew_seconds = 20
max_retries = 3
retention_days = 7
concurrency = 4
max_jobs = 10000
idempotent_origins = [] # 必须同时在 HTTP allowlist 中；接收端应实现 Idempotency-Key 去重

[mail]
driver = "disabled"
from_address = "noreply@example.com"
from_name = "HertaBase"
allowed_from_addresses = []
max_recipients = 50
max_subject_bytes = 998
max_body_bytes = 1048576
max_header_bytes = 8192
timeout_ms = 10000

[mail.smtp]
host = "smtp.example.com"
port = 587
username = ""
password = ""
tls = "starttls"

[storage]
type = "s3"
max_file_size = 10485760
file_token_ttl_seconds = 300

[storage.s3]
endpoint = "https://s3.us-east-1.amazonaws.com"
bucket = "hertabase-assets"
region = "us-east-1"
prefix = "hertabase"
force_path_style = false
allow_http = false
# access key / secret key 只能经环境变量注入
```

## 5. 数据目录结构

HertaBase 将所有状态数据集中于 `--data-dir` (默认 `hb_data/`) 中，以便于无痛备份与迁移：

* `hb_data/database/` — SurrealDB 底层 SurrealKV 的键值对存储文件。
* `hb_data/auth/jwt-secret` — 自动生成的 HS256 密钥；必须与数据库一起备份并限制文件权限。
* `hb_data/storage/` — 本地存储模式下，用户上传的附件与文件存放于此。
* `hb_data/web/` — 网页部署功能托管的前端项目目录；每个直接子目录代表一个项目。
* `hb_data/web_backup/` — 网页项目按项目名和时间戳组织的文件版本历史，不写入数据库。
* `hb_data/logs/` — 系统自动归档的持久化日志文件。
* `hb_data/backups/` — 数据库快照与自动备份。

## 6. 日志系统

集成 `tracing` 框架。生产环境下，建议设置 `HB_LOG_FORMAT=json` 与 `HB_LOG_LEVEL=info` 以配合 ELK/Fluentd 采集。开发调试时，系统自动切换为
`pretty` 终端带色彩输出，级别下调至 `debug` 以跟踪 SurrealQL 执行情况与 Hook 运行流。

服务端日志和 HTTP 请求日志独立控制入库。服务端日志默认写入 `info` 及以上级别；HTTP 日志默认开启，
只记录方法、路径、状态码、身份类型与 ID、referer、连接 IP、user-agent 和创建时间，不记录请求头、请求体或响应体。

## 7. 生产模式 vs 开发模式

* **开发模式 (`--dev`)**：跳过 CORS 源校验，放行所有 Origin；开启更详尽的路由匹配日志与详细错误栈注入到 HTTP 响应体中。
* **生产模式 (默认)**：启用所有安全头，严格校验 API Rules，错误响应被泛化屏蔽（避免内部路径泄露），日志仅报告严重异常。
