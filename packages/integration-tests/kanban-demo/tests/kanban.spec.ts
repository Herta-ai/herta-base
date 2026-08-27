import { createHmac } from 'node:crypto';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { JWT_SECRET, serverUrl, startServer, stopServer } from './server';
import { SseConnection, SseConnectionError } from './sse-helper';

type ApiError = { code: number; error: string; message: string; details: unknown };
type Envelope<T = unknown> = { data: T | null; meta: unknown; error: ApiError | null };
type Result<T = unknown> = { status: number; headers: Headers; body: Envelope<T> };
type Client = { token?: string; request<T = unknown>(method: string, path: string, body?: BodyInit | object): Promise<Result<T>> };

const ADMIN_EMAIL = 'admin@example.com';
const ADMIN_PASSWORD = 'correct horse battery staple';
const PASSWORD = 'correct password 123';

let anonymous: Client;
let admin: Client;
let owner: Client;
let assignee: Client;
let member: Client;
let outsider: Client;
let ownerToken = '';
let ownerId = '';
let assigneeId = '';
let memberId = '';
let outsiderId = '';
let teamWorkspaceId = '';
let outsiderWorkspaceId = '';
let publicTaskId = '';
let privateTaskId = '';

function client(token?: string): Client {
  return {
    token,
    async request<T>(method: string, path: string, body?: BodyInit | object): Promise<Result<T>> {
      const headers = new Headers();
      if (token) headers.set('Authorization', `Bearer ${token}`);
      let requestBody: BodyInit | undefined;
      if (body instanceof FormData || typeof body === 'string' || body instanceof Blob) {
        requestBody = body;
      } else if (body !== undefined) {
        headers.set('content-type', 'application/json');
        requestBody = JSON.stringify(body);
      }
      const response = await fetch(`${serverUrl()}${path}`, { method, headers, body: requestBody });
      return { status: response.status, headers: response.headers, body: await response.json() as Envelope<T> };
    },
  };
}

function success<T>(result: Result<T>, status: number): T {
  expect(result.status).toBe(status);
  expect(result.body.error).toBeNull();
  expect(result.body.meta).toBeNull();
  expect(result.body.data).not.toBeNull();
  return result.body.data as T;
}

function paged<T>(result: Result<T[]>, total: number): T[] {
  expect(result.status).toBe(200);
  expect(result.body.error).toBeNull();
  expect(result.body.meta).toEqual(expect.objectContaining({ total }));
  return result.body.data as T[];
}

function failure(result: Result, status: number, code: string): void {
  expect(result.status).toBe(status);
  expect(result.body).toEqual(expect.objectContaining({ data: null, meta: null }));
  expect(result.body.error).toEqual(expect.objectContaining({ code: status, error: code }));
}

async function register(email: string, displayName: string): Promise<{ api: Client; id: string; token: string }> {
  const response = await anonymous.request<any>('POST', '/api/auth/kb_users/register', {
    email, password: PASSWORD, displayName,
  });
  const auth = success(response, 201);
  return { api: client(auth.accessToken), id: auth.user.id, token: auth.accessToken };
}

function encodedId(id: string): string { return encodeURIComponent(id); }

function expiredToken(validToken: string): string {
  const header = { alg: 'HS256', typ: 'JWT' };
  const payload = JSON.parse(Buffer.from(validToken.split('.')[1], 'base64url').toString('utf8'));
  payload.iat = Math.floor(Date.now() / 1000) - 7_200;
  payload.exp = Math.floor(Date.now() / 1000) - 3_600;
  const content = `${Buffer.from(JSON.stringify(header)).toString('base64url')}.${Buffer.from(JSON.stringify(payload)).toString('base64url')}`;
  const signature = createHmac('sha256', JWT_SECRET).update(content).digest('base64url');
  return `${content}.${signature}`;
}

function uploadForm(files: Array<[string, string, string, string]>, data?: object): FormData {
  const form = new FormData();
  if (data) form.append('data', JSON.stringify(data));
  for (const [field, name, type, content] of files) {
    form.append(field, new Blob([content], { type }), name);
  }
  return form;
}

