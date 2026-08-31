import React, { useState } from 'react';
import { useSelector } from '@tanstack/react-store';
import { Edit3, Lock, Trash2, Users } from 'lucide-react';
import { toast } from 'sonner';
import { authStore } from '../../store/auth';
import { useDeleteTask, useUpdateTask } from '../../hooks/use-kanban-data';
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from '../ui/dialog';
import { Button } from '../ui/button';
import { Avatar, AvatarFallback } from '../ui/avatar';
import { Separator } from '../ui/separator';
import { PriorityBadge } from './priority-badge';
import { StatusBadge } from './status-badge';
import { AttachmentManager } from './attachment-manager';
import { TaskComments } from './task-comments';
import { TaskForm } from './task-form';
import { formatDate, getInitials, getRandomColor } from '../../lib/utils';
import type { KbTask, KbUser, UpdateTaskPayload } from '../../types/kanban';

export interface TaskDetailDialogProps {
  task: KbTask | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  workspaceMembers?: KbUser[];
}

export function TaskDetailDialog({
  task,
  open,
  onOpenChange,
  workspaceMembers = [],
}: TaskDetailDialogProps) {
  const currentUser = useSelector(authStore, (s) => s.user);
  const [isEditing, setIsEditing] = useState(false);

  const updateTask = useUpdateTask();
  const deleteTask = useDeleteTask();

  if (!task) return null;

  const isOwner = task.expand?.workspace?.owner === currentUser?.id;
  const isAssignee = task.assignees?.includes(currentUser?.id || '');
  const canEdit = !task.is_private || isOwner || isAssignee;

  const handleUpdate = async (payload: UpdateTaskPayload) => {
    try {
      await updateTask.mutateAsync({
        id: task.id,
        data: payload,
        workspaceId: task.workspace,
      });
      toast.success('任务信息已更新');
      setIsEditing(false);
    } catch (err) {
      console.error(err);
      toast.error(`更新失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  const handleDelete = async () => {
    if (!window.confirm(`确定要彻底删除任务「${task.title}」吗？`)) return;
    try {
      await deleteTask.mutateAsync({ id: task.id, workspaceId: task.workspace });
      toast.success('任务已成功删除');
      onOpenChange(false);
    } catch (err) {
      toast.error(`删除失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  const assignedUsers = task.expand?.assignees || [];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto p-6">
        <DialogHeader className="space-y-2">
          <div className="flex flex-wrap items-center justify-between gap-2 pr-6">
            <div className="flex items-center space-x-2">
              <StatusBadge status={task.status} />
              <PriorityBadge priority={task.priority} />
              {task.is_private && (
                <span className="inline-flex items-center text-xs text-amber-500 font-medium bg-amber-500/10 px-2 py-0.5 rounded border border-amber-500/20">
                  <Lock className="mr-1 h-3 w-3" /> 私有任务
                </span>
              )}
            </div>

            {canEdit && (
              <div className="flex items-center space-x-1">
                <Button
                  size="sm"
                  variant={isEditing ? 'secondary' : 'outline'}
                  className="h-7 text-xs"
                  onClick={() => setIsEditing(!isEditing)}
                >
                  <Edit3 className="mr-1 h-3 w-3" />
                  {isEditing ? '返回查看' : '编辑属性'}
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-7 w-7 text-muted-foreground hover:text-destructive"
                  onClick={handleDelete}
                  title="删除任务"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </div>
            )}
          </div>

          {!isEditing && (
            <DialogTitle className="text-xl font-bold tracking-tight text-foreground pt-1">
              {task.title}
            </DialogTitle>
          )}
          <DialogDescription className="text-xs text-muted-foreground">
            创建于 {formatDate(task.created_at)} · 任务 ID: <code className="bg-muted px-1 py-0.5 rounded text-[11px]">{task.id}</code>
          </DialogDescription>
        </DialogHeader>

        {isEditing ? (
          <div className="py-2">
            <TaskForm
              initialData={task}
              workspaceId={task.workspace}
              workspaceMembers={workspaceMembers}
              onSubmit={handleUpdate}
              onCancel={() => setIsEditing(false)}
              isSubmitting={updateTask.isPending}
            />
          </div>
        ) : (
          <div className="space-y-5 py-2">
            {/* Description */}
            <div className="space-y-1.5">
              <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                任务说明
              </h4>
              <div className="rounded-lg bg-muted/20 border p-3.5 text-sm leading-relaxed text-foreground whitespace-pre-wrap">
                {task.description || (
                  <span className="italic text-muted-foreground text-xs">暂无详细描述</span>
                )}
              </div>
            </div>

            {/* Assignees */}
            <div className="space-y-1.5">
              <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider flex items-center">
                <Users className="mr-1 h-3.5 w-3.5" /> 指派成员
              </h4>
              {assignedUsers.length === 0 ? (
                <span className="text-xs text-muted-foreground">未指派特定成员</span>
              ) : (
                <div className="flex flex-wrap gap-2">
                  {assignedUsers.map((user) => {
                    const name = user.displayName || user.email;
                    return (
                      <div
                        key={user.id}
                        className="inline-flex items-center space-x-1.5 rounded-full border bg-background px-2.5 py-1 text-xs shadow-sm"
                      >
                        <Avatar className="h-5 w-5">
                          <AvatarFallback className={getRandomColor(name)}>
                            {getInitials(name)}
                          </AvatarFallback>
                        </Avatar>
                        <span className="font-medium text-foreground">{name}</span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>

            <Separator />

            {/* Attachments Section */}
            <AttachmentManager task={task} canEdit={canEdit} />

            <Separator />

            {/* Comments Section */}
            <TaskComments task={task} />
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
