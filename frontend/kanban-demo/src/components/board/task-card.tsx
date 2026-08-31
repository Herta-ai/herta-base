import React from 'react';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { GripVertical, Lock, MessageSquare, Paperclip } from 'lucide-react';
import { PriorityBadge } from '../task/priority-badge';
import { Avatar, AvatarFallback } from '../ui/avatar';
import { getInitials, getRandomColor } from '../../lib/utils';
import type { KbTask } from '../../types/kanban';

export interface TaskCardProps {
  task: KbTask;
  onClick: (task: KbTask) => void;
}

export function TaskCard({ task, onClick }: TaskCardProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: task.id,
    data: {
      type: 'Task',
      task,
    },
  });

  const style = {
    transform: CSS.Translate.toString(transform),
    transition,
  };

  const assignees = task.expand?.assignees || [];
  const attachmentCount = task.attachments?.length || 0;

  return (
    <div
      ref={setNodeRef}
      style={style}
      onClick={() => onClick(task)}
      className={`group relative rounded-xl border bg-card p-3.5 shadow-sm transition-all duration-200 hover:shadow-md hover:border-primary/40 cursor-pointer ${
        isDragging ? 'opacity-40 ring-2 ring-primary shadow-lg scale-105 z-50' : 'opacity-100'
      }`}
    >
      {/* Top row: Priority & Privacy & Drag handle */}
      <div className="flex items-center justify-between gap-1 mb-2">
        <div className="flex items-center space-x-1.5">
          <PriorityBadge priority={task.priority} />
          {task.is_private && (
            <span
              className="inline-flex items-center text-[10px] text-amber-500 font-medium bg-amber-500/10 px-1.5 py-0.5 rounded border border-amber-500/20"
              title="私有卡片 - 仅负责人与指派成员可见"
            >
              <Lock className="h-3 w-3 mr-0.5" />
              私有
            </span>
          )}
        </div>

        <button
          type="button"
          {...attributes}
          {...listeners}
          onClick={(e) => e.stopPropagation()}
          className="opacity-0 group-hover:opacity-100 p-1 text-muted-foreground hover:text-foreground cursor-grab active:cursor-grabbing transition-opacity rounded"
          title="拖拽排序"
        >
          <GripVertical className="h-3.5 w-3.5" />
        </button>
      </div>

      {/* Title */}
      <h4 className="text-sm font-semibold text-foreground leading-snug line-clamp-2 mb-1.5 group-hover:text-primary transition-colors">
        {task.title}
      </h4>

      {/* Description Snippet */}
      {task.description && (
        <p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed mb-3">
          {task.description}
        </p>
      )}

      {/* Bottom row: Indicators & Assignees */}
      <div className="flex items-center justify-between pt-1 border-t border-border/50 text-xs text-muted-foreground">
        <div className="flex items-center space-x-2.5">
          {attachmentCount > 0 && (
            <span className="flex items-center text-[11px] text-muted-foreground hover:text-foreground">
              <Paperclip className="h-3 w-3 mr-1" />
              {attachmentCount}
            </span>
          )}
          {/* Comments count indicator */}
          <span className="flex items-center text-[11px] text-muted-foreground hover:text-foreground">
            <MessageSquare className="h-3 w-3 mr-1" />
            讨论
          </span>
        </div>

        {/* Assignees Avatars */}
        {assignees.length > 0 && (
          <div className="flex -space-x-1.5 overflow-hidden">
            {assignees.slice(0, 3).map((user) => {
              const name = user.displayName || user.email;
              return (
                <Avatar key={user.id} className="h-5 w-5 border border-background shadow-xs">
                  <AvatarFallback className={`${getRandomColor(name)} text-[9px]`}>
                    {getInitials(name)}
                  </AvatarFallback>
                </Avatar>
              );
            })}
            {assignees.length > 3 && (
              <span className="flex h-5 w-5 items-center justify-center rounded-full bg-muted text-[9px] font-semibold text-muted-foreground border border-background">
                +{assignees.length - 3}
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
