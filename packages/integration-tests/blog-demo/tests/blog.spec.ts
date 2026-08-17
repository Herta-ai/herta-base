import { beforeAll, afterAll, describe, expect, it } from 'vitest';
import axios, { AxiosInstance, AxiosResponse } from 'axios';
import { serverUrl, startServer, stopServer } from './server';

type Envelope<T = any> = {
  data: T | null;
  meta: any;
  error: { code: number; error: string; message: string; details: any } | null;
};

const ADMIN_EMAIL = 'admin@example.com';
const ADMIN_PASSWORD = 'correct horse battery staple';
const PASSWORD = 'correct password 123';

let admin: AxiosInstance;
let author1: AxiosInstance;
let author2: AxiosInstance;
let anonymous: AxiosInstance;
let author1Id = '';
let author2Id = '';
let publicPostId = '';
let privatePostId = '';
let publicCommentId = '';
let privateCommentId = '';
let attributedPrivateCommentId = '';

function success<T>(response: AxiosResponse<Envelope<T>>, status: number): T {
  expect(response.status).toBe(status);
  expect(response.data).toHaveProperty('data');
  expect(response.data).toHaveProperty('meta', null);
  expect(response.data).toHaveProperty('error', null);
  expect(response.data.data).not.toBeNull();
  return response.data.data as T;
}

function paged<T>(response: AxiosResponse<Envelope<T>>, status: number, total: number): T {
  expect(response.status).toBe(status);
  expect(response.data).toHaveProperty('data');
  expect(response.data.meta).toEqual(expect.objectContaining({ total }));
  expect(response.data.error).toBeNull();
  return response.data.data as T;
}

function failure(response: AxiosResponse<Envelope<null>>, status: number, code: string): void {
  expect(response.status).toBe(status);
  expect(response.data).toEqual(expect.objectContaining({ data: null, meta: null }));
  expect(response.data.error).toEqual(expect.objectContaining({ code: status, error: code }));
  expect(response.data.error).toHaveProperty('message');
  expect(response.data.error).toHaveProperty('details');
}

function withToken(token: string): AxiosInstance {
  return axios.create({
    baseURL: serverUrl(),
    validateStatus: () => true,
    headers: { Authorization: `Bearer ${token}` },
  });
}

function jwtPayload(token: string): Record<string, any> {
  return JSON.parse(Buffer.from(token.split('.')[1], 'base64url').toString('utf8')) as Record<string, any>;
}

