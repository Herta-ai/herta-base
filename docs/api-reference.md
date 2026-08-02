# HertaBase API 参考文档

## 1. 概述

HertaBase 采用 RESTful 风格的 API 设计理念，致力于提供清晰、一致且易于使用的接口。API 的设计以资源为中心，使用标准的 HTTP 动词（GET、POST、PATCH、DELETE）进行操作，并充分利用 HTTP 状态码来表示请求的结果。所有接口均返回标准化的 JSON 结构，以便于前端和客户端的解析与处理。

作为 HertaBase 七个核心阶段（Phases）的重要组成部分，第一阶段（基础架构与动态 ORM）已经确立了动态 Collection CRUD 以及自动生成的 OpenAPI 规范，为后续的鉴权（Phase 2）与实时订阅（Phase 4）打下了坚实的基础。

## 2. 基础 URL 结构

所有的公共 API 路由均以前缀 `/api/` 开头。例如：
`https://your-hertabase-domain.com/api/`

管理端 API 则以 `/_/` 作为特殊标识。

## 3. 认证接口 (Authentication)

认证接口处理用户注册、登录以及会话管理，对应 HertaBase 鉴权与权限引擎（Phase 2）。

### POST /api/auth/register

- **描述**: 注册新用户并创建记录。
- **请求体**:

  ```json
  {
    "email": "user@example.com",
    "password": "securepassword123",
    "passwordConfirm": "securepassword123"
  }
  ```

- **响应示例**:

  ```json
  {
    "data": { "id": "user_123", "email": "user@example.com" },
    "meta": {},
    "error": null
  }
  ```

- **所需权限**: 公开

### POST /api/auth/login

- **描述**: 用户登录并签发 JWT。
- **请求体**:

  ```json
  {
    "email": "user@example.com",
    "password": "securepassword123"
  }
  ```

- **响应示例**:

  ```json
  {
    "data": {
      "token": "eyJhbGciOiJIUzI1NiIsInR...",
      "user": { "id": "user_123", "email": "user@example.com" }
    },
    "meta": {},
    "error": null
  }
  ```

- **所需权限**: 公开

### POST /api/auth/refresh

- **描述**: 刷新当前的 JWT Token。
- **所需权限**: 已认证用户

### GET /api/auth/me

- **描述**: 获取当前登录用户的详细信息。
- **所需权限**: 已认证用户

## 4. 集合操作接口 (Collection CRUD)

所有动态集合的数据操作，接口路径中的 `{collection}` 替换为实际集合名称。

### GET /api/collections/{collection}/records

- **描述**: 获取记录列表。支持分页、排序和过滤。
- **请求示例**: `GET /api/collections/posts/records?page=1&perPage=20`
- **响应示例**:

  ```json
  {
    "data": [
      { "id": "post_1", "title": "Hello HertaBase" }
    ],
    "meta": { "total": 1, "page": 1, "perPage": 20 },
    "error": null
  }
  ```

- **所需权限**: 由 API Rules 动态决定 (Phase 2)

### GET /api/collections/{collection}/records/{id}

- **描述**: 获取单条记录。
- **所需权限**: 由 API Rules 动态决定

### POST /api/collections/{collection}/records

- **描述**: 创建一条新记录。
- **请求体示例**:

  ```json
  { "title": "New Post", "content": "Welcome to HertaBase." }
  ```

- **所需权限**: 由 API Rules 动态决定

### PATCH /api/collections/{collection}/records/{id}

- **描述**: 部分更新指定记录。
- **请求体示例**:

  ```json
  { "title": "Updated Title" }
  ```

- **所需权限**: 由 API Rules 动态决定

### DELETE /api/collections/{collection}/records/{id}

- **描述**: 删除指定记录。
- **所需权限**: 由 API Rules 动态决定

## 5. 实时订阅接口 (Realtime Subscription)

基于 Phase 4（实时订阅引擎）的实现。

### GET /api/realtime/{collection}

- **描述**: 建立 Server-Sent Events (SSE) 连接，实时接收 SurrealDB 数据变更推送。
- **所需权限**: 由 API Rules 动态决定

## 6. 文件存储接口 (File Storage)

基于 Phase 5（文件存储模块）的实现，兼容 LocalFS 与 S3。

### POST /api/files/upload

- **描述**: 上传文件，返回存储元数据。
- **所需权限**: 由 API Rules 动态决定（通常需要认证用户）

### GET /api/files/{fileId}

- **描述**: 获取/下载文件内容。
- **所需权限**: 由文件权限决定

## 7. 管理端接口 (Admin API)

这些接口保留给管理控制台使用（Phase 6）。

- **集合管理**: `/_/collections` 等，用于 Schema 动态转换与管理。
- **用户管理**: `/_/users` 等，管理系统中的所有用户及超级管理员 `_admins`。

## 8. 通用查询参数

CRUD 和列表接口广泛支持以下参数：

- `page`: 当前页码（默认 1）。
- `perPage`: 每页记录数（默认 30）。
- `sort`: 排序规则，例如 `sort=-created_at,title` （`-` 表示降序）。
- `filter`: SurrealQL 风格的条件过滤，例如 `filter=(status='active')`。
- `expand`: 关联字段的急切加载（Eager Loading），例如 `expand=author,comments`。

## 9. 标准响应格式

所有接口（除流式接口外）均返回以下 JSON 封套（Envelope）格式：

```json
{
  "data": { ... },     // 请求成功时的数据载荷
  "meta": { ... },     // 分页信息或其他元数据
  "error": {           // 请求失败时的错误详情（成功时为 null）
    "code": 404,
    "message": "Record not found",
    "details": {}
  }
}
```

## 10. 速率限制 (Rate Limiting)

为了保证服务的可用性，HertaBase API 将提供请求速率限制。客户端可参考以下响应头：

- `X-RateLimit-Limit`: 周期内允许的最大请求数。
- `X-RateLimit-Remaining`: 当前周期内剩余的请求数。
- `X-RateLimit-Reset`: 速率限制重置的 UNIX 时间戳。
