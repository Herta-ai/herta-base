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

* `HB_JWT_SECRET`：JWT 签名密钥（如果未设置且文件中无记录，系统首次启动将随机生成并写入配置）。
* `HB_JWT_ACCESS_TTL`：访问令牌存活时间。
* `HB_JWT_REFRESH_TTL`：刷新令牌存活时间。

**数据库连接**

* `HB_DB_ENGINE`：Phase 1 可选值为 `surrealkv`（默认，持久化）或 `memory`（仅供测试）。TiKV 集群支持留到 Phase 7。

**日志与安全**

* `HB_LOG_LEVEL`：日志级别 `trace`, `debug`, `info`, `warn`, `error`。
* `HB_LOG_FORMAT`：日志输出格式，可选 `json` (机器友好) 或 `pretty` (人眼友好)。
* `HB_CORS_ORIGINS`：逗号分隔的 CORS 允许来源列表。
* `HB_MAX_REQUEST_BODY_SIZE`：全局最大请求体大小限制。

**文件存储 (Phase 5)**

* `HB_STORAGE_TYPE`：存储策略 `local` 或 `s3`。
* `HB_S3_ENDPOINT`, `HB_S3_BUCKET`, `HB_S3_ACCESS_KEY`, `HB_S3_SECRET_KEY`：S3 云存储认证信息。

**JS Sandbox (Phase 3)**

* `HB_JS_MEMORY_LIMIT`：Hook 运行内存上限。
* `HB_JS_TIMEOUT_MS`：Hook 运行超时熔断时间。
* `HB_JS_NETWORK_ALLOWLIST`：Hook 允许访问的外网域名白名单。

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

[auth]
access_ttl = "15m"
refresh_ttl = "7d"
# jwt_secret = "YOUR_SUPER_SECRET_KEY" # 建议通过环境变量 HB_JWT_SECRET 提供

[security.cors]
origins = ["https://my-app.com", "https://admin.herta.ai"]

[jsvm]
memory_limit_mb = 16
timeout_ms = 100
network_allowlist = ["api.github.com"]

[storage]
type = "s3"
endpoint = "https://s3.us-east-1.amazonaws.com"
bucket = "hertabase-assets"
# access_key / secret_key 推荐经环境变量注入
```

## 5. 数据目录结构

HertaBase 将所有状态数据集中于 `--data-dir` (默认 `hb_data/`) 中，以便于无痛备份与迁移：

* `hb_data/database/` — SurrealDB 底层 SurrealKV 的键值对存储文件。
* `hb_data/storage/` — 本地存储模式下，用户上传的附件与文件存放于此。
* `hb_data/logs/` — 系统自动归档的持久化日志文件。
* `hb_data/backups/` — 数据库快照与自动备份。

## 6. 日志系统

集成 `tracing` 框架。生产环境下，建议设置 `HB_LOG_FORMAT=json` 与 `HB_LOG_LEVEL=info` 以配合 ELK/Fluentd 采集。开发调试时，系统自动切换为 `pretty` 终端带色彩输出，级别下调至 `debug` 以跟踪 SurrealQL 执行情况与 Hook 运行流。

## 7. 生产模式 vs 开发模式

* **开发模式 (`--dev`)**：跳过 CORS 源校验，放行所有 Origin；开启更详尽的路由匹配日志与详细错误栈注入到 HTTP 响应体中。
* **生产模式 (默认)**：启用所有安全头，严格校验 API Rules，错误响应被泛化屏蔽（避免内部路径泄露），日志仅报告严重异常。
