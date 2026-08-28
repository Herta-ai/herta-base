import type { Transport } from './transport'
import type {
  ApiEnvelope,
  RealtimeEvent,
  RealtimeStatus,
  RealtimeSubscription,
  ReconnectOptions,
  SubscribeOptions,
} from './types'
import { HertaError } from './errors'
import { encodePath } from './utils'

const DEFAULT_RECONNECT: Required<ReconnectOptions> = {
  enabled: true,
  initialDelayMs: 500,
  maxDelayMs: 30_000,
  multiplier: 2,
  jitter: 0.2,
  maxAttempts: Number.POSITIVE_INFINITY,
}

interface ParsedSseEvent {
  type: string
  id?: string
  data: unknown
}

export class SseParser {
  private readonly decoder = new TextDecoder()
  private buffer = ''

  push(chunk: Uint8Array): ParsedSseEvent[] {
    this.buffer += this.decoder.decode(chunk, { stream: true })
    return this.drain(false)
  }

  finish(): ParsedSseEvent[] {
    this.buffer += this.decoder.decode()
    return this.drain(true)
  }

  private drain(flush: boolean): ParsedSseEvent[] {
    const events: ParsedSseEvent[] = []
    while (true) {
      const boundary = this.buffer.match(/\r?\n\r?\n/)
      if (!boundary || boundary.index === undefined)
        break
      const frame = this.buffer.slice(0, boundary.index)
      this.buffer = this.buffer.slice(boundary.index + boundary[0].length)
      const event = parseFrame(frame)
      if (event)
        events.push(event)
    }
    if (flush && this.buffer.trim()) {
      const event = parseFrame(this.buffer)
      this.buffer = ''
      if (event)
        events.push(event)
    }
    return events
  }
}

export async function subscribeToCollection<TRecord>(
  transport: Transport,
  collection: string,
  options: SubscribeOptions<TRecord> = {},
): Promise<RealtimeSubscription<TRecord>> {
  const subscription = new RealtimeSubscriptionImpl(transport, collection, options)
  await subscription.start()
  return subscription
}

class RealtimeSubscriptionImpl<TRecord> implements RealtimeSubscription<TRecord> {
  private currentStatus: RealtimeStatus = 'connecting'
  private readonly eventListeners = new Set<(event: RealtimeEvent<TRecord>) => void>()
  private readonly statusListeners = new Set<(status: RealtimeStatus) => void>()
  private readonly errorListeners = new Set<(error: Error) => void>()
  private readonly reconnect: Required<ReconnectOptions>
  private controller: AbortController | null = null
  private reader: ReadableStreamDefaultReader<Uint8Array> | null = null
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private attempts = 0
  private closed = false

  constructor(
    private readonly transport: Transport,
    private readonly collection: string,
    private readonly options: SubscribeOptions<TRecord>,
  ) {
    this.reconnect = normalizeReconnect(options.reconnect)
    if (options.onEvent)
      this.eventListeners.add(options.onEvent)
    if (options.onStatus)
      this.statusListeners.add(options.onStatus)
    if (options.onError)
      this.errorListeners.add(options.onError)
    if (options.signal) {
      if (options.signal.aborted)
        this.closed = true
      else options.signal.addEventListener('abort', () => this.close(), { once: true })
    }
  }

  get status(): RealtimeStatus {
    return this.currentStatus
  }

  async start(): Promise<void> {
    if (this.closed) {
      throw new HertaError('Realtime subscription was aborted before connecting', {
        kind: 'abort',
      })
    }
    await this.connect(true)
  }

  onEvent(listener: (event: RealtimeEvent<TRecord>) => void): () => void {
    this.eventListeners.add(listener)
    return () => this.eventListeners.delete(listener)
  }

  onStatus(listener: (status: RealtimeStatus) => void): () => void {
    this.statusListeners.add(listener)
    return () => this.statusListeners.delete(listener)
  }

  onError(listener: (error: Error) => void): () => void {
    this.errorListeners.add(listener)
    return () => this.errorListeners.delete(listener)
  }

  close(): void {
    if (this.closed)
      return
    this.closed = true
    if (this.reconnectTimer)
      clearTimeout(this.reconnectTimer)
    this.reconnectTimer = null
    this.controller?.abort()
    void this.reader?.cancel().catch(() => undefined)
    this.reader = null
    this.setStatus('closed')
  }

  private async connect(initial: boolean): Promise<void> {
    if (this.closed)
      return
    this.controller?.abort()
    const controller = new AbortController()
    this.controller = controller
    this.setStatus(initial ? 'connecting' : 'reconnecting')
    const query = this.options.filter ? { filter: this.options.filter } : undefined
    const response = await this.transport.response(`/api/realtime/${encodePath(this.collection)}`, {
      query,
      signal: controller.signal,
      timeoutMs: 0,
    })
    if (!response.body) {
      throw new HertaError('SSE response has no readable body', { kind: 'protocol' })
    }
    this.reader = response.body.getReader()
    void this.pump(this.reader, controller).catch((error: unknown) => {
      if (!this.closed && !controller.signal.aborted)
        this.handleDisconnect(asError(error))
    })
  }

