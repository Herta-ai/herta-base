import { API_BASE_URL } from './hb';

export type InitializationStatus = 'checking' | 'ready' | 'uninitialized' | 'unreachable';

export interface CheckResult {
  status: InitializationStatus;
  message?: string;
  missingCollections?: string[];
  serverReachable: boolean;
}

const REQUIRED_COLLECTIONS = ['kb_users', 'kb_workspaces', 'kb_tasks', 'kb_comments'];

export async function checkDatabaseInitialized(): Promise<CheckResult> {
  try {
    const res = await fetch(`${API_BASE_URL}/api-doc/openapi.json`, {
      headers: { Accept: 'application/json' },
    });

    if (!res.ok) {
      // If openapi endpoint is not 200, try hitting collection endpoint directly
      return await checkByCollectionPing();
    }

    const openapi = await res.json();
    const paths = openapi?.paths || {};
    const missing: string[] = [];

    for (const col of REQUIRED_COLLECTIONS) {
      const pathPattern1 = `/api/collections/${col}/records`;
      const pathPattern2 = `/api/auth/${col}/login`;
      const exists = Boolean(paths[pathPattern1] || paths[pathPattern2]);
      if (!exists) {
        missing.push(col);
      }
    }

    if (missing.length > 0) {
      return {
        status: 'uninitialized',
        message: `缺少核心集合: ${missing.join(', ')}`,
        missingCollections: missing,
        serverReachable: true,
      };
    }

    return {
      status: 'ready',
      serverReachable: true,
    };
  } catch (error) {
    return {
      status: 'unreachable',
      message: `无法连接到 HertaBase 服务端 (${error instanceof Error ? error.message : String(error)})。请确认后端服务已启动。`,
      serverReachable: false,
    };
  }
}

async function checkByCollectionPing(): Promise<CheckResult> {
  try {
    const res = await fetch(`${API_BASE_URL}/api/collections/kb_tasks/records?perPage=1`);
    if (res.status === 404) {
      return {
        status: 'uninitialized',
        message: '未检测到 kb_tasks 集合',
        missingCollections: REQUIRED_COLLECTIONS,
        serverReachable: true,
      };
    }
    return {
      status: 'ready',
      serverReachable: true,
    };
  } catch (error) {
    return {
      status: 'unreachable',
      message: `连接失败: ${error instanceof Error ? error.message : String(error)}`,
      serverReachable: false,
    };
  }
}
