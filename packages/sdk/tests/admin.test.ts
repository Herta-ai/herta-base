import type { AuthSession } from '../src/admin'
import { describe, expect, it, vi } from 'vitest'

import { HertaBaseAdminClient, MemoryAuthStore } from '../src/admin'

function envelope(
  data: unknown,
  meta: Record<string, unknown> | null = null,
  status = 200,
): Response {
  return new Response(JSON.stringify({ data, meta, error: null }), { status })
}

describe('hertaBaseAdminClient', () => {
  it('manages collections and paged logs through admin routes', async () => {
    const calls: Array<{ url: string, init?: RequestInit }> = []
    const definition = {
      name: 'posts',
      type: 'base' as const,
      schema_mode: 'strict' as const,
      fields: [{ name: 'title', type: 'text' as const, required: true }],
    }
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      calls.push({ url: String(input), init })
      const path = new URL(String(input)).pathname
      if (path === '/api/admin/logs') {
        return envelope(
          [
            {
              id: '_logs:one',
              created_at: '2026-08-28T00:00:00Z',
              log_type: 'request',
              level: 'error',
              message: 'failed',
              target: 'http',
            },
          ],
          { total: 1, page: 1, perPage: 100 },
        )
      }
      return envelope(definition, null, 201)
    })
    const admin = new HertaBaseAdminClient({
      baseUrl: 'https://example.test',
      fetch: fetcher,
    })

    await expect(admin.collections.create(definition)).resolves.toEqual(definition)
    const logs = await admin.logs.list({ level: 'error', perPage: 100 })

    expect(new URL(calls[0]!.url).pathname).toBe('/_/collections')
    expect(calls[0]!.init?.method).toBe('POST')
    expect(logs.total).toBe(1)
    expect(logs.items[0]?.log_type).toBe('request')
    expect(new URL(calls[1]!.url).searchParams.get('level')).toBe('error')
  })

  it('builds multipart web deployments and rollback requests', async () => {
    const calls: Array<{ url: string, init?: RequestInit }> = []
    const project = {
      name: 'docs',
      alias: '/web/docs',
      spaFallback: true,
      cacheControl: 'public, max-age=60',
      notFound: null,
      deployedAt: '2026-08-28T00:00:00Z',
      deployed: true,
    }
    const admin = new HertaBaseAdminClient({
      baseUrl: 'https://example.test',
      fetch: async (input, init) => {
        calls.push({ url: String(input), init })
        return envelope(project)
      },
    })

    await admin.webProjects.deploy({
      archive: { blob: new Blob(['zip']), filename: 'docs.zip' },
      alias: '/web/docs',
      spaFallback: true,
    })
    await admin.webProjects.rollback('docs', '2026-08-28-12-00-00')

    expect(new URL(calls[0]!.url).pathname).toBe('/api/admin/auth/me')
    const form = calls[1]!.init?.body as FormData
    expect((form.get('archive') as File).name).toBe('docs.zip')
    expect(form.get('alias')).toBe('/web/docs')
    expect(new URL(calls[2]!.url).pathname).toBe('/_/web-projects/docs/rollback')
    expect(calls[2]!.init?.body).toBe(JSON.stringify({ version: '2026-08-28-12-00-00' }))
  })

  it('validates the admin session before uploading a web archive', async () => {
    const session: AuthSession = {
      accessToken: 'access-old',
      refreshToken: 'refresh-old',
      tokenType: 'Bearer',
      expiresIn: 900,
      expiresAt: Date.now() + 900_000,
      scope: { kind: 'admin' },
      user: {
        id: '_admins:one',
        collection: '_admins',
        email: 'admin@example.com',
        role: 'admin',
        verified: true,
        admin: true,
      },
    }
    const store = new MemoryAuthStore(session)
    const calls: string[] = []
    const admin = new HertaBaseAdminClient({
      baseUrl: 'https://example.test',
      authStore: store,
      fetch: async (input) => {
        const path = new URL(String(input)).pathname
        calls.push(path)
        return new Response(
          JSON.stringify({
            data: null,
            meta: null,
            error: { code: 401, message: 'invalid', error: 'HB_UNAUTHORIZED' },
          }),
          { status: 401 },
        )
      },
    })

    await expect(admin.webProjects.deploy({
      archive: { blob: new Blob(['zip']), filename: 'docs.zip' },
    })).rejects.toMatchObject({ status: 401, code: 'HB_UNAUTHORIZED' })

    expect(calls).toEqual(['/api/admin/auth/me', '/api/admin/auth/refresh'])
    expect(await store.get()).toBeNull()
  })
})
