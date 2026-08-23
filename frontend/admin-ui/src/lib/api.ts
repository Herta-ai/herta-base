import axios, { AxiosError, type InternalAxiosRequestConfig } from 'axios';

import { getAccessToken, getRefreshToken, updateTokens, clearAuthSession } from '../store/auth';

export const api = axios.create({
  baseURL: '',
  headers: {
    'Content-Type': 'application/json',
  },
  timeout: 30000,
});

// 并发请求挂起队列
let isRefreshing = false;
let failedQueue: Array<{
  resolve: (token: string) => void;
  reject: (error: unknown) => void;
}> = [];

const processQueue = (error: unknown, token: string | null = null) => {
  failedQueue.forEach((prom) => {
    if (error) {
      prom.reject(error);
    } else if (token) {
      prom.resolve(token);
    }
  });
  failedQueue = [];
};

// 请求拦截器：注入 Access Token
api.interceptors.request.use(
  (config: InternalAxiosRequestConfig) => {
    const token = getAccessToken();
    if (token && !config.headers.Authorization) {
      config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
  },
  (error) => Promise.reject(error),
);

// 响应拦截器：捕获 401 自动无感刷新 Token
api.interceptors.response.use(
  (response) => response,
  async (error: AxiosError) => {
    const originalRequest = error.config as InternalAxiosRequestConfig & {
      _retry?: boolean;
    };

    if (!originalRequest) {
      return Promise.reject(error);
    }

    const isAuthError = error.response?.status === 401;
    const isAuthRoute =
      originalRequest.url?.includes('/api/admin/auth/login') ||
      originalRequest.url?.includes('/api/admin/auth/refresh');

    if (isAuthError && !originalRequest._retry && !isAuthRoute) {
      if (isRefreshing) {
        return new Promise<string>((resolve, reject) => {
          failedQueue.push({ resolve, reject });
        })
          .then((token) => {
            originalRequest.headers.Authorization = `Bearer ${token}`;
            return api(originalRequest);
          })
          .catch((err) => Promise.reject(err));
      }

      originalRequest._retry = true;
      isRefreshing = true;

      const refreshToken = getRefreshToken();
      if (!refreshToken) {
        clearAuthSession();
        window.location.href = '/webui/login';
        return Promise.reject(error);
      }

      try {
        const { data } = await axios.post('/api/admin/auth/refresh', {
          refreshToken,
        });

        const newAccessToken = data.data?.accessToken || data.accessToken;
        const newRefreshToken = data.data?.refreshToken || data.refreshToken;

        if (newAccessToken && newRefreshToken) {
          updateTokens(newAccessToken, newRefreshToken);
          processQueue(null, newAccessToken);
          originalRequest.headers.Authorization = `Bearer ${newAccessToken}`;
          return api(originalRequest);
        } else {
          throw new Error('Invalid refresh response');
        }
      } catch (refreshErr) {
        processQueue(refreshErr, null);
        clearAuthSession();
        window.location.href = '/webui/login';
        return Promise.reject(refreshErr);
      } finally {
        isRefreshing = false;
      }
    }

    return Promise.reject(error);
  },
);

// ----------------------------------------------------
// 类型定义与 API 封装 (完全对齐 OpenAPI / Rust models)
// ----------------------------------------------------

export interface ApiResponse<T> {
  data: T;
  meta?: {
    page?: number;
    perPage?: number;
    total?: number;
    totalItems?: number;
    totalPages?: number;
    [key: string]: unknown;
  };
  error?: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  } | null;
}

export type FieldTypeName =
  | 'text'
  | 'number'
  | 'bool'
  | 'datetime'
  | 'json'
  | 'file'
  | 'relation'
  | 'select'
  | 'email'
  | 'url';

export interface FieldDef {
  name: string;
  type: FieldTypeName | string;
  required?: boolean;
  options?: Record<string, unknown> | null;
}

export interface IndexDef {
  name: string;
  fields: string[];
  unique?: boolean;
}

export interface CollectionRules {
  list?: string | boolean | null;
  view?: string | boolean | null;
  create?: string | boolean | null;
  update?: string | boolean | null;
  delete?: string | boolean | null;
}

export interface CollectionModel {
  name: string;
  type: 'base' | 'auth';
  schema_mode?: 'strict' | 'schema-less' | 'mixed';
  fields: FieldDef[];
  indexes?: IndexDef[];
  rules?: CollectionRules;
  created?: string;
  updated?: string;
}

export interface UpdateCollectionRequest {
  fields?: FieldDef[];
  indexes?: IndexDef[];
  rules?: CollectionRules | null;
}

export interface RecordModel {
  id: string;
  [key: string]: unknown;
}

export interface LogEntry {
  id: string;
  created_at: string;
  log_type: 'server' | 'request';
  level: string;
  message: string;
  target: string;
  method?: string;
  path?: string;
  status_code?: number;
  referer?: string;
  remote_ip?: string;
  user_agent?: string;
  auth_type?: string;
  user_id?: string;
  user_collection?: string;
}

export interface LogQueryParams {
  page?: number;
  perPage?: number;
  level?: string;
  logType?: 'server' | 'request';
  q?: string;
  target?: string;
  path?: string;
  statusCode?: number;
  from?: string;
  to?: string;
}

