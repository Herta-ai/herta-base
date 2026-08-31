import React from 'react';
import { cn } from '../../lib/utils';
import { COLUMNS, type TaskStatus } from '../../types/kanban';

export interface StatusBadgeProps {
  status: TaskStatus;
  className?: string;
}

export function StatusBadge({ status, className }: StatusBadgeProps) {
  const col = COLUMNS.find((c) => c.id === status) || COLUMNS[0];

  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium border',
        col.color,
        className,
      )}
    >
      <span className={cn('mr-1.5 h-1.5 w-1.5 rounded-full', col.dotColor)} />
      {col.title}
    </span>
  );
}
