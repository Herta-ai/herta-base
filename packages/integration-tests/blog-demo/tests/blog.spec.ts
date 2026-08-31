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
let superUser: AxiosInstance;
let author1: AxiosInstance;
let author2: AxiosInstance;
let author3: AxiosInstance;
let anonymous: AxiosInstance;
let superUserId = '';
let author1Id = '';
let author2Id = '';
let author3Id = '';
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

describe.sequential('博客系统契约集成测试', () => {
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
        list: 'is_public = true OR author = $auth.record OR $auth.role = "admin"',
        view: 'is_public = true OR author = $auth.record OR $auth.role = "admin"',
        create: '$record.author = $auth.record OR $auth.role = "admin"',
        update: '$auth.role = "admin" OR (author = $auth.record AND ($request.body.author IS NONE OR $request.body.author = $auth.id))',
        delete: '$auth.role = "admin" OR author = $auth.record',
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
        list: 'post.is_public = true OR post.author = $auth.record OR author = $auth.record OR $auth.role = "admin"',
        view: 'post.is_public = true OR post.author = $auth.record OR author = $auth.record OR $auth.role = "admin"',
        create: '$record.author = $auth.record AND ($record.post.is_public = true OR $record.post.author = $auth.record OR $auth.role = "admin")',
        update: 'author = $auth.record OR $auth.role = "admin"',
        delete: '$auth.role = "admin" OR author = $auth.record OR post.author = $auth.record',
      },
    });
    success(comments, 201);
  });

  afterAll(async () => {
    await stopServer();
  });

  it('用户注册与登录认证 (ID与JWT完整性)', async () => {
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

    const registered3 = await anonymous.post('/api/auth/blog_users/register', {
      email: 'author3@example.com', password: PASSWORD,
    });
    const auth3 = success(registered3, 201) as any;
    author3Id = auth3.user.id;
    author3 = withToken(auth3.accessToken);

    // 注册超级用户并由管理员写入 role = 'admin'
    const registeredSuper = await anonymous.post('/api/auth/blog_users/register', {
      email: 'super@example.com', password: PASSWORD, displayName: 'Super Admin',
    });
    const superAuth = success(registeredSuper, 201) as any;
    superUserId = superAuth.user.id;
    const rolePatch = await admin.patch(`/api/collections/blog_users/records/${encodeURIComponent(superUserId)}`, { role: 'admin' });
    success(rolePatch, 200);

    const superLogin = await anonymous.post('/api/auth/blog_users/login', {
      email: 'super@example.com', password: PASSWORD,
    });
    const superLoggedIn = success(superLogin, 200) as any;
    expect(jwtPayload(superLoggedIn.accessToken).role).toBe('admin');
    superUser = withToken(superLoggedIn.accessToken);
  });

  it('文章关联创建与无模式字段保留', async () => {
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

  it('文章详情查询与关联展开 (Expand)', async () => {
    const full = await author1.get(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}?expand=author`);
    const fullPost = success(full, 200) as any;
    expect(fullPost.id).toBe(publicPostId);
    expect(fullPost.author).toBe(author1Id);
    expect(fullPost.expand.author.id).toBe(author1Id);

    const key = publicPostId.slice('blog_posts:'.length);
    const bare = await author1.get(`/api/collections/blog_posts/records/${key}`);
    expect((success(bare, 200) as any).id).toBe(publicPostId);

    const wrongCollection = await author1.get(`/api/collections/blog_posts/records/${encodeURIComponent(`other:${key}`)}`);
    failure(wrongCollection, 404, 'HB_RECORD_NOT_FOUND');

    for (const relation of ['author1', 'blog_users:', 'not a record', 'other:abc']) {
      const invalid = await author1.post('/api/collections/blog_posts/records', {
        title: 'invalid', content: '', is_public: true, author: relation,
      });
      failure(invalid, 400, 'HB_VALIDATION_ERROR');
    }
  });

  it('文章公开/私有可见性与归属保护', async () => {
    const anonymousPosts = await anonymous.get('/api/collections/blog_posts/records');
    expect((paged(anonymousPosts, 200, 1) as any[]).map((post) => post.id)).toEqual([publicPostId]);
    const author1Posts = await author1.get('/api/collections/blog_posts/records');
    expect((paged(author1Posts, 200, 2) as any[]).map((post) => post.id)).toEqual(expect.arrayContaining([publicPostId, privatePostId]));
    const author2Posts = await author2.get('/api/collections/blog_posts/records');
    expect((paged(author2Posts, 200, 1) as any[]).map((post) => post.id)).toEqual([publicPostId]);

    const outsiderGet = await author2.get(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`);
    failure(outsiderGet, 404, 'HB_RECORD_NOT_FOUND');
    const transfer = await author1.patch(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}`, { author: author2Id });
    failure(transfer, 403, 'HB_FORBIDDEN');
    const update = await author2.patch(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}`, { content: 'hacked' });
    failure(update, 403, 'HB_FORBIDDEN');
    const remove = await author2.delete(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}`);
    failure(remove, 403, 'HB_FORBIDDEN');
  });

  it('评论发布与可见性行级规则', async () => {
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

  it('系统超管越权与软删除保护', async () => {
    const adminList = await admin.get('/api/collections/blog_posts/records');
    expect((paged(adminList, 200, 2) as any[]).length).toBe(2);
    const attributedPrivateComment = await admin.post('/api/collections/blog_comments/records', {
      post: privatePostId, content: 'admin-created for author two', author: author2Id,
    });
    attributedPrivateCommentId = (success(attributedPrivateComment, 201) as any).id;
    const commentAuthorView = await author2.get(`/api/collections/blog_comments/records/${encodeURIComponent(attributedPrivateCommentId)}`);
    expect((success(commentAuthorView, 200) as any).id).toBe(attributedPrivateCommentId);
    const anonymousView = await anonymous.get(`/api/collections/blog_comments/records/${encodeURIComponent(attributedPrivateCommentId)}`);
    failure(anonymousView, 404, 'HB_RECORD_NOT_FOUND');
    const adminUpdate = await admin.patch(`/api/collections/blog_posts/records/${encodeURIComponent(publicPostId)}`, { author: author2Id, title: 'Moderated' });
    const updated = success(adminUpdate, 200) as any;
    expect(updated.author).toBe(author2Id);
    const adminDeleteComment = await admin.delete(`/api/collections/blog_comments/records/${encodeURIComponent(privateCommentId)}`);
    success(adminDeleteComment, 200);
  });

  it('软删除记录隐藏与重复操作防护', async () => {
    const deletePost = await author1.delete(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`);
    success(deletePost, 200);
    const getDeleted = await author1.get(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`);
    failure(getDeleted, 404, 'HB_RECORD_NOT_FOUND');
    const deleteAgain = await author1.delete(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`);
    failure(deleteAgain, 404, 'HB_RECORD_NOT_FOUND');
    const patch = await author1.patch(`/api/collections/blog_posts/records/${encodeURIComponent(privatePostId)}`, { content: 'too late' });
    failure(patch, 404, 'HB_RECORD_NOT_FOUND');
    const list = await author1.get('/api/collections/blog_posts/records');
    expect((paged(list, 200, 1) as any[]).map((post) => post.id)).toEqual([publicPostId]);
  });

  it('文章删除权限 (超管删全站，创作者仅删自己)', async () => {
    // author1 创建一篇文章
    const created1 = await author1.post('/api/collections/blog_posts/records', {
      title: 'Author1 Post to Delete', content: 'content', is_public: true, author: author1Id,
    });
    const post1 = success(created1, 201) as any;

    // author2（普通用户，非作者）尝试删除 author1 的文章 -> 拒绝 403
    const outsiderDelete = await author2.delete(`/api/collections/blog_posts/records/${encodeURIComponent(post1.id)}`);
    failure(outsiderDelete, 403, 'HB_FORBIDDEN');

    // superUser（超级用户 role='admin'）删除 author1 的文章 -> 成功 200
    const superDelete = await superUser.delete(`/api/collections/blog_posts/records/${encodeURIComponent(post1.id)}`);
    success(superDelete, 200);

    // author2 创建文章，author2 本人删除 -> 成功 200
    const created2 = await author2.post('/api/collections/blog_posts/records', {
      title: 'Author2 Post', content: 'content', is_public: true, author: author2Id,
    });
    const post2 = success(created2, 201) as any;
    const author2Delete = await author2.delete(`/api/collections/blog_posts/records/${encodeURIComponent(post2.id)}`);
    success(author2Delete, 200);
  });

  it('评论删除权限 (超管/博主/评论者/访客多维校验)', async () => {
    // author1 创建一篇文章
    const postRes = await author1.post('/api/collections/blog_posts/records', {
      title: 'Post For Comment Permission Tests', content: 'content', is_public: true, author: author1Id,
    });
    const testPost = success(postRes, 201) as any;

    // author2 在 author1 的文章下发表评论 1
    const c1Res = await author2.post('/api/collections/blog_comments/records', {
      post: testPost.id, content: 'Comment by author2', author: author2Id,
    });
    const comment1 = success(c1Res, 201) as any;

    // author3 在 author1 的文章下发表评论 2 与 评论 3
    const c2Res = await author3.post('/api/collections/blog_comments/records', {
      post: testPost.id, content: 'Comment 2 by author3', author: author3Id,
    });
    const comment2 = success(c2Res, 201) as any;

    const c3Res = await author3.post('/api/collections/blog_comments/records', {
      post: testPost.id, content: 'Comment 3 by author3', author: author3Id,
    });
    const comment3 = success(c3Res, 201) as any;

    // 4. 其余人员无权删除：author2 尝试删除 author3 在 author1 文章下的评论 2 -> 拒绝 403
    const strangerDelete = await author2.delete(`/api/collections/blog_comments/records/${encodeURIComponent(comment2.id)}`);
    failure(strangerDelete, 403, 'HB_FORBIDDEN');

    // 3. 评论作者可删除自己在任何文章下发表的评论：author2 删除自己的评论 1 -> 成功 200
    const commentAuthorDelete = await author2.delete(`/api/collections/blog_comments/records/${encodeURIComponent(comment1.id)}`);
    success(commentAuthorDelete, 200);

    // 2. 文章作者可删除自己文章下的所有评论：author1 删除 author3 发表的评论 2 -> 成功 200
    const postAuthorDelete = await author1.delete(`/api/collections/blog_comments/records/${encodeURIComponent(comment2.id)}`);
    success(postAuthorDelete, 200);

    // 1. 超级用户可删除全站任意评论：superUser 删除 author3 发表的评论 3 -> 成功 200
    const superDeleteComment = await superUser.delete(`/api/collections/blog_comments/records/${encodeURIComponent(comment3.id)}`);
    success(superDeleteComment, 200);
  });

  it('动态端口服务启动与停止', async () => {
    const first = serverUrl();
    await stopServer();
    const second = await startServer();
    expect(second).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/);
    expect((await fetch(`${second}/api-doc/openapi.json`)).status).toBe(200);
    expect(first).toMatch(/^http:\/\/127\.0\.0\.1:\d+$/);
  });
});
