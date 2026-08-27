export type SseEvent<T = unknown> = { event: string; data: T; id?: string };

export class SseConnectionError extends Error {
  constructor(public readonly status: number, public readonly code: string | null, body: string) {
    super(`SSE connection failed with HTTP ${status}${code ? ` ${code}` : ''}: ${body}`);
  }
}

export class SseTimeoutError extends Error {}

type Waiter = {
  event: string;
  predicate: (data: unknown) => boolean;
  resolve: (event: SseEvent) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};

export class SseConnection {
  private readonly controller = new AbortController();
  private readonly queue: SseEvent[] = [];
  private readonly waiters: Waiter[] = [];
  private reader: ReadableStreamDefaultReader<Uint8Array> | null = null;
  private pumpPromise: Promise<void> | null = null;

  static async connect(url: string, token?: string): Promise<SseConnection> {
    const connection = new SseConnection();
    const response = await fetch(url, {
      headers: token ? { Authorization: `Bearer ${token}` } : undefined,
      signal: connection.controller.signal,
    });
    if (!response.ok) {
      const body = await response.text();
      let code: string | null = null;
      try { code = JSON.parse(body)?.error?.error ?? null; } catch { /* plain response */ }
      throw new SseConnectionError(response.status, code, body);
    }
    if (!response.body) throw new Error('SSE response has no body');
    connection.reader = response.body.getReader();
    connection.pumpPromise = connection.pump();
    return connection;
  }

  async waitForEvent<T = unknown>(
    event: string,
    predicate: (data: T) => boolean = () => true,
    timeoutMs = 3_000,
  ): Promise<SseEvent<T>> {
    const index = this.queue.findIndex(
      (candidate) => candidate.event === event && predicate(candidate.data as T),
    );
    if (index >= 0) return this.queue.splice(index, 1)[0] as SseEvent<T>;

    return new Promise((resolve, reject) => {
      const waiter: Waiter = {
        event,
        predicate: predicate as (data: unknown) => boolean,
        resolve: resolve as (event: SseEvent) => void,
        reject,
        timer: setTimeout(() => {
          const waiterIndex = this.waiters.indexOf(waiter);
          if (waiterIndex >= 0) this.waiters.splice(waiterIndex, 1);
          reject(new SseTimeoutError(
            `timed out waiting for SSE event '${event}'; queued events: ${this.queue.map((item) => item.event).join(', ')}`,
          ));
        }, timeoutMs),
      };
      this.waiters.push(waiter);
    });
  }

  async expectNoEvent<T = unknown>(
    event: string,
    predicate: (data: T) => boolean = () => true,
    timeoutMs = 300,
  ): Promise<void> {
    try {
      const received = await this.waitForEvent(event, predicate, timeoutMs);
      throw new Error(`unexpected SSE event '${event}': ${JSON.stringify(received.data)}`);
    } catch (error) {
      if (error instanceof SseTimeoutError) return;
      throw error;
    }
  }

  async close(): Promise<void> {
    this.controller.abort();
    try { await this.reader?.cancel(); } catch { /* already closed */ }
    try { await this.pumpPromise; } catch (error) {
      if (!(error instanceof DOMException && error.name === 'AbortError')) throw error;
    }
    this.rejectWaiters(new Error('SSE connection closed'));
  }

  private async pump(): Promise<void> {
    const decoder = new TextDecoder();
    let buffer = '';
    try {
      while (this.reader) {
        const { done, value } = await this.reader.read();
        if (done) break;
        buffer += decoder.decode(value, { stream: true });
        while (true) {
          const boundary = buffer.match(/\r?\n\r?\n/);
          if (!boundary || boundary.index === undefined) break;
          const frame = buffer.slice(0, boundary.index);
          buffer = buffer.slice(boundary.index + boundary[0].length);
          this.dispatch(this.parseFrame(frame));
        }
      }
    } catch (error) {
      if (!this.controller.signal.aborted) this.rejectWaiters(error as Error);
    } finally {
      this.reader = null;
      if (!this.controller.signal.aborted) this.rejectWaiters(new Error('SSE stream ended'));
    }
  }

  private parseFrame(frame: string): SseEvent | null {
    let event = 'message';
    let id: string | undefined;
    const data: string[] = [];
    for (const line of frame.split(/\r?\n/)) {
      if (!line || line.startsWith(':')) continue;
      const separator = line.indexOf(':');
      const field = separator < 0 ? line : line.slice(0, separator);
      const value = separator < 0 ? '' : line.slice(separator + 1).replace(/^ /, '');
      if (field === 'event') event = value;
      if (field === 'id') id = value;
      if (field === 'data') data.push(value);
    }
    if (data.length === 0) return null;
    const raw = data.join('\n');
    let parsed: unknown = raw;
    try { parsed = JSON.parse(raw); } catch { /* non-JSON SSE data */ }
    return { event, data: parsed, id };
  }

  private dispatch(candidate: SseEvent | null): void {
    if (!candidate) return;
    const waiterIndex = this.waiters.findIndex(
      (waiter) => waiter.event === candidate.event && waiter.predicate(candidate.data),
    );
    if (waiterIndex < 0) {
      this.queue.push(candidate);
      return;
    }
    const [waiter] = this.waiters.splice(waiterIndex, 1);
    clearTimeout(waiter.timer);
    waiter.resolve(candidate);
  }

  private rejectWaiters(error: Error): void {
    for (const waiter of this.waiters.splice(0)) {
      clearTimeout(waiter.timer);
      waiter.reject(error);
    }
  }
}
