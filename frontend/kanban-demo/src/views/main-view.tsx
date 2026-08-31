import React, { useEffect, useMemo, useState } from 'react';
import { useSelector } from '@tanstack/react-store';
import { Kanban, Loader2, Plus } from 'lucide-react';
import { workspaceStore, setActiveWorkspaceId } from '../store/workspace';
import { useTasks, useUsers, useWorkspaces } from '../hooks/use-kanban-data';
import { useRealtimeTasks } from '../hooks/use-realtime-tasks';
import { AppHeader } from '../components/layout/app-header';
import { KanbanBoard } from '../components/board/kanban-board';
import { TaskTable } from '../components/table/task-table';
import { CreateWorkspaceDialog } from '../components/workspace/create-workspace-dialog';
import { Button } from '../components/ui/button';

export function MainView() {
  const { activeWorkspaceId, viewMode } = useSelector(workspaceStore, (s) => s);

  const { data: workspaces = [], isLoading: isLoadingWs } = useWorkspaces();
  const { data: tasks = [], isLoading: isLoadingTasks } = useTasks(activeWorkspaceId);
  const { data: users = [] } = useUsers();

  const [createWsOpen, setCreateWsOpen] = useState(false);

  // Auto-select first workspace if none or invalid
  useEffect(() => {
    if (workspaces.length > 0) {
      const exists = workspaces.some((w) => w.id === activeWorkspaceId);
      if (!activeWorkspaceId || !exists) {
        setActiveWorkspaceId(workspaces[0].id);
      }
    }
  }, [workspaces, activeWorkspaceId]);

  // Realtime SSE Hook
  const { status: realtimeStatus } = useRealtimeTasks(activeWorkspaceId);

  // Active Workspace Members
  const activeWorkspace = workspaces.find((w) => w.id === activeWorkspaceId);
  const activeWorkspaceMembers = useMemo(() => {
    if (!activeWorkspace) return [];
    return users.filter((u) => activeWorkspace.members?.includes(u.id));
  }, [activeWorkspace, users]);

  if (isLoadingWs) {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center space-y-3">
        <Loader2 className="h-8 w-8 animate-spin text-primary" />
        <p className="text-xs text-muted-foreground">正在加载工作区数据...</p>
      </div>
    );
  }

  if (workspaces.length === 0) {
    return (
      <div className="min-h-screen flex flex-col">
        <AppHeader realtimeStatus={realtimeStatus} workspaceMembers={[]} />
        <div className="flex-1 flex flex-col items-center justify-center p-6 text-center space-y-4">
          <div className="h-16 w-16 rounded-2xl bg-muted/60 flex items-center justify-center text-muted-foreground border">
            <Kanban className="h-8 w-8" />
          </div>
          <div className="space-y-1">
            <h2 className="text-lg font-bold text-foreground">暂无可用的工作区</h2>
            <p className="text-xs text-muted-foreground max-w-sm">
              您当前还没有加入任何敏捷工作区。请点击下方按钮新建您的第一个项目看板。
            </p>
          </div>
          <Button onClick={() => setCreateWsOpen(true)} className="text-xs">
            <Plus className="mr-1.5 h-3.5 w-3.5" />
            新建工作区
          </Button>

          <CreateWorkspaceDialog open={createWsOpen} onOpenChange={setCreateWsOpen} />
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex flex-col bg-background selection:bg-primary/20">
      <AppHeader realtimeStatus={realtimeStatus} workspaceMembers={activeWorkspaceMembers} />

      <main className="flex-1 flex flex-col overflow-hidden">
        {isLoadingTasks ? (
          <div className="flex-1 flex items-center justify-center space-x-2 py-12">
            <Loader2 className="h-5 w-5 animate-spin text-primary" />
            <span className="text-xs text-muted-foreground">正在拉取看板任务流...</span>
          </div>
        ) : viewMode === 'board' ? (
          <KanbanBoard
            tasks={tasks}
            workspaceId={activeWorkspaceId || ''}
            workspaceMembers={activeWorkspaceMembers}
          />
        ) : (
          <TaskTable
            tasks={tasks}
            workspaceId={activeWorkspaceId || ''}
            workspaceMembers={activeWorkspaceMembers}
          />
        )}
      </main>
    </div>
  );
}
