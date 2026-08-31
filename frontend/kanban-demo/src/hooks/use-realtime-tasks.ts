import { useEffect, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { hb } from '../lib/hb';
import { kanbanKeys } from './use-kanban-data';
import type { KbTask } from '../types/kanban';
import type { RealtimeStatus } from '@hb/sdk';

export function useRealtimeTasks(workspaceId?: string | null) {
  const [status, setStatus] = useState<RealtimeStatus>('closed');
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!workspaceId) {
      setStatus('closed');
      return;
    }

    let isSubscribed = true;
    let cleanup: (() => void) | undefined;

    async function subscribe() {
      try {
        setStatus('connecting');
        const tasksCollection = hb.collection<KbTask>('kb_tasks');
        const sub = await tasksCollection.subscribe({
          filter: `workspace = '${workspaceId}'`,
          onStatus(newStatus) {
            if (isSubscribed) setStatus(newStatus);
            if (newStatus === 'connected') {
              queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(workspaceId!) });
            }
          },
          onEvent(event) {
            if (!isSubscribed) return;
            if (event.type === 'create') {
              queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(workspaceId!) });
              toast.info(`团队动态：新增任务「${event.data.record.title}」`, { duration: 3000 });
            } else if (event.type === 'update') {
              queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(workspaceId!) });
              queryClient.invalidateQueries({ queryKey: kanbanKeys.task(event.data.record.id) });
            } else if (event.type === 'delete') {
              queryClient.invalidateQueries({ queryKey: kanbanKeys.tasks(workspaceId!) });
              toast.warning('团队动态：有任务已被移除', { duration: 2500 });
            }
          },
          onError(err) {
            console.error('Realtime SSE error:', err);
            if (isSubscribed) setStatus('closed');
          },
        });

        cleanup = () => {
          sub.close();
        };
      } catch (error) {
        console.error('Failed to establish SSE connection:', error);
        if (isSubscribed) setStatus('closed');
      }
    }

    void subscribe();

    return () => {
      isSubscribed = false;
      cleanup?.();
    };
  }, [workspaceId, queryClient]);

  return { status };
}
