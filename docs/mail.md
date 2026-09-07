# 邮件发送与内存测试邮箱

HertaBase 已实现 Rust `Mailer`、SMTP 发送及管理员邮件发送接口。测试邮箱使用 Node.js 的
[smtp-server](https://nodemailer.com/extras/smtp-server/) 接收、
[mailparser](https://nodemailer.com/extras/mailparser/) 解析邮件，提供网页和查询 API。
邮件数据全部存于内存，重启即清空，不需要 Docker、Redis 或数据库。

`$app.mailer.send`、JS Hook、注册验证、密码重置和事务 outbox 尚未实现。
Rust `Mailer::send` 是非事务操作，调用者必须确保已经脱离数据库事务。

## 1. 本地快速测试

需要 Rust、Node.js 20.19+ 和 pnpm。首次运行 `pnpm install --frozen-lockfile`。
在仓库根目录打开三个终端；以下固定管理员凭据仅用于新建的本地测试实例。

**终端 1：启动内存测试邮箱（PowerShell / POSIX 通用）**

```sh
pnpm dev:mailbox
```

SMTP 默认 `127.0.0.1:1025`，网页为 [http://127.0.0.1:8025](http://127.0.0.1:8025)。
该邮箱仅监听本机，无需认证，不支持 TLS，不向外转发邮件。

**终端 2：PowerShell 启动 HertaBase**

```powershell
$env:HB_MAIL_DRIVER = "smtp"
$env:HB_SMTP_HOST = "127.0.0.1"
$env:HB_SMTP_PORT = "1025"
$env:HB_SMTP_TLS = "none"
$env:HB_MAIL_FROM_ADDRESS = "noreply@example.com"
$env:HB_BOOTSTRAP_ADMIN_EMAIL = "admin@example.com"
$env:HB_BOOTSTRAP_ADMIN_PASSWORD = "correct horse battery staple"
cargo run -p herta_server -- serve --db-engine memory --dev --host 127.0.0.1
```

**终端 2：POSIX 启动 HertaBase**

```sh
export HB_MAIL_DRIVER=smtp
export HB_SMTP_HOST=127.0.0.1
export HB_SMTP_PORT=1025
export HB_SMTP_TLS=none
export HB_MAIL_FROM_ADDRESS=noreply@example.com
export HB_BOOTSTRAP_ADMIN_EMAIL=admin@example.com
export HB_BOOTSTRAP_ADMIN_PASSWORD='correct horse battery staple'
cargo run -p herta_server -- serve --db-engine memory --dev --host 127.0.0.1
```

已有生产 SMTP 凭据的终端应先移除 `HB_SMTP_USERNAME` / `HB_SMTP_PASSWORD` 再运行本地示例。
首次编译如缺少嵌入的管理页面，先运行 `pnpm build:sdk` 和 `pnpm build:ui`。
测试邮箱数据完全不落盘；HertaBase 自身仍可能创建数据目录、密钥和静态文件目录。

**终端 3：PowerShell 登录并发送**

```powershell
$login = Invoke-RestMethod -Method Post -Uri http://127.0.0.1:8080/api/admin/auth/login `
  -ContentType "application/json" `
  -Body '{"email":"admin@example.com","password":"correct horse battery staple"}'
$body = '{"to":[{"address":"reader@example.com"}],"subject":"HertaBase test","text":"Hello from HertaBase","html":"<strong>Hello from HertaBase</strong>","headers":{"X-Event-Id":"manual-test"}}'
$sent = Invoke-RestMethod -Method Post -Uri http://127.0.0.1:8080/api/admin/mail/send `
  -Headers @{ Authorization = "Bearer " + $login.data.accessToken } `
  -ContentType "application/json" -Body $body
$sent.data
$messageId = [uri]::EscapeDataString($sent.data.messageId)
Invoke-RestMethod "http://127.0.0.1:8025/api/messages?messageId=$messageId"
```

**终端 3：POSIX 登录并发送**

```sh
TOKEN=$(curl --fail-with-body -s http://127.0.0.1:8080/api/admin/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"admin@example.com","password":"correct horse battery staple"}' \
  | node --input-type=module -e 'let s="";for await(const c of process.stdin)s+=c;console.log(JSON.parse(s).data.accessToken)')
curl --fail-with-body -s http://127.0.0.1:8080/api/admin/mail/send \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"to":[{"address":"reader@example.com"}],"subject":"HertaBase test","text":"Hello from HertaBase","html":"<strong>Hello from HertaBase</strong>","headers":{"X-Event-Id":"manual-test"}}'
curl --fail-with-body -s 'http://127.0.0.1:8025/api/messages?recipient=reader%40example.com'
```

打开收件箱网页即可查看纯文本、隔离 HTML、邮件头，并下载 `.eml` 原文。
也可在 [Swagger UI](http://127.0.0.1:8080/swagger-ui/) 中授权管理员 JWT 后发送。

## 2. 发送契约

`POST /api/admin/mail/send` 仅接受管理员 Bearer Token，普通用户返回 403，匿名返回 401。

```json
{
  "from": { "address": "noreply@example.com", "name": "HertaBase" },
  "to": [{ "address": "reader@example.com", "name": "Reader" }],
  "subject": "Welcome",
  "text": "Welcome to HertaBase",
  "html": "<strong>Welcome to HertaBase</strong>",
  "headers": { "X-Event-Id": "event-123" }
}
```

`from` 可省略，使用配置默认值；`to` 与 `subject` 必填，`text` / `html` 至少一种非空。
仅支持 `X-*` 自定义头，禁止控制字符和 CR/LF 注入；未知字段（包括 attachments/cc/bcc）被拒绝。
发送回执使用与 MIME 原文一致的 Message-ID：

```json
{
  "data": { "messageId": "<uuid@hertabase.local>", "status": "accepted" },
  "meta": null,
  "error": null
}
```

`accepted` 表示 SMTP 接收端在 DATA 后确认接收，不代表公网邮箱最终投递。
测试中必须用 Message-ID 查询收件箱并核对内容。超时或连接中断可能发生在接收后，接口不自动重试。

| 错误 | HTTP | 含义 |
| --- | --- | --- |
| `HB_VALIDATION_ERROR` | 400 | 地址、发件人、主题、头字段或收件人数不合法 |
| `HB_PAYLOAD_TOO_LARGE` | 413 | 正文、自定义头或 HTTP 请求体超限 |
| `HB_CAPABILITY_UNAVAILABLE` | 503 | 邮件驱动为 disabled |
| `HB_MAIL_SEND_FAILED` | 502 | 连接、TLS、认证、收件人或 DATA 提交失败 |
| `HB_MAIL_TIMEOUT` | 504 | 提交超时，接收结果可能未知 |

所有调用均经过 `Mailer`；API 测试可以通过 `ApiState::new_with_services` 注入替身。
生产 SMTP 使用 `required`（直接 TLS，通常端口 465）或 `starttls`（强制升级，通常端口 587）；
均验证证书，不提供跳过证书验证选项。完整配置及默认值见 [配置参考](configuration.md)。

## 3. 内存收件箱接口和限制

| 方法和路径 | 功能 |
| --- | --- |
| `GET /api/messages?recipient=...&messageId=...` | 按 SMTP 信封收件人及 Message-ID 精确筛选，返回 `{messages,total,storedBytes}`，最新邮件在前 |
| `GET /api/messages/:id` | 解析详情，包含 envelope、from、to、subject、text、html、headers、headerLines |
| `GET /api/messages/:id/raw` | 下载原始 RFC 822 邮件 |
| `GET /api/messages/:id/preview` | 清理后的 HTML 预览，强制 CSP sandbox，禁止脚本、导航和外部资源 |
| `DELETE /api/messages/:id` | 删除单封，已不存在时返回 404 |
| `DELETE /api/messages` | 清空收件箱 |

网页每两秒刷新，邮件内容不写入浏览器持久化存储。收件箱拒绝跨源请求和非本机 Host。

| 环境变量 | 默认值 |
| --- | --- |
| `HB_MAILBOX_SMTP_PORT` | `1025` |
| `HB_MAILBOX_HTTP_PORT` | `8025` |
| `HB_MAILBOX_MAX_MESSAGE_BYTES` | `2097152`，单封原文字节上限 |
| `HB_MAILBOX_MAX_MESSAGES` | `100` |
| `HB_MAILBOX_MAX_TOTAL_BYTES` | `33554432`，累计原文字节上限 |

达到封数或累计原文容量时淘汰最早邮件。单封超过单封/累计容量上限时 SMTP 返回 552，不保存。
累计字节仅计原文；解析对象和网页服务还有额外内存开销。最多同时接入 10 个 SMTP 客户端。

程序可通过 `import { startMailbox } from '@hb/mailbox'` 启动，传入 `smtpPort: 0, httpPort: 0`
获得随机端口；返回 `{smtpPort,httpPort,url,close}`。`close()` 关闭监听和已有连接并清空邮件，支持重复调用。
仅供测试代码注入的 `behavior` 支持 `rejectRecipient`、`rejectData`、`delayMs`，不通过 HTTP 暴露。

## 4. 自动测试

```sh
pnpm --filter @hb/mailbox test
pnpm test:integration:mail
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

邮件集成命令会构建 SDK、管理页面和 debug 服务程序，创建独立临时数据目录及随机端口，
启动内存数据库，通过管理员 HTTP 请求发送真实 SMTP 邮件，再核对收件内容与唯一 Message-ID。
还覆盖权限、校验、禁用服务、拒收、超时、TLS 不降级与连接失败；结束时清理进程、端口和临时目录。
