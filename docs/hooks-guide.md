# HertaBase Hook 开发指南

## 1. 概述

在 HertaBase 的架构中（Phase 3: JS 扩展运行时），Hook 提供了一种机制，允许开发者在核心数据操作的生命周期中注入自定义逻辑。Hook 本质上是 Serverless 的 JavaScript 函数，它们通过 Rust 侧集成的 `rquickjs` 引擎在沙盒环境中执行。通过 Rust 到 JS 的 FFI 映射，开发者可以直接在 JS 中访问数据库和应用上下文，实现诸如数据校验、自动填充、级联操作以及发送外部通知等功能。

## 2. 文件命名与存放规范

- **存放位置**: 所有的 Hook 脚本必须存放在项目工作目录的 `hb_hooks/` 文件夹中。
- **命名规范**: 文件名必须遵循 `{lifecycle}_{collection}.js` 的模式。
  - 例如：`before_create_posts.js`
- **可用生命周期 (Lifecycles)**:
  - `before_create`, `after_create`
  - `before_update`, `after_update`
  - `before_delete`, `after_delete`
  - `before_list`, `after_list`

## 3. 全局 `$app` API 参考

沙盒环境自动注入了 `$app` 全局对象，提供与底层 Rust 核心（`herta_core` / `herta_db`）交互的桥梁。

- `$app.db.query(sql, vars?)`: 执行原生的 SurrealQL 查询，返回 Promise。
- `$app.db.select(table)`: 查询指定表中的所有记录。
- `$app.db.create(table, data)`: 在指定表中创建新记录。
- `$app.db.update(table, data)`: 更新指定记录（数据需包含 id）。
- `$app.db.delete(table, id)`: 删除指定 id 的记录。
- `$app.logger.info(msg) / .warn(msg) / .error(msg)`: 记录结构化日志，对应后端的 `tracing` 系统（Phase 7）。
- `$app.env(key)`: 读取环境变量。出于安全考虑，仅允许读取在配置文件中明确白名单 (allowlisted) 的环境变量。

## 4. Hook 上下文对象 (Context)

每次 Hook 触发时，都会注入一个名为 `context` 的全局对象，包含当前操作的详细信息。

- `context.request`: 触发该操作的 HTTP 请求信息（如由 API 触发），包含 headers、body、method、path。
- `context.user`: 当前认证的用户信息。如果请求未认证，则为 `null`。
- `context.collection`: 当前操作的目标集合名称。
- `context.record`: 当前操作的具体记录数据（主要在 `after_*` 钩子中提供完整数据，或在 `before_update` 中提供当前状态）。

## 5. 返回值语义

Hook 函数的执行结果将直接影响数据库操作的最终走向：

- **允许操作**: `return true` 或者没有显式返回（返回 `undefined`）。
- **拒绝操作**: `return false`，HertaBase 会终止当前操作，并向客户端返回 403 Forbidden 错误。
- **自定义错误拒绝**: `throw new Error("Custom message")`，操作将被终止，并向客户端返回 400 Bad Request 以及该自定义错误信息。

## 6. 异步支持 (Async Support)

基于 `rquickjs` 的 AsyncRuntime 集成，所有的 `$app.db` 方法都返回 Promises，且支持标准的 async/await 语法。Hook 函数可以（且推荐）定义为异步函数。

## 7. 沙盒限制 (Sandbox Limitations)

为保证系统的稳定和安全，JS 运行时沙盒在 `herta_jsvm` 层面实施了严格限制：

- **内存限制**: 单次执行默认最大可用内存为 16MB（可配置）。
- **执行超时**: 单次执行时间默认限制为 100 毫秒，超时将被强制中断（可配置）。
- **网络访问**: 默认禁止发起外部网络请求。可以通过配置 allowlist 开启 `fetch` 权限以调用特定的外部 API。
- **文件系统访问**: 完全禁止访问主机文件系统。

## 8. TypeScript 支持

通过使用 HertaBase 提供的 `@hb/types` 包，开发者可以在编写 Hook 时获得完整的 IDE 自动补全和类型检查功能。
只需在 JS 文件的顶部加入：

```javascript
/// <reference types="@hb/types" />
```

## 9. 完整示例

### 示例 1: 输入校验与自动填充字段 (before_create_posts.js)

```javascript
/// <reference types="@hb/types" />

async function main() {
    // 数据校验
    if (!context.request.body.title) {
        throw new Error("文章标题不能为空");
    }

    // 自动填充发布者ID和摘要
    context.request.body.author_id = context.user.id;
    context.request.body.excerpt = context.request.body.content.substring(0, 100);

    $app.logger.info(`Processing new post by user ${context.user.id}`);
    return true;
}

await main();
```

### 示例 2: 级联删除与发送通知 (after_delete_users.js)

```javascript
/// <reference types="@hb/types" />

async function main() {
    const deletedUserId = context.record.id;
    
    // 级联删除用户的所有文章
    await $app.db.query("DELETE posts WHERE author_id = $id", { id: deletedUserId });
    
    // 发送外部通知 (需网络访问白名单支持)
    $app.logger.info(`User ${deletedUserId} deleted, resources cleaned up.`);
}

await main();
```

## 10. 调试 (Debugging)

若要查看 Hook 的执行日志以及 `$app.logger` 输出，可通过 `hertabase` 命令行工具启动服务并观察控制台输出，或者在管理后台的系统日志面板中进行查阅。开启结构化日志跟踪后，错误堆栈与上下文信息将更易于追踪。
