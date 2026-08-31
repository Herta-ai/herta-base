import {
  HertaBaseAdminClient,
  isHertaError,
  type CollectionDefinition,
  type FieldDefinition,
  type IndexDefinition,
  type CollectionRules,
  type UpdateCollectionDefinition,
  type LogEntry,
  type LogListOptions,
  type WebProject,
  type WebProjectDeploy,
  type WebProjectPatch,
  type HertaRecord,
  type Page,
  type DeletedResource,
} from '@hb/sdk/admin';

import { sdkAuthStore } from '../store/auth';

export function resolveApiBaseUrl(): string {
  if (typeof window !== 'undefined' && window.location?.origin) {
    return window.location.origin;
  }
  return 'http://127.0.0.1:8080';
}

export const API_BASE_URL = resolveApiBaseUrl();

// 实例化官方 HertaBaseAdminClient 单例并挂载持久化 AuthStore
export const adminClient = new HertaBaseAdminClient({
  baseUrl: API_BASE_URL,
  authStore: sdkAuthStore,
});

export { isHertaError };

// 导出强类型定义 (完全对齐 SDK 与 Rust 后端 models)
export type CollectionModel = CollectionDefinition;
export type FieldDef = FieldDefinition;
export type IndexDef = IndexDefinition;
export type UpdateCollectionRequest = UpdateCollectionDefinition;
export type RecordModel = HertaRecord & Record<string, unknown>;
export type WebProjectModel = WebProject;
export type FieldTypeName = string;
export type { CollectionRules, LogEntry, LogListOptions as LogQueryParams, Page, DeletedResource };

// 常用 API 接口封装 (通过官方 SDK 直接调用)
export const hbApi = {
  // 认证
  auth: {
    login: (credentials: { email: string; password: string }) =>
      adminClient.auth.login(credentials),
    refresh: () =>
      adminClient.auth.refresh(),
    me: () =>
      adminClient.auth.me(),
    logout: () =>
      adminClient.auth.logout(),
  },

  // 集合管理
  collections: {
    list: () => adminClient.collections.list(),
    get: (name: string) => adminClient.collections.get(name),
    create: (data: CollectionDefinition) => adminClient.collections.create(data),
    update: (name: string, data: UpdateCollectionDefinition) =>
      adminClient.collections.update(name, data),
    delete: (name: string) => adminClient.collections.delete(name),
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
    ) => adminClient.collection<RecordModel>(collectionName).list(params),
    get: (collectionName: string, id: string, params?: { expand?: string }) =>
      adminClient.collection<RecordModel>(collectionName).get(id, params),
    create: (collectionName: string, data: Record<string, unknown>) =>
      adminClient.collection<RecordModel>(collectionName).create(data),
    update: (collectionName: string, id: string, data: Record<string, unknown>) =>
      adminClient.collection<RecordModel>(collectionName).update(id, data),
    delete: (collectionName: string, id: string) =>
      adminClient.collection<RecordModel>(collectionName).delete(id),
  },

  // 日志管理
  logs: {
    list: (params?: LogListOptions) => adminClient.logs.list(params),
  },

  // 静态站点管理
  webProjects: {
    list: () => adminClient.webProjects.list(),
    get: (name: string) => adminClient.webProjects.get(name),
    deploy: (input: WebProjectDeploy) => adminClient.webProjects.deploy(input),
    patch: (name: string, data: WebProjectPatch) => adminClient.webProjects.update(name, data),
    delete: (name: string) => adminClient.webProjects.delete(name),
    versions: (name: string) => adminClient.webProjects.versions(name),
    rollback: (name: string, version: string) => adminClient.webProjects.rollback(name, version),
  },

  // 系统与扩展
  system: {
    getOpenApiSpec: () => adminClient.request<Record<string, unknown>>('/api-doc/openapi.json'),

    executeSql: (query: string) =>
      adminClient.request<{
        results: unknown[];
        executionTimeMs: number;
        affectedRows?: number;
      }>('/api/admin/sql/execute', { method: 'POST', body: { query } }),

    getSettings: () => adminClient.request<Record<string, unknown>>('/api/admin/settings'),
    updateSettings: (settings: Record<string, unknown>) =>
      adminClient.request<Record<string, unknown>>('/api/admin/settings', {
        method: 'PATCH',
        body: settings,
      }),

    getCronTasks: () =>
      adminClient.request<
        Array<{
          name: string;
          schedule: string;
          nextRun?: string;
          status: string;
          lastExecutionMs?: number;
        }>
      >('/api/admin/cron/tasks'),

    exportData: (collectionNames: string[]) =>
      adminClient.request<{
        version: string;
        exportedAt: string;
        collections: CollectionDefinition[];
        data: Record<string, HertaRecord[]>;
      }>('/api/admin/migration/export', {
        method: 'POST',
        body: { collections: collectionNames },
      }),

    importData: (payload: {
      collections?: CollectionDefinition[];
      data?: Record<string, HertaRecord[]>;
    }) =>
      adminClient.request<{ importedCollections: number; importedRecords: number }>(
        '/api/admin/migration/import',
        { method: 'POST', body: payload },
      ),
  },
};
