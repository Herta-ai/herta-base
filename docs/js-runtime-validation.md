# JavaScript 运行时实施验证

当前处于实施阶段，完整六步验收尚未完成。测试通过只代表下面明确列出的范围，不代表六步计划已全部完成。

## 2026-09-09 阶段验证

2026-09-09，Windows x64/MSVC，Rust 1.97.1：

| 检查 | 结果 |
| --- | --- |
| workspace check | 通过，包含服务器运行时装配 |
| workspace/all-targets clippy，`-D warnings` | 通过；vendored 依赖保留上游 dead_code 和功能别名弃用警告 |
| workspace tests | 通过，包含原有 Auth/API/数据库/存储回归和新增运行时用例；其后新增列表视图、严格 Auth profile 更新和示例也已分别通过 |
| QuickJS worker | 同步/Promise 循环中断、OOM、栈溢出、未处理拒绝、I/O 等待预算、队列、关闭、初始化微任务、重放、原子重载、异步分支隔离均有通过用例 |
| Mem 和 SurrealKv | 事务内可见性、回滚、Schema/Record 共用 session、冲突、提交后 LIVE、丢弃提交等待、原身份/核对凭证访问控制通过 |
| JS + Mem/SurrealKv | pool_size=1 嵌套保存、必填字段补填、Rules 原始输入、捕获内层失败仍回滚、required unset、afterCommit 执行通过 |
| Collection + Mem/SurrealKv | 版本变化、陈旧 save、无变化 save、禁止修改已有 Schema、并发提交的 409/已回滚核对状态、DDL 回滚与删除清理前禁止重建通过 |
| 原 HTTP 回归 | 16 项通过，包含 operation 状态查询 |
| 新 JS/HTTP 集成 | 4 项通过：路由参数、正文缓存/限额、鉴权前置、原始响应/envelope/204、HEAD/OPTIONS、滑动限流；JSON/multipart 补填字段、嵌套写入回滚、上传账本清理；afterCommit 死循环后的已提交结果；邮件门禁/收据 |
| SDK | 19 项测试、typecheck、lint 通过，含 204 返回 undefined |
| Auth/Collection 集成 | 注册双事件顺序、密码只处理一次、凭据隔离、必填 profile 补填、账户/令牌共同回滚、捕获内层失败、登录验证前置、刷新重放保护、Auth afterCommit 中断、严格 Auth profile 更新通过；集合创建/更新/删除 Hook、原型隔离、权限、只读视图、Schema 混入 Record 事务拒绝、OpenAPI/文件后置动作通过 |
| @hb/types | 已接通 API 的声明和正反向类型测试通过；未完成的外部适配器尚不纳入声明 |
| 可运行示例 | examples/js-runtime/main.js 的 bootstrap/serve/shutdown、双写事务回滚及注册补填通过集成测试 |
| 完整 release、SDK/类型、全部 HTTP/Auth/外部能力集成 | 尚未完成本次最终验收 |
| Linux、macOS | 未执行，不计入通过项 |

## 2026-09-28 增量验证

2026-09-28 增量实测（Windows x64/MSVC）：

- `herta_http` 11 项通过：混合 DNS 地址拒绝、固定解析结果、重定向 rebinding/白名单/跨源凭据清理、307/308 保留方法和正文、跳转次数上限、流式字节限额、总超时不重发、子进程环境代理隔离、受信 TLS 原主机校验、无效 CA 和 HTTPS 降级拒绝。
- core 日历用例和 jsvm 调度用例通过：六段 AND、范围/步长、纽约 DST 跳时/重复、Lord Howe 半小时 DST、重载同名互斥/旧快照重试、稳定 runId/新 attemptId/全局状态隔离、停止重试、任务总预算、错误候选保留旧快照。
- API 应用消息 5 项通过：跨 Auth Collection audience 隔离、排队期间角色变更/令牌撤销、慢消费/字节额度断开、登录/连接配额/断连清理、真实到期 SSE、JS 事务拒绝及 afterCommit 发布一次。
- workspace check 和 all-targets clippy 通过；SDK 23 项测试通过，新增 topic 独立模型、AbortSignal、无重放重连和令牌刷新；修正测试声明后 typecheck 通过。
- FileService 7 项通过（其中 1 项为崩溃测试子进程入口）：本地/内存逻辑目录、分页、同键并发覆盖、完整新对象预留、失败清理保持配额、move 部分成功、权限/事务门禁、SurrealKv 进程直接退出后重启恢复；缺失的未知 PUT 继续保留配额。
- outbox 4 项集成和 1 项退避状态用例通过：Mem/SurrealKv 共用事务、重复键/不同 payload 冲突、捕获错误仍 rollback-only、SMTP unknown 不重发、管理员核对、过期租约恢复、陈旧 worker 不得确认新租约、终态保留与 unknown 不清除、能力撤销、有界停服。
- JS/API 集成增至 14 项并全部通过，包含二进制文件/e.file/HEAD、outbox 与 Record 共同回滚、运维接口鉴权、已发出 commit 而 JS 未收到确认时返回可核对的 503，核对结果为 committed 且记录仅写一次。
- worker 增至 16 项并全部通过，新增卡住的宿主 finish 有界返回、保留容量直至真正收束、恢复后 worker 可继续使用。清理任务数由 pool_size 限制。
- SMTP 3 项通过，新增真实本地 SMTP 协议收件与 Message-ID 对照；SDK lint/typecheck、23 项测试及扩展类型测试再次通过。
- 最新 workspace tests 使用 `-j 1` 全部通过。新增 S3 协议故障用例也通过：真实 object_store S3 适配器连接本地签名请求接收器，验证 COPY 成功但批量 DELETE 被拒、PUT 已保存但返回错误、配额保留和恢复；这不等于真实云端/MinIO 的部署验收。
- 首轮统一入口全部通过（`target/verification/win32-x64-2026-09-28T12-57-58.648Z/report.json`），包含 release、SDK 和 blog/kanban/mail 集成回归。该报告早于下面的追加修复，不替代这些修复的最终复验。Linux/macOS 未执行。

