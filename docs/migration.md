# 迁移与升级指南

本文档说明 HertaBase 系统的升级流程、Schema 迁移机制以及版本兼容策略。

## 1. 版本控制策略

HertaBase 严格遵循语义化版本控制（Semantic Versioning, SemVer）。版本号格式为 `MAJOR.MINOR.PATCH`。在 Pre-Alpha 阶段，可能出现频繁的版本更迭。

## 2. Schema 迁移策略

### 自动迁移

HertaBase 在启动时会自动跟踪 Collection Schema 的变更并应用所需的迁移。

### 迁移日志

所有的 Schema 修改历史和系统迁移记录均存储在 `_migrations` 系统表中，系统将根据该表记录比对需要执行的操作。

### 回滚支持

目前自动回滚功能正在计划中。

## 3. 升级流程

进行版本升级时，系统管理员需遵循以下步骤：

1. **备份数据**：在升级前完整备份数据目录。
2. **替换二进制文件**：使用新版本的 `hertabase` 二进制文件替换旧版本。
3. **执行迁移**：运行 `hertabase migrate`（或在 `hertabase serve` 启动时系统将自动应用迁移）。
4. **检查日志**：确认控制台及 `_migrations` 表中的迁移日志无异常报错。

## 4. 破坏性变更策略

- 主版本（Major）可能包含不兼容的 API 变更。
- 官方将针对主版本升级提供专用的迁移脚本。
- 次版本（Minor）中废弃的 API 或特性将触发弃用警告，并将在下一个主版本中移除。

## 5. 数据导出与导入

系统支持以下数据导出/导入方式：

- 借助 HertaBase CLI 进行 JSON 格式数据的导入导出（例如 `hertabase export` / `hertabase import`）。
- 使用 SurrealDB 的原生导出机制进行底层数据备份。

## 6. 存储引擎间迁移

随着项目发展，系统可能涉及从单机嵌入式向分布式引擎演进：

- **RocksDB 至 TiKV 迁移路径**：系统将提供特定的 CLI 指令，用于在不同存储引擎之间同步和转换数据。
- 版本升级中如遇数据目录结构变化，发行说明中将详细告知适配方案。

## 项目阶段 (Roadmap)

- Phase 1: 基础架构与动态 ORM — Salvo + SurrealDB 互通，动态 Collection CRUD，数据类型转换层，OpenAPI 自动生成
- Phase 2: 鉴权与权限引擎 — 用户系统（_users/_admins 表），JWT 签发与中间件，API Rules 动态规则引擎
- Phase 3: JS 扩展运行时 — rquickjs AsyncRuntime 集成，Rust→JS FFI 映射，生命周期 Hook 挂载与执行
- Phase 4: 实时订阅引擎 — 基于 SurrealDB LIVE SELECT + Salvo SSE 的数据变更推送
- Phase 5: 文件存储模块 — 抽象 Storage trait，LocalFS + S3 兼容云存储适配器
- Phase 6: 管理后台与单体打包 — Vue Admin UI 开发，rust-embed 静态嵌入，Salvo 静态文件服务
- Phase 7: 生产加固与分布式 — CLI 工具 (clap)，结构化日志 (tracing)，RocksDB→TiKV 集群支持
