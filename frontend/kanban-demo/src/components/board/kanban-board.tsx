import React, { useState } from 'react';
import {
  DndContext,
  DragOverlay,
  PointerSensor,
  TouchSensor,
  closestCorners,
  useSensor,
  useSensors,
  type DragEndEvent,
  type DragOverEvent,
  type DragStartEvent,
} from '@dnd-kit/core';
import confetti from 'canvas-confetti';
import { toast } from 'sonner';
import { useSelector } from '@tanstack/react-store';
import { KanbanColumn } from './kanban-column';
import { TaskCard } from './task-card';
import { TaskDetailDialog } from '../task/task-detail-dialog';
import { TaskForm } from '../task/task-form';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '../ui/dialog';
import { COLUMNS, type CreateTaskPayload, type KbTask, type KbUser, type TaskStatus } from '../../types/kanban';
import { useCreateTask, useMoveTask } from '../../hooks/use-kanban-data';
import { workspaceStore } from '../../store/workspace';
import { authStore } from '../../store/auth';

export interface KanbanBoardProps {
  tasks: KbTask[];
  workspaceId: string;
  workspaceMembers?: KbUser[];
}

export function KanbanBoard({ tasks, workspaceId, workspaceMembers = [] }: KanbanBoardProps) {
  const currentUser = useSelector(authStore, (s) => s.user);
  const { searchQuery, selectedPriorities, selectedAssignees, onlyMyTasks } = useSelector(
    workspaceStore,
    (s) => s,
  );

  const [activeTask, setActiveTask] = useState<KbTask | null>(null);
  const [selectedTask, setSelectedTask] = useState<KbTask | null>(null);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [createDefaultStatus, setCreateDefaultStatus] = useState<TaskStatus>('todo');

  const createTask = useCreateTask();
  const moveTask = useMoveTask();

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 5,
      },
    }),
    useSensor(TouchSensor, {
      activationConstraint: {
        delay: 150,
        tolerance: 5,
      },
    }),
  );

  // Filter Tasks
  const filteredTasks = tasks.filter((task) => {
    // Search query
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      const matchTitle = task.title.toLowerCase().includes(q);
      const matchDesc = task.description?.toLowerCase().includes(q);
      if (!matchTitle && !matchDesc) return false;
    }

    // Priority filter
    if (selectedPriorities.length > 0) {
      if (!selectedPriorities.includes(task.priority)) return false;
    }

    // Assignee filter
    if (selectedAssignees.length > 0) {
      const taskAssignees = task.assignees || [];
      const hasMatch = selectedAssignees.some((id) => taskAssignees.includes(id));
      if (!hasMatch) return false;
    }

    // Only my tasks filter
    if (onlyMyTasks && currentUser) {
      const isAssigned = task.assignees?.includes(currentUser.id);
      if (!isAssigned) return false;
    }

    return true;
  });

  const getColumnTasks = (status: TaskStatus) => {
    return filteredTasks
      .filter((t) => t.status === status)
      .sort((a, b) => (a.order || 0) - (b.order || 0));
  };

  const handleDragStart = (event: DragStartEvent) => {
    const { active } = event;
    const task = tasks.find((t) => t.id === active.id);
    if (task) setActiveTask(task);
  };

  const handleDragOver = (_event: DragOverEvent) => {
    // Handled in dragEnd
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveTask(null);

    if (!over) return;

    const activeTaskId = String(active.id);
    const overId = String(over.id);

    const task = tasks.find((t) => t.id === activeTaskId);
    if (!task) return;

    // Check if dropped directly on a column
    const isOverColumn = COLUMNS.some((col) => col.id === overId);
    let targetStatus: TaskStatus = task.status;
    let targetOrder = task.order;

    if (isOverColumn) {
      targetStatus = overId as TaskStatus;
      const targetColumnTasks = tasks.filter((t) => t.status === targetStatus && t.id !== activeTaskId);
      const lastTask = targetColumnTasks[targetColumnTasks.length - 1];
      targetOrder = lastTask ? (lastTask.order || 0) + 10 : 10;
    } else {
      // Dropped on another task
      const overTask = tasks.find((t) => t.id === overId);
      if (overTask) {
        targetStatus = overTask.status;
        const targetColumnTasks = tasks
          .filter((t) => t.status === targetStatus)
          .sort((a, b) => (a.order || 0) - (b.order || 0));

        const overIndex = targetColumnTasks.findIndex((t) => t.id === overId);
        if (overIndex >= 0) {
          const prevTask = targetColumnTasks[overIndex - 1];
          const nextTask = targetColumnTasks[overIndex + 1];

          if (!prevTask) {
            targetOrder = (overTask.order || 10) / 2;
          } else if (!nextTask) {
            targetOrder = (overTask.order || 10) + 10;
          } else {
            targetOrder = ((prevTask.order || 0) + (overTask.order || 0)) / 2;
          }
        }
      }
    }

    if (task.status === targetStatus && Math.abs(task.order - targetOrder) < 0.001) {
      return;
    }

    try {
      if (targetStatus === 'done' && task.status !== 'done') {
        confetti({
          particleCount: 80,
          spread: 60,
          origin: { y: 0.7 },
        });
      }

      await moveTask.mutateAsync({
        taskId: task.id,
        status: targetStatus,
        order: targetOrder,
        workspaceId,
      });
    } catch (err) {
      console.error(err);
      toast.error(`移动任务失败: ${err instanceof Error ? err.message : '权限不足或网络异常'}`);
    }
  };

  const handleQuickAdd = async (title: string, status: TaskStatus) => {
    const colTasks = getColumnTasks(status);
    const lastOrder = colTasks[colTasks.length - 1]?.order || 0;

    try {
      await createTask.mutateAsync({
        title,
        status,
        priority: 'medium',
        workspace: workspaceId,
        assignees: currentUser ? [currentUser.id] : [],
        order: lastOrder + 10,
        is_private: false,
      });
      toast.success(`已创建任务「${title}」`);
    } catch (err) {
      console.error(err);
      toast.error(`创建失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  const handleDetailedCreate = async (payload: CreateTaskPayload) => {
    try {
      await createTask.mutateAsync(payload);
      toast.success('任务已成功创建');
      setCreateDialogOpen(false);
    } catch (err) {
      console.error(err);
      toast.error(`创建失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCorners}
      onDragStart={handleDragStart}
      onDragOver={handleDragOver}
      onDragEnd={handleDragEnd}
    >
      <div className="flex flex-1 gap-4 overflow-x-auto p-6 items-start min-h-[calc(100vh-8rem)]">
        {COLUMNS.map((column) => (
          <KanbanColumn
            key={column.id}
            id={column.id}
            title={column.title}
            dotColor={column.dotColor}
            tasks={getColumnTasks(column.id)}
            onTaskClick={(t) => setSelectedTask(t)}
            onAddTask={handleQuickAdd}
            onOpenCreateDialog={(status) => {
              setCreateDefaultStatus(status);
              setCreateDialogOpen(true);
            }}
          />
        ))}
      </div>

      {/* Drag Overlay Ghost */}
      <DragOverlay>
        {activeTask ? (
          <div className="w-[300px] pointer-events-none rotate-2 opacity-90 shadow-2xl">
            <TaskCard task={activeTask} onClick={() => {}} />
          </div>
        ) : null}
      </DragOverlay>

      {/* Task Details Dialog */}
      <TaskDetailDialog
        task={selectedTask}
        open={Boolean(selectedTask)}
        onOpenChange={(open) => !open && setSelectedTask(null)}
        workspaceMembers={workspaceMembers}
      />

      {/* Detailed Create Task Dialog */}
      <Dialog open={createDialogOpen} onOpenChange={setCreateDialogOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>创建敏捷任务</DialogTitle>
            <DialogDescription>
              向工作区添加新卡片，指定指派成员、泳道状态与优先级。
            </DialogDescription>
          </DialogHeader>
          <TaskForm
            workspaceId={workspaceId}
            workspaceMembers={workspaceMembers}
            defaultStatus={createDefaultStatus}
            onSubmit={handleDetailedCreate as any}
            onCancel={() => setCreateDialogOpen(false)}
            isSubmitting={createTask.isPending}
          />
        </DialogContent>
      </Dialog>
    </DndContext>
  );
}
