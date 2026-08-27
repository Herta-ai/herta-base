# HertaBase 数据模型

## 1. 概述

HertaBase 的底层数据存储构建在强大的 **SurrealDB** 之上，充分利用了其独特的文档-图 (Document-Graph) 混合数据库特性。作为 HertaBase 阶段 1 (Phase 1) 和阶段 2 (Phase 2) 的核心模块，`herta_db` 将抽象的数据模型动态映射至 SurrealDB，为开发者提供了极其灵活的数据结构设计以及强大且高效的关联查询能力。

## 2. 集合类型 (Collection Types)

系统中的表被称为集合 (Collections)，主要分为以下三大类：

- **基础集合 (Base Collection)**: 对应标准的文档集合，存储常规的业务数据（如文章、评论、商品等）。
- **鉴权集合 (Auth Collection)**: 继承自基础集合，系统会自动添加 `email`, `password_hash`, `token_key`, `verified`, `role` 和登录锁定字段，并创建唯一邮箱索引。敏感字段不会通过 REST 返回。
- **系统集合 (System Collections)**: 维护 HertaBase 内部运行状态的核心集合，不对普通用户开放：
  - `_admins`: 存储超级管理员信息。
  - `_collections`: 存储集合的 Schema 配置和元数据。
  - `_users`: 默认提供的一个鉴权集合模板。

Phase 3 的 JS 扩展来自 `hb_hooks/`，编译结果和注册表驻留内存，不使用 `_hooks` 系统集合。
需要可靠投递的外部副作用应使用单独的 outbox 业务集合，而不是把执行状态混入扩展注册表。

## 3. 字段类型映射 (Field Types Mapping)

在 HertaBase 的管理后台（或 Schema 定义中），开发者使用的字段类型会在 `herta_db` 中透明地转换为相应的 SurrealDB 类型：

| HertaBase 类型 | 描述 | 对应的 SurrealDB 类型 |
| --- | --- | --- |
| `text` | 普通文本或长文本 | `string` |
| `number` | 整数或浮点数 | `number` / `decimal` |
| `bool` | 布尔值 | `bool` |
| `datetime` | 日期和时间 | `datetime` |
| `json` | 任意 JSON 对象或数组 | `object` / `array` |
| `file` | 记录绑定的服务端文件引用 | `string` 或 `array<string>`，由 `maxSelect` 决定 |
| `relation` | 关联记录引用 | `record<collection>` 或 `array<record<collection>>` |
| `select` | 枚举选项 | `string` / `array` (带约束校验) |
| `email` | 电子邮件地址 | `string` (带格式校验) |
| `url` | 网址链接 | `string` (带格式校验) |

### file 字段选项

`options.maxSelect` 默认 `1`；值为 `1` 时存储服务端生成的文件名字符串，大于 `1` 时存储字符串数组，最大为 `100`。`options.maxSize` 限制单文件字节数，`options.mimeTypes` 和 `options.extensions` 分别限制 MIME 与不带点的扩展名。

文件引用不包含目录分隔符，客户端不能通过 JSON 写入非空引用。文件上传使用记录 POST/PATCH 的 multipart 协议；PATCH 缺席保留、`null`/`[]` 清空、同字段上传整体替换，也可用 `appendFiles` 对多文件字段追加。详见 [文件存储与上传](storage.md)。

## 4. Schema 模式 (Schema Modes)

根据业务需求的严格程度，HertaBase 支持不同的 Schema 校验模式：

- **Schema-less (无模式)**: 数据库接受任意额外字段。适合于快速原型开发或高度动态的数据结构。
- **Schema-full (严格模式)**: 严格限制仅允许在 Schema 中预定义的字段，拒绝包含未知字段的写入操作，保证数据结构的高一致性。
- **Mixed (混合模式)**: 强制校验设定的必填字段或格式，但也允许附加任意未定义的额外字段。

## 5. 系统字段 (System Fields)

HertaBase 为每条记录自动生成和维护以下系统级基础字段：

- `id`: 记录的唯一标识，格式为 `collection_name:random_string` (完全对应 SurrealDB 的 Record ID)。所有 API 响应、鉴权用户、JWT `sub` 和实时事件都返回完整 ID。
- `created_at`: 记录创建时间的 ISO 8601 字符串或时间戳。
- `updated_at`: 记录最后更新时间的 ISO 8601 字符串或时间戳。
- `deleted_at`: 软删除时间（默认为 `null`，非 `null` 表示记录已被软删除）。系统查询和普通 CRUD 会过滤已软删除记录；当前没有恢复或公开硬删除接口。

## 6. 关联查询 (Relations)

得益于 SurrealDB 的图数据库特性，HertaBase 在处理数据关联时非常高效。

- **关系类型**: 原生支持 `record<target>` 单值和 `array<record<target>>` 多值关系。写入必须使用完整的 `target:key`，不能使用裸 key、空 ID、错误集合或畸形 ID；`maxSelect` 限制多值数组长度，必填多值关系不能为空。响应会将原生 RecordId 归一化为完整字符串，并在 `expand` 中保留原关系 ID。
- **图数据边 (Graph Edges)**: HertaBase 可以利用 SurrealDB 提供的边 (Edges) 来定义记录之间的关系，从而突破传统关系型数据库在多对多连接查询时的性能瓶颈。
- **Eager Loading (`expand` 参数)**: 客户端通过 API 请求时，只需在请求体中附加 `?expand=author,comments.user` 等参数，`herta_db` 层会自动将其转换为相应的 SurrealQL 并获取完整数据结构，无需发起多次请求。

## 7. 索引与性能注意事项 (Indexes and Performance)

系统允许为集合定义不同类型的索引以优化查询：

- **普通索引 (Index)**: 针对高频过滤 (filter) 的字段。
- **唯一索引 (Unique Index)**: 确保某个字段或多字段组合的数据唯一性。
在规划数据模型时，应针对应用常用的 `filter` 及 `sort` 条件添加对应索引，特别是当数据集规模庞大时（如集群环境 Phase 7 下）。

## 8. Schema 定义格式 (Schema Definition Format)

每个集合的数据模型在后台由一个 JSON 结构描述，保存在 `_collections` 中。示例：

```json
{
  "name": "posts",
  "type": "base",
  "schema_mode": "strict",
  "fields": [
    {
      "name": "title",
      "type": "text",
      "required": true
    },
    {
      "name": "author",
      "type": "relation",
      "options": {
        "collection": "users",
        "maxSelect": 1
      }
    }
  ],
  "indexes": [
    { "name": "idx_author", "fields": ["author"] }
  ],
  "rules": {
    "list": true,
    "view": "$record.author = $auth.record",
    "create": "$record.author = $auth.record",
    "update": "$record.author = $auth.record",
    "delete": null
  }
}
```

## 9. 结构迁移 (Migration)

字段和索引更新仍是只追加的；PATCH 可以整体替换 `rules`。规则的 `null` 表示仅管理员，`false` 或空字符串表示拒绝，`true` 表示公开，字符串是经过 AST 校验的 SurrealQL 标量布尔表达式。规则上下文中 `$auth.id` 是完整 ID 字符串，兼容文本比较；`$auth.record` 是原生 `RecordId`，用于 relation 比较。创建规则中的 `$record` 已将 relation 字段转换为原生 RecordId。

记录路径 `{id}` 同时接受完整 `collection:key` 和裸 `key`。完整 ID 的集合名必须与路径集合一致，否则返回 404。普通列表和 CRUD 默认过滤 `deleted_at` 非空的软删除记录；软删除后没有恢复接口，更新和重复删除都会被拒绝。
