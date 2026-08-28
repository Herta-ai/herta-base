import type { AuthChangeListener, AuthSession, AuthStore } from './types'

export class MemoryAuthStore implements AuthStore {
  private session: AuthSession | null

  constructor(initialSession: AuthSession | null = null) {
    this.session = initialSession
  }

  get(): AuthSession | null {
    return this.session
  }

  set(session: AuthSession): void {
    this.session = session
  }

  clear(): void {
    this.session = null
  }
}

type SessionRefresher = (session: AuthSession) => Promise<AuthSession>

export class AuthState {
  private readonly listeners = new Set<AuthChangeListener>()
  private refresher: SessionRefresher | null = null
  private refreshPromise: Promise<AuthSession> | null = null

  constructor(
    private readonly store: AuthStore,
    private readonly refreshSkewMs: number,
  ) {}

  bindRefresher(refresher: SessionRefresher): void {
    this.refresher = refresher
  }

  async get(): Promise<AuthSession | null> {
    return this.store.get()
  }

  async set(session: AuthSession): Promise<void> {
    await this.store.set(session)
    this.emit(session)
  }

  async clear(): Promise<void> {
    await this.store.clear()
    this.emit(null)
  }

  onChange(listener: AuthChangeListener): () => void {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  async accessToken(): Promise<string | null> {
    let session = await this.get()
    if (!session)
      return null
    if (session.expiresAt - this.refreshSkewMs <= Date.now()) {
      session = await this.refresh(session.accessToken)
    }
    return session.accessToken
  }

  async refresh(failedAccessToken?: string): Promise<AuthSession> {
    const current = await this.get()
    if (!current)
      throw new Error('No authentication session is available')
    if (failedAccessToken && current.accessToken !== failedAccessToken)
      return current
    if (this.refreshPromise)
      return this.refreshPromise
    if (!this.refresher)
      throw new Error('Authentication refresh is not configured')

    this.refreshPromise = this.refresher(current)
      .then(async (session) => {
        await this.set(session)
        return session
      })
      .catch(async (error: unknown) => {
        await this.clear()
        throw error
      })
      .finally(() => {
        this.refreshPromise = null
      })
    return this.refreshPromise
  }

  private emit(session: AuthSession | null): void {
    for (const listener of this.listeners) listener(session)
  }
}
