import type { AuthState } from './auth-store'
import type { Transport } from './transport'
import type {
  AuthChangeListener,
  AuthResponse,
  AuthScope,
  AuthSession,
  Credentials,
  ProfiledAuthUser,
} from './types'
import { HertaError } from './errors'
import { authSessionExpiry, sameScope, scopePath } from './utils'

export function responseToSession<TProfile extends object>(
  response: AuthResponse<TProfile>,
  scope: AuthScope,
): AuthSession<TProfile> {
  return {
    ...response,
    expiresAt: authSessionExpiry(response.expiresIn),
    scope,
  }
}

export class AuthClient {
  constructor(
    protected readonly transport: Transport,
    protected readonly state: AuthState,
    protected readonly scope: AuthScope,
  ) {}

  forCollection(collection: string): AuthClient {
    if (!collection.trim()) {
      throw new HertaError('Auth collection name must not be empty', {
        kind: 'configuration',
      })
    }
    return new AuthClient(this.transport, this.state, { kind: 'collection', collection })
  }

  async register<TProfile extends object = object>(
    credentials: Credentials<TProfile>,
  ): Promise<AuthSession<TProfile>> {
    if (this.scope.kind === 'admin') {
      throw new HertaError('Administrator registration is not supported', {
        kind: 'configuration',
      })
    }
    const response = await this.transport.request<AuthResponse<TProfile>>(
      `${scopePath(this.scope)}/register`,
      {
        method: 'POST',
        body: { email: credentials.email, password: credentials.password, ...credentials.profile },
        auth: false,
        retryAuth: false,
      },
    )
    const session = responseToSession(response, this.scope)
    await this.state.set(session)
    return session
  }

  async login<TProfile extends object = object>(
    credentials: Pick<Credentials<TProfile>, 'email' | 'password'>,
  ): Promise<AuthSession<TProfile>> {
    const response = await this.transport.request<AuthResponse<TProfile>>(
      `${scopePath(this.scope)}/login`,
      {
        method: 'POST',
        body: credentials,
        auth: false,
        retryAuth: false,
      },
    )
    const session = responseToSession(response, this.scope)
    await this.state.set(session)
    return session
  }

  async refresh(): Promise<AuthSession> {
    const session = await this.requireBoundSession()
    return this.state.refresh(session.accessToken)
  }

  async me<TProfile extends object = object>(): Promise<ProfiledAuthUser<TProfile>> {
    await this.requireBoundSession()
    return this.transport.request<ProfiledAuthUser<TProfile>>(`${scopePath(this.scope)}/me`)
  }

  getSession(): Promise<AuthSession | null> {
    return this.state.get()
  }

  async setSession(session: AuthSession): Promise<void> {
    await this.state.set(session)
  }

  logout(): Promise<void> {
    return this.state.clear()
  }

  onChange(listener: AuthChangeListener): () => void {
    return this.state.onChange(listener)
  }

  protected async requireBoundSession(): Promise<AuthSession> {
    const session = await this.state.get()
    if (!session) {
      throw new HertaError('Authentication is required', {
        kind: 'configuration',
        code: 'HB_AUTH_REQUIRED',
      })
    }
    if (!sameScope(session.scope, this.scope)) {
      throw new HertaError('The active session belongs to a different authentication scope', {
        kind: 'configuration',
      })
    }
    return session
  }
}

export class AdminAuthClient {
  private readonly delegate: AuthClient

  constructor(transport: Transport, state: AuthState) {
    this.delegate = new AuthClient(transport, state, { kind: 'admin' })
  }

  login<TProfile extends object = object>(
    credentials: Pick<Credentials<TProfile>, 'email' | 'password'>,
  ): Promise<AuthSession<TProfile>> {
    return this.delegate.login(credentials)
  }

  refresh(): Promise<AuthSession> {
    return this.delegate.refresh()
  }

  me<TProfile extends object = object>(): Promise<ProfiledAuthUser<TProfile>> {
    return this.delegate.me<TProfile>()
  }

  getSession(): Promise<AuthSession | null> {
    return this.delegate.getSession()
  }

  setSession(session: AuthSession): Promise<void> {
    return this.delegate.setSession(session)
  }

  logout(): Promise<void> {
    return this.delegate.logout()
  }

  onChange(listener: AuthChangeListener): () => void {
    return this.delegate.onChange(listener)
  }
}