export interface WebProjectModel {
  name: string;
  alias?: string | null;
  spaFallback: boolean;
  cacheControl: string;
  notFound?: string | null;
  deployedAt: string;
  deployed: boolean;
}

// 常用 API 接口封装
export const hbApi = {
  // 认证
  auth: {
    login: (credentials: { email: string; password: string }) =>
      api.post<
        ApiResponse<{
          accessToken: string;
          refreshToken: string;
          user: { id: string; email: string; role?: string; collection?: string };
        }>
      >('/api/admin/auth/login', credentials),
    refresh: (refreshToken: string) =>
      api.post<
        ApiResponse<{
          accessToken: string;
          refreshToken: string;
        }>
      >('/api/admin/auth/refresh', { refreshToken }),
    me: () =>
      api.get<
        ApiResponse<{
          id: string;
          email: string;
          role?: string;
          collection?: string;
        }>
      >('/api/admin/auth/me'),
  },

  // 集合管理 (注意：对齐后端路由 /_/collections)
  collections: {
    list: () => api.get<ApiResponse<CollectionModel[]>>('/_/collections'),
    get: (name: string) => api.get<ApiResponse<CollectionModel>>(`/_/collections/${name}`),
    create: (data: CollectionModel) =>
      api.post<ApiResponse<CollectionModel>>('/_/collections', data),
    update: (name: string, data: UpdateCollectionRequest) =>
      api.patch<ApiResponse<CollectionModel>>(`/_/collections/${name}`, data),
    delete: (name: string) =>
      api.delete<ApiResponse<{ name: string; deleted: boolean }>>(`/_/collections/${name}`),
  },

  // 记录管理
  records: {
    list: (
      collectionName: string,
      params?: {
        page?: number;
        perPage?: number;
        filter?: string;
        sort?: string;
        expand?: string;
      },
    ) =>
      api.get<ApiResponse<RecordModel[]>>(`/api/collections/${collectionName}/records`, { params }),
    get: (collectionName: string, id: string, params?: { expand?: string }) =>
      api.get<ApiResponse<RecordModel>>(`/api/collections/${collectionName}/records/${id}`, {
        params,
      }),
    create: (collectionName: string, data: Record<string, unknown>) =>
      api.post<ApiResponse<RecordModel>>(`/api/collections/${collectionName}/records`, data),
    update: (collectionName: string, id: string, data: Record<string, unknown>) =>
      api.patch<ApiResponse<RecordModel>>(`/api/collections/${collectionName}/records/${id}`, data),
    delete: (collectionName: string, id: string) =>
      api.delete<ApiResponse<{ success: boolean }>>(
        `/api/collections/${collectionName}/records/${id}`,
      ),
  },

  logs: {
    list: (params?: LogQueryParams) =>
      api.get<ApiResponse<LogEntry[]>>('/api/admin/logs', { params }),
  },

  webProjects: {
    list: () => api.get<ApiResponse<WebProjectModel[]>>('/_/web-projects'),
    deploy: (form: FormData) =>
      api.post<ApiResponse<WebProjectModel>>('/_/web-projects', form, {
        headers: { 'Content-Type': 'multipart/form-data' },
      }),
    patch: (
      name: string,
      data: Partial<Pick<WebProjectModel, 'alias' | 'spaFallback' | 'cacheControl' | 'notFound'>>,
    ) =>
      api.patch<ApiResponse<WebProjectModel>>(`/_/web-projects/${encodeURIComponent(name)}`, data),
    delete: (name: string) =>
      api.delete<ApiResponse<{ name: string; deleted: boolean }>>(
        `/_/web-projects/${encodeURIComponent(name)}`,
      ),
    versions: (name: string) =>
      api.get<ApiResponse<string[]>>(`/_/web-projects/${encodeURIComponent(name)}/versions`),
    rollback: (name: string, version: string) =>
      api.post<ApiResponse<WebProjectModel>>(
        `/_/web-projects/${encodeURIComponent(name)}/rollback`,
        { version },
      ),
  },

  // 系统与扩展
  system: {
    getOpenApiSpec: () => api.get<Record<string, unknown>>('/api-doc/openapi.json'),

    executeSql: (query: string) =>
      api.post<
        ApiResponse<{
          results: unknown[];
          executionTimeMs: number;
          affectedRows?: number;
        }>
      >('/api/admin/sql/execute', { query }),

    getSettings: () => api.get<ApiResponse<Record<string, unknown>>>('/api/admin/settings'),
    updateSettings: (settings: Record<string, unknown>) =>
      api.patch<ApiResponse<Record<string, unknown>>>('/api/admin/settings', settings),

    getCronTasks: () =>
      api.get<
        ApiResponse<
          Array<{
            name: string;
            schedule: string;
            nextRun?: string;
            status: string;
            lastExecutionMs?: number;
          }>
        >
      >('/api/admin/cron/tasks'),

    exportData: (collectionNames: string[]) =>
      api.post<
        ApiResponse<{
          version: string;
          exportedAt: string;
          collections: CollectionModel[];
          data: Record<string, RecordModel[]>;
        }>
      >('/api/admin/migration/export', { collections: collectionNames }),

    importData: (payload: {
      collections?: CollectionModel[];
      data?: Record<string, RecordModel[]>;
    }) =>
      api.post<ApiResponse<{ importedCollections: number; importedRecords: number }>>(
        '/api/admin/migration/import',
        payload,
      ),
  },
};
