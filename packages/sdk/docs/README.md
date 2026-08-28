# HertaBase SDK 文档

本文档对应 `@hb/sdk` 0.1.x，并以当前 HertaBase Rust 路由和响应结构为准。

## 能力矩阵

| 能力                       | 主入口 | Admin 入口             | 说明                         |
| -------------------------- | ------ | ---------------------- | ---------------------------- |
| 用户注册、登录、刷新、`me` | 是     | 管理员登录、刷新、`me` | 支持动态 Auth Collection     |
| Collection CRUD            | 是     | 是                     | Admin Token 自动绕过业务规则 |
| filter、sort、expand、分页 | 是     | 是                     | 与 REST API 参数一致         |
| multipart 文件字段         | 是     | 是                     | 支持替换、追加和清空         |
| 文件令牌、下载、HEAD       | 是     | 是                     | 返回原始 `Response`          |
| SSE 实时订阅               | 是     | 是                     | 支持自动重连，无历史重放     |
| Collection Schema 管理     | 否     | 是                     | `@hb/sdk/admin` 独立入口     |
| 日志和网页部署             | 否     | 是                     | 管理员专用                   |

## 阅读顺序

1. [快速开始](getting-started.md)
2. [认证与会话](authentication.md)
3. [Collection 与记录](collections.md)
4. [文件](files.md)
5. [实时订阅](realtime.md)
6. [错误处理](errors.md)

按需阅读[管理端](admin.md)、[客户端配置](configuration.md)和[API 参考](api-reference.md)。完整应用示例位于 [tutorials](tutorials/)；参与 SDK 开发时，可编译示例位于 [examples](examples/)。

## 兼容约定

- Relation 值始终使用完整 `collection:key` ID。
- 服务端返回的记录字段保持原始命名，不做 camelCase 转换。
- JSON 接口成功后自动解包 `data`；列表额外将 `meta` 转换为 `Page<T>`。
- 文件内容不是 JSON envelope，SDK 保留原始 `Response`。
