# HertaBase 数据模型

## 1. 概述

HertaBase 的底层数据存储构建在强大的 **SurrealDB** 之上，充分利用了其独特的文档-图 (Document-Graph) 混合数据库特性。作为 HertaBase 阶段 1 (Phase 1) 和阶段 2 (Phase 2) 的核心模块，`herta_db` 将抽象的数据模型动态映射至 SurrealDB，为开发者提供了极其灵活的数据结构设计以及强大且高效的关联查询能力。

## 2. 集合类型 (Collection Types)

系统中的表被称为集合 (Collections)，主要分为以下三大类：

- **基础集合 (Base Collection)**: 对应标准的文档集合，存储常规的业务数据（如文章、评论、商品等）。
- **鉴权集合 (Auth Collection)**: 继承自基础集合，系统会自动为其添加鉴权所需的专用字段（如 `email`, `password_hash`, `token_key`, `verified` 等）。该类型从 Phase 2 开始可创建；Phase 1 会明确拒绝 `type: auth`。
- **系统集合 (System Collections)**: 维护 HertaBase 内部运行状态的核心集合，不对普通用户开放：
  - `_admins`: 存储超级管理员信息。
  - `_collections`: 存储集合的 Schema 配置和元数据。
  - `_users`: 默认提供的一个鉴权集合模板。
  - `_hooks`: 用于记录 Hook 脚本状态（若配置为数据库驱动的 Hook）。

## 3. 字段类型映射 (Field Types Mapping)

在 HertaBase 的管理后台（或 Schema 定义中），开发者使用的字段类型会在 `herta_db` 中透明地转换为相应的 SurrealDB 类型：

| HertaBase 类型 | 描述 | 对应的 SurrealDB 类型 |
| --- | --- | --- |
| `text` | 普通文本或长文本 | `string` |
| `number` | 整数或浮点数 | `number` / `decimal` |
| `bool` | 布尔值 | `bool` |
| `datetime` | 日期和时间 | `datetime` |
| `json` | 任意 JSON 对象或数组 | `object` / `array` |
| `file` | 文件引用（对接 Phase 5 存储模块） | `string` (存储 FileID 或路径) |
| `relation` | 关联记录引用 | `record` |
| `select` | 枚举选项 | `string` / `array` (带约束校验) |
| `email` | 电子邮件地址 | `string` (带格式校验) |
| `url` | 网址链接 | `string` (带格式校验) |

## 4. Schema 模式 (Schema Modes)

根据业务需求的严格程度，HertaBase 支持不同的 Schema 校验模式：

- **Schema-less (无模式)**: 数据库接受任意额外字段。适合于快速原型开发或高度动态的数据结构。
- **Schema-full (严格模式)**: 严格限制仅允许在 Schema 中预定义的字段，拒绝包含未知字段的写入操作，保证数据结构的高一致性。
- **Mixed (混合模式)**: 强制校验设定的必填字段或格式，但也允许附加任意未定义的额外字段。

## 5. 系统字段 (System Fields)

HertaBase 为每条记录自动生成和维护以下系统级基础字段：

- `id`: 记录的唯一标识，格式为 `collection_name:random_string` (完全对应 SurrealDB 的 Record ID)。
- `created_at`: 记录创建时间的 ISO 8601 字符串或时间戳。
- `updated_at`: 记录最后更新时间的 ISO 8601 字符串或时间戳。
- `deleted_at`: 软删除时间（默认为 `null`，非 `null` 表示记录已被软删除）。系统默认查询会自动过滤已软删除的记录，并提供软删除恢复与硬删除（彻底物理删除）支持。

## 6. 关联查询 (Relations)

得益于 SurrealDB 的图数据库特性，HertaBase 在处理数据关联时非常高效。

- **关系类型**: 原生支持一对一 (One-to-one)、一对多 (One-to-many) 与多对多 (Many-to-many) 关系。
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
  ]
}
```

## 9. 结构迁移 (Migration)

Phase 1 提供非破坏性的增量 Schema 更新，只允许增加字段和索引。字段删除、改名、类型转换、索引删除以及迁移历史记录将在后续迁移阶段实现。
