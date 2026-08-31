import React, { useState } from 'react';
import { Check, Plus, X } from 'lucide-react';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import type { TaskStatus } from '../../types/kanban';

export interface AddTaskCardProps {
  status: TaskStatus;
  onAdd: (title: string, status: TaskStatus) => Promise<void>;
}

export function AddTaskCard({ status, onAdd }: AddTaskCardProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [title, setTitle] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim() || isSubmitting) return;

    setIsSubmitting(true);
    try {
      await onAdd(title.trim(), status);
      setTitle('');
      setIsOpen(false);
    } catch (err) {
      console.error(err);
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen) {
    return (
      <button
        type="button"
        onClick={() => setIsOpen(true)}
        className="flex w-full items-center justify-center space-x-1.5 rounded-lg border border-dashed border-border/80 p-2.5 text-xs font-medium text-muted-foreground transition-all duration-150 hover:border-primary/50 hover:bg-primary/5 hover:text-primary cursor-pointer"
      >
        <Plus className="h-3.5 w-3.5" />
        <span>添加卡片</span>
      </button>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="rounded-xl border bg-card p-3 shadow-sm space-y-2">
      <Input
        placeholder="输入新任务标题..."
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        className="h-8 text-xs"
        autoFocus
      />
      <div className="flex items-center justify-end space-x-1.5">
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className="h-7 px-2 text-xs"
          onClick={() => {
            setIsOpen(false);
            setTitle('');
          }}
          disabled={isSubmitting}
        >
          <X className="h-3.5 w-3.5 mr-1" /> 取消
        </Button>
        <Button
          type="submit"
          size="sm"
          className="h-7 px-2.5 text-xs"
          disabled={!title.trim() || isSubmitting}
        >
          <Check className="h-3.5 w-3.5 mr-1" />
          {isSubmitting ? '添加中...' : '添加'}
        </Button>
      </div>
    </form>
  );
}