  private async pump(
    reader: ReadableStreamDefaultReader<Uint8Array>,
    controller: AbortController,
  ): Promise<void> {
    const parser = new SseParser()
    while (!this.closed && !controller.signal.aborted) {
      const { done, value } = await reader.read()
      if (done)
        break
      for (const event of parser.push(value)) await this.dispatch(event)
    }
    for (const event of parser.finish()) await this.dispatch(event)
    if (!this.closed && !controller.signal.aborted) {
      this.handleDisconnect(new HertaError('Realtime stream ended', { kind: 'network' }))
    }
  }

  private async dispatch(parsed: ParsedSseEvent): Promise<void> {
    if (!isKnownEvent(parsed.type))
      return
    const event = {
      type: parsed.type,
      data: parsed.data,
      ...(parsed.id ? { id: parsed.id } : {}),
    } as RealtimeEvent<TRecord>
    this.emitEvent(event)
    if (parsed.type === 'connected') {
      this.attempts = 0
      this.setStatus('connected')
      return
    }
    if (parsed.type === 'error') {
      const envelope = parsed.data as ApiEnvelope<never>
      if (envelope?.error?.error === 'HB_TOKEN_EXPIRED') {
        try {
          await this.transport.authState.refresh()
          if (this.reconnect.enabled)
            this.scheduleReconnect(0)
          else this.close()
        }
        catch (error) {
          this.fail(asError(error))
        }
      }
      else {
        this.fail(
          new HertaError(envelope?.error?.message ?? 'Realtime server error', {
            kind: 'api',
            status: envelope?.error?.code,
            code: envelope?.error?.error,
            details: envelope?.error?.details,
          }),
        )
      }
    }
  }

  private handleDisconnect(error: Error): void {
    if (this.closed)
      return
    this.emitError(error)
    if (
      !this.retryable(error)
      || !this.reconnect.enabled
      || this.attempts >= this.reconnect.maxAttempts
    ) {
      this.close()
      return
    }
    const base = Math.min(
      this.reconnect.initialDelayMs * this.reconnect.multiplier ** this.attempts,
      this.reconnect.maxDelayMs,
    )
    const spread = base * this.reconnect.jitter
    // Reconnect jitter is not used for a security-sensitive decision.
    // eslint-disable-next-line sonarjs/pseudo-random
    const delay = Math.max(0, base - spread + Math.random() * spread * 2)
    this.attempts += 1
    this.scheduleReconnect(delay)
  }

  private scheduleReconnect(delay: number): void {
    if (this.closed || this.reconnectTimer)
      return
    this.setStatus('reconnecting')
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      void this.connect(false).catch((error: unknown) => this.handleDisconnect(asError(error)))
    }, delay)
  }

  private retryable(error: Error): boolean {
    if (!(error instanceof HertaError))
      return true
    if (error.kind === 'network' || error.kind === 'timeout')
      return true
    return error.status === 429 || (error.status !== undefined && error.status >= 500)
  }

  private fail(error: Error): void {
    this.emitError(error)
    this.close()
  }

  private emitEvent(event: RealtimeEvent<TRecord>): void {
    for (const listener of this.eventListeners) safelyCall(() => listener(event))
  }

  private emitError(error: Error): void {
    for (const listener of this.errorListeners) safelyCall(() => listener(error))
  }

  private setStatus(status: RealtimeStatus): void {
    if (this.currentStatus === status)
      return
    this.currentStatus = status
    for (const listener of this.statusListeners) safelyCall(() => listener(status))
  }
}

function parseFrame(frame: string): ParsedSseEvent | null {
  let type = 'message'
  let id: string | undefined
  const data: string[] = []
  for (const line of frame.split(/\r?\n/)) {
    if (!line || line.startsWith(':'))
      continue
    const separator = line.indexOf(':')
    const field = separator < 0 ? line : line.slice(0, separator)
    const value = separator < 0 ? '' : line.slice(separator + 1).replace(/^ /, '')
    if (field === 'event')
      type = value
    else if (field === 'id')
      id = value
    else if (field === 'data')
      data.push(value)
  }
  if (!data.length)
    return null
  const raw = data.join('\n')
  let parsed: unknown = raw
  try {
    parsed = JSON.parse(raw)
  }
  catch {
    // Non-JSON data is preserved for forward compatibility.
  }
  return { type, data: parsed, ...(id ? { id } : {}) }
}

function isKnownEvent(type: string): type is RealtimeEvent<unknown>['type'] {
  return ['connected', 'create', 'update', 'delete', 'ping', 'error'].includes(type)
}

function normalizeReconnect(
  input: boolean | ReconnectOptions | undefined,
): Required<ReconnectOptions> {
  if (input === false)
    return { ...DEFAULT_RECONNECT, enabled: false }
  if (input === true || input === undefined)
    return { ...DEFAULT_RECONNECT }
  return { ...DEFAULT_RECONNECT, ...input }
}

function safelyCall(callback: () => void): void {
  try {
    callback()
  }
  catch {
    // Listener failures must not break the stream pump.
  }
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error))
}
