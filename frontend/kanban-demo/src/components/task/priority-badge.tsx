import React from 'react';
import { AlertCircle, AlertTriangle, ArrowDown, ArrowUp } from 'lucide-react';
import { cn } from '../../lib/utils';
import { PRIORITIES, type TaskPriority } from '../../types/kanban';

export interface PriorityBadgeProps {
  priority: TaskPriority;
  className?: string;
  showIcon?: boolean;
}

export function PriorityBadge({ priority, className, showIcon = true }: PriorityBadgeProps) {
  const config = PRIORITIES.find((p) => p.id === priority) || PRIORITIES[3];

  const renderIcon = () => {
    switch (priority) {
      case 'urgent':
        return <AlertCircle className="mr-1 h-3 w-3 text-red-500" />;
      case 'high':
        return <ArrowUp className="mr-1 h-3 w-3 text-amber-500" />;
      case 'medium':
        return <AlertTriangle className="mr-1 h-3 w-3 text-blue-500" />;
      case 'low':
      default:
        return <ArrowDown className="mr-1 h-3 w-3 text-slate-500" />;
    }
  };

  return (
    <span
      className={cn(
        'inline-flex items-center rounded-md border px-2 py-0.5 text-xs font-medium',
        config.color,
        className,
      )}
    >
      {showIcon && renderIcon()}
      {config.label}
    </span>
  );
}
