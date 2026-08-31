import React, { useState } from 'react';
import { useSelector } from '@tanstack/react-store';
import { toast } from 'sonner';
import { authStore } from '../../store/auth';
import { useCreateWorkspace, useUsers } from '../../hooks/use-kanban-data';
import { setActiveWorkspaceId } from '../../store/workspace';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '../ui/dialog';
import { Button } from '../ui/button';
import { Input } from '../ui/input';
import { Avatar, AvatarFallback } from '../ui/avatar';
import { getInitials, getRandomColor } from '../../lib/utils';

export interface CreateWorkspaceDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateWorkspaceDialog({ open, onOpenChange }: CreateWorkspaceDialogProps) {
  const currentUser = useSelector(authStore, (s) => s.user);
  const { data: users = [] } = useUsers();
  const createWorkspace = useCreateWorkspace();

  const [name, setName] = useState('');
  const [selectedMembers, setSelectedMembers] = useState<string[]>([]);

  const toggleMember = (userId: string) => {
    setSelectedMembers((prev) =>
      prev.includes(userId) ? prev.filter((id) => id !== userId) : [...prev, userId],
    );
  };

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !currentUser) return;

    try {
      const allMembers = Array.from(new Set([currentUser.id, ...selectedMembers]));
      const created = await createWorkspace.mutateAsync({
        name: name.trim(),
        owner: currentUser.id,
        members: allMembers,
      });

      setActiveWorkspaceId(created.id);
      toast.success(`工作区「${created.name}」已创建`);
      setName('');
      setSelectedMembers([]);
      onOpenChange(false);
    } catch (err) {
      console.error(err);
      toast.error(`创建失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>新建敏捷工作区</DialogTitle>
          <DialogDescription>
            创建独立的项目看板，并邀请团队成员加入协同推进。
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleCreate} className="space-y-4 py-2">
          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-foreground">
              工作区名称 <span className="text-red-500">*</span>
            </label>
            <Input
              placeholder="例如：🚀 Q3 增长看板"
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
              autoFocus
            />
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-foreground">
              选择初始成员 ({selectedMembers.length + 1})
            </label>
            <div className="max-h-48 overflow-y-auto space-y-1 rounded-md border p-2 bg-muted/20">
              {users.map((u) => {
                const isCurrent = u.id === currentUser?.id;
                const isSelected = isCurrent || selectedMembers.includes(u.id);
                const displayName = u.displayName || u.email;

                return (
                  <div
                    key={u.id}
                    onClick={() => !isCurrent && toggleMember(u.id)}
                    className={`flex items-center justify-between rounded-lg p-2 text-xs transition-colors cursor-pointer ${
                      isSelected ? 'bg-primary/10 border border-primary/20' : 'hover:bg-muted'
                    }`}
                  >
                    <div className="flex items-center space-x-2">
                      <Avatar className="h-6 w-6">
                        <AvatarFallback className={getRandomColor(displayName)}>
                          {getInitials(displayName)}
                        </AvatarFallback>
                      </Avatar>
                      <div>
                        <p className="font-semibold text-foreground">{displayName}</p>
                        <p className="text-[10px] text-muted-foreground">{u.email}</p>
                      </div>
                    </div>

                    {isCurrent && (
                      <span className="text-[10px] text-primary font-medium bg-primary/10 px-1.5 py-0.5 rounded">
                        负责人 (创建者)
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          <DialogFooter className="pt-2">
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              取消
            </Button>
            <Button type="submit" disabled={!name.trim() || createWorkspace.isPending}>
              {createWorkspace.isPending ? '创建中...' : '立即创建'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