追加实现和回归：

- FileService/恢复套件现有 12 项通过（含 2 个崩溃子进程入口）。新增管理员 `/api/admin/file-operations` 核对释放、当前对象存在/元数据失败/有效版本拒绝、原核对回执幂等及管理员鉴权；Mem/SurrealKv 上传补偿在删除拒绝和删除确认丢失后重试，unknown 保留对象；真实进程退出后恢复上传/Collection 清理账本和 OpenAPI，完成清理前禁止重建同名集合。
- worker 17 项通过。宿主调用现在经过有界 mpsc 通道交给 Tokio；排队和执行中的调用共同占用容量，JSON 请求字节预留跟随已接受调用，JS 等待者消失后仍保留。新增并发耗尽、原生 I/O 未结束时下一根调用不得占用清理容量、收束后恢复测试。
- 本地真实 HTTP 接收端与 outbox 数据库/租约/发送链联合用例通过：已接受但丢失响应时，普通 origin 只发一次并进入 unknown；幂等 origin 重试使用同一键、覆盖脚本自带的伪造幂等头，接收端仅执行一次；重试耗尽仍 unknown。测试专用 loopback 构造器由 dev-dependency 的 test-support feature 开启，服务器正常构建保持公网地址校验。
- outbox 新增持久化不确定性标志，后续明确未发送的失败或授权撤销不能将此前 unknown 降为 failed。发送前的临时 DNS/超时使用独立、最多三次的 1/2/4 秒重试预算；attempts 仍只统计进入发送阶段的次数。租约维护查询不返回任务正文。
- Auth 并发屏障套件 4 项通过（含 1 个迁移子进程入口）。测试发现相同值 token_key UPDATE 被优化为无写入，现改用私有 `_hb_auth_issuance` 版本产生真实写冲突；Mem/SurrealKv 覆盖登录/刷新与撤销、删除、角色变更，以及刷新重放并发。旧严格 Auth Collection 启动迁移与字段脱敏通过。
- `scripts/verify-lifecycle.py` 的真实进程测试已在 Windows debug 和 release 二进制上通过：bootstrap 前未绑定、serve 已绑定但尚未处理请求、热重载不重复生命周期、失败重载保留旧快照、CTRL_BREAK 优雅停服、排空在途请求、shutdown 可用宿主服务、重启核对停服记录。Unix 入口使用 SIGTERM，尚未执行。
- 一次追加统一验收的普通 check 读取了不匹配的 core 编译元数据，失败报告保留于 `win32-x64-2026-09-28T13-23-39.026Z`；同次 clippy/tests/release/lifecycle/SDK/集成通过。随后统一入口 `win32-x64-2026-09-28T13-59-07.584Z/report.json` 全部通过；该轮结果早于下面的分页与二进制修改，不替代修改后的最终复验，也不改写旧失败报告。

本次续做：

