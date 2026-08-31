import React, { useMemo, useState } from 'react';
import {
  getCoreRowModel,
  getFilteredRowModel,
  getPaginationRowModel,
  getSortedRowModel,
  useLegacyTable,
} from '@tanstack/react-table/legacy';
import { flexRender } from '@tanstack/react-table';
import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from 'lucide-react';
import { toast } from 'sonner';
import { useSelector } from '@tanstack/react-store';
import { getTaskTableColumns } from './table-columns';
import { TaskDetailDialog } from '../task/task-detail-dialog';
import { Button } from '../ui/button';
import { useDeleteTask, useUpdateTask } from '../../hooks/use-kanban-data';
import { workspaceStore } from '../../store/workspace';
import { authStore } from '../../store/auth';
import type { KbTask, KbUser, TaskStatus } from '../../types/kanban';

export type TableSortingState = Array<{ id: string; desc: boolean }>;

export interface TaskTableProps {
  tasks: KbTask[];
  workspaceId: string;
  workspaceMembers?: KbUser[];
}

export function TaskTable({ tasks, workspaceId, workspaceMembers = [] }: TaskTableProps) {
  const currentUser = useSelector(authStore, (s) => s.user);
  const { searchQuery, selectedPriorities, selectedAssignees, onlyMyTasks } = useSelector(
    workspaceStore,
    (s) => s,
  );

  const [sorting, setSorting] = useState<TableSortingState>([]);
  const [selectedTask, setSelectedTask] = useState<KbTask | null>(null);

  const updateTask = useUpdateTask();
  const deleteTask = useDeleteTask();

  // Filter Tasks
  const filteredData = useMemo(() => {
    return tasks.filter((task) => {
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase();
        const matchTitle = task.title.toLowerCase().includes(q);
        const matchDesc = task.description?.toLowerCase().includes(q);
        if (!matchTitle && !matchDesc) return false;
      }
      if (selectedPriorities.length > 0) {
        if (!selectedPriorities.includes(task.priority)) return false;
      }
      if (selectedAssignees.length > 0) {
        const taskAssignees = task.assignees || [];
        const hasMatch = selectedAssignees.some((id) => taskAssignees.includes(id));
        if (!hasMatch) return false;
      }
      if (onlyMyTasks && currentUser) {
        const isAssigned = task.assignees?.includes(currentUser.id);
        if (!isAssigned) return false;
      }
      return true;
    });
  }, [tasks, searchQuery, selectedPriorities, selectedAssignees, onlyMyTasks, currentUser]);

  const columns = useMemo(
    () =>
      getTaskTableColumns({
        task: {} as KbTask,
        onView: (task) => setSelectedTask(task),
        onDelete: async (task) => {
          if (!window.confirm(`确定要彻底删除任务「${task.title}」吗？`)) return;
          try {
            await deleteTask.mutateAsync({ id: task.id, workspaceId });
            toast.success('任务已成功删除');
          } catch (err) {
            toast.error(`删除失败: ${err instanceof Error ? err.message : '权限不足'}`);
          }
        },
        onStatusChange: async (task, newStatus: TaskStatus) => {
          try {
            await updateTask.mutateAsync({
              id: task.id,
              data: { status: newStatus },
              workspaceId,
            });
            toast.success(`状态已变更至「${newStatus}」`);
          } catch (err) {
            toast.error(`变更失败: ${err instanceof Error ? err.message : '权限不足'}`);
          }
        },
      }),
    [workspaceId, deleteTask, updateTask],
  );

  const table = useLegacyTable({
    data: filteredData,
    columns,
    state: {
      sorting,
    },
    onSortingChange: setSorting as any,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getPaginationRowModel: getPaginationRowModel(),
    initialState: {
      pagination: {
        pageIndex: 0,
        pageSize: 15,
      },
    },
  });

  return (
    <div className="flex-1 p-6 space-y-4">
      <div className="rounded-xl border bg-card shadow-xs overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse text-sm">
            <thead>
              {table.getHeaderGroups().map((headerGroup: any) => (
                <tr key={headerGroup.id} className="border-b bg-muted/40 text-xs text-muted-foreground">
                  {headerGroup.headers.map((header: any) => (
                    <th key={header.id} className="p-3 font-medium">
                      {header.isPlaceholder
                        ? null
                        : flexRender(header.column.columnDef.header, header.getContext())}
                    </th>
                  ))}
                </tr>
              ))}
            </thead>
            <tbody className="divide-y">
              {table.getRowModel().rows.length === 0 ? (
                <tr>
                  <td colSpan={columns.length} className="h-32 text-center text-xs text-muted-foreground">
                    未检索到符合条件的任务
                  </td>
                </tr>
              ) : (
                table.getRowModel().rows.map((row: any) => (
                  <tr
                    key={row.id}
                    className="hover:bg-muted/30 transition-colors"
                  >
                    {row.getVisibleCells().map((cell: any) => (
                      <td key={cell.id} className="p-3 align-middle">
                        {flexRender(cell.column.columnDef.cell, cell.getContext())}
                      </td>
                    ))}
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Pagination Controls */}
        <div className="flex items-center justify-between border-t px-4 py-3 text-xs text-muted-foreground">
          <div>
            共 <span className="font-semibold text-foreground">{filteredData.length}</span> 项任务
          </div>
          <div className="flex items-center space-x-2">
            <span>
              第 <span className="font-semibold text-foreground">{table.getState().pagination.pageIndex + 1}</span> /{' '}
              {table.getPageCount() || 1} 页
            </span>
            <div className="flex items-center space-x-1">
              <Button
                variant="outline"
                size="icon"
                className="h-7 w-7"
                onClick={() => table.setPageIndex(0)}
                disabled={!table.getCanPreviousPage()}
              >
                <ChevronsLeft className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                className="h-7 w-7"
                onClick={() => table.previousPage()}
                disabled={!table.getCanPreviousPage()}
              >
                <ChevronLeft className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                className="h-7 w-7"
                onClick={() => table.nextPage()}
                disabled={!table.getCanNextPage()}
              >
                <ChevronRight className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                className="h-7 w-7"
                onClick={() => table.setPageIndex(table.getPageCount() - 1)}
                disabled={!table.getCanNextPage()}
              >
                <ChevronsRight className="h-3.5 w-3.5" />
              </Button>
            </div>
          </div>
        </div>
      </div>

      {/* Task Details Dialog */}
      <TaskDetailDialog
        task={selectedTask}
        open={Boolean(selectedTask)}
        onOpenChange={(open) => !open && setSelectedTask(null)}
        workspaceMembers={workspaceMembers}
      />
    </div>
  );
}
