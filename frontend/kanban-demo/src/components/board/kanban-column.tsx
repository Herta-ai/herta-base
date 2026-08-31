import React from 'react';
import { useDroppable } from '@dnd-kit/core';
import { SortableContext, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { Plus } from 'lucide-react';
import { TaskCard } from './task-card';
import { AddTaskCard } from './add-task-card';
import { Badge } from '../ui/badge';
import type { KbTask, TaskStatus } from '../../types/kanban';

export interface KanbanColumnProps {
  id: TaskStatus;
  title: string;
  dotColor: string;
  tasks: KbTask[];
  onTaskClick: (task: KbTask) => void;
  onAddTask: (title: string, status: TaskStatus) => Promise<void>;
  onOpenCreateDialog: (status: TaskStatus) => void;
}

export function KanbanColumn({
  id,
  title,
  dotColor,
  tasks,
  onTaskClick,
  onAddTask,
  onOpenCreateDialog,
}: KanbanColumnProps) {
  const { setNodeRef, isOver } = useDroppable({
    id,
    data: {
      type: 'Column',
      columnId: id,
    },
  });

  const taskIds = tasks.map((t) => t.id);

  return (
    <div
      ref={setNodeRef}
      className={`flex flex-col flex-1 min-w-[280px] max-w-[340px] rounded-2xl border bg-muted/40 p-3 shadow-xs transition-colors duration-150 ${
        isOver ? 'bg-primary/5 ring-2 ring-primary/40 border-primary/40' : ''
      }`}
    >
      {/* Column Header */}
      <div className="flex items-center justify-between px-1 py-1.5 mb-2">
        <div className="flex items-center space-x-2">
          <span className={`h-2.5 w-2.5 rounded-full ${dotColor}`} />
          <h3 className="font-semibold text-sm text-foreground tracking-tight">{title}</h3>
          <Badge variant="secondary" className="h-5 px-1.5 text-[11px] font-mono">
            {tasks.length}
          </Badge>
        </div>

        <button
          type="button"
          onClick={() => onOpenCreateDialog(id)}
          className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-muted transition-colors cursor-pointer"
          title={`在「${title}」中创建详细任务`}
        >
          <Plus className="h-4 w-4" />
        </button>
      </div>

      {/* Task Cards Container */}
      <div className="flex-1 space-y-2.5 overflow-y-auto min-h-[160px] p-0.5">
        <SortableContext items={taskIds} strategy={verticalListSortingStrategy}>
          {tasks.map((task) => (
            <TaskCard key={task.id} task={task} onClick={onTaskClick} />
          ))}
        </SortableContext>
      </div>

      {/* Quick Add at Bottom */}
      <div className="pt-2">
        <AddTaskCard status={id} onAdd={onAddTask} />
      </div>
    </div>
  );
}
