import { HertaBaseClient, type AuthSession, type AuthStore } from '@hb/sdk';
import { HertaBaseAdminClient } from '@hb/sdk/admin';

// 获取有效 Base URL：浏览器环境下默认使用 window.location.origin，支持 VITE_API_BASE_URL 覆盖
export function resolveApiBaseUrl(): string {
  const custom = (import.meta as any).env?.VITE_API_BASE_URL;
  if (custom && custom !== '/') {
    return custom;
  }
  if (typeof window !== 'undefined' && window.location?.origin) {
    return window.location.origin;
  }
  return 'http://127.0.0.1:8080';
}

export const API_BASE_URL = resolveApiBaseUrl();

/**
 * 本地持久化 AuthStore，确保页面刷新后 SDK 仍然携带 JWT 令牌调用后端
 */
export class LocalStorageAuthStore implements AuthStore {
  private key = 'herta_kanban_auth_session';

  get(): AuthSession | null {
    if (typeof window === 'undefined') return null;
    const raw = localStorage.getItem(this.key);
    if (!raw) return null;
    try {
      return JSON.parse(raw);
    } catch {
      return null;
    }
  }

  set(session: AuthSession): void {
    if (typeof window === 'undefined') return;
    localStorage.setItem(this.key, JSON.stringify(session));
  }

  clear(): void {
    if (typeof window === 'undefined') return;
    localStorage.removeItem(this.key);
  }
}

export const persistentAuthStore = new LocalStorageAuthStore();

// 初始化单例客户端，挂载持久化 AuthStore
export const hb = new HertaBaseClient({
  baseUrl: API_BASE_URL,
  authStore: persistentAuthStore,
});

export const hbAdmin = new HertaBaseAdminClient({
  baseUrl: API_BASE_URL,
});

export async function getFileDownloadUrl(
  collection: string,
  recordId: string,
  field: string,
  filename: string,
): Promise<string> {
  try {
    const tokenRes = await hb.files.issueToken({
      collection,
      recordId,
      field,
    });
    return hb.files.buildDownloadUrl(
      { collection, recordId, field, filename },
      tokenRes.token,
    );
  } catch {
    // Fallback directly to public / authorized url if token fails
    const rawRecordId = recordId.includes(':') ? recordId.split(':')[1] : recordId;
    return `${API_BASE_URL}/api/files/${collection}/${rawRecordId}/${field}/${filename}`;
  }
}
