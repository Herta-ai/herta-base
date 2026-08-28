import type { AuthSession } from '../src/index'

import { describe, expect, it, vi } from 'vitest'
import { HertaBaseClient, MemoryAuthStore } from '../src/index'

const user = {
  id: 'kb_users:one',
  collection: 'kb_users',
  email: 'user@example.com',
  role: 'user',
  verified: true,
  admin: false,
}

function authResponse(accessToken: string, refreshToken = 'refresh-new'): Response {
  return envelope({
    accessToken,
    refreshToken,
    tokenType: 'Bearer',
    expiresIn: 900,
    user,
  })
}

function envelope(data: unknown, status = 200): Response {
  return new Response(JSON.stringify({ data, meta: null, error: null }), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

describe('authentication', () => {
  it('uses dynamic auth collection routes and persists the session', async () => {
    const store = new MemoryAuthStore()
    let receivedUrl = ''
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      receivedUrl = String(input)
      return authResponse('access-one')
    })
    const client = new HertaBaseClient({
      baseUrl: 'https://example.test',
      fetch: fetcher,
      authStore: store,
    })

    const session = await client.auth.forCollection('kb_users').register({
      email: 'user@example.com',
      // Test fixture only; no credential is used outside the mocked request.
      // eslint-disable-next-line sonarjs/no-hardcoded-passwords
      password: 'password123',
      profile: { displayName: 'User' },
    })

    expect(new URL(receivedUrl).pathname).toBe('/api/auth/kb_users/register')
    expect(session.scope).toEqual({ kind: 'collection', collection: 'kb_users' })
    expect((await store.get())?.accessToken).toBe('access-one')
  })

  it('single-flights proactive refresh for concurrent requests', async () => {
    const expired: AuthSession = {
      accessToken: 'access-old',
      refreshToken: 'refresh-old',
      tokenType: 'Bearer',
      expiresIn: 0,
      expiresAt: Date.now() - 1,
      scope: { kind: 'collection', collection: 'kb_users' },
      user,
    }
    const store = new MemoryAuthStore(expired)
    let refreshes = 0
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = new URL(String(input)).pathname
      if (path.endsWith('/refresh')) {
        refreshes += 1
        await Promise.resolve()
        return authResponse('access-new')
      }
      expect(new Headers(init?.headers).get('authorization')).toBe('Bearer access-new')
      return envelope([])
    })
    const client = new HertaBaseClient({
      baseUrl: 'https://example.test',
      fetch: fetcher,
      authStore: store,
    })

    await Promise.all([client.collection('tasks').list(), client.collection('comments').list()])

    expect(refreshes).toBe(1)
    expect((await store.get())?.accessToken).toBe('access-new')
  })

  it('refreshes and retries once after HB_TOKEN_EXPIRED', async () => {
    const active: AuthSession = {
      accessToken: 'access-old',
      refreshToken: 'refresh-old',
      tokenType: 'Bearer',
      expiresIn: 900,
      expiresAt: Date.now() + 900_000,
      scope: { kind: 'default' },
      user,
    }
    let records = 0
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      const path = new URL(String(input)).pathname
      if (path.endsWith('/refresh'))
        return authResponse('access-new')
      records += 1
      if (records === 1) {
        return new Response(
          JSON.stringify({
            data: null,
            meta: null,
            error: { code: 401, message: 'expired', error: 'HB_TOKEN_EXPIRED' },
          }),
          { status: 401 },
        )
      }
      return envelope({ id: 'posts:one' })
    })
    const client = new HertaBaseClient({
      baseUrl: 'https://example.test',
      fetch: fetcher,
      authStore: new MemoryAuthStore(active),
    })

    await expect(client.collection('posts').get('one')).resolves.toMatchObject({ id: 'posts:one' })
    expect(records).toBe(2)
  })

  it('clears the session when refresh rotation fails', async () => {
    const expired: AuthSession = {
      accessToken: 'access-old',
      refreshToken: 'refresh-old',
      tokenType: 'Bearer',
      expiresIn: 0,
      expiresAt: 0,
      scope: { kind: 'default' },
      user,
    }
    const store = new MemoryAuthStore(expired)
    const client = new HertaBaseClient({
      baseUrl: 'https://example.test',
      fetch: async () =>
        new Response(
          JSON.stringify({
            data: null,
            meta: null,
            error: { code: 401, message: 'invalid', error: 'HB_UNAUTHORIZED' },
          }),
          { status: 401 },
        ),
      authStore: store,
    })

    await expect(client.collection('posts').list()).rejects.toBeTruthy()
    expect(await store.get()).toBeNull()
  })
})