- Storage 9 项通过，覆盖 10,003 个同目录对象的有界分页和清理、字典排序、前缀游标隔离、上一页对象已删除后的续页、Windows junction 拒绝。S3 协议接收器新增每两项一个服务端页，实际适配器跨服务端页与宿主 keyset 页的续页和删除用例通过。
- FileService/恢复 13 项通过（含原有 2 个子进程入口），新增 Mem/SurrealKv 各 206 个孤儿/临时对象的分页扫描、删除失败后的持久化配额、重新装配后的账本恢复、双目录引用保护及其他命名空间保留。
- worker 20 项通过。新增共享 HostBudget/HostBuffer、直接 Uint8Array 通道；取消后原生调用仍持有缓冲预留，收束后释放；JS 保留多个二进制结果会按 QuickJS 堆限额触发 OOM；旧 JSON 宿主的字节数组文件响应及旧 dispatcher 返回值兼容通过。
- JS/API 16 项通过。2 MiB 文件视图写入、偏移、调用后修改原数组、空数组及读取隔离通过；2 MiB 非 UTF-8 请求经 `request.bytes()`、文件写入和 `e.file()` 完整返回，通过字节通道且保留 text/json 语义。Auth/Record/multipart 现有回归全部通过。
- 请求与响应字节不再展开成 JSON 数字数组；原 JSON dispatcher/HostServices 接口保留兼容入口。桥接 JSON 序列化先计算大小、预留额度，再分配输出缓冲。
- 实际提交边界套件 3 项通过（含 1 个子进程入口），专项 clippy 通过。测试监听 SurrealDB 自身的 `kvs::tx::commit` span，在引擎函数内部暂停真实提交，无生产故障开关：Mem/SurrealKv 在提交入口取消 owner 并丢弃等待者，finish 等待收束、两条写入共同提交且各发一次 LIVE；SurrealKv 在入口和完成边界强制终止进程，重启分别恢复 rolled_back/committed，业务数据与提交标记一致。这是进程丢失及引擎边界验证，不是断电或 WAL 中途扇区损坏测试。专项日志：`target/js-commit-engine-workspace.log`。
- 最新完整统一复验全部通过：`target/verification/win32-x64-2026-09-28T15-24-37.035Z/report.json`。覆盖最终代码的格式、workspace check/all-targets clippy/tests、release、真实 release 生命周期、SDK lint/typecheck/tests、扩展类型及 blog/kanban/mail 集成；包含上面的 20 项 worker 测试和提交边界套件。Linux/macOS 均为 `not_run`。前一轮 `15-08-04.602Z` 也全部通过，但早于最后的旧宿主文件响应兼容修复。

本地/内存 Storage 的有界 list/copy、覆盖写入和 Windows junction 拒绝新增 7 项测试已通过；
JS FileService 的逻辑目录、配额、分页与恢复账本已接入。一次 workspace tests 在编译阶段因并行
rustc/LLVM 内存耗尽失败；改用 `-j 1` 后完整通过。统一入口默认使用单编译任务，可用
`CARGO_BUILD_JOBS` 显式覆盖。

仍需收敛的实现项：宿主适配器内部及 JSON 值树的完整临时分配计量、读取完整请求正文前的执行槽/缓冲接纳。
目前 HTTP 层仍先读取有大小上限的正文，再进入运行时队列；不能将其记为第 16 节要求的前置接纳已完成。
真实云端 S3 部署未执行；现有 S3 用例是本地协议接收器。
这些缺口不因上述测试通过而视为完成。

## 统一入口

已安装 Rust 工具链、Node.js 20+、pnpm 和 Python 3.11+ 后，从仓库根目录执行：

```sh
node scripts/verify.mjs --runtime
node scripts/verify.mjs
```

第一条验证 core/db/jsvm；第二条执行 workspace 格式、check、clippy、tests、release、真实 release 进程生命周期、SDK lint/typecheck/tests、扩展类型检查及现有集成回归。`--list` 只列出命令。缺失的类型检查入口会明确失败，不能被 pnpm 的“未找到脚本”提示误记为通过。

每次运行将逐项日志和 `report.json` 写入 `target/verification/`，记录当前平台、通过、失败和未执行项。报告不会替其他平台作出结论。

Windows 使用 x64 MSVC Build Tools（含 C++ 和 Windows SDK）、CMake；可在 Developer PowerShell 中运行。非标准 CMake 安装位置通过 `CMAKE` 指向实际可执行文件。仓库配置允许 AWS-LC 使用随 crate 分发的 NASM 对象。受限令牌可能无法打开用于安全脚本发现的目录句柄，应在普通开发者终端运行验证，不能因此削弱目录访问实现。

Linux 安装本地 C/C++ 编译器、构建工具、CMake 和 python3；macOS 安装 Xcode Command Line Tools、CMake 和 python3。安装项目指定 Rust 工具链并执行 `pnpm install --frozen-lockfile` 后使用同一入口。平台专属测试采用条件编译；Windows junction 已在 Windows 执行，其余平台不据此计为通过。

## 版本补丁

固定 rquickjs 0.11.0 和 SurrealDB 3.2.3，维护可追溯补丁：显式数据库事务的 LIVE 通知、SDK 提交错误的类型保留、QuickJS reaction/await 的上下文恢复，以及 rquickjs JobException 的 Context 引用所有权。SDK 补丁确保已确认的事务冲突返回 409；Context 补丁修复了真实 afterCommit 中断测试触发的 GC 断言，并保留断言继续验证销毁行为。源码来源、哈希和移除条件见 [vendor/README.md](../vendor/README.md)。不得将上游引擎的能力列表当作这些行为在本项目已验证的证据。