describe.sequential('kanban collaborative contract integration', () => {
  beforeAll(async () => {
    await startServer();
    anonymous = client();
    const login = await anonymous.request<any>('POST', '/api/admin/auth/login', {
      email: ADMIN_EMAIL, password: ADMIN_PASSWORD,
    });
    admin = client(success(login, 200).accessToken);

    success(await admin.request('POST', '/_/collections', {
      name: 'kb_users', type: 'auth', schema_mode: 'schema-less',
      rules: { list: true, view: true, create: true, update: true, delete: true },
    }), 201);
    success(await admin.request('POST', '/_/collections', {
      name: 'kb_workspaces', type: 'base', schema_mode: 'strict',
      fields: [
        { name: 'name', type: 'text', required: true },
        { name: 'owner', type: 'relation', required: true, options: { collection: 'kb_users', maxSelect: 1 } },
        { name: 'members', type: 'relation', required: true, options: { collection: 'kb_users', maxSelect: 20 } },
      ],
      rules: {
        list: 'owner = $auth.record OR $auth.record IN members',
        view: 'owner = $auth.record OR $auth.record IN members',
        create: '$record.owner = $auth.record AND $auth.record IN $record.members',
        update: 'owner = $auth.record',
        delete: 'owner = $auth.record',
      },
    }), 201);
    success(await admin.request('POST', '/_/collections', {
      name: 'kb_tasks', type: 'base', schema_mode: 'strict',
      fields: [
        { name: 'title', type: 'text', required: true },
        { name: 'description', type: 'text' },
        { name: 'priority', type: 'select', required: true, options: { values: ['low', 'medium', 'high', 'urgent'] } },
        { name: 'status', type: 'select', required: true, options: { values: ['todo', 'in_progress', 'in_review', 'done'] } },
        { name: 'workspace', type: 'relation', required: true, options: { collection: 'kb_workspaces', maxSelect: 1 } },
        { name: 'assignees', type: 'relation', options: { collection: 'kb_users', maxSelect: 10 } },
        { name: 'attachments', type: 'file', options: { maxSelect: 5, extensions: ['png', 'jpg', 'pdf', 'zip'] } },
        { name: 'order', type: 'number', required: true },
        { name: 'is_private', type: 'bool', required: true },
      ],
      rules: {
        list: 'workspace.owner = $auth.record OR $auth.record IN workspace.members',
        view: 'workspace.owner = $auth.record OR $auth.record IN workspace.members',
        create: '$record.workspace.owner = $auth.record OR $auth.record IN $record.workspace.members',
        update: '(workspace.owner = $auth.record OR $auth.record IN workspace.members) AND (is_private = false OR workspace.owner = $auth.record OR $auth.record IN assignees) AND $request.body.workspace IS NONE',
        delete: '(workspace.owner = $auth.record OR $auth.record IN workspace.members) AND (is_private = false OR workspace.owner = $auth.record OR $auth.record IN assignees)',
      },
    }), 201);
    success(await admin.request('POST', '/_/collections', {
      name: 'kb_comments', type: 'base', schema_mode: 'strict',
      fields: [
        { name: 'task', type: 'relation', required: true, options: { collection: 'kb_tasks', maxSelect: 1 } },
        { name: 'author', type: 'relation', required: true, options: { collection: 'kb_users', maxSelect: 1 } },
        { name: 'content', type: 'text', required: true },
      ],
      rules: {
        list: 'task.workspace.owner = $auth.record OR $auth.record IN task.workspace.members',
        view: 'task.workspace.owner = $auth.record OR $auth.record IN task.workspace.members',
        create: '$record.author = $auth.record AND ($record.task.workspace.owner = $auth.record OR $auth.record IN $record.task.workspace.members)',
        update: 'author = $auth.record',
        delete: 'author = $auth.record',
      },
    }), 201);

    const ownerRegistration = await register('owner@example.com', 'Owner');
    owner = ownerRegistration.api; ownerId = ownerRegistration.id; ownerToken = ownerRegistration.token;
    const assigneeRegistration = await register('assignee@example.com', 'Assignee');
    assignee = assigneeRegistration.api; assigneeId = assigneeRegistration.id;
    const memberRegistration = await register('member@example.com', 'Member');
    member = memberRegistration.api; memberId = memberRegistration.id;
    const outsiderRegistration = await register('outsider@example.com', 'Outsider');
    outsider = outsiderRegistration.api; outsiderId = outsiderRegistration.id;
  });

  afterAll(async () => { await stopServer(); });

  it('creates isolated workspaces and enforces owner/member rules', async () => {
    const team = success(await owner.request<any>('POST', '/api/collections/kb_workspaces/records', {
      name: 'Team workspace', owner: ownerId, members: [ownerId, assigneeId, memberId],
    }), 201);
    teamWorkspaceId = team.id;
    const other = success(await outsider.request<any>('POST', '/api/collections/kb_workspaces/records', {
      name: 'Outsider workspace', owner: outsiderId, members: [outsiderId],
    }), 201);
    outsiderWorkspaceId = other.id;

    failure(await member.request('PATCH', `/api/collections/kb_workspaces/records/${encodedId(teamWorkspaceId)}`, { name: 'hijacked' }), 403, 'HB_FORBIDDEN');
    failure(await outsider.request('GET', `/api/collections/kb_workspaces/records/${encodedId(teamWorkspaceId)}`), 404, 'HB_RECORD_NOT_FOUND');
  });

  it('enforces cross-workspace and private-task write restrictions', async () => {
    const task = success(await owner.request<any>('POST', '/api/collections/kb_tasks/records', {
      title: 'Public card', description: 'shared', priority: 'high', status: 'todo',
      workspace: teamWorkspaceId, assignees: [assigneeId], order: 10, is_private: false,
    }), 201);
    publicTaskId = task.id;
    const privateTask = success(await owner.request<any>('POST', '/api/collections/kb_tasks/records', {
      title: 'Private card', description: 'assigned only', priority: 'urgent', status: 'todo',
      workspace: teamWorkspaceId, assignees: [assigneeId], order: 20, is_private: true,
    }), 201);
    privateTaskId = privateTask.id;

    failure(await member.request('POST', '/api/collections/kb_tasks/records', {
      title: 'Cross workspace', priority: 'low', status: 'todo', workspace: outsiderWorkspaceId,
      assignees: [memberId], order: 1, is_private: false,
    }), 403, 'HB_FORBIDDEN');
    failure(await member.request('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}`, { status: 'done' }), 403, 'HB_FORBIDDEN');
    failure(await outsider.request('GET', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}`), 404, 'HB_RECORD_NOT_FOUND');
    failure(await owner.request('PATCH', `/api/collections/kb_tasks/records/${encodedId(publicTaskId)}`, { workspace: outsiderWorkspaceId }), 403, 'HB_FORBIDDEN');
  });

  it('binds relation filters natively and preserves IDs during deep expansion', async () => {
    const params = new URLSearchParams({
      filter: `workspace = '${teamWorkspaceId}'`,
      expand: 'assignees,workspace.owner,workspace.members',
      sort: 'order',
    });
    const tasks = paged<any>(await member.request('GET', `/api/collections/kb_tasks/records?${params}`), 2);
    expect(tasks.map((task) => task.id)).toEqual([publicTaskId, privateTaskId]);
    expect(tasks[0].workspace).toBe(teamWorkspaceId);
    expect(tasks[0].assignees).toEqual([assigneeId]);
    expect(tasks[0].expand.assignees[0].id).toBe(assigneeId);
    expect(tasks[0].expand.workspace.owner.id).toBe(ownerId);
    expect(tasks[0].expand.workspace.members.map((user: any) => user.id)).toEqual([ownerId, assigneeId, memberId]);

    expect(paged(await member.request<any>('GET', `/api/collections/kb_tasks/records?${new URLSearchParams({ filter: `workspace = '${outsiderWorkspaceId}'` })}`), 0)).toEqual([]);
  });

  it('streams filtered updates, rejects bad tokens and data-backed forbidden filters', async () => {
    for (const token of ['invalid-token', expiredToken(ownerToken)]) {
      await expect(SseConnection.connect(`${serverUrl()}/api/realtime/kb_tasks`, token)).rejects.toMatchObject({
        status: 401,
        code: token === 'invalid-token' ? 'HB_UNAUTHORIZED' : 'HB_TOKEN_EXPIRED',
      } satisfies Partial<SseConnectionError>);
    }
    const forbiddenUrl = `${serverUrl()}/api/realtime/kb_tasks?${new URLSearchParams({ filter: `workspace = '${teamWorkspaceId}'` })}`;
    await expect(SseConnection.connect(forbiddenUrl, outsider.token)).rejects.toMatchObject({ status: 403, code: 'HB_FORBIDDEN' });

    const connection = await SseConnection.connect(forbiddenUrl, ownerToken);
    try {
      await connection.waitForEvent('connected');
      const updated = success(await assignee.request<any>('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}`, {
        status: 'in_progress', order: 15,
      }), 200);
      expect(updated.status).toBe('in_progress');
      const event = await connection.waitForEvent<any>('update');
      expect(event.data.record).toEqual(expect.objectContaining({ id: privateTaskId, status: 'in_progress', order: 15 }));

      success(await outsider.request('POST', '/api/collections/kb_tasks/records', {
        title: 'Other workspace card', priority: 'low', status: 'todo', workspace: outsiderWorkspaceId,
        assignees: [outsiderId], order: 1, is_private: false,
      }), 201);
      await connection.expectNoEvent<any>('create', (data) => data.record?.workspace === outsiderWorkspaceId);
    } finally {
      await connection.close();
    }
  });

  it('appends, replaces, token-downloads, and validates task attachments', async () => {
    const firstUpload = uploadForm([
      ['attachments', 'design.png', 'image/png', 'png-design'],
      ['attachments', 'brief.pdf', 'application/pdf', 'pdf-brief'],
    ]);
    const initial = success(await assignee.request<any>('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}?appendFiles=attachments`, firstUpload), 200);
    expect(initial.attachments).toHaveLength(2);
    const [design, brief] = initial.attachments as string[];

    const tokenResult = success(await assignee.request<any>('POST', '/api/files/token', {
      collection: 'kb_tasks', recordId: privateTaskId, field: 'attachments',
    }), 200);
    const rawId = privateTaskId.slice('kb_tasks:'.length);
    const download = await fetch(`${serverUrl()}/api/files/kb_tasks/${rawId}/attachments/${design}?token=${encodeURIComponent(tokenResult.token)}`);
    expect(download.status).toBe(200);
    expect(await download.text()).toBe('png-design');

    const appended = success(await owner.request<any>('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}?appendFiles=attachments`, uploadForm([
      ['attachments', 'archive.zip', 'application/zip', 'zip-archive'],
    ])), 200);
    expect(appended.attachments.slice(0, 2)).toEqual([design, brief]);
    expect(appended.attachments).toHaveLength(3);

    const replaced = success(await owner.request<any>('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}`, uploadForm([
      ['attachments', 'replacement.jpg', 'image/jpeg', 'jpg-replacement'],
    ])), 200);
    expect(replaced.attachments).toHaveLength(1);
    expect(replaced.attachments[0]).not.toBe(design);
    const oldDownload = await fetch(`${serverUrl()}/api/files/kb_tasks/${rawId}/attachments/${design}`, {
      headers: { Authorization: `Bearer ${ownerToken}` },
    });
    expect(oldDownload.status).toBe(404);

    failure(await owner.request('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}`, uploadForm([
      ['attachments', 'malware.exe', 'application/octet-stream', 'bad'],
    ])), 400, 'HB_VALIDATION_ERROR');
    failure(await owner.request('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}?appendFiles=attachments`, uploadForm([
      ['attachments', '1.png', 'image/png', '1'], ['attachments', '2.png', 'image/png', '2'],
      ['attachments', '3.png', 'image/png', '3'], ['attachments', '4.png', 'image/png', '4'],
      ['attachments', '5.png', 'image/png', '5'],
    ])), 400, 'HB_VALIDATION_ERROR');
    failure(await owner.request('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}?appendFiles=attachments`, uploadForm([
      ['attachments', 'conflict.png', 'image/png', 'conflict'],
    ], { attachments: [] })), 400, 'HB_VALIDATION_ERROR');
    failure(await owner.request('PATCH', `/api/collections/kb_tasks/records/${encodedId(privateTaskId)}?appendFiles=attachments,attachments`, uploadForm([
      ['attachments', 'duplicate.png', 'image/png', 'duplicate'],
    ])), 400, 'HB_VALIDATION_ERROR');
  });

  it('enforces comment authorship and soft-delete record errors', async () => {
    failure(await member.request('POST', '/api/collections/kb_comments/records', {
      task: publicTaskId, author: ownerId, content: 'forged author',
    }), 403, 'HB_FORBIDDEN');
    const comment = success(await member.request<any>('POST', '/api/collections/kb_comments/records', {
      task: publicTaskId, author: memberId, content: 'Looks good',
    }), 201);
    expect(comment.author).toBe(memberId);
    failure(await assignee.request('PATCH', `/api/collections/kb_comments/records/${encodedId(comment.id)}`, { content: 'hijacked' }), 403, 'HB_FORBIDDEN');

    success(await owner.request('DELETE', `/api/collections/kb_tasks/records/${encodedId(publicTaskId)}`), 200);
    failure(await owner.request('GET', `/api/collections/kb_tasks/records/${encodedId(publicTaskId)}`), 404, 'HB_RECORD_NOT_FOUND');
    failure(await owner.request('PATCH', `/api/collections/kb_tasks/records/${encodedId(publicTaskId)}`, { status: 'done' }), 404, 'HB_RECORD_NOT_FOUND');
  });
});
