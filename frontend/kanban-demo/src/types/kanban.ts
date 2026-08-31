export type TaskStatus = 'todo' | 'in_progress' | 'in_review' | 'done';
export type TaskPriority = 'low' | 'medium' | 'high' | 'urgent';

export interface KbUser {
  id: string;
  email: string;
  displayName?: string;
  avatar?: string;
  created_at?: string;
  updated_at?: string;
}

export interface KbWorkspace {
  id: string;
  name: string;
  owner: string; // Relation ID: "kb_users:..."
  members: string[]; // Relation IDs: ["kb_users:..."]
  created_at?: string;
  updated_at?: string;
  expand?: {
    owner?: KbUser;
    members?: KbUser[];
  };
}

export interface KbTask {
  id: string;
  title: string;
  description?: string;
  priority: TaskPriority;
  status: TaskStatus;
  workspace: string; // Relation ID: "kb_workspaces:..."
  assignees?: string[]; // Relation IDs: ["kb_users:..."]
  attachments?: string[]; // Filenames in storage
  order: number;
  is_private: boolean;
  created_at?: string;
  updated_at?: string;
  expand?: {
    workspace?: KbWorkspace;
    assignees?: KbUser[];
  };
}

export interface CreateTaskPayload {
  title: string;
  description?: string;
  priority: TaskPriority;
  status: TaskStatus;
  workspace: string;
  assignees?: string[];
  order: number;
  is_private: boolean;
}

export interface UpdateTaskPayload {
  title?: string;
  description?: string;
  priority?: TaskPriority;
  status?: TaskStatus;
  workspace?: string;
  assignees?: string[];
  order?: number;
  is_private?: boolean;
}

export interface KbComment {
  id: string;
  task: string; // Relation ID: "kb_tasks:..."
  author: string; // Relation ID: "kb_users:..."
  content: string;
  created_at?: string;
  updated_at?: string;
  expand?: {
    author?: KbUser;
  };
}

export interface CreateCommentPayload {
  task: string;
  author: string;
  content: string;
}

export interface DemoAccount {
  email: string;
  name: string;
  role: string;
  description: string;
  color: string;
}

export const DEMO_ACCOUNTS: DemoAccount[] = [
  {
    email: 'owner@example.com',
    name: '项目负责人 (Owner)',
    role: 'Owner',
    description: '拥有全部工作区管理与私密卡片读写权限',
    color: 'bg-indigo-500',
  },
  {
    email: 'assignee@example.com',
    name: '主要开发 (Assignee)',
    role: 'Assignee',
    description: '核心团队成员，可协同推进任务及查看指派的私有任务',
    color: 'bg-emerald-500',
  },
  {
    email: 'member@example.com',
    name: '团队成员 (Member)',
    role: 'Member',
    description: '普通团队协作成员，可读写公开卡片并参与讨论',
    color: 'bg-blue-500',
  },
  {
    email: 'outsider@example.com',
    name: '外部访客 (Outsider)',
    role: 'Outsider',
    description: '非本工作区成员，用于测试越权访问 403/404 隔离拦截',
    color: 'bg-zinc-500',
  },
];

export const COLUMNS: { id: TaskStatus; title: string; color: string; dotColor: string }[] = [
  { id: 'todo', title: '待处理', color: 'border-slate-500/30 bg-slate-500/5', dotColor: 'bg-slate-400' },
  { id: 'in_progress', title: '进行中', color: 'border-blue-500/30 bg-blue-500/5', dotColor: 'bg-blue-500' },
  { id: 'in_review', title: '评审中', color: 'border-amber-500/30 bg-amber-500/5', dotColor: 'bg-amber-500' },
  { id: 'done', title: '已完成', color: 'border-emerald-500/30 bg-emerald-500/5', dotColor: 'bg-emerald-500' },
];

export const PRIORITIES: { id: TaskPriority; label: string; color: string; badgeVariant: 'default' | 'secondary' | 'destructive' | 'outline' }[] = [
  { id: 'urgent', label: '紧急', color: 'text-red-500 border-red-500/30 bg-red-500/10', badgeVariant: 'destructive' },
  { id: 'high', label: '高', color: 'text-amber-500 border-amber-500/30 bg-amber-500/10', badgeVariant: 'secondary' },
  { id: 'medium', label: '中', color: 'text-blue-500 border-blue-500/30 bg-blue-500/10', badgeVariant: 'outline' },
  { id: 'low', label: '低', color: 'text-slate-500 border-slate-500/30 bg-slate-500/10', badgeVariant: 'outline' },
];
