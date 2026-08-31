import React from 'react';
import type { LegacyColumnDef } from '@tanstack/react-table/legacy';
import { ArrowUpDown, Lock, MoreHorizontal, Paperclip } from 'lucide-react';
import { PriorityBadge } from '../task/priority-badge';
import { StatusBadge } from '../task/status-badge';
import { Avatar, AvatarFallback } from '../ui/avatar';
import { Button } from '../ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import { formatDate, getInitials, getRandomColor } from '../../lib/utils';
import { COLUMNS, type KbTask, type TaskStatus } from '../../types/kanban';

export interface ColumnActionsProps {
  task: KbTask;
  onView: (task: KbTask) => void;
  onDelete: (task: KbTask) => void;
  onStatusChange: (task: KbTask, status: TaskStatus) => void;
}

export function getTaskTableColumns(actions: ColumnActionsProps): LegacyColumnDef<KbTask, any>[] {
  return [
    {
      accessorKey: 'status',
      header: ({ column }: { column: any }) => (
        <Button
          variant="ghost"
          size="sm"
          className="-ml-3 h-8 text-xs font-semibold"
          onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
        >
          状态
          <ArrowUpDown className="ml-1.5 h-3 w-3" />
        </Button>
      ),
      cell: ({ row }: { row: any }) => {
        const task: KbTask = row.original;
        return (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button type="button" className="cursor-pointer">
                <StatusBadge status={task.status} />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              <DropdownMenuLabel className="text-xs">快速切换泳道状态</DropdownMenuLabel>
              <DropdownMenuSeparator />
              {COLUMNS.map((col) => (
                <DropdownMenuItem
                  key={col.id}
                  onClick={() => actions.onStatusChange(task, col.id)}
                  className="text-xs"
                >
                  <span className={`mr-2 h-2 w-2 rounded-full ${col.dotColor}`} />
                  {col.title}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        );
      },
    },
    {
      accessorKey: 'priority',
      header: ({ column }: { column: any }) => (
        <Button
          variant="ghost"
          size="sm"
          className="-ml-3 h-8 text-xs font-semibold"
          onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
        >
          优先级
          <ArrowUpDown className="ml-1.5 h-3 w-3" />
        </Button>
      ),
      cell: ({ row }: { row: any }) => <PriorityBadge priority={row.original.priority} />,
    },
    {
      accessorKey: 'title',
      header: ({ column }: { column: any }) => (
        <Button
          variant="ghost"
          size="sm"
          className="-ml-3 h-8 text-xs font-semibold"
          onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
        >
          任务名称
          <ArrowUpDown className="ml-1.5 h-3 w-3" />
        </Button>
      ),
      cell: ({ row }: { row: any }) => {
        const task: KbTask = row.original;
        return (
          <div
            className="flex items-center space-x-2 cursor-pointer hover:text-primary transition-colors max-w-md truncate"
            onClick={() => actions.onView(task)}
          >
            {task.is_private && (
              <Lock className="h-3.5 w-3.5 text-amber-500 shrink-0" />
            )}
            <span className="font-semibold text-sm truncate">{task.title}</span>
          </div>
        );
      },
    },
    {
      id: 'assignees',
      header: '执行人',
      cell: ({ row }: { row: any }) => {
        const assignees = row.original.expand?.assignees || [];
        if (assignees.length === 0) {
          return <span className="text-xs text-muted-foreground">未指派</span>;
        }
        return (
          <div className="flex items-center space-x-1.5">
            <div className="flex -space-x-1 overflow-hidden">
              {assignees.slice(0, 3).map((u: any) => {
                const name = u.displayName || u.email;
                return (
                  <Avatar key={u.id} className="h-5 w-5 border border-background">
                    <AvatarFallback className={`${getRandomColor(name)} text-[9px]`}>
                      {getInitials(name)}
                    </AvatarFallback>
                  </Avatar>
                );
              })}
            </div>
            <span className="text-xs text-muted-foreground truncate max-w-[120px]">
              {assignees.map((u: any) => u.displayName || u.email.split('@')[0]).join(', ')}
            </span>
          </div>
        );
      },
    },
    {
      id: 'attachments',
      header: '附件',
      cell: ({ row }: { row: any }) => {
        const count = row.original.attachments?.length || 0;
        if (count === 0) return <span className="text-xs text-muted-foreground">-</span>;
        return (
          <span className="inline-flex items-center text-xs text-muted-foreground">
            <Paperclip className="mr-1 h-3 w-3" />
            {count}
          </span>
        );
      },
    },
    {
      accessorKey: 'created_at',
      header: ({ column }: { column: any }) => (
        <Button
          variant="ghost"
          size="sm"
          className="-ml-3 h-8 text-xs font-semibold"
          onClick={() => column.toggleSorting(column.getIsSorted() === 'asc')}
        >
          创建时间
          <ArrowUpDown className="ml-1.5 h-3 w-3" />
        </Button>
      ),
      cell: ({ row }: { row: any }) => (
        <span className="text-xs text-muted-foreground">
          {formatDate(row.original.created_at, 'YYYY-MM-DD')}
        </span>
      ),
    },
    {
      id: 'actions',
      cell: ({ row }: { row: any }) => {
        const task: KbTask = row.original;
        return (
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="ghost" size="icon" className="h-7 w-7">
                <MoreHorizontal className="h-4 w-4" />
                <span className="sr-only">打开菜单</span>
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onClick={() => actions.onView(task)} className="text-xs">
                查看详情 / 编辑
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onClick={() => actions.onDelete(task)}
                className="text-xs text-destructive focus:text-destructive"
              >
                删除任务
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        );
      },
    },
  ];
}
