# 教程：Node.js 服务

Node.js 20+ 可以直接使用 SDK，无需 fetch polyfill。

```ts
import { createWriteStream } from "node:fs";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { HertaBaseClient } from "@hb/sdk";

const hb = new HertaBaseClient({
  baseUrl: process.env.HB_URL,
  headers: { "user-agent": "hertabase-sync/1.0" },
});
```

长期服务应实现加密的 `AuthStore`，不要把 Refresh Token 写入普通日志或源码。进程重启后，通过 `setSession()` 恢复包含 scope 和 expiresAt 的完整 Session。

## 分页同步

```ts
let pageNumber = 1;
while (true) {
  const page = await hb.collection<RemoteRecord>("records").list({
    page: pageNumber,
    perPage: 500,
    sort: "created_at",
  });
  await persist(page.items);
  if (pageNumber * page.perPage >= page.total) break;
  pageNumber += 1;
}
```

## 流式文件下载

```ts
const response = await hb.files.download(reference);
if (!response.body) throw new Error("missing body");
await pipeline(Readable.fromWeb(response.body), createWriteStream(targetPath));
```

不要自动重试未知状态的 POST/PATCH；网络断开时服务端可能已经完成写入。对同步任务可在业务模型中增加幂等键。完整类型示例见 [../examples/node-service.ts](../examples/node-service.ts)。
