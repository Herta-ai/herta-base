import type { AuthSession } from '../src/index'

import { describe, expect, it, vi } from 'vitest'
import { HertaBaseClient, MemoryAuthStore } from '../src/index'
import { SseParser } from '../src/realtime'

describe('sSE parsing and subscriptions', () => {
  it('parses arbitrary chunks and multiline data', () => {
    const parser = new SseParser()
    const encoder = new TextEncoder()

    expect(parser.push(encoder.encode('event: update\nid: one\nda'))).toEqual([])
    expect(parser.push(encoder.encode('ta: {"record":\ndata: "value"}\n\n'))).toEqual([
      { type: 'update', id: 'one', data: { record: 'value' } },
    ])
  })

  it('delivers connected and change events from a fetch stream', async () => {
    const encoder = new TextEncoder()
    const stream = new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(
          encoder.encode(
            'event: connected\ndata: {"subscriptionId":"sub","collection":"tasks","timestamp":"now"}\n\n',
          ),
        )
        controller.enqueue(
          encoder.encode(
            'event: update\nid: event-one\ndata: {"id":"tasks:one","action":"update","record":{"id":"tasks:one"},"timestamp":"now"}\n\n',
          ),
        )
      },
    })
    const events: string[] = []
    const client = new HertaBaseClient({
      baseUrl: 'https://example.test',
      fetch: async () => new Response(stream, { status: 200 }),
    })

    const subscription = await client.collection('tasks').subscribe({
      reconnect: false,
      onEvent: event => events.push(event.type),
    })
    await vi.waitFor(() => expect(events).toEqual(['connected', 'update']))
    expect(subscription.status).toBe('connected')

    subscription.close()
    expect(subscription.status).toBe('closed')
  })

  it('reconnects after EOF and stops after close', async () => {
    const encoder = new TextEncoder()
    let calls = 0
    const client = new HertaBaseClient({
      baseUrl: 'https://example.test',
      fetch: async () => {
        calls += 1
        return new Response(
          new ReadableStream<Uint8Array>({
            start(controller) {
              controller.enqueue(
                encoder.encode(
                  `event: connected\ndata: {"subscriptionId":"${calls}","collection":"tasks","timestamp":"now"}\n\n`,
                ),
              )
              controller.close()
            },
          }),
        )
      },
    })
    const subscription = await client.collection('tasks').subscribe({
      reconnect: { initialDelayMs: 0, maxDelayMs: 0, jitter: 0, maxAttempts: 2 },
    })

    await vi.waitFor(() => expect(calls).toBeGreaterThanOrEqual(2))
    subscription.close()
    const callsAtClose = calls
    await new Promise(resolve => setTimeout(resolve, 10))
    expect(calls).toBe(callsAtClose)
  })

  it('rotates an expired token from a stream error before reconnecting', async () => {
    const user = {
      id: '_users:one',
      collection: '_users',
      email: 'user@example.com',
      role: 'user',
      verified: true,
      admin: false,
    }
    const session: AuthSession = {
      accessToken: 'access-old',
      refreshToken: 'refresh-old',
      tokenType: 'Bearer',
      expiresIn: 900,
      expiresAt: Date.now() + 900_000,
      scope: { kind: 'default' },
      user,
    }
    const store = new MemoryAuthStore(session)
    const encoder = new TextEncoder()
    let streams = 0
    const client = new HertaBaseClient({
      baseUrl: 'https://example.test',
      authStore: store,
      fetch: async (input) => {
        const path = new URL(String(input)).pathname
        if (path.endsWith('/refresh')) {
          return new Response(
            JSON.stringify({
              data: {
                accessToken: 'access-new',
                refreshToken: 'refresh-new',
                tokenType: 'Bearer',
                expiresIn: 900,
                user,
              },
              meta: null,
              error: null,
            }),
          )
        }
        streams += 1
        return new Response(
          new ReadableStream<Uint8Array>({
            start(controller) {
              controller.enqueue(
                encoder.encode(
                  `event: connected\ndata: {"subscriptionId":"${streams}","collection":"tasks","timestamp":"now"}\n\n`,
                ),
              )
              if (streams === 1) {
                controller.enqueue(
                  encoder.encode(
                    'event: error\ndata: {"data":null,"meta":null,"error":{"code":401,"message":"expired","error":"HB_TOKEN_EXPIRED"}}\n\n',
                  ),
                )
                controller.close()
              }
            },
          }),
        )
      },
    })
    const subscription = await client.collection('tasks').subscribe({
      reconnect: { initialDelayMs: 0, maxDelayMs: 0, jitter: 0 },
    })

    await vi.waitFor(() => expect(streams).toBe(2))
    expect((await store.get())?.accessToken).toBe('access-new')
    subscription.close()
  })

  it('rejects an initial forbidden connection without retrying', async () => {
    const fetcher = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            data: null,
            meta: null,
            error: { code: 403, message: 'forbidden', error: 'HB_FORBIDDEN' },
          }),
          { status: 403 },
        ),
    )
    const client = new HertaBaseClient({ baseUrl: 'https://example.test', fetch: fetcher })

    await expect(client.collection('tasks').subscribe()).rejects.toMatchObject({
      kind: 'api',
      code: 'HB_FORBIDDEN',
    })
    expect(fetcher).toHaveBeenCalledTimes(1)
  })
})
