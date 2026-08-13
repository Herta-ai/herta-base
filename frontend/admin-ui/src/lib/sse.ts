import { useEffect, useRef, useState, useCallback } from 'react'
import { getAccessToken } from '../store/auth'

export type RealtimeAction = 'create' | 'update' | 'delete' | 'connected' | 'ping'

export interface RealtimeEvent<T = Record<string, unknown>> {
  action: RealtimeAction
  collection: string
  record?: T
  clientId?: string
  timestamp: string
}

export type RealtimeHandler<T = Record<string, unknown>> = (
  event: RealtimeEvent<T>,
) => void

export class RealtimeClient {
  private eventSource: EventSource | null = null
  private collection: string
  private listeners: Set<RealtimeHandler> = new Set()
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private reconnectAttempts = 0
  private maxReconnectDelay = 10000
  public isConnected = false
  public onStatusChange?: (status: 'connected' | 'connecting' | 'offline') => void

  constructor(collection: string) {
    this.collection = collection
  }

  public connect() {
    if (this.eventSource) {
      this.disconnect()
    }

    this.onStatusChange?.('connecting')
    const token = getAccessToken() || ''
    const url = `/api/realtime/${encodeURIComponent(this.collection)}${
      token ? `?token=${encodeURIComponent(token)}` : ''
    }`

    try {
      this.eventSource = new EventSource(url)

      this.eventSource.onopen = () => {
        this.isConnected = true
        this.reconnectAttempts = 0
        this.onStatusChange?.('connected')
      }

      this.eventSource.addEventListener('connected', (e: MessageEvent) => {
        try {
          const data = JSON.parse(e.data || '{}')
          this.emit({
            action: 'connected',
            collection: this.collection,
            clientId: data.clientId,
            timestamp: new Date().toISOString(),
          })
        } catch {
          // ignore
        }
      })

      const actions: RealtimeAction[] = ['create', 'update', 'delete']
      actions.forEach((action) => {
        this.eventSource?.addEventListener(action, (e: MessageEvent) => {
          try {
            const data = JSON.parse(e.data || '{}')
            this.emit({
              action,
              collection: this.collection,
              record: data.record || data,
              timestamp: new Date().toISOString(),
            })
          } catch (err) {
            console.error('[SSE] parse message error:', err)
          }
        })
      })

      this.eventSource.onerror = () => {
        this.isConnected = false
        this.onStatusChange?.('offline')
        this.scheduleReconnect()
      }
    } catch (err) {
      console.error('[SSE] init error:', err)
      this.isConnected = false
      this.onStatusChange?.('offline')
      this.scheduleReconnect()
    }
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) return
    this.reconnectAttempts++
    const delay = Math.min(1000 * Math.pow(1.5, this.reconnectAttempts), this.maxReconnectDelay)
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      this.connect()
    }, delay)
  }

  public subscribe(handler: RealtimeHandler) {
    this.listeners.add(handler)
    return () => {
      this.listeners.delete(handler)
    }
  }

  private emit(event: RealtimeEvent) {
    this.listeners.forEach((handler) => {
      try {
        handler(event)
      } catch (err) {
        console.error('[SSE] handler error:', err)
      }
    })
  }

  public disconnect() {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    if (this.eventSource) {
      this.eventSource.close()
      this.eventSource = null
    }
    this.isConnected = false
    this.onStatusChange?.('offline')
  }
}

/**
 * React Hook for subscribing to Realtime SSE events for a collection
 */
export function useRealtimeCollection<T = Record<string, unknown>>(
  collectionName: string | null | undefined,
  onEvent?: RealtimeHandler<T>,
) {
  const [status, setStatus] = useState<'connected' | 'connecting' | 'offline'>('connecting')
  const [lastEvent, setLastEvent] = useState<RealtimeEvent<T> | null>(null)
  const clientRef = useRef<RealtimeClient | null>(null)
  const onEventRef = useRef(onEvent)
  onEventRef.current = onEvent

  const handleEvent = useCallback((event: RealtimeEvent<Record<string, unknown>>) => {
    const typedEvent = event as unknown as RealtimeEvent<T>
    setLastEvent(typedEvent)
    onEventRef.current?.(typedEvent)
  }, [])

  useEffect(() => {
    if (!collectionName) return

    const client = new RealtimeClient(collectionName)
    clientRef.current = client
    client.onStatusChange = (newStatus) => setStatus(newStatus)

    const unsubscribe = client.subscribe(handleEvent)
    client.connect()

    return () => {
      unsubscribe()
      client.disconnect()
      clientRef.current = null
    }
  }, [collectionName, handleEvent])

  return {
    status,
    lastEvent,
    reconnect: () => clientRef.current?.connect(),
  }
}
