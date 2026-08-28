# 客户端配置

```ts
const hb = new HertaBaseClient({
  baseUrl: "https://api.example.com",
  timeoutMs: 30_000,
  refreshSkewMs: 30_000,
  authStore,
  headers: async () => ({ "x-client-version": appVersion }),
  fetch: instrumentedFetch,
});
```

| 配置            | 默认值                  | 说明                                    |
| --------------- | ----------------------- | --------------------------------------- |
| `baseUrl`       | `http://localhost:8080` | 服务根 URL，不包含 `/api`               |
| `fetch`         | `globalThis.fetch`      | 可用于代理、测试、指标和自定义 TLS 环境 |
| `headers`       | 无                      | 静态 HeadersInit 或每请求执行的异步函数 |
| `timeoutMs`     | 30000                   | HTTP 默认超时；0 表示禁用               |
| `authStore`     | `MemoryAuthStore`       | Session 存储                            |
| `refreshSkewMs` | 30000                   | Access Token 提前刷新窗口               |

逐请求 `headers` 会覆盖默认同名 header。SDK 只在调用方没有提供 Authorization 时自动注入 Bearer Token；multipart 请求不会手工设置 Content-Type，以便运行时生成 boundary。

## 自定义 fetch

自定义函数必须兼容标准 fetch 签名并返回标准 `Response`。可在其中增加可观测性，但不要读取流式响应 body，也不要将 Authorization 输出到日志。

## Node 与浏览器

Node.js 20+ 原生提供 fetch、FormData、Blob 和 Web Streams。浏览器构建不包含 Node 内置模块。传 Blob 上传时，若对象不是带 name 的 File，应使用 `{ blob, filename }` 保留扩展名，服务端会基于扩展名和 MIME 校验。
