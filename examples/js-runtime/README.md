# 已实现运行时示例

从仓库根目录启动，沿用现有管理员初始化和数据库配置。PowerShell：

```powershell
$env:HB_JS_ENABLED='true'
$env:HB_HOOKS_DIR='./examples/js-runtime'
cargo run -p herta_server -- serve
```

Linux/macOS 使用同样变量：

```sh
HB_JS_ENABLED=true HB_HOOKS_DIR=./examples/js-runtime cargo run -p herta_server -- serve
```

bootstrap 会建立 `demo_posts`、`demo_audit` 和 `demo_members`。重新启动保留已有定义；热重载不会重新执行生命周期。

- `POST /api/auth/demo_members/register`，JSON 为 `{"email":"reader@example.com","password":"choose-a-password"}`：Auth Hook 补填必填 name，账户、刷新令牌和审计记录共同提交。
- 管理员登录后，带 Bearer 调用 `POST /api/custom/demo-transaction`，JSON 为 `{"title":"Hello World"}`：Record Hook 补填 slug，并与审计记录共同提交。增加 `"fail":true` 可验证二者均回滚。
- `GET /api/collections/demo_posts/records` 公开列出已提交文章；`GET /api/custom/demo-health` 返回 204。

`services.js` 另外注册每分钟的日志任务，以及三个管理员路由：

- `GET /api/custom/demo-http` 请求固定的 `https://example.com/`；需设置 `HB_JS_HTTP_ENABLED=true`、`HB_JS_HTTP_ALLOWLIST='["https://example.com"]'`。
- `POST /api/custom/demo-mail`，JSON 为 `{"address":"reader@example.com"}`；需按 [邮件配置](../../docs/mail.md) 启用 SMTP，并设置 `HB_JS_MAIL_ENABLED=true`。
- `POST /api/custom/demo-message`，JSON 为 `{"reportId":"one","memberId":"demo_members:..."}`；需设置 `HB_JS_REALTIME_ENABLED=true`。对应成员登录 SDK 后执行 `hb.realtime.subscribe('reports/ready', { onEvent: console.log })` 即可接收。发布无断线重放。

`cronAdd` 只注册任务，服务器在 serve 成功后开始调度；每分钟记录一次任务 ID。可设置 `HB_JS_CRON_ENABLED=false` 关闭。

- `POST /api/custom/demo-file` 写入 JSON，`GET /api/custom/demo-file` 返回该逻辑文件；需 `HB_JS_FILES_ENABLED=true`。覆盖需要预留完整新文件大小。
- `POST /api/custom/demo-outbox`，JSON 为 `{"address":"reader@example.com","key":"welcome-001"}`：审计记录和邮件任务共同提交；增加 `"fail":true` 验证共同回滚。需启用 SMTP、`HB_JS_MAIL_ENABLED=true`、`HB_JS_OUTBOX_ENABLED=true`。相同 key 和相同邮件复用任务；不同邮件返回 409。
- 管理员使用 `GET /api/admin/outbox`、`GET /api/admin/outbox/{id}` 查看状态。unknown 不自动重发；核实后调用 `POST /api/admin/outbox/{id}/resolve`，正文为 `{"resolution":"accepted","note":"接收端已确认"}` 或 `{"resolution":"not_sent","note":"已确认原尝试不再进行且未被接受"}`。后者会重新排队。
