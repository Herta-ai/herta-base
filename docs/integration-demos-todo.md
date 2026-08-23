# HertaBase 业务集成测试 Demo 规划与 TODO 清单

本文档针对 HertaBase 目前测试用例单一（仅有基础 `blog-demo`）的现状，系统性规划并拆解可用于全面验证 HertaBase 核心特性的**业务系统测试 Demo 矩阵**，明确每个 Demo 的业务特点、集合模型、核心测试能力、边缘与异常场景覆盖及具体实施 TODO。

---

## 📌 一、 建设背景与目标

### 1.1 现状分析

当前 [`packages/integration-tests/blog-demo`](file:///E:/project/rustProject/herta-base/packages/integration-tests/blog-demo) 仅验证了：

- 基础 CRUD 流程与 HTTP Envelope 格式
- 单一 Auth 集合（`blog_users`）与简单 Relation 所有权校验（`$auth.record`）
- 基础的匿名/公开/私有读写规则与软删除机制

### 1.2 目标

通过建立多个真实且独立的**领域业务测试套件**，全面压测与闭环验证 HertaBase 各阶段核心能力：

1. **多模数据库与图关系**（SurrealDB 原生 Relation、多层级 `$expand`、混合 Schema）
2. **细粒度权限与多鉴权体系**（ABAC API Rules、多动态 Auth 集合、JWT 轮换）
3. **实时订阅总线**（Salvo SSE + SurrealDB `LIVE SELECT`、条件过滤流、生命周期管理）
4. **文件存储与多媒体**（Multipart 上传、短期文件 Token、Range 流式切片播放、清理一致性）
5. **JS 扩展运行时**（Event Hooks 拦截修改、`routerAdd` 自定义 API、`cronAdd` 定时任务、`$app` 数据库/网络调用）
6. **前端静态与 SPA 托管**（`/web/{project}/` 部署、SPA 路由 Fallback、版本回滚）

---

## 🧭 二、 业务 Demo 规划矩阵

| 序号 | 业务 Demo 套件名称 | 对应真实场景 | 重点验证能力 | 优先级 |
| :--- | :--- | :--- | :--- | :--- |
| **01** | `kanban-demo` | 敏捷看板 / Linear / Trello | 实时 SSE 协同、多层级图关系展开、RBAC 团队权限、卡片多附件 | **P0** |
| **02** | `ecommerce-demo` | 电商商城 / 订单履约中心 | JS Hooks 状态机、自定义结算 API、Cron 定时关单、严格 Schema 校验 | **P0** |
| **03** | `chat-realtime-demo` | 实时群聊 / Discord / 飞书 | SSE 条件过滤订阅、多对多群组成员规则、消息撤回/软删除 SSE、私密文件 Token | **P0** |
| **04** | `lms-media-demo` | 在线教育 / 音视频知识库 | 多 Auth 集合（教师/学员）、HTTP Range 视频分片播放、树形章节目录 | **P1** |
| **05** | `iot-telemetry-demo` | 物联网设备 / 实时监控告警 | Schema-less 异构时序上报、实时阈值告警流、JS 异常钩子处理、设备心跳巡检 | **P1** |
| **06** | `o2o-dispatch-demo` | 外卖派单 / 预约出行调度 | 三方 Auth 集合（用户/骑手/商家）、状态流转竞态锁、复合 API Rules | **P2** |
| **07** | `cms-hosting-demo` | Headless CMS / 前端静态托管 | 动态 Schema 创建、OpenAPI 自动生成、`/web/{project}/` 部署与回滚 | **P2** |

---

## 🛠️ 三、 详细业务 Demo 规格与 TODO 清单

---

### Demo 01: 敏捷看板与协同系统 (`kanban-demo`)

#### 1. 业务特点

模拟团队项目看板，包含工作区（Workspace）、看板泳道（Column/Board）、任务卡片（Task）、子任务（Subtask）、附件及任务评论。支持多成员同屏操作与状态实时同步。

#### 2. 数据集合与模型设计

- **`kb_users`** (`type: "auth"`): 团队成员账号。
- **`kb_workspaces`** (`type: "base"`): 工作区，含 `owner` (`relation<kb_users>`), `members` (`array<relation<kb_users>>`)。
- **`kb_tasks`** (`type: "base"`): 任务项。
  - `title` (`text`, required), `description` (`text`), `priority` (`select: low|medium|high|urgent`)
  - `status` (`select: todo|in_progress|in_review|done`)
  - `workspace` (`relation<kb_workspaces>`, required)
  - `assignees` (`array<relation<kb_users>>`)
  - `attachments` (`file`, `maxSelect: 5`, `extensions: ["png", "jpg", "pdf", "zip"]`)
  - `order` (`number`, 泳道内排序权重)
- **`kb_comments`** (`type: "base"`): 任务评论，含 `task` (`relation<kb_tasks>`), `author` (`relation<kb_users>`), `content` (`text`)。

#### 3. 核心测试能力

- **SurrealDB 多层关系展开**：请求 `GET /api/collections/kb_tasks/records?expand=assignees,workspace.owner,workspace.members` 验证深层嵌套关联。
- **实时 SSE 协同推送**：客户端 A 订阅 `GET /api/realtime/kb_tasks?filter=workspace = 'kb_workspaces:123'`；客户端 B 拖拽修改任务 `status` 和 `order`，断言客户端 A 收到精确的 `update` SSE 事件。
- **复合 API Rules**：
  - `list/view`: `$auth.id IN workspace.members OR workspace.owner = $auth.record`
  - `create/update`: 仅工作区成员可写；非 Assignee 无法修改私有任务。
- **任务附件管理**：Multipart 上传多个设计稿/文档，修改卡片时追加/替换附件。

#### 4. 边缘用例与异常覆盖

- [ ] 非工作区成员尝试通过 ID 探测查看私有任务，断言返回 `404 HB_RECORD_NOT_FOUND` 或 `403 HB_FORBIDDEN`。
- [ ] 跨工作区关联任务（`task.workspace` 指向无权限的工作区），断言创建失败。
- [ ] 订阅 SSE 时使用非法/过期 JWT 或越权 filter，断言返回 `401 HB_UNAUTHORIZED` / `403 HB_FORBIDDEN`。
- [ ] 上传超过 5 个文件或非法扩展名（如 `.exe`），断言返回 `400 HB_VALIDATION_ERROR`。

#### 5. 实施 TODO

- [ ] 创建 `packages/integration-tests/kanban-demo/` 目录与配置
- [ ] 编写测试服务端生命周期及工作区初始化脚本
- [ ] 编写多用户同屏拖拽与 SSE 实时同步断言用例
- [ ] 编写附件上传、更新与下载 Token 校验用例

---

### Demo 02: 电商订单与履约状态机 (`ecommerce-demo`)

#### 1. 业务特点

模拟完整的电商购买与履约流程：商品展示、购物车、订单状态机（待支付 -> 已支付 -> 备货中 -> 已发货 -> 已完成 / 已取消）、库存扣减与超时未支付关单。

#### 2. 数据集合与模型设计

- **`shop_customers`** (`type: "auth"`): 顾客账户。
- **`shop_products`** (`type: "base"`, `schema_mode: "strict"`):
  - `name` (`text`, required), `sku` (`text`, required, unique)
  - `price` (`number`, required), `stock` (`number`, required)
  - `images` (`file`, `maxSelect: 10`, `mimeTypes: ["image/jpeg", "image/png", "image/webp"]`)
  - `attributes` (`json`, 存储规格参数如尺寸、颜色)
- **`shop_orders`** (`type: "base"`):
  - `order_no` (`text`, required, unique)
  - `customer` (`relation<shop_customers>`, required)
  - `items` (`json`, 购买的商品快照及数量)
  - `total_amount` (`number`, required)
  - `status` (`select: PENDING_PAY|PAID|PROCESSING|SHIPPED|COMPLETED|CANCELLED`)
  - `payment_deadline` (`datetime`)

#### 3. 核心测试能力

- **JS 扩展运行时 (Phase 3)**：
  - `routerAdd("POST", "/api/shop/checkout")`: 自定义结算接口，验证库存是否充足，原子创建订单并计算支付截止时间。
  - `onRecordUpdateRequest("shop_orders")`: 校验订单状态机流转合法性（如禁止从 `CANCELLED` 变为 `PAID`）。
  - `onRecordUpdate("shop_orders")`: 订单变为 `PAID` 时，自动在事务外触发模拟出库通知与发票生成。
- **Cron 定时任务**：
  - `cronAdd("cancel_unpaid_orders", "*/1 * * * *", ...)`: 定时扫描 `status = 'PENDING_PAY' AND payment_deadline < time::now()` 的订单，批量标记为 `CANCELLED` 并回补 `shop_products.stock`。
- **严格 Schema 与数据校验**：验证价格不能为负数，库存修改不可突破下限。

#### 4. 边缘用例与异常覆盖

- [ ] **高并发超卖测试**：多客户端并发发起相同 SKU 的结账请求，断言库存扣减不出现负数，超出库存请求返回库存不足错误。
- [ ] **非法状态流转攻击**：顾客尝试直接 PATCH 将自己订单的 `status` 从 `PENDING_PAY` 改为 `PAID`，断言被 API Rules 或 Hook 拦截拒绝。
- [ ] **跨用户越权查看订单**：顾客 A 尝试读取顾客 B 的订单详情，断言返回 `403 HB_FORBIDDEN`。
- [ ] **未支付超时自动关单**：快进时间或触发 Cron 任务，验证订单状态变为 `CANCELLED`，且对应商品库存精确恢复。

#### 5. 实施 TODO

- [ ] 创建 `packages/integration-tests/ecommerce-demo/` 目录
- [ ] 编写 JS Hook 脚本（包含结算路由、状态机守卫、定时关单任务）
- [ ] 编写库存扣减原子性与并发结算测试用例
- [ ] 编写订单状态流转与 Cron 自动关单断言用例

---

### Demo 03: 实时即时通讯与在线协作群聊 (`chat-realtime-demo`)

#### 1. 业务特点

模拟多频道（Channel）群聊与私聊（Direct Message）系统，支持消息实时收发、消息撤回（软删除）、在线成员列表与私有媒体文件传输。

#### 2. 数据集合与模型设计

- **`chat_users`** (`type: "auth"`): 聊天用户，含 `avatar` (`file`), `status` (`select: online|offline|dnd`)。
- **`chat_channels`** (`type: "base"`): 频道。
  - `name` (`text`, required), `is_private` (`bool`, required)
  - `owner` (`relation<chat_users>`, required)
  - `members` (`array<relation<chat_users>>`)
- **`chat_messages`** (`type: "base"`): 聊天消息。
  - `channel` (`relation<chat_channels>`, required)
  - `sender` (`relation<chat_users>`, required)
  - `content` (`text`), `media` (`file`, `maxSelect: 3`)
  - `reply_to` (`relation<chat_messages>`)
  - `is_pinned` (`bool`)

#### 3. 核心测试能力

- **SSE 条件订阅与消息广播**：
  - 用户订阅 `GET /api/realtime/chat_messages?filter=channel = 'chat_channels:ch_1'`。
  - 验证同一频道的所有在线成员均能以毫秒级延迟收到 `create` 事件；非本频道成员无法收到事件。
- **消息撤回与软删除推送**：
  - 发送者发起 `DELETE /api/collections/chat_messages/records/{id}`。
  - 验证记录更新 `deleted_at`，同时向所有订阅端广播 `delete` SSE 事件，携带有准确的记录 ID。
- **私密聊天媒体与文件令牌**：
  - 聊天中上传图片/语音附件，调用 `POST /api/files/token` 获取临时 Token 并带参访问 `GET /api/files/.../audio.mp3?token=...`。
  - 验证非该频道成员即便拥有文件名也无法获取 Token 或直接下载。

#### 4. 边缘用例与异常覆盖

- [ ] **越权偷听频道消息**：未加入私有频道 `ch_private` 的用户尝试建立针对该频道的 SSE 连接，断言连接建立阶段被拒绝并返回 `403 HB_FORBIDDEN`。
- [ ] **篡改发送者身份**：用户 A 发送消息时在 body 中伪造 `sender: 'chat_users:user_b'`，断言被 `$record.sender = $auth.record` 规则拒绝。
- [ ] **撤回他人消息**：普通群员尝试 DELETE 管理员或其他群员的消息，断言返回 `403 HB_FORBIDDEN`。
- [ ] **SSE 心跳与断线重连容错**：长时间无消息时，断言收到标准 `event: ping` 保持长连接活跃。

#### 5. 实施 TODO

- [ ] 创建 `packages/integration-tests/chat-realtime-demo/` 目录
- [ ] 编写频道成员鉴权规则与消息数据表
- [ ] 编写基于 SSE EventSource 的多客户端并行消息监听与断言
- [ ] 编写消息撤回、图片附件权限与 Token 下载测试

---

### Demo 04: 在线教育与多媒体知识库 (`lms-media-demo`)

#### 1. 业务特点

模拟多角色在线网校平台，区分教师（Instructor）与学员（Student）两套完全独立的登录与权限体系，支持课程树形章节导航、课后 JSON 动态测验以及视频课件的 Range 分片流式播放。

#### 2. 数据集合与模型设计

- **`lms_instructors`** (`type: "auth"`): 教师账户。
- **`lms_students`** (`type: "auth"`): 学员账户。
- **`lms_courses`** (`type: "base"`): 课程。
  - `title` (`text`, required), `instructor` (`relation<lms_instructors>`, required)
  - `cover` (`file`, `mimeTypes: ["image/png", "image/jpeg"]`)
  - `is_published` (`bool`)
- **`lms_lessons`** (`type: "base"`): 章节课时。
  - `course` (`relation<lms_courses>`, required)
  - `parent_lesson` (`relation<lms_lessons>`, 可选，形成目录树)
  - `video_file` (`file`, `maxSize: 524288000`, `extensions: ["mp4", "webm"]`)
  - `quiz_data` (`json`, 存储动态测验题库)
- **`lms_enrollments`** (`type: "base"`): 选课记录，含 `student` (`relation<lms_students>`), `course` (`relation<lms_courses>`), `progress` (`number`)。

#### 3. 核心测试能力

- **双动态 Auth 集合**：
  - 分别向 `/api/auth/lms_instructors/login` 和 `/api/auth/lms_students/login` 进行鉴权，验证 JWT `sub` 包含准确的 collection 前缀与角色隔离。
- **HTTP Range 流式音视频分片读取**：
  - 发起 `GET /api/files/lms_lessons/.../video.mp4` 携带 `Range: bytes=0-1048575`。
  - 断言服务端返回 `206 Partial Content`、正确的 `Content-Range` 与 `Content-Length`，验证流媒体断点续传能力。
- **ETag 与 304 缓存协商**：
  - 携带 `If-None-Match` 请求课件资源，验证无修改时返回 `304 Not Modified`。
- **嵌套树形目录查询**：通过 `expand=parent_lesson.parent_lesson` 获取多级章节结构。

#### 4. 边缘用例与异常覆盖

- [ ] 学员凭证尝试调用只有教师允许的 `create/update` 接口，断言返回 `403 HB_FORBIDDEN`。
- [ ] 请求无效的 Range 范围（如 `Range: bytes=99999999-` 超出文件大小），断言返回 `416 Range Not Satisfiable`。
- [ ] 未选课学员尝试下载私有视频文件，断言签发 Token 失败。
- [ ] 教师删除章节课时，断言底层存储适配器自动清理对应的物理视频文件。

#### 5. 实施 TODO

- [ ] 创建 `packages/integration-tests/lms-media-demo/` 目录
- [ ] 编写双 Auth 集合与关联选课规则
- [ ] 编写大文件上传与 HTTP `Range` / `ETag` 协议断言测试
- [ ] 编写树形递归数据查询与测验 JSON 校验

---

### Demo 05: IoT 物联网设备与实时告警中心 (`iot-telemetry-demo`)

#### 1. 业务特点

模拟海量异构物联网传感器（环境温湿度传感器、电网传感器、车载 GPS）的数据高频上报、Schema-less 动态解析、超限实时告警与设备离线巡检。

#### 2. 数据集合与模型设计

- **`iot_devices`** (`type: "base"`): 设备台账。
  - `device_key` (`text`, unique, required), `device_type` (`select: env_sensor|power_meter|gps_tracker`)
  - `status` (`select: ONLINE|OFFLINE|ERROR`)
  - `last_heartbeat` (`datetime`)
- **`iot_telemetry`** (`type: "base"`, `schema_mode: "schema-less"`): 传感器时序上报数据（温湿度含 `temp`, `humidity`；电表含 `voltage`, `current`, `power`；GPS 含 `lat`, `lng`, `speed`）。
- **`iot_alerts`** (`type: "base"`): 告警事件。
  - `device` (`relation<iot_devices>`, required)
  - `severity` (`select: INFO|WARNING|CRITICAL`)
  - `message` (`text`, required), `resolved` (`bool`)

#### 3. 核心测试能力

- **Schema-less 异构写入**：同属 `iot_telemetry` 集合，同时接纳不同传感器上传的任意非固定格式 JSON 载荷，验证字段自由存储与索引检索。
- **JS Hook 实时阈值判定 (Phase 3)**：
  - `onRecordCreate("iot_telemetry")`: 接收到新上报数据后，自动根据传感器类型判定阈值（例如 `temp > 80.0`），超限时直接自动创建 `iot_alerts` 告警记录并更新设备状态为 `ERROR`。
- **监控大屏实时告警推送 (Phase 4)**：
  - 监控中心客户端订阅 `GET /api/realtime/iot_alerts?filter=severity = 'CRITICAL' AND resolved = false`。
  - 验证异常数据一经写入，监控端瞬间收到 `create` 告警推送。
- **设备心跳巡检定时任务 (Cron)**：
  - 注册 `cronAdd("device_heartbeat_check", "*/1 * * * *", ...)`，找出 `last_heartbeat < 3分钟前` 的设备，批量标记为 `OFFLINE`。

#### 4. 边缘用例与异常覆盖

- [ ] 批量快速上报 100 条异构遥测数据，断言数据无丢失且 Schema-less 动态字段准确落库。
- [ ] Hook 中触发次生告警时若发生逻辑异常，断言保证遥测数据本体安全提交或按预期回滚。
- [ ] 告警被解决（`resolved = true`）后，断言原先仅监听未解决告警的 SSE 订阅端不会收到该记录的后续常规更新。

#### 5. 实施 TODO

- [ ] 创建 `packages/integration-tests/iot-telemetry-demo/` 目录
- [ ] 编写设备遥测上报与阈值判定 JS Hook
- [ ] 编写 Schema-less 批量写入与实时告警推送断言测试
- [ ] 编写设备离线 Cron 定时扫描测试

---

### Demo 06: Headless CMS 与前端项目一键托管 (`cms-hosting-demo`)

#### 1. 业务特点

模拟多站点 CMS 内容管理与前端 SPA 站点部署托管（类似 PocketBase + Vercel / Netlify 单机版）。内容编排者动态定义内容模型，发布文章，并通过单二进制内置的静态托管服务部署前端展示页面。

#### 2. 数据集合与核心能力

- **动态 Schema 与 OpenAPI 自动生成**：
  - 运行时通过管理员 API `POST /_/collections` 动态创建全新的业务集合（如 `news_articles`, `job_posts`）。
  - 立即请求 `GET /api/openapi.json` 或访问 Swagger 规范，断言新集合的 CRUD 路径与 Schema 已经实时注册并对外可见。
- **用户 Web 产物部署 (Phase 6)**：
  - 管理员上传构建好的单页应用（SPA）zip 包到 `HB_DATA_DIR/web/my-blog`。
  - 请求 `GET /web/my-blog/` 验证 index.html 正确返回。
  - 请求 `GET /web/my-blog/posts/123` 验证 SPA History fallback 重定向至 `index.html`。
  - 验证版本备份与一键回滚能力。

#### 3. 实施 TODO

- [ ] 创建 `packages/integration-tests/cms-hosting-demo/` 目录
- [ ] 编写动态创建 Collection 并实时验证 OpenAPI 文档变更测试
- [ ] 编写前端静态产物上传、解压、SPA 路由访问与回滚断言测试

---

## 🏗️ 四、 集成测试工程架构与实现规范

为了保证新增的业务 Demo 风格统一、执行可靠，所有测试套件必须遵循以下工程标准：

```text
packages/integration-tests/
├── blog-demo/                  # [已存在] 基础博客测试
│   ├── tests/
│   │   ├── server.ts           # 服务拉起、随机端口分配与进程生命周期管理
│   │   └── blog.spec.ts        # 博客测试用例
│   └── package.json
├── kanban-demo/                # [P0] 看板协同与实时流测试
│   ├── hooks/                  # 挂载的 JS 扩展脚本（如需）
│   ├── tests/
│   │   ├── server.ts
│   │   ├── sse-helper.ts       # EventSource 测试封装工具
│   │   └── kanban.spec.ts
│   └── package.json
├── ecommerce-demo/             # [P0] 电商状态机与 JS Hook 测试
├── chat-realtime-demo/         # [P0] 实时聊天与多对多权限测试
├── lms-media-demo/             # [P1] 多媒体与 Range 分片播放测试
├── iot-telemetry-demo/         # [P1] 异构上报与告警中心测试
└── cms-hosting-demo/           # [P2] 动态 Schema 与 Web 部署测试
```

### 4.1 测试执行规范

1. **端口动态分配**：禁止硬编码端口，必须使用 `get-port` 或 `net.createServer` 获取空闲回环端口，确保并行测试互不冲突。
2. **纯净内存/临时目录运行**：测试启动时使用内存数据库或隔离的临时目录（`HB_DATA_DIR`），测试结束后通过树杀进程（Tree Kill）彻底清理临时资源。
3. **断言封装**：统一使用标准的 `{ data, meta, error }` Envelope 辅助函数（如 `success()`, `failure()`, `paged()`）进行 HTTP 状态与错误码断言。
4. **SSE 监听助手**：封装基于 `eventsource` 或 `fetch` 流式读取的 Promise 工具，支持带超时等待特定类型的事件（如 `waitForEvent(stream, 'create', predicate)`）。

---

## 📅 五、 阶段实施进度追踪

- [x] **Demo 00: Blog Demo**（已完成基础 CRUD、Auth、简单 Relation 校验）
- [ ] **Demo 01: Kanban Demo**（设计完成，待创建测试套件）
- [ ] **Demo 02: E-Commerce Demo**（设计完成，待创建测试套件）
- [ ] **Demo 03: Chat Realtime Demo**（设计完成，待创建测试套件）
- [ ] **Demo 04: LMS Media Demo**（设计完成，待创建测试套件）
- [ ] **Demo 05: IoT Telemetry Demo**（设计完成，待创建测试套件）
- [ ] **Demo 06: CMS & Web Hosting Demo**（设计完成，待创建测试套件）
