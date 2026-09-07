# 邮件链路集成测试

在仓库根目录运行 `pnpm test:integration:mail`。需要 Node.js 20.19+、pnpm 和 Rust 编译环境。

测试会构建 debug HertaBase，启动独立内存数据库和 SMTP 收件箱，调用管理员接口发送邮件，
通过 Message-ID 查询真实 SMTP 收件内容。测试同时覆盖鉴权、校验、拒收、超时、认证失败及 TLS 不降级。
所有 SMTP 目标都在本机随机端口；临时目录、进程和端口在结束时清理。

详见 [邮件发送文档](../../../docs/mail.md)。
