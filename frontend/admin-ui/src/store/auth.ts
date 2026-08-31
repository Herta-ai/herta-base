import type { AuthSession, AuthStore } from '@hb/sdk/admin';
import { Store } from '@tanstack/react-store';

export interface AdminUser {
  id: string;
  email: string;
  collection?: string;
  role?: string;
  verified?: boolean;
}

export interface AuthState {
  admin: AdminUser | null;
  accessToken: string | null;
  refreshToken: string | null;
  isAuthenticated: boolean;
}

const AUTH_SESSION_KEY = 'hb_admin_session';

function loadInitialState(): AuthState {
  try {
    const raw = localStorage.getItem(AUTH_SESSION_KEY);
    if (raw) {
      const session = JSON.parse(raw) as AuthSession;
      if (session?.accessToken && session?.user) {
        return {
          admin: session.user,
          accessToken: session.accessToken,
          refreshToken: session.refreshToken || null,
          isAuthenticated: true,
        };
      }
    }

    // 兼容迁移旧版存储 Key
    const oldAccessToken = localStorage.getItem('hb_access_token');
    const oldRefreshToken = localStorage.getItem('hb_refresh_token');
    const oldUserStr = localStorage.getItem('hb_admin_user');
    if (oldAccessToken && oldUserStr) {
      const user = JSON.parse(oldUserStr);
      const session: AuthSession = {
        accessToken: oldAccessToken,
        refreshToken: oldRefreshToken || '',
        tokenType: 'Bearer',
        expiresIn: 86400,
        expiresAt: Date.now() + 86400000,
        scope: { kind: 'admin' },
        user,
      };
      localStorage.setItem(AUTH_SESSION_KEY, JSON.stringify(session));
      return {
        admin: user,
        accessToken: oldAccessToken,
        refreshToken: oldRefreshToken || null,
        isAuthenticated: true,
      };
    }
  } catch {
    // ignore
  }

  return {
    admin: null,
    accessToken: null,
    refreshToken: null,
    isAuthenticated: false,
  };
}

export const authStore = new Store<AuthState>(loadInitialState());

export class SdkAuthStoreAdapter implements AuthStore {
  get(): AuthSession | null {
    try {
      const raw = localStorage.getItem(AUTH_SESSION_KEY);
      if (raw) {
        return JSON.parse(raw) as AuthSession;
      }
      const oldAccessToken = localStorage.getItem('hb_access_token');
      const oldRefreshToken = localStorage.getItem('hb_refresh_token');
      const oldUserStr = localStorage.getItem('hb_admin_user');
      if (oldAccessToken && oldUserStr) {
        const user = JSON.parse(oldUserStr);
        const session: AuthSession = {
          accessToken: oldAccessToken,
          refreshToken: oldRefreshToken || '',
          tokenType: 'Bearer',
          expiresIn: 86400,
          expiresAt: Date.now() + 86400000,
          scope: { kind: 'admin' },
          user,
        };
        localStorage.setItem(AUTH_SESSION_KEY, JSON.stringify(session));
        return session;
      }
      return null;
    } catch {
      return null;
    }
  }

  set(session: AuthSession): void {
    try {
      localStorage.setItem(AUTH_SESSION_KEY, JSON.stringify(session));
      authStore.setState(() => ({
        admin: session.user,
        accessToken: session.accessToken,
        refreshToken: session.refreshToken || null,
        isAuthenticated: Boolean(session.accessToken && session.user),
      }));
    } catch {
      // ignore
    }
  }

  clear(): void {
    try {
      localStorage.removeItem(AUTH_SESSION_KEY);
      authStore.setState(() => ({
        admin: null,
        accessToken: null,
        refreshToken: null,
        isAuthenticated: false,
      }));
    } catch {
      // ignore
    }
  }
}

export const sdkAuthStore = new SdkAuthStoreAdapter();

export function setAuthSession(session: AuthSession) {
  sdkAuthStore.set(session);
}

export function clearAuthSession() {
  sdkAuthStore.clear();
}

export function getAccessToken(): string | null {
  return authStore.state.accessToken || sdkAuthStore.get()?.accessToken || null;
}

export function getRefreshToken(): string | null {
  return authStore.state.refreshToken || sdkAuthStore.get()?.refreshToken || null;
}
