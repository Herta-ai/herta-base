import { AuthState, MemoryAuthStore } from "./auth-store";
import { Transport } from "./transport";
import type { AuthResponse, AuthSession, HertaBaseClientOptions } from "./types";
import { responseToSession } from "./auth";
import { scopePath } from "./utils";

const DEFAULT_BASE_URL = "http://localhost:8080";
const DEFAULT_TIMEOUT_MS = 30_000;
const DEFAULT_REFRESH_SKEW_MS = 30_000;

export class ClientRuntime {
  readonly authState: AuthState;
  readonly transport: Transport;

  constructor(options: HertaBaseClientOptions = {}) {
    const fetcher = options.fetch ?? globalThis.fetch?.bind(globalThis);
    if (!fetcher) throw new Error("A fetch implementation is required");
    this.authState = new AuthState(
      options.authStore ?? new MemoryAuthStore(),
      options.refreshSkewMs ?? DEFAULT_REFRESH_SKEW_MS,
    );
    this.transport = new Transport({
      baseUrl: options.baseUrl ?? DEFAULT_BASE_URL,
      fetch: fetcher,
      headers: options.headers,
      timeoutMs: options.timeoutMs ?? DEFAULT_TIMEOUT_MS,
      authState: this.authState,
    });
    this.authState.bindRefresher((session) => this.rotateSession(session));
  }

  private async rotateSession(session: AuthSession): Promise<AuthSession> {
    const response = await this.transport.request<AuthResponse>(
      `${scopePath(session.scope)}/refresh`,
      {
        method: "POST",
        body: { refreshToken: session.refreshToken },
        auth: false,
        retryAuth: false,
      },
    );
    return responseToSession(response, session.scope);
  }
}
