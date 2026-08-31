import React, { useState } from 'react';
import { useSelector } from '@tanstack/react-store';
import {
  Check,
  ChevronDown,
  Filter,
  Kanban,
  LayoutGrid,
  Plus,
  Search,
  Table as TableIcon,
  X,
} from 'lucide-react';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import { LiveIndicator } from './live-indicator';
import { ThemeToggle } from './theme-toggle';
import { UserSwitchMenu } from './user-switch-menu';
import { CreateWorkspaceDialog } from '../workspace/create-workspace-dialog';
import { TaskForm } from '../task/task-form';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '../ui/dialog';
import {
  resetFilters,
  setActiveWorkspaceId,
  setOnlyMyTasks,
  setSearchQuery,
  setViewMode,
  togglePriorityFilter,
  workspaceStore,
} from '../../store/workspace';
import { useCreateTask, useWorkspaces } from '../../hooks/use-kanban-data';
import { PRIORITIES, type CreateTaskPayload, type KbUser, type TaskPriority } from '../../types/kanban';
import { toast } from 'sonner';
import type { RealtimeStatus } from '@hb/sdk';

export interface AppHeaderProps {
  realtimeStatus: RealtimeStatus;
  workspaceMembers?: KbUser[];
}

export function AppHeader({ realtimeStatus, workspaceMembers = [] }: AppHeaderProps) {
  const { data: workspaces = [] } = useWorkspaces();
  const {
    activeWorkspaceId,
    searchQuery,
    selectedPriorities,
    onlyMyTasks,
    viewMode,
  } = useSelector(workspaceStore, (s) => s);

  const [createWsOpen, setCreateWsOpen] = useState(false);
  const [createTaskOpen, setCreateTaskOpen] = useState(false);

  const createTask = useCreateTask();

  const activeWorkspace = workspaces.find((w) => w.id === activeWorkspaceId) || workspaces[0];

  const handleCreateTask = async (payload: CreateTaskPayload) => {
    if (!activeWorkspaceId) return;
    try {
      await createTask.mutateAsync(payload);
      toast.success('任务已成功创建');
      setCreateTaskOpen(false);
    } catch (err) {
      toast.error(`创建失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  const hasActiveFilters = searchQuery || selectedPriorities.length > 0 || onlyMyTasks;

  return (
    <header className="sticky top-0 z-40 w-full border-b bg-background/95 backdrop-blur-md supports-[backdrop-filter]:bg-background/80">
      {/* Top Navbar */}
      <div className="flex h-14 items-center justify-between px-4 sm:px-6">
        {/* Left: Logo & Workspace Switcher */}
        <div className="flex items-center space-x-3 sm:space-x-4">
          <div className="flex items-center space-x-2">
            <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-sm">
              <Kanban className="h-4 w-4" />
            </div>
            <span className="hidden font-bold text-base tracking-tight sm:inline-block">
              HertaKanban
            </span>
          </div>

          <div className="h-4 w-[1px] bg-border hidden sm:block" />

          {/* Workspace Dropdown */}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm" className="h-8 max-w-[200px] justify-between text-xs">
                <span className="truncate font-medium">
                  {activeWorkspace ? activeWorkspace.name : '选择工作区'}
                </span>
                <ChevronDown className="ml-1.5 h-3.5 w-3.5 opacity-60 shrink-0" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-56">
              <DropdownMenuLabel className="text-xs text-muted-foreground">
                切换工作区
              </DropdownMenuLabel>
              <DropdownMenuSeparator />
              {workspaces.map((ws) => {
                const isCurrent = ws.id === activeWorkspaceId;
                return (
                  <DropdownMenuItem
                    key={ws.id}
                    onClick={() => setActiveWorkspaceId(ws.id)}
                    className="flex items-center justify-between text-xs cursor-pointer"
                  >
                    <span className="truncate">{ws.name}</span>
                    {isCurrent && <Check className="h-3.5 w-3.5 text-primary shrink-0" />}
                  </DropdownMenuItem>
                );
              })}
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={() => setCreateWsOpen(true)}
                className="text-xs text-primary font-medium cursor-pointer"
              >
                <Plus className="mr-1.5 h-3.5 w-3.5" />
                新建敏捷工作区...
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>

          {/* View Mode Toggle */}
          <div className="flex items-center rounded-lg border bg-muted/30 p-0.5">
            <button
              type="button"
              onClick={() => setViewMode('board')}
              className={`flex items-center space-x-1 rounded-md px-2.5 py-1 text-xs font-medium transition-colors cursor-pointer ${
                viewMode === 'board'
                  ? 'bg-background text-foreground shadow-xs'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              <LayoutGrid className="h-3.5 w-3.5" />
              <span>看板</span>
            </button>
            <button
              type="button"
              onClick={() => setViewMode('table')}
              className={`flex items-center space-x-1 rounded-md px-2.5 py-1 text-xs font-medium transition-colors cursor-pointer ${
                viewMode === 'table'
                  ? 'bg-background text-foreground shadow-xs'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              <TableIcon className="h-3.5 w-3.5" />
              <span>表格</span>
            </button>
          </div>
        </div>

        {/* Right Action Bar */}
        <div className="flex items-center space-x-2 sm:space-x-3">
          <LiveIndicator status={realtimeStatus} />

          {activeWorkspaceId && (
            <Button
              size="sm"
              className="h-8 text-xs font-medium shadow-xs"
              onClick={() => setCreateTaskOpen(true)}
            >
              <Plus className="mr-1.5 h-3.5 w-3.5" />
              新建任务
            </Button>
          )}

          <ThemeToggle />
          <UserSwitchMenu />
        </div>
      </div>

      {/* Sub-header: Search & Quick Filters */}
      <div className="flex flex-wrap items-center justify-between gap-2 border-t px-4 py-2 bg-muted/10 sm:px-6">
        <div className="flex flex-1 items-center space-x-2 min-w-[240px] max-w-md">
          <div className="relative w-full">
            <Search className="absolute left-2.5 top-2 h-3.5 w-3.5 text-muted-foreground" />
            <Input
              placeholder="搜索任务标题或描述..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="h-7 pl-8 pr-8 text-xs bg-background/50"
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => setSearchQuery('')}
                className="absolute right-2 top-1.5 text-muted-foreground hover:text-foreground"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            )}
          </div>
        </div>

        {/* Priority Filter Chips */}
        <div className="flex flex-wrap items-center space-x-1.5 text-xs">
          <span className="text-[11px] text-muted-foreground flex items-center mr-1">
            <Filter className="h-3 w-3 mr-1" /> 筛选:
          </span>

          <button
            type="button"
            onClick={() => setOnlyMyTasks(!onlyMyTasks)}
            className={`rounded-full px-2.5 py-0.5 text-xs font-medium border transition-colors cursor-pointer ${
              onlyMyTasks
                ? 'bg-primary text-primary-foreground border-primary'
                : 'bg-background text-muted-foreground border-border hover:bg-muted'
            }`}
          >
            我的任务
          </button>

          {PRIORITIES.map((p) => {
            const isSelected = selectedPriorities.includes(p.id);
            return (
              <button
                type="button"
                key={p.id}
                onClick={() => togglePriorityFilter(p.id as TaskPriority)}
                className={`rounded-full px-2 py-0.5 text-[11px] font-medium border transition-colors cursor-pointer ${
                  isSelected
                    ? 'bg-primary text-primary-foreground border-primary'
                    : 'bg-background text-muted-foreground border-border hover:bg-muted'
                }`}
              >
                {p.label}
              </button>
            );
          })}

          {hasActiveFilters && (
            <button
              type="button"
              onClick={resetFilters}
              className="text-[11px] text-muted-foreground hover:text-foreground underline pl-1 cursor-pointer"
            >
              清空
            </button>
          )}
        </div>
      </div>

      {/* Create Workspace Dialog */}
      <CreateWorkspaceDialog open={createWsOpen} onOpenChange={setCreateWsOpen} />

      {/* Create Task Dialog */}
      <Dialog open={createTaskOpen} onOpenChange={setCreateTaskOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>创建敏捷任务</DialogTitle>
            <DialogDescription>
              向「{activeWorkspace?.name}」添加新卡片，指定指派成员与泳道状态。
            </DialogDescription>
          </DialogHeader>
          {activeWorkspaceId && (
            <TaskForm
              workspaceId={activeWorkspaceId}
              workspaceMembers={workspaceMembers}
              defaultStatus="todo"
              onSubmit={handleCreateTask as any}
              onCancel={() => setCreateTaskOpen(false)}
              isSubmitting={createTask.isPending}
            />
          )}
        </DialogContent>
      </Dialog>
    </header>
  );
}
