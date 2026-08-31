import React, { useState } from 'react';
import { Lock, Unlock, Users } from 'lucide-react';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Textarea } from '../ui/textarea';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../ui/select';
import { COLUMNS, PRIORITIES, type CreateTaskPayload, type KbTask, type KbUser, type TaskPriority, type TaskStatus, type UpdateTaskPayload } from '../../types/kanban';
import { Avatar, AvatarFallback } from '../ui/avatar';
import { getInitials, getRandomColor } from '../../lib/utils';

export interface TaskFormProps {
  initialData?: KbTask | null;
  workspaceId: string;
  workspaceMembers?: KbUser[];
  defaultStatus?: TaskStatus;
  onSubmit: (payload: CreateTaskPayload | UpdateTaskPayload) => Promise<void>;
  onCancel: () => void;
  isSubmitting?: boolean;
}

export function TaskForm({
  initialData,
  workspaceId,
  workspaceMembers = [],
  defaultStatus = 'todo',
  onSubmit,
  onCancel,
  isSubmitting = false,
}: TaskFormProps) {
  const [title, setTitle] = useState(initialData?.title || '');
  const [description, setDescription] = useState(initialData?.description || '');
  const [priority, setPriority] = useState<TaskPriority>(initialData?.priority || 'medium');
  const [status, setStatus] = useState<TaskStatus>(initialData?.status || defaultStatus);
  const [isPrivate, setIsPrivate] = useState<boolean>(initialData?.is_private ?? false);
  const [selectedAssignees, setSelectedAssignees] = useState<string[]>(initialData?.assignees || []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim()) return;

    if (initialData) {
      await onSubmit({
        title: title.trim(),
        description: description.trim() || undefined,
        priority,
        status,
        assignees: selectedAssignees,
        is_private: isPrivate,
      });
    } else {
      await onSubmit({
        title: title.trim(),
        description: description.trim() || undefined,
        priority,
        status,
        workspace: workspaceId,
        assignees: selectedAssignees,
        order: Date.now() % 100000,
        is_private: isPrivate,
      });
    }
  };

  const toggleAssignee = (userId: string) => {
    setSelectedAssignees((prev) =>
      prev.includes(userId) ? prev.filter((id) => id !== userId) : [...prev, userId],
    );
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      {/* Title */}
      <div className="space-y-1">
        <label className="text-xs font-semibold text-foreground">
          任务标题 <span className="text-red-500">*</span>
        </label>
        <Input
          placeholder="输入敏捷任务标题..."
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          required
          autoFocus
        />
      </div>

      {/* Description */}
      <div className="space-y-1">
        <label className="text-xs font-semibold text-foreground">任务描述</label>
        <Textarea
          placeholder="补充任务背景、验收标准或实施说明..."
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          rows={3}
        />
      </div>

      {/* Status & Priority */}
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-1">
          <label className="text-xs font-semibold text-foreground">状态泳道</label>
          <Select value={status} onValueChange={(v) => setStatus(v as TaskStatus)}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {COLUMNS.map((col) => (
                <SelectItem key={col.id} value={col.id}>
                  <div className="flex items-center">
                    <span className={`mr-2 h-2 w-2 rounded-full ${col.dotColor}`} />
                    {col.title}
                  </div>
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1">
          <label className="text-xs font-semibold text-foreground">优先级</label>
          <Select value={priority} onValueChange={(v) => setPriority(v as TaskPriority)}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PRIORITIES.map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {/* Assignees Selection */}
      <div className="space-y-1.5">
        <label className="text-xs font-semibold text-foreground flex items-center">
          <Users className="mr-1 h-3.5 w-3.5 text-muted-foreground" />
          指派成员 ({selectedAssignees.length})
        </label>
        <div className="flex flex-wrap gap-1.5 rounded-md border p-2 bg-muted/20">
          {workspaceMembers.length === 0 ? (
            <span className="text-xs text-muted-foreground">暂无可指派的成员</span>
          ) : (
            workspaceMembers.map((member) => {
              const isSelected = selectedAssignees.includes(member.id);
              const name = member.displayName || member.email;
              return (
                <button
                  type="button"
                  key={member.id}
                  onClick={() => toggleAssignee(member.id)}
                  className={`inline-flex items-center space-x-1.5 rounded-full px-2.5 py-1 text-xs font-medium border transition-colors cursor-pointer ${
                    isSelected
                      ? 'bg-primary text-primary-foreground border-primary'
                      : 'bg-background text-muted-foreground border-border hover:bg-muted'
                  }`}
                >
                  <Avatar className="h-4 w-4">
                    <AvatarFallback className={getRandomColor(name)}>
                      {getInitials(name)}
                    </AvatarFallback>
                  </Avatar>
                  <span>{name}</span>
                </button>
              );
            })
          )}
        </div>
      </div>

      {/* Privacy Toggle */}
      <div className="flex items-center justify-between rounded-lg border p-3 bg-muted/10">
        <div className="space-y-0.5">
          <div className="flex items-center space-x-1.5">
            {isPrivate ? (
              <Lock className="h-4 w-4 text-amber-500" />
            ) : (
              <Unlock className="h-4 w-4 text-muted-foreground" />
            )}
            <span className="text-xs font-semibold text-foreground">
              {isPrivate ? '私有卡片 (Private)' : '公开卡片 (Public)'}
            </span>
          </div>
          <p className="text-[11px] text-muted-foreground">
            {isPrivate
              ? '仅工作区负责人和指派成员具备读写与查看权限'
              : '工作区内所有成员均可协同读写与推进'}
          </p>
        </div>
        <Button
          type="button"
          size="sm"
          variant={isPrivate ? 'secondary' : 'outline'}
          className="h-7 text-xs"
          onClick={() => setIsPrivate(!isPrivate)}
        >
          {isPrivate ? '设为公开' : '设为私有'}
        </Button>
      </div>

      {/* Form Buttons */}
      <div className="flex justify-end space-x-2 pt-2">
        <Button type="button" variant="outline" onClick={onCancel} disabled={isSubmitting}>
          取消
        </Button>
        <Button type="submit" disabled={!title.trim() || isSubmitting}>
          {isSubmitting ? '保存中...' : initialData ? '保存修改' : '创建任务'}
        </Button>
      </div>
    </form>
  );
}
