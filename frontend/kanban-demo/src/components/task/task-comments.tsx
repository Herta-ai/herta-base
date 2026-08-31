import React, { useState } from 'react';
import { useSelector } from '@tanstack/react-store';
import { Loader2, MessageSquare, Send, Trash2 } from 'lucide-react';
import { toast } from 'sonner';
import { authStore } from '../../store/auth';
import { useCreateComment, useDeleteComment, useTaskComments } from '../../hooks/use-kanban-data';
import { Button } from '../ui/button';
import { Textarea } from '../ui/textarea';
import { Avatar, AvatarFallback } from '../ui/avatar';
import { formatRelativeTime, getInitials, getRandomColor } from '../../lib/utils';
import type { KbTask } from '../../types/kanban';

export interface TaskCommentsProps {
  task: KbTask;
}

export function TaskComments({ task }: TaskCommentsProps) {
  const currentUser = useSelector(authStore, (s) => s.user);
  const { data: comments = [], isLoading } = useTaskComments(task.id);
  const createComment = useCreateComment();
  const deleteComment = useDeleteComment();

  const [content, setContent] = useState('');

  const handlePost = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!content.trim() || !currentUser) return;

    try {
      await createComment.mutateAsync({
        task: task.id,
        author: currentUser.id,
        content: content.trim(),
      });
      setContent('');
      toast.success('评论已发表');
    } catch (err) {
      console.error(err);
      toast.error(`发表失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  const handleDelete = async (commentId: string) => {
    try {
      await deleteComment.mutateAsync({ id: commentId, taskId: task.id });
      toast.success('评论已删除');
    } catch (err) {
      toast.error(`删除失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center space-x-2">
        <MessageSquare className="h-4 w-4 text-muted-foreground" />
        <h4 className="text-sm font-semibold text-foreground">
          任务讨论 ({comments.length})
        </h4>
      </div>

      {/* New Comment Input */}
      {currentUser ? (
        <form onSubmit={handlePost} className="space-y-2">
          <div className="flex space-x-2">
            <Avatar className="h-7 w-7 shrink-0 mt-1">
              <AvatarFallback className={getRandomColor(currentUser.displayName || currentUser.email)}>
                {getInitials(currentUser.displayName || currentUser.email)}
              </AvatarFallback>
            </Avatar>
            <div className="flex-1 space-y-2">
              <Textarea
                placeholder="撰写评论或更新进展..."
                value={content}
                onChange={(e) => setContent(e.target.value)}
                rows={2}
                className="resize-none text-xs min-h-[60px]"
              />
              <div className="flex justify-end">
                <Button
                  type="submit"
                  size="sm"
                  disabled={!content.trim() || createComment.isPending}
                  className="h-7 text-xs"
                >
                  {createComment.isPending ? (
                    <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                  ) : (
                    <Send className="mr-1 h-3 w-3" />
                  )}
                  发送
                </Button>
              </div>
            </div>
          </div>
        </form>
      ) : (
        <div className="text-xs text-muted-foreground bg-muted/40 p-2 rounded">
          请先登录后发表评论
        </div>
      )}

      {/* Comments List */}
      <div className="space-y-3 pt-2">
        {isLoading ? (
          <div className="flex justify-center py-4 text-xs text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin mr-1" /> 加载评论中...
          </div>
        ) : comments.length === 0 ? (
          <div className="text-center py-4 text-xs text-muted-foreground">
            暂无评论，留下第一条讨论吧
          </div>
        ) : (
          comments.map((comment) => {
            const author = comment.expand?.author;
            const authorName = author?.displayName || author?.email || '匿名成员';
            const isMyComment = currentUser?.id === comment.author;

            return (
              <div
                key={comment.id}
                className="group relative flex space-x-3 rounded-lg border bg-muted/20 p-3 text-xs"
              >
                <Avatar className="h-6 w-6 shrink-0 mt-0.5">
                  <AvatarFallback className={getRandomColor(authorName)}>
                    {getInitials(authorName)}
                  </AvatarFallback>
                </Avatar>
                <div className="flex-1 space-y-1">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center space-x-2">
                      <span className="font-semibold text-foreground">{authorName}</span>
                      <span className="text-[10px] text-muted-foreground">
                        {formatRelativeTime(comment.created_at)}
                      </span>
                    </div>
                    {isMyComment && (
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-5 w-5 opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive transition-opacity"
                        onClick={() => handleDelete(comment.id)}
                        disabled={deleteComment.isPending}
                        title="删除我的评论"
                      >
                        <Trash2 className="h-3 w-3" />
                      </Button>
                    )}
                  </div>
                  <p className="text-foreground/90 whitespace-pre-wrap leading-relaxed">
                    {comment.content}
                  </p>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
