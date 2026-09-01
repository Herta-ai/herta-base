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

**邮件服务**

* `HB_MAIL_DRIVER`：邮件驱动，可选 `disabled`（默认）或 `smtp`。
* `HB_MAIL_FROM_ADDRESS`, `HB_MAIL_FROM_NAME`：默认发件地址和显示名称。
* `HB_SMTP_HOST`, `HB_SMTP_PORT`, `HB_SMTP_USERNAME`, `HB_SMTP_PASSWORD`：SMTP 连接信息。
* `HB_SMTP_TLS`：TLS 策略，可选 `required`, `starttls`, `none`；生产环境不允许 `none`。

邮件服务配置只负责建立宿主 `Mailer`。脚本还必须通过 `HB_JS_MAIL_ENABLED` 单独获得发送授权，
SMTP 凭据永远不会暴露给 `$app.env()`。

**JS Sandbox (Phase 3)**

* `HB_JS_ENABLED`：是否启用 JS 扩展运行时。
* `HB_JS_MEMORY_LIMIT_MB`, `HB_JS_STACK_LIMIT_KB`：单个执行上下文的内存与栈上限。
* `HB_JS_EXECUTION_TIMEOUT_MS`：同步 JS CPU 执行上限。
* `HB_JS_ASYNC_TIMEOUT_MS`：包含数据库和外部 I/O 的单次总时长上限。
* `HB_JS_POOL_SIZE`, `HB_JS_QUEUE_CAPACITY`：运行时池大小和有界等待队列容量。
* `HB_JS_RAW_QUERY_ENABLED`：是否允许使用受审计的 `$app.db.query`。
* `HB_JS_ENV_ALLOWLIST`：`$app.env()` 可读取的非敏感环境变量名列表。
* `HB_JS_HTTP_ENABLED`, `HB_JS_HTTP_ALLOWLIST`：是否允许出站 HTTP 及 `host:port` 白名单。
* `HB_JS_HTTP_TIMEOUT_MS`, `HB_JS_HTTP_MAX_RESPONSE_BYTES`：出站请求总超时和响应上限。
* `HB_JS_FILES_ENABLED`, `HB_JS_FILES_ROOT`：是否允许文件操作及其沙盒根目录。
* `HB_JS_FILES_QUOTA_BYTES`, `HB_JS_FILES_MAX_FILE_BYTES`：文件总配额和单文件上限。
* `HB_JS_MAIL_ENABLED`：是否允许通过宿主 Mailer 发送邮件。
* `HB_JS_CRON_ENABLED`, `HB_JS_CRON_TIMEZONE`：是否启用定时任务及默认 IANA 时区。

`HB_JS_HTTP_ALLOWLIST` 只接受明确的 HTTP(S) 主机与端口，不能用于放开私网和云元数据
地址。SMTP、JWT、数据库和对象存储密钥不得加入 `HB_JS_ENV_ALLOWLIST`。

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

[jsvm]
enabled = true
memory_limit_mb = 16
stack_limit_kb = 512
execution_timeout_ms = 100
async_timeout_ms = 5000
pool_size = 4
queue_capacity = 128
raw_query_enabled = false
env_allowlist = ["PUBLIC_APP_URL"]

[jsvm.http]
enabled = false
allowlist = ["api.github.com:443"]
max_redirects = 3
max_request_bytes = 1048576
max_response_bytes = 4194304
timeout_ms = 5000

[jsvm.files]
enabled = false
root = "./hb_data/js-files"
quota_bytes = 104857600
max_file_bytes = 10485760

[jsvm.mail]
enabled = false
max_recipients = 20
max_message_bytes = 1048576

[jsvm.cron]
enabled = true
timezone = "UTC"
max_runtime_ms = 30000

[mail]
driver = "disabled"
from_address = "noreply@example.com"
from_name = "HertaBase"

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
