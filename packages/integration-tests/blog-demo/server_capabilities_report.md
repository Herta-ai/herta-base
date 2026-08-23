# 博客业务服务端能力报告

本报告仅包含通过 `tests/blog.spec.ts` 在真实的 `hertabase` 进程与内存数据库环境下验证通过的行为。

## 已验证的行为特性

- Release 版本的服务端二进制可以在动态选择的回环端口（Loopback Port）上启动，通过公共 OpenAPI 端点进行就绪检测，并支持停止后再次启动，不依赖固定端口。
- Bootstrap 初始化管理员登录正常工作，管理员可以创建基础集合（Base Collection）和鉴权集合（Auth Collection）。
- Auth 注册与登录均返回标准的响应 Envelope 结构。Auth 用户 ID 与 JWT `sub` Claim 均采用完整的 `collection:key` 形式。
- 无模式集合（Schema-less Collection）在保留额外字段的同时，仍能对预声明的字段进行格式与约束校验。
- 关联关系（Relation）写入必须使用完整的 ID 格式，且会拒绝裸 Key、空 ID、畸形 ID 以及指向错误目标集合的 ID。
- 记录路径（Record Path）既接受完整 ID，也接受裸 Key。若路径中的完整 ID 指向了其他集合，则返回 404。
- `$auth.record` 支持在 list、view、create、update 和 delete 规则中进行原生 Relation 所有权校验。`$auth.id` 保持为完整的字符串 ID，用于规则与请求 JSON 的比较。
- 公开文章对匿名访客可见；私有文章仅对其作者和管理员可见。
- 文章作者在创建时不可伪造，在常规更新时也不可转移。跨用户更新与删除操作将返回 403 `HB_FORBIDDEN`。
- `expand=author` 会保留原有的完整 Relation ID，并附加已授权展开的关联记录数据。
- 公开文章下的评论对公众可见；私有文章下的评论仅限文章作者、评论作者与管理员查看，且外部用户无法创建私有文章的评论。
- 管理员可完全绕过集合的业务规则（API Rules）。
- 被软删除的记录会从 GET 和列表查询结果中自动消失，且后续针对该记录的 PATCH 或 DELETE 请求均会被拒绝。
- 成功与失败请求均采用文档规范的 `{ data, meta, error }` Envelope 结构，包含稳定的 HTTP 状态码与 Herta 错误码。

## 本测试套件未覆盖的范围

- 持久化数据库引擎、从实验性字符串关联数据的迁移、集群或多进程行为。
- 多租户隔离、组织架构层级、分级管理员角色，或超出上述博客规则之外的行级安全策略。
- Refresh Token 轮转、账户防爆破锁定、邮箱验证、密码找回、OAuth 第三方登录或 Realtime 实时订阅。
- Multipart 多部分文件存储、S3/LocalFS 存储行为、JS Hooks、自定义路由、部署、备份或性能/负载极限。
- 通用的 PocketBase 规则语法（如 `@request.auth.id`）；本套件仅验证 HertaBase 的 `$auth`、`$record` 和 `$request` 语法。
- 软删除数据恢复 API 或公开的硬删除 API。
