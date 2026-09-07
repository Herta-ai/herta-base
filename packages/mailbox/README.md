# HertaBase 内存测试收件箱

Node.js 20.19+，无需数据库或 Docker。仓库根目录运行：

```sh
pnpm dev:mailbox
```

SMTP：`127.0.0.1:1025`，网页：[http://127.0.0.1:8025](http://127.0.0.1:8025)。
邮件只保存在内存里，重启清空；不向外转发。

```js
import { startMailbox } from "@hb/mailbox";

const mailbox = await startMailbox({ smtpPort: 0, httpPort: 0 });
console.log(mailbox.smtpPort, mailbox.url);
// Connect an SMTP client, then query `${mailbox.url}/api/messages`.
await mailbox.close();
```

完整配置、接口与发送步骤见 [邮件发送](../../docs/mail.md)。
单独测试：`pnpm --filter @hb/mailbox test`；完整链路：`pnpm test:integration:mail`。