describe.sequential('blog contract integration', () => {
  beforeAll(async () => {
    await startServer();
    anonymous = axios.create({ baseURL: serverUrl(), validateStatus: () => true });
    const adminLogin = await anonymous.post('/api/admin/auth/login', {
      email: ADMIN_EMAIL,
      password: ADMIN_PASSWORD,
    });
    const adminAuth = success(adminLogin, 200) as any;
    expect(adminAuth.user.id).toMatch(/^_admins:/);
    expect(jwtPayload(adminAuth.accessToken).sub).toBe(adminAuth.user.id);
    admin = withToken(adminAuth.accessToken);

    const users = await admin.post('/_/collections', {
      name: 'blog_users',
      type: 'auth',
      schema_mode: 'schema-less',
      rules: { list: true, view: true, create: true, update: true, delete: true },
    });
    success(users, 201);

    const posts = await admin.post('/_/collections', {
      name: 'blog_posts',
      type: 'base',
      schema_mode: 'schema-less',
      fields: [
        { name: 'title', type: 'text', required: true },
        { name: 'content', type: 'text' },
        { name: 'is_public', type: 'bool', required: true },
        { name: 'author', type: 'relation', required: true, options: { collection: 'blog_users', maxSelect: 1 } },
      ],
      rules: {
        list: 'is_public = true OR author = $auth.record',
        view: 'is_public = true OR author = $auth.record',
        create: '$record.author = $auth.record',
        update: 'author = $auth.record AND ($request.body.author IS NONE OR $request.body.author = $auth.id)',
        delete: 'author = $auth.record',
      },
    });
    success(posts, 201);

    const comments = await admin.post('/_/collections', {
      name: 'blog_comments',
      type: 'base',
      schema_mode: 'schema-less',
      fields: [
        { name: 'post', type: 'relation', required: true, options: { collection: 'blog_posts', maxSelect: 1 } },
        { name: 'content', type: 'text', required: true },
        { name: 'author', type: 'relation', required: true, options: { collection: 'blog_users', maxSelect: 1 } },
      ],
      rules: {
        list: 'post.is_public = true OR post.author = $auth.record OR author = $auth.record',
        view: 'post.is_public = true OR post.author = $auth.record OR author = $auth.record',
        create: '$record.author = $auth.record AND ($record.post.is_public = true OR $record.post.author = $auth.record)',
        update: 'author = $auth.record',
        delete: 'author = $auth.record',
      },
    });
    success(comments, 201);
  });

  afterAll(async () => {
    await stopServer();
  });

  it('registers and logs in with full auth IDs and JWT subjects', async () => {
    const registered1 = await anonymous.post('/api/auth/blog_users/register', {
      email: 'author1@example.com', password: PASSWORD, displayName: 'Author One',
    });
    const auth1 = success(registered1, 201) as any;
    author1Id = auth1.user.id;
    expect(author1Id).toMatch(/^blog_users:[A-Za-z0-9_-]+$/);
    expect(auth1.user.displayName).toBe('Author One');
    expect(jwtPayload(auth1.accessToken).sub).toBe(author1Id);
    author1 = withToken(auth1.accessToken);

    const login1 = await anonymous.post('/api/auth/blog_users/login', {
      email: 'author1@example.com', password: PASSWORD,
    });
    const loggedIn1 = success(login1, 200) as any;
    expect(loggedIn1.user.id).toBe(author1Id);

    const registered2 = await anonymous.post('/api/auth/blog_users/register', {
      email: 'author2@example.com', password: PASSWORD,
    });
    const auth2 = success(registered2, 201) as any;
    author2Id = auth2.user.id;
    expect(author2Id).toMatch(/^blog_users:/);
    author2 = withToken(auth2.accessToken);
  });

  it('enforces native relation create rules and preserves schema-less fields', async () => {
    const forged = await author1.post('/api/collections/blog_posts/records', {
      title: 'forged', content: 'no', is_public: true, author: author2Id,
    });
    failure(forged, 403, 'HB_FORBIDDEN');

    const created = await author1.post('/api/collections/blog_posts/records', {
      title: 'Public post', content: 'hello', is_public: true, author: author1Id, extra_field: 'kept',
    });
    const post = success(created, 201) as any;
    publicPostId = post.id;
    expect(publicPostId).toMatch(/^blog_posts:/);
    expect(post.author).toBe(author1Id);
    expect(post.extra_field).toBe('kept');

    const privateCreated = await author1.post('/api/collections/blog_posts/records', {
      title: 'Private post', content: 'secret', is_public: false, author: author1Id,
    });
    privatePostId = (success(privateCreated, 201) as any).id;
  });

  it('accepts full and bare record paths, rejects mismatched collections, and expands relations', async () => {
    const full = await author1.get(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}?expand=author`);
    const fullPost = success(full, 200) as any;
    expect(fullPost.id).toBe(publicPostId);
    expect(fullPost.author).toBe(author1Id);
    expect(fullPost.expand.author.id).toBe(author1Id);

    const key = publicPostId.slice('blog_posts:'.length);
    const bare = await author1.get(`/api/collections/blog_posts/records/${key}`);
    expect((success(bare, 200) as any).id).toBe(publicPostId);

    const wrongCollection = await author1.get(`/api/collections/blog_posts/records/${encodeURIComponent(`other:${key}`)}`);
    failure(wrongCollection, 404, 'HB_NOT_FOUND');

    for (const relation of ['author1', 'blog_users:', 'not a record', 'other:abc']) {
      const invalid = await author1.post('/api/collections/blog_posts/records', {
        title: 'invalid', content: '', is_public: true, author: relation,
      });
      failure(invalid, 400, 'HB_VALIDATION_ERROR');
    }
  });

  it('applies public/private post visibility and blocks ownership changes', async () => {
    const anonymousPosts = await anonymous.get('/api/collections/blog_posts/records');
    expect((paged(anonymousPosts, 200, 1) as any[]).map((post) => post.id)).toEqual([publicPostId]);
    const author1Posts = await author1.get('/api/collections/blog_posts/records');
    expect((paged(author1Posts, 200, 2) as any[]).map((post) => post.id)).toEqual(expect.arrayContaining([publicPostId, privatePostId]));
    const author2Posts = await author2.get('/api/collections/blog_posts/records');
    expect((paged(author2Posts, 200, 1) as any[]).map((post) => post.id)).toEqual([publicPostId]);

    const outsiderGet = await author2.get(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`);
    failure(outsiderGet, 404, 'HB_NOT_FOUND');
    const transfer = await author1.patch(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}`, { author: author2Id });
    failure(transfer, 403, 'HB_FORBIDDEN');
    const update = await author2.patch(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}`, { content: 'hacked' });
    failure(update, 403, 'HB_FORBIDDEN');
    const remove = await author2.delete(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}`);
    failure(remove, 403, 'HB_FORBIDDEN');
  });

  it('enforces public and private comment visibility and authorship', async () => {
    const forged = await author2.post('/api/collections/blog_comments/records', {
      post: publicPostId, content: 'forged', author: author1Id,
    });
    failure(forged, 403, 'HB_FORBIDDEN');

    const publicComment = await author2.post('/api/collections/blog_comments/records', {
      post: publicPostId, content: 'public comment', author: author2Id,
    });
    publicCommentId = (success(publicComment, 201) as any).id;
    const privateComment = await author1.post('/api/collections/blog_comments/records', {
      post: privatePostId, content: 'private comment', author: author1Id,
    });
    privateCommentId = (success(privateComment, 201) as any).id;

    const publicComments = await anonymous.get('/api/collections/blog_comments/records');
    expect((paged(publicComments, 200, 1) as any[]).map((comment) => comment.id)).toEqual([publicCommentId]);
    const privateByOutsider = await author2.get('/api/collections/blog_comments/records');
    expect((paged(privateByOutsider, 200, 1) as any[]).map((comment) => comment.id)).toEqual([publicCommentId]);
    const privateByAuthor = await author1.get('/api/collections/blog_comments/records');
    expect((paged(privateByAuthor, 200, 2) as any[]).map((comment) => comment.id)).toEqual(expect.arrayContaining([publicCommentId, privateCommentId]));

    const outsiderPrivateComment = await author2.post('/api/collections/blog_comments/records', {
      post: privatePostId, content: 'not allowed', author: author2Id,
    });
    failure(outsiderPrivateComment, 403, 'HB_FORBIDDEN');
  });

  it('lets administrators bypass business rules but not soft-delete protection', async () => {
    const adminList = await admin.get('/api/collections/blog_posts/records');
    expect((paged(adminList, 200, 2) as any[]).length).toBe(2);
    const attributedPrivateComment = await admin.post('/api/collections/blog_comments/records', {
      post: privatePostId, content: 'admin-created for author two', author: author2Id,
    });
    attributedPrivateCommentId = (success(attributedPrivateComment, 201) as any).id;
    const commentAuthorView = await author2.get(`/api/collections/blog_comments/records/${encodeURIComponent(attributedPrivateCommentId)}`);
    expect((success(commentAuthorView, 200) as any).id).toBe(attributedPrivateCommentId);
    const anonymousView = await anonymous.get(`/api/collections/blog_comments/records/${encodeURIComponent(attributedPrivateCommentId)}`);
    failure(anonymousView, 404, 'HB_NOT_FOUND');
    const adminUpdate = await admin.patch(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}`, { author: author2Id, title: 'Moderated' });
    const updated = success(adminUpdate, 200) as any;
    expect(updated.author).toBe(author2Id);
    const adminDeleteComment = await admin.delete(`/api/collections/blog_comments/records/${encodeURIComponent(privateCommentId)}`);
    success(adminDeleteComment, 200);
  });

  it('hides soft-deleted records and refuses repeated CRUD operations', async () => {
    const deletePost = await author1.delete(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`);
    success(deletePost, 200);
    const getDeleted = await author1.get(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`);
    failure(getDeleted, 404, 'HB_NOT_FOUND');
    const deleteAgain = await author1.delete(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`);
    failure(deleteAgain, 403, 'HB_FORBIDDEN');
    const patch = await author1.patch(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`, { content: 'too late' });
    failure(patch, 403, 'HB_FORBIDDEN');
    const list = await author1.get('/api/collections/blog_posts/records');
    expect((paged(list, 200, 1) as any[]).map((post) => post.id)).toEqual([publicPostId]);
  });

  it('can start and stop a fresh server on another dynamic port', async () => {
    const first = serverUrl();
    await stopServer();
    const second = await startServer();
    expect(second).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/);
    expect((await fetch(`${second}/api-doc/openapi.json`)).status).toBe(200);
    expect(first).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/);
  });
});
