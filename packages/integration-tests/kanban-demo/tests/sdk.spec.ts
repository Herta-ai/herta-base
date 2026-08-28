import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { HertaBaseClient, HertaError } from '@hb/sdk';
import { HertaBaseAdminClient } from '@hb/sdk/admin';

import { serverUrl, startServer, stopServer } from './server';

const ADMIN_EMAIL = 'admin@example.com';
const ADMIN_PASSWORD = 'correct horse battery staple';
const PASSWORD = 'correct password 123';

interface SdkTask {
  id: string;
  title: string;
  owner: string;
  status: 'todo' | 'done';
  attachments?: string[];
  expand?: { owner?: { id: string; email: string } };
}

type CreateTask = Omit<SdkTask, 'id' | 'attachments' | 'expand'>;
type UpdateTask = Partial<Pick<SdkTask, 'title' | 'status'>>;

let admin: HertaBaseAdminClient;
let client: HertaBaseClient;
let userId = '';

describe.sequential('TypeScript SDK contract', () => {
  beforeAll(async () => {
    await startServer();
    admin = new HertaBaseAdminClient({ baseUrl: serverUrl() });
    await admin.auth.login({ email: ADMIN_EMAIL, password: ADMIN_PASSWORD });
    await admin.collections.create({
      name: 'sdk_users',
      type: 'auth',
      schema_mode: 'schema-less',
      rules: { list: true, view: true, create: true, update: true, delete: true },
    });
    await admin.collections.create({
      name: 'sdk_tasks',
      type: 'base',
      schema_mode: 'strict',
      fields: [
        { name: 'title', type: 'text', required: true },
        {
          name: 'owner',
          type: 'relation',
          required: true,
          options: { collection: 'sdk_users', maxSelect: 1 },
        },
        { name: 'status', type: 'select', required: true, options: { values: ['todo', 'done'] } },
        {
          name: 'attachments',
          type: 'file',
          options: { maxSelect: 3, extensions: ['txt'], mimeTypes: ['text/plain'] },
        },
      ],
      rules: { list: true, view: true, create: true, update: true, delete: true },
    });

    client = new HertaBaseClient({ baseUrl: serverUrl() });
    const session = await client.auth.forCollection('sdk_users').register({
      email: 'sdk@example.com',
      password: PASSWORD,
      profile: { displayName: 'SDK User' },
    });
    userId = session.user.id;
  });

  afterAll(async () => {
    await stopServer();
  });

  it('performs typed CRUD, relation filtering and expansion', async () => {
    const tasks = client.collection<SdkTask, CreateTask, UpdateTask>('sdk_tasks');
    const created = await tasks.create({
      title: 'SDK contract',
      owner: userId,
      status: 'todo',
    });
    const page = await tasks.list({
      filter: `owner = '${userId}'`,
      expand: 'owner',
      sort: 'title',
    });

    expect(page.total).toBe(1);
    expect(page.items[0]?.id).toBe(created.id);
    expect(page.items[0]?.owner).toBe(userId);
    expect(page.items[0]?.expand?.owner?.id).toBe(userId);
    await expect(tasks.get('missing')).rejects.toMatchObject({
      code: 'HB_RECORD_NOT_FOUND',
      status: 404,
    } satisfies Partial<HertaError>);
  });

  it('uploads, appends and token-downloads record files', async () => {
    const tasks = client.collection<SdkTask, CreateTask, UpdateTask>('sdk_tasks');
    const task = (await tasks.list()).items[0]!;
    const uploaded = await tasks.updateWithFiles(
      task.id,
      {
        files: {
          attachments: { blob: new Blob(['sdk-file'], { type: 'text/plain' }), filename: 'sdk.txt' },
        },
      },
      { appendFiles: ['attachments'] },
    );
    const filename = uploaded.attachments?.[0];
    expect(filename).toBeTruthy();

    const fileToken = await client.files.issueToken({
      collection: 'sdk_tasks', recordId: task.id, field: 'attachments',
    });
    const response = await fetch(
      client.files.buildDownloadUrl(
        { collection: 'sdk_tasks', recordId: task.id, field: 'attachments', filename: filename! },
        fileToken.token,
      ),
    );
    expect(response.status).toBe(200);
    expect(await response.text()).toBe('sdk-file');
  });

  it('receives realtime updates and closes cleanly', async () => {
    const tasks = client.collection<SdkTask, CreateTask, UpdateTask>('sdk_tasks');
    const task = (await tasks.list()).items[0]!;
    let resolveConnected!: () => void;
    let resolveUpdate!: (task: SdkTask) => void;
    const connected = new Promise<void>((resolve) => { resolveConnected = resolve; });
    const updated = new Promise<SdkTask>((resolve) => { resolveUpdate = resolve; });
    const subscription = await tasks.subscribe({
      reconnect: false,
      filter: `owner = '${userId}'`,
      onEvent(event) {
        if (event.type === 'connected') resolveConnected();
        if (event.type === 'update') resolveUpdate(event.data.record);
      },
    });
    try {
      await connected;
      await tasks.update(task.id, { status: 'done' });
      await expect(updated).resolves.toMatchObject({ id: task.id, status: 'done' });
    } finally {
      subscription.close();
    }
  });
});
