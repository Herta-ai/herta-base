import React, { useState } from 'react';
import { Download, FileIcon, FileText, ImageIcon, Loader2, Paperclip, Trash2, Upload } from 'lucide-react';
import { toast } from 'sonner';
import { useQueryClient } from '@tanstack/react-query';
import { getFileDownloadUrl, hb } from '../../lib/hb';
import { kanbanKeys } from '../../hooks/use-kanban-data';
import { Button } from '../ui/button';
import { cn } from '../../lib/utils';
import type { KbTask } from '../../types/kanban';

export interface AttachmentManagerProps {
  task: KbTask;
  canEdit?: boolean;
}

const ALLOWED_EXTENSIONS = ['png', 'jpg', 'jpeg', 'pdf', 'zip'];
const MAX_ATTACHMENTS = 5;

export function AttachmentManager({ task, canEdit = true }: AttachmentManagerProps) {
  const queryClient = useQueryClient();
  const [isUploading, setIsUploading] = useState(false);
  const [downloadingFile, setDownloadingFile] = useState<string | null>(null);

  const attachments = task.attachments || [];

  const handleDownload = async (filename: string) => {
    try {
      setDownloadingFile(filename);
      const url = await getFileDownloadUrl('kb_tasks', task.id, 'attachments', filename);
      const link = document.createElement('a');
      link.href = url;
      link.target = '_blank';
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
    } catch (err) {
      console.error(err);
      toast.error(`下载失败: ${err instanceof Error ? err.message : '权限不足或 Token 签发失败'}`);
    } finally {
      setDownloadingFile(null);
    }
  };

  const handleDelete = async (filename: string) => {
    if (!canEdit) return;
    try {
      const remaining = attachments.filter((f) => f !== filename);
      await hb.collection('kb_tasks').update(task.id, { attachments: remaining });
      queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(task.workspace) });
      queryClient.invalidateQueries({ queryKey: kanbanKeys.task(task.id) });
      toast.success(`已删除附件 ${filename}`);
    } catch (err) {
      toast.error(`删除失败: ${err instanceof Error ? err.message : '权限不足'}`);
    }
  };

  const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0) return;

    if (attachments.length + files.length > MAX_ATTACHMENTS) {
      toast.error(`最多只允许上传 ${MAX_ATTACHMENTS} 个附件`);
      e.target.value = '';
      return;
    }

    const uploadFilesList: File[] = [];
    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      const ext = file.name.split('.').pop()?.toLowerCase();
      if (!ext || !ALLOWED_EXTENSIONS.includes(ext)) {
        toast.error(`不支持的文件类型: ${file.name} (仅支持 ${ALLOWED_EXTENSIONS.join(', ')})`);
        e.target.value = '';
        return;
      }
      uploadFilesList.push(file);
    }

    setIsUploading(true);
    try {
      const filesPayload: Record<string, { blob: Blob; filename: string }[]> = {
        attachments: uploadFilesList.map((f) => ({ blob: f, filename: f.name })),
      };

      await hb.collection('kb_tasks').updateWithFiles(
        task.id,
        { files: filesPayload as any },
        { appendFiles: ['attachments'] },
      );

      queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(task.workspace) });
      queryClient.invalidateQueries({ queryKey: kanbanKeys.task(task.id) });
      toast.success(`成功上传 ${uploadFilesList.length} 个附件`);
    } catch (err) {
      console.error(err);
      toast.error(`上传失败: ${err instanceof Error ? err.message : '网络或权限错误'}`);
    } finally {
      setIsUploading(false);
      e.target.value = '';
    }
  };

  const getFileIcon = (filename: string) => {
    const ext = filename.split('.').pop()?.toLowerCase();
    if (['png', 'jpg', 'jpeg'].includes(ext || '')) {
      return <ImageIcon className="h-4 w-4 text-blue-500" />;
    }
    if (ext === 'pdf') {
      return <FileText className="h-4 w-4 text-red-500" />;
    }
    return <FileIcon className="h-4 w-4 text-amber-500" />;
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold flex items-center text-foreground">
          <Paperclip className="mr-1.5 h-4 w-4 text-muted-foreground" />
          任务附件 ({attachments.length}/{MAX_ATTACHMENTS})
        </h4>
        {canEdit && attachments.length < MAX_ATTACHMENTS && (
          <label className="cursor-pointer">
            <input
              type="file"
              multiple
              accept=".png,.jpg,.jpeg,.pdf,.zip"
              className="hidden"
              onChange={handleFileUpload}
              disabled={isUploading}
            />
            <Button size="sm" variant="outline" className="h-7 text-xs" disabled={isUploading} asChild>
              <span>
                {isUploading ? <Loader2 className="mr-1 h-3 w-3 animate-spin" /> : <Upload className="mr-1 h-3 w-3" />}
                上传附件
              </span>
            </Button>
          </label>
        )}
      </div>

      {attachments.length === 0 ? (
        <div className="rounded-lg border border-dashed p-4 text-center text-xs text-muted-foreground">
          暂无附件，点击上方按钮上传设计稿或文档（支持 PNG, JPG, PDF, ZIP）
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {attachments.map((file) => (
            <div
              key={file}
              className="flex items-center justify-between rounded-lg border bg-muted/30 p-2 text-xs transition-colors hover:bg-muted/60"
            >
              <div className="flex items-center space-x-2 truncate pr-2">
                {getFileIcon(file)}
                <span className="truncate font-medium text-foreground" title={file}>
                  {file}
                </span>
              </div>
              <div className="flex items-center space-x-1 shrink-0">
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-7 w-7 text-muted-foreground hover:text-foreground"
                  onClick={() => handleDownload(file)}
                  disabled={downloadingFile === file}
                  title="下载/查看附件 (Token 鉴权)"
                >
                  {downloadingFile === file ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Download className="h-3.5 w-3.5" />
                  )}
                </Button>
                {canEdit && (
                  <Button
                    size="icon"
                    variant="ghost"
                    className="h-7 w-7 text-muted-foreground hover:text-destructive"
                    onClick={() => handleDelete(file)}
                    title="移除附件"
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                  </Button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
