import { Store } from '@tanstack/react-store';
import { hb } from '../lib/hb';
import type { KbUser } from '../types/kanban';

export interface AuthState {
  user: KbUser | null;
  token: string | null;
  isAuthenticated: boolean;
  isLoading: boolean;
}

const STORAGE_KEY_AUTH = 'hb_kanban_auth_state';

function loadInitialAuthState(): AuthState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_AUTH);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed?.token && parsed?.user) {
        return {
          user: parsed.user,
          token: parsed.token,
          isAuthenticated: true,
          isLoading: false,
        };
      }
    }
  } catch (err) {
    console.error('Failed to restore auth from localStorage:', err);
  }
  return {
    user: null,
    token: null,
    isAuthenticated: false,
    isLoading: false,
  };
}

export const authStore = new Store<AuthState>(loadInitialAuthState());

export function setAuthSession(user: KbUser, token: string) {
  authStore.setState(() => ({
    user,
    token,
    isAuthenticated: true,
    isLoading: false,
  }));
  try {
    localStorage.setItem(STORAGE_KEY_AUTH, JSON.stringify({ user, token }));
  } catch (e) {
    console.error(e);
  }
}

export async function loginWithEmail(email: string, password = 'correct password 123'): Promise<KbUser> {
  authStore.setState((prev) => ({ ...prev, isLoading: true }));
  try {
    const session = await hb.auth.forCollection('kb_users').login({ email, password });
    const user: KbUser = {
      id: session.user.id,
      email: session.user.email,
      displayName: (session.user as any).profile?.displayName || (session.user as any).displayName || email.split('@')[0],
      avatar: (session.user as any).profile?.avatar || (session.user as any).avatar,
    };
    setAuthSession(user, session.accessToken);
    return user;
  } catch (error) {
    authStore.setState((prev) => ({ ...prev, isLoading: false }));
    throw error;
  }
}

export async function registerWithEmail(email: string, displayName: string, password = 'correct password 123'): Promise<KbUser> {
  authStore.setState((prev) => ({ ...prev, isLoading: true }));
  try {
    const session = await hb.auth.forCollection('kb_users').register({
      email,
      password,
      profile: { displayName },
    });
    const user: KbUser = {
      id: session.user.id,
      email: session.user.email,
      displayName: (session.user as any).profile?.displayName || displayName,
      avatar: (session.user as any).profile?.avatar,
    };
    setAuthSession(user, session.accessToken);
    return user;
  } catch (error) {
    authStore.setState((prev) => ({ ...prev, isLoading: false }));
    throw error;
  }
}

export async function logoutUser() {
  await hb.auth.logout();
  authStore.setState(() => ({
    user: null,
    token: null,
    isAuthenticated: false,
    isLoading: false,
  }));
  localStorage.removeItem(STORAGE_KEY_AUTH);
}
