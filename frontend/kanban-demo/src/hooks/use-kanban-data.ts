import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { hb } from '../lib/hb';
import type {
  CreateCommentPayload,
  CreateTaskPayload,
  KbComment,
  KbTask,
  KbUser,
  KbWorkspace,
  TaskPriority,
  TaskStatus,
  UpdateTaskPayload,
} from '../types/kanban';

// Key Factory
export const kanbanKeys = {
  allWorkspaces: ['workspaces'] as const,
  workspace: (id: string) => ['workspace', id] as const,
  allUsers: ['users'] as const,
  tasks: (workspaceId: string) => ['tasks', workspaceId] as const,
  task: (id: string) => ['task', id] as const,
  comments: (taskId: string) => ['comments', taskId] as const,
};

// 1. Workspaces Hooks
export function useWorkspaces() {
  return useQuery({
    queryKey: kanbanKeys.allWorkspaces,
    queryFn: async (): Promise<KbWorkspace[]> => {
      const page = await hb.collection<KbWorkspace>('kb_workspaces').list({
        expand: 'owner,members',
        perPage: 50,
      });
      return page.items;
    },
  });
}

export function useWorkspace(id?: string | null) {
  return useQuery({
    queryKey: kanbanKeys.workspace(id || ''),
    queryFn: async (): Promise<KbWorkspace | null> => {
      if (!id) return null;
      return await hb.collection<KbWorkspace>('kb_workspaces').get(id, {
        expand: 'owner,members',
      });
    },
    enabled: Boolean(id),
  });
}

export function useCreateWorkspace() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: { name: string; owner: string; members: string[] }): Promise<KbWorkspace> => {
      return await hb.collection<KbWorkspace>('kb_workspaces').create(payload);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: kanbanKeys.allWorkspaces });
    },
  });
}

// 2. Users Hooks
export function useUsers() {
  return useQuery({
    queryKey: kanbanKeys.allUsers,
    queryFn: async (): Promise<KbUser[]> => {
      const page = await hb.collection<KbUser>('kb_users').list({
        perPage: 100,
      });
      return page.items;
    },
  });
}

// 3. Tasks Hooks
export function useTasks(workspaceId?: string | null) {
  return useQuery({
    queryKey: kanbanKeys.tasks(workspaceId || ''),
    queryFn: async (): Promise<KbTask[]> => {
      if (!workspaceId) return [];
      const page = await hb.collection<KbTask>('kb_tasks').list({
        filter: `workspace = '${workspaceId}'`,
        expand: 'assignees,workspace.owner,workspace.members',
        sort: 'order',
        perPage: 100,
      });
      return page.items;
    },
    enabled: Boolean(workspaceId),
  });
}

export function useCreateTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: CreateTaskPayload): Promise<KbTask> => {
      return await hb.collection<KbTask, CreateTaskPayload>('kb_tasks').create(payload);
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(variables.workspace) });
    },
  });
}

export function useUpdateTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, data }: { id: string; data: UpdateTaskPayload; workspaceId: string }): Promise<KbTask> => {
      return await hb.collection<KbTask, any, UpdateTaskPayload>('kb_tasks').update(id, data);
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(variables.workspaceId) });
      queryClient.invalidateQueries({ queryKey: kanbanKeys.task(variables.id) });
    },
  });
}

export function useDeleteTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id }: { id: string; workspaceId: string }): Promise<void> => {
      await hb.collection('kb_tasks').delete(id);
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(variables.workspaceId) });
    },
  });
}

export function useMoveTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      taskId,
      status,
      order,
      workspaceId,
    }: {
      taskId: string;
      status: TaskStatus;
      order: number;
      workspaceId: string;
    }): Promise<KbTask> => {
      return await hb.collection<KbTask, any, { status: TaskStatus; order: number }>('kb_tasks').update(taskId, {
        status,
        order,
      });
    },
    onMutate: async ({ taskId, status, order, workspaceId }) => {
      await queryClient.cancelQueries({ queryKey: kanbanKeys.tasks(workspaceId) });
      const previousTasks = queryClient.getQueryData<KbTask[]>(kanbanKeys.tasks(workspaceId));

      if (previousTasks) {
        queryClient.setQueryData<KbTask[]>(kanbanKeys.tasks(workspaceId), (old) => {
          if (!old) return [];
          return old.map((t) => (t.id === taskId ? { ...t, status, order } : t));
        });
      }

      return { previousTasks, workspaceId };
    },
    onError: (_err, _variables, context) => {
      if (context?.previousTasks && context.workspaceId) {
        queryClient.setQueryData(kanbanKeys.tasks(context.workspaceId), context.previousTasks);
      }
    },
    onSettled: (_data, _error, variables) => {
      queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(variables.workspaceId) });
    },
  });
}

// 4. Comments Hooks
export function useTaskComments(taskId?: string | null) {
  return useQuery({
    queryKey: kanbanKeys.comments(taskId || ''),
    queryFn: async (): Promise<KbComment[]> => {
      if (!taskId) return [];
      const page = await hb.collection<KbComment>('kb_comments').list({
        filter: `task = '${taskId}'`,
        expand: 'author',
        sort: 'created_at',
        perPage: 50,
      });
      return page.items;
    },
    enabled: Boolean(taskId),
  });
}

export function useCreateComment() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (payload: CreateCommentPayload): Promise<KbComment> => {
      return await hb.collection<KbComment, CreateCommentPayload>('kb_comments').create(payload);
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: kanbanKeys.comments(variables.task) });
    },
  });
}

export function useDeleteComment() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id }: { id: string; taskId: string }): Promise<void> => {
      await hb.collection('kb_comments').delete(id);
    },
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: kanbanKeys.comments(variables.taskId) });
    },
  });
}
