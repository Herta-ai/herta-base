import { hb, hbAdmin } from './hb';
import type { TaskPriority, TaskStatus } from '../types/kanban';

export interface SeedProgressCallback {
  (step: string, percent: number): void;
}

export const DEFAULT_PASSWORD = 'correct password 123';

export async function initializeAndSeedKanban(
  adminEmail = 'admin@example.com',
  adminPassword = 'correct horse battery staple',
  onProgress?: SeedProgressCallback,
): Promise<{ ownerToken: string; workspaceId: string }> {
  // Step 1: Admin Login
  onProgress?.('正在验证管理员身份...', 10);
  await hbAdmin.auth.login({ email: adminEmail, password: adminPassword });

  // Step 2: Create Collections if not present
  onProgress?.('正在创建与配置 kb_users 集合...', 25);
  try {
    await hbAdmin.collections.create({
      name: 'kb_users',
      type: 'auth',
      schema_mode: 'schema-less',
      rules: { list: true, view: true, create: true, update: true, delete: true },
    });
  } catch (err) {
    console.warn('kb_users already exists or error:', err);
  }

  onProgress?.('正在创建与配置 kb_workspaces 集合...', 35);
  try {
    await hbAdmin.collections.create({
      name: 'kb_workspaces',
      type: 'base',
      schema_mode: 'strict',
      fields: [
        { name: 'name', type: 'text', required: true },
        { name: 'owner', type: 'relation', required: true, options: { collection: 'kb_users', maxSelect: 1 } },
        { name: 'members', type: 'relation', required: true, options: { collection: 'kb_users', maxSelect: 20 } },
      ],
      rules: {
        list: 'owner = $auth.record OR $auth.record IN members',
        view: 'owner = $auth.record OR $auth.record IN members',
        create: '$record.owner = $auth.record AND $auth.record IN $record.members',
        update: 'owner = $auth.record',
        delete: 'owner = $auth.record',
      },
    });
  } catch (err) {
    console.warn('kb_workspaces already exists or error:', err);
  }

  onProgress?.('正在创建与配置 kb_tasks 集合...', 45);
  try {
    await hbAdmin.collections.create({
      name: 'kb_tasks',
      type: 'base',
      schema_mode: 'strict',
      fields: [
        { name: 'title', type: 'text', required: true },
        { name: 'description', type: 'text' },
        { name: 'priority', type: 'select', required: true, options: { values: ['low', 'medium', 'high', 'urgent'] } },
        { name: 'status', type: 'select', required: true, options: { values: ['todo', 'in_progress', 'in_review', 'done'] } },
        { name: 'workspace', type: 'relation', required: true, options: { collection: 'kb_workspaces', maxSelect: 1 } },
        { name: 'assignees', type: 'relation', options: { collection: 'kb_users', maxSelect: 10 } },
        { name: 'attachments', type: 'file', options: { maxSelect: 5, extensions: ['png', 'jpg', 'pdf', 'zip'] } },
        { name: 'order', type: 'number', required: true },
        { name: 'is_private', type: 'bool', required: true },
      ],
      rules: {
        list: 'workspace.owner = $auth.record OR $auth.record IN workspace.members',
        view: 'workspace.owner = $auth.record OR $auth.record IN workspace.members',
        create: '$record.workspace.owner = $auth.record OR $auth.record IN $record.workspace.members',
        update: '(workspace.owner = $auth.record OR $auth.record IN workspace.members) AND (is_private = false OR workspace.owner = $auth.record OR $auth.record IN assignees) AND $request.body.workspace IS NONE',
        delete: '(workspace.owner = $auth.record OR $auth.record IN workspace.members) AND (is_private = false OR workspace.owner = $auth.record OR $auth.record IN assignees)',
      },
    });
  } catch (err) {
    console.warn('kb_tasks already exists or error:', err);
  }

  onProgress?.('正在创建与配置 kb_comments 集合...', 55);
  try {
    await hbAdmin.collections.create({
      name: 'kb_comments',
      type: 'base',
      schema_mode: 'strict',
      fields: [
        { name: 'task', type: 'relation', required: true, options: { collection: 'kb_tasks', maxSelect: 1 } },
        { name: 'author', type: 'relation', required: true, options: { collection: 'kb_users', maxSelect: 1 } },
        { name: 'content', type: 'text', required: true },
      ],
      rules: {
        list: 'task.workspace.owner = $auth.record OR $auth.record IN task.workspace.members',
        view: 'task.workspace.owner = $auth.record OR $auth.record IN task.workspace.members',
        create: '$record.author = $auth.record AND ($record.task.workspace.owner = $auth.record OR $auth.record IN $record.task.workspace.members)',
        update: 'author = $auth.record',
        delete: 'author = $auth.record',
      },
    });
  } catch (err) {
    console.warn('kb_comments already exists or error:', err);
  }

  // Step 3: Register Demo Users
  onProgress?.('正在注册 4 个预设演示角色 (Owner, Assignee, Member, Outsider)...', 65);
  const usersAuth = hb.auth.forCollection('kb_users');

  async function ensureUser(email: string, displayName: string): Promise<string> {
    try {
      const reg = await usersAuth.register({
        email,
        password: DEFAULT_PASSWORD,
        profile: { displayName },
      });
      return reg.user.id;
    } catch {
      // If already registered, login to get ID
      const login = await usersAuth.login({ email, password: DEFAULT_PASSWORD });
      return login.user.id;
    }
  }

  const ownerId = await ensureUser('owner@example.com', '项目负责人 (Owner)');
  const assigneeId = await ensureUser('assignee@example.com', '核心开发 (Assignee)');
  const memberId = await ensureUser('member@example.com', '团队成员 (Member)');
  const outsiderId = await ensureUser('outsider@example.com', '外部访客 (Outsider)');

  // Step 4: Login as Owner to seed Workspaces & Tasks
  onProgress?.('正在以 Owner 身份创建初始工作区...', 75);
  const ownerSession = await usersAuth.login({ email: 'owner@example.com', password: DEFAULT_PASSWORD });

  const wsCollection = hb.collection<any>('kb_workspaces');
  const existingWs = await wsCollection.list({ perPage: 10 });
  let mainWorkspaceId = existingWs.items[0]?.id;

  if (!mainWorkspaceId) {
    const mainWs = await wsCollection.create({
      name: '🚀 核心产品敏捷看板',
      owner: ownerId,
      members: [ownerId, assigneeId, memberId],
    });
    mainWorkspaceId = mainWs.id;

    // Also create outsider workspace as Outsider
    await usersAuth.login({ email: 'outsider@example.com', password: DEFAULT_PASSWORD });
    await wsCollection.create({
      name: '📦 外部隔离项目 (Outsider)',
      owner: outsiderId,
      members: [outsiderId],
    });

    // Switch back to owner
    await usersAuth.login({ email: 'owner@example.com', password: DEFAULT_PASSWORD });
  }

  // Step 5: Seed Sample Tasks
  onProgress?.('正在灌入真实敏捷任务与评论数据...', 85);
  const tasksCollection = hb.collection<any>('kb_tasks');
  const taskCount = (await tasksCollection.list({ filter: `workspace = '${mainWorkspaceId}'`, perPage: 1 })).total;

  if (taskCount === 0) {
    const seedTasks: Array<{
      title: string;
      description: string;
      priority: TaskPriority;
      status: TaskStatus;
      assignees: string[];
      order: number;
      is_private: boolean;
      comment?: string;
    }> = [
      {
        title: 'SurrealDB 多层级图关系与 $expand 深度展开',
        description: '验证并优化 `GET /api/collections/kb_tasks/records?expand=assignees,workspace.owner,workspace.members` 的嵌套解析性能。',
        priority: 'urgent',
        status: 'todo',
        assignees: [assigneeId],
        order: 10,
        is_private: false,
        comment: '已开始设计关系解析的缓存层，预计明天完成联调。',
      },
      {
        title: '实现 SSE 实时连接心跳与断线重连容错',
        description: '在长连接断开或网络闪断时，客户端自动指数退避重连，并在重连后重新拉取当前工作区最新数据。',
        priority: 'high',
        status: 'todo',
        assignees: [memberId],
        order: 20,
        is_private: false,
      },
      {
        title: '看板卡片跨泳道拖拽与乐观更新重构',
        description: '使用 `@dnd-kit` 替换传统拖拽，提供毫秒级就地 UI 响应与后端状态同步，增强移动端触控体验。',
        priority: 'high',
        status: 'in_progress',
        assignees: [assigneeId, memberId],
        order: 10,
        is_private: false,
        comment: '拖拽动画已在桌面端调优完成，正在适配移动端手势。',
      },
      {
        title: '任务多文件附件 Token 鉴权下载与格式校验',
        description: '对卡片上传的设计稿/文档进行格式限定（png, jpg, pdf, zip），并通过 `/api/files/token` 签发临时安全访问令牌。',
        priority: 'urgent',
        status: 'in_progress',
        assignees: [assigneeId],
        order: 20,
        is_private: true,
        comment: '私密卡片仅 Owner 和 Assignee 具备查看与下载权限。',
      },
      {
        title: '基于 TanStack Table 的高级多维筛选与排序视图',
        description: '支持按优先级、状态、执行人、搜索关键字进行组合筛选，并支持自定义列显示与一键导出。',
        priority: 'medium',
        status: 'in_review',
        assignees: [memberId],
        order: 10,
        is_private: false,
      },
      {
        title: '深浅色主题平滑过渡与 CSS 变量系统优化',
        description: '适配 Linear / shadcn 风格深色模式，提供细腻的边框与微光投影，支持跟随系统设置自动切换。',
        priority: 'low',
        status: 'in_review',
        assignees: [assigneeId],
        order: 20,
        is_private: false,
      },
      {
        title: '完成 HertaBase Rust 核心 HTTP Envelope 规范化',
        description: '全面统一 `{ data, meta, error }` 顶层字段规范与全局异常错误码映射体系。',
        priority: 'urgent',
        status: 'done',
        assignees: [ownerId],
        order: 10,
        is_private: false,
        comment: '已全部合并入主干并通过 100% 集成测试。',
      },
      {
        title: '搭建 React 19 + TanStack 全家桶敏捷看板前端工程',
        description: '集成 React 19、Vite 8、TanStack Router/Store/Table/Query 与 Tailwind CSS，实现生产级架构。',
        priority: 'high',
        status: 'done',
        assignees: [ownerId, assigneeId],
        order: 20,
        is_private: false,
      },
    ];

    const commentsCollection = hb.collection<any>('kb_comments');
    for (const item of seedTasks) {
      const createdTask = await tasksCollection.create({
        title: item.title,
        description: item.description,
        priority: item.priority,
        status: item.status,
        workspace: mainWorkspaceId,
        assignees: item.assignees,
        order: item.order,
        is_private: item.is_private,
      });

      if (item.comment) {
        await commentsCollection.create({
          task: createdTask.id,
          author: ownerId,
          content: item.comment,
        });
      }
    }
  }

  onProgress?.('初始化全部完成！正在进入系统...', 100);

  return {
    ownerToken: ownerSession.accessToken,
    workspaceId: mainWorkspaceId,
  };
}
