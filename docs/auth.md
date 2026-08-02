# 鉴权与权限引擎 (Authentication & Permissions)

## 1. 概述

HertaBase 采用基于 JWT (JSON Web Token) 的无状态鉴权架构。系统将用户严格区分为系统管理员与普通终端用户，通过统一的 API Rules（动态规则引擎）实现细粒度的行列级访问控制 (Row-Level Security)。此模块属于 **Phase 2: 鉴权与权限引擎** 的核心实现。

## 2. 用户类型

HertaBase 将访问实体分为三种类型：

* **Admin 用户 (管理员)**：存储于内置的 `_admins` 系统表中。拥有系统的最高控制权，可完全绕过 API Rules 访问所有集合 (Collection) 与系统配置。
* **普通用户**：存储于标记为 "Auth" 类型的动态集合中（如 `users` 集合）。其数据访问受限于对应集合的 API Rules。
* **匿名/访客用户**：未携带有效身份凭据的请求。仅能访问 API Rules 显式允许公开访问的操作。

## 3. 鉴权流程 (Authentication Flow)

HertaBase 的标准鉴权流程如下：

1. **注册 (Registration)**：用户提供邮箱与密码进行注册。系统可配置开启可选的邮箱验证流程。
2. **登录 (Login)**：用户认证成功后，系统签发并返回一对 Token：
   * **Access Token**：用于后续 API 请求鉴权。
   * **Refresh Token**：用于在 Access Token 过期后获取新的 Token 凭据。
3. **令牌刷新 (Token Refresh)**：使用 Refresh Token 换取新的 Access Token 与 Refresh Token。
4. **JWT 结构**：
   * **Header**：包含算法信息 (如 `{"alg": "HS256", "typ": "JWT"}`)。
   * **Payload**：包含实体信息，如 `user_id` (用户标识), `collection` (所属集合), `role` (角色, 若有), 以及 `exp` (过期时间)。
   * **Signature**：基于配置的 Secret Key 生成的签名。

### 鉴权流程图 (文字描述)

客户端发送凭据 -> HertaBase 验证 (Argon2id) -> 成功则生成 JWT 载荷 -> 使用系统密钥签名 -> 返回 Access/Refresh Token -> 客户端在后续请求的 `Authorization: Bearer <token>` 头部携带 -> HertaBase 中间件验证签名与有效期 -> 解析身份信息附加至请求上下文。

## 4. JWT 配置与管理

* **密钥管理 (Secret Key)**：系统首次启动时将自动生成高强度随机密钥并持久化于配置中。
* **Access Token TTL**：默认有效期为 **15 分钟**，支持自定义配置。
* **Refresh Token TTL**：默认有效期为 **7 天**，支持自定义配置。
* **令牌轮转 (Token Rotation)**：Refresh Token 设计为一次性使用 (Single-use)，刷新后即刻失效，以降低凭据泄露风险。

## 5. API Rules 动态规则引擎

API Rules 是 HertaBase 安全模型的核心，负责针对不同操作进行基于属性的访问控制 (ABAC)。

* **规则定义**：规则配置于每个 Collection 上，并细化至具体的数据操作：`list` (列表), `view` (单条记录), `create` (创建), `update` (更新), `delete` (删除)。
* **规则语法**：采用完全兼容 SurrealQL 的布尔表达式。
* **内置变量**：
  * `@hb` 注入上下文：`$auth.id` (当前用户 ID), `$auth.email` (当前用户邮箱), `$auth.role` (当前用户角色)。
  * 数据记录上下文：`$record.user_id` (当前行记录的关联用户), 等等。
* **特殊规则值**：
  * `""` (空字符串)：拒绝所有访问 (Deny All)。
  * `true`：允许所有访问 (公开访问, Allow All)。
  * `null`：仅限 Admin 管理员访问 (默认安全策略)。
* **规则示例**：
  * 仅所有者可访问：`$auth.id = $record.user_id`
  * 基于角色的访问：`$auth.role = 'moderator'`

## 6. 安全措施 (Security Measures)

* **防注入**：所有 API Rules 在执行前均被编译为参数化的 SurrealQL WHERE 子句 (Parameterized Queries)，绝不使用字符串拼接。
* **输入清理**：规则评估执行前会经过严格的输入校验与净化。
* **速率限制 (Rate Limiting)**：针对注册、登录等敏感鉴权端点默认启用速率限制，防止恶意自动化攻击。
* **密码存储**：采用业界标准的 **Argon2id** 算法进行哈希处理。
* **防爆破机制**：登录接口集成指数退避 (Exponential Backoff) 机制与账户锁定，有效抵御暴力破解。

## 7. 演进路线

在未来的 **HertaBase 迭代中** (Phase 7 及以后)，将引入完整的 OAuth2 / OIDC (Single Sign-On) 支持，允许通过 Google、GitHub 等第三方身份提供商快速接入系统 (规划中)。
