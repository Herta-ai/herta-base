import type { RealtimeEvent as SdkRealtimeEvent, RealtimeStatus, RealtimeSubscription } from '@hb/sdk/admin';
import { useEffect, useRef, useState, useCallback } from 'react';

import { adminClient } from './api';

export type RealtimeAction = 'create' | 'update' | 'delete' | 'connected' | 'ping';

export interface RealtimeEvent<T = Record<string, unknown>> {
  action: RealtimeAction;
  collection: string;
  record?: T;
  clientId?: string;
  timestamp: string;
}

export type RealtimeHandler<T = Record<string, unknown>> = (event: RealtimeEvent<T>) => void;

/**
 * React Hook for subscribing to Realtime SSE events for a collection via official SDK
 */
export function useRealtimeCollection<T = Record<string, unknown>>(
  collectionName: string | null | undefined,
  onEvent?: RealtimeHandler<T>,
) {
  const [status, setStatus] = useState<'connected' | 'connecting' | 'offline'>('connecting');
  const [lastEvent, setLastEvent] = useState<RealtimeEvent<T> | null>(null);
  const subscriptionRef = useRef<RealtimeSubscription<any> | null>(null);
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  const handleEvent = useCallback((event: RealtimeEvent<T>) => {
    setLastEvent(event);
    onEventRef.current?.(event);
  }, []);

  useEffect(() => {
    if (!collectionName) return;

    let isMounted = true;
    const controller = new AbortController();

    async function startSubscription() {
      try {
        const sub = await adminClient.collection(collectionName!).subscribe({
          signal: controller.signal,
          onStatus: (st: RealtimeStatus) => {
            if (!isMounted) return;
            if (st === 'connected') setStatus('connected');
            else if (st === 'connecting' || st === 'reconnecting') setStatus('connecting');
            else setStatus('offline');
          },
          onEvent: (event: SdkRealtimeEvent<any>) => {
            if (!isMounted) return;
            if (event.type === 'connected') {
              handleEvent({
                action: 'connected',
                collection: event.data.collection || collectionName!,
                clientId: event.data.subscriptionId,
                timestamp: event.data.timestamp || new Date().toISOString(),
              });
            } else if (event.type === 'create' || event.type === 'update' || event.type === 'delete') {
              handleEvent({
                action: event.data.action || event.type,
                collection: collectionName!,
                record: event.data.record as T | undefined,
                timestamp: event.data.timestamp || new Date().toISOString(),
              });
            } else if (event.type === 'ping') {
              handleEvent({
                action: 'ping',
                collection: collectionName!,
                timestamp: event.data.timestamp || new Date().toISOString(),
              });
            }
          },
          onError: (err: Error) => {
            if (!isMounted) return;
            console.warn(`[Realtime] connection status for ${collectionName}:`, err.message);
            setStatus('offline');
          },
        });

        if (isMounted) {
          subscriptionRef.current = sub;
        } else {
          sub.close();
        }
      } catch (err: unknown) {
        if (!isMounted) return;
        console.warn(`[Realtime] unable to subscribe to ${collectionName}:`, err);
        setStatus('offline');
      }
    }

    startSubscription();

    return () => {
      isMounted = false;
      controller.abort();
      if (subscriptionRef.current) {
        subscriptionRef.current.close();
        subscriptionRef.current = null;
      }
    };
  }, [collectionName, handleEvent]);

  return {
    status,
    lastEvent,
    reconnect: () => {
      if (subscriptionRef.current) {
        subscriptionRef.current.close();
        subscriptionRef.current = null;
      }
    },
  };
}
