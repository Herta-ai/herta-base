import { HertaBaseClient, isHertaError } from '@hb/sdk'
import { HertaBaseAdminClient } from '@hb/sdk/admin'
import type { BlogPost, BlogComment, BlogUser } from '../types/blog'

// 获取有效 Base URL：浏览器环境下默认使用 window.location.origin，支持 VITE_API_BASE_URL 覆盖
export function resolveApiBaseUrl(): string {
  const custom = (import.meta as any).env?.VITE_API_BASE_URL
  if (custom && custom !== '/') {
    return custom
  }
  if (typeof window !== 'undefined' && window.location?.origin) {
    return window.location.origin
  }
  return 'http://127.0.0.1:8080'
}

export const API_BASE_URL = resolveApiBaseUrl()

// 初始化单例客户端
export const hb = new HertaBaseClient({
  baseUrl: API_BASE_URL,
})

export { isHertaError, HertaBaseAdminClient }

/**
 * 获取文章集合客户端
 */
export function getPostsCollection() {
  return hb.collection<BlogPost, Partial<BlogPost>, Partial<BlogPost>>('blog_posts')
}

/**
 * 获取评论集合客户端
 */
export function getCommentsCollection() {
  return hb.collection<BlogComment, Partial<BlogComment>, Partial<BlogComment>>('blog_comments')
}

/**
 * 获取博客用户鉴权客户端
 */
export function getAuthClient() {
  return hb.auth.forCollection('blog_users')
}

/**
 * 检查 HertaBase 数据库与 blog 集合是否已经初始化完毕
 */
export async function isDatabaseInitialized(): Promise<boolean> {
  try {
    const postsCol = getPostsCollection()
    // 尝试拉取文章集合（如果集合不存在，服务端会返回 404 错误）
    await postsCol.list({ perPage: 1 })
    return true
  } catch (err: any) {
    return false
  }
}

/**
 * 检查 HertaBase 服务端连接状态
 */
export async function checkServerStatus(): Promise<{
  connected: boolean
  hasCollections: boolean
  message: string
}> {
  try {
    const res = await fetch(`${API_BASE_URL.replace(/\/$/, '')}/api-doc/openapi.json`, {
      method: 'GET',
      headers: { Accept: 'application/json' },
    })

    if (!res.ok && res.status !== 404) {
      return {
        connected: false,
        hasCollections: false,
        message: `服务器响应异常: ${res.status}`,
      }
    }

    const ready = await isDatabaseInitialized()
    if (ready) {
      return {
        connected: true,
        hasCollections: true,
        message: 'HertaBase 服务正常，博客数据库已初始化完毕',
      }
    } else {
      return {
        connected: true,
        hasCollections: false,
        message: 'HertaBase 连接成功，需要进行数据库与超管账号初始化',
      }
    }
  } catch (err: any) {
    return {
      connected: false,
      hasCollections: false,
      message: err?.message || '无法连接到 HertaBase 服务',
    }
  }
}

/**
 * 执行完整的后端数据库集合初始化、超管账号创建与 HelloWorld 文章发布
 */
export async function performDatabaseInitialization(params: {
  hbAdminEmail: string
  hbAdminPassword: string
  blogAdminEmail: string
  blogAdminPassword: string
  blogAdminName: string
  blogAdminBio?: string
  onProgress?: (step: string) => void
}): Promise<{
  superAdminUser: BlogUser
  helloWorldPost: BlogPost
}> {
  const { onProgress } = params

  // 1. 实例化 Admin SDK 并使用 HertaBase 后端管理员账号登录
  onProgress?.('正在验证 HertaBase 后端管理员账号...')
  const adminClient = new HertaBaseAdminClient({ baseUrl: API_BASE_URL })
  await adminClient.auth.login({
    email: params.hbAdminEmail.trim(),
    password: params.hbAdminPassword,
  })

  // 2. 创建 blog_users 鉴权集合
  onProgress?.('正在创建用户鉴权集合 (blog_users)...')
  try {
    await adminClient.collections.create({
      name: 'blog_users',
      type: 'auth',
      schema_mode: 'schema-less',
      rules: {
        list: true,
        view: true,
        create: true,
        update: true,
        delete: true,
      },
    })
  } catch (err: any) {
    console.warn('blog_users 集合可能已存在:', err?.message)
  }

  // 3. 创建 blog_posts 文章集合与 API 规则
  onProgress?.('正在创建文章集合 (blog_posts) 与行级权限规则...')
  try {
    await adminClient.collections.create({
      name: 'blog_posts',
      type: 'base',
      schema_mode: 'schema-less',
      fields: [
        { name: 'title', type: 'text', required: true },
        { name: 'slug', type: 'text' },
        { name: 'content', type: 'text' },
        { name: 'excerpt', type: 'text' },
        { name: 'cover_image', type: 'text' },
        { name: 'is_public', type: 'bool', required: true },
        { name: 'featured', type: 'bool' },
        { name: 'category', type: 'text' },
        { name: 'tags', type: 'json' },
        { name: 'views', type: 'number' },
        { name: 'likes', type: 'number' },
        {
          name: 'author',
          type: 'relation',
          required: true,
          options: { collection: 'blog_users', maxSelect: 1 },
        },
      ],
      rules: {
        list: 'is_public = true OR author = $auth.record',
        view: 'is_public = true OR author = $auth.record',
        create: '$record.author = $auth.record',
        update: 'author = $auth.record',
        delete: 'author = $auth.record',
      },
    })
  } catch (err: any) {
    console.warn('blog_posts 集合可能已存在:', err?.message)
  }

  // 4. 创建 blog_comments 评论集合
  onProgress?.('正在创建评论集合 (blog_comments)...')
  try {
    await adminClient.collections.create({
      name: 'blog_comments',
      type: 'base',
      schema_mode: 'schema-less',
      fields: [
        {
          name: 'post',
          type: 'relation',
          required: true,
          options: { collection: 'blog_posts', maxSelect: 1 },
        },
        { name: 'content', type: 'text', required: true },
        {
          name: 'author',
          type: 'relation',
          options: { collection: 'blog_users', maxSelect: 1 },
        },
        { name: 'author_name', type: 'text' },
        { name: 'author_email', type: 'text' },
      ],
      rules: {
        list: true,
        view: true,
        create: true,
        update: 'author = $auth.record',
        delete: 'author = $auth.record',
      },
    })
  } catch (err: any) {
    console.warn('blog_comments 集合可能已存在:', err?.message)
  }

  // 5. 注册 Blog 超级管理员账号
  onProgress?.('正在注册并授权博客超级管理员账号...')
  const auth = hb.auth.forCollection('blog_users')
  let session
  try {
    session = await auth.register<{ displayName: string; role: string; bio: string }>({
      email: params.blogAdminEmail.trim(),
      password: params.blogAdminPassword,
      profile: {
        displayName: params.blogAdminName.trim() || '站长',
        role: '站长 / 超级管理员',
        bio: params.blogAdminBio?.trim() || '博客超级管理员，致力于记录技术演进与思考。',
      },
    })
  } catch (regErr: any) {
    // 若已存在，则尝试直接登录
    session = await auth.login<{ displayName: string; role: string; bio: string }>({
      email: params.blogAdminEmail.trim(),
      password: params.blogAdminPassword,
    })
  }

  // 6. 使用该超管账号发布 HelloWorld 文章
  onProgress?.('正在初始化并发布首篇 HelloWorld 欢迎文章...')
  const postsCol = hb.collection<BlogPost>('blog_posts')
  const now = new Date().toISOString()
  const helloWorldPost = await postsCol.create({
    title: '世界，你好！欢迎使用 HertaBlog',
    slug: 'hello-world-welcome-to-hertablog',
    content: `# 世界，你好！欢迎使用 HertaBlog

恭喜！您的 **HertaBase** 后端数据底座与 **HertaBlog** 前端系统已全部初始化成功！

---

## 🌟 核心特性概览

1. **Rust 强劲驱动**：基于 HertaBase 单二进制高性能 BaaS 引擎，自带内存安全与行级 API Rules 权限控制。
2. **WordPress 优雅排版**：采用中文字体栈、代码语法高亮、自动生成大纲目录（TOC）与实时读者互动。
3. **Gutenberg 写作工作室**：双栏即时预览 Markdown 编辑器，配齐属性面板（Slug、分类、标签、封面与公开/草稿切换）。
4. **前后端一体化部署**：API Base URL 默认为 \`/\`，构建后可一键使用 HertaBase 前端静态托管功能上线。

---

## 🚀 开启创作之旅

您现在已经登录为超级管理员，可以随时点击右上角的 **“写文章”** 或进入 **“文章管理后台”** 开始书写您的技术文章。

\`\`\`ts
// 祝您的技术博客之旅充满乐趣与收获！
console.log("Hello, HertaBase World!");
\`\`\`
`,
    excerpt: '恭喜！您的 HertaBase 后端数据底座与 HertaBlog 前端系统已全部初始化成功！欢迎开启您的技术创作之旅。',
    cover_image: 'https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=1200&auto=format&fit=crop&q=80',
    is_public: true,
    featured: true,
    category: '架构设计',
    tags: ['HertaBase', 'HelloWorld', '架构设计'],
    views: 1,
    likes: 1,
    author: session.user.id,
    created_at: now,
    updated_at: now,
  } as any)

  const superAdminUser: BlogUser = {
    id: session.user.id,
    email: session.user.email,
    displayName: session.user.displayName || params.blogAdminName,
    role: '站长 / 超级管理员',
    bio: params.blogAdminBio,
  }

  return {
    superAdminUser,
    helloWorldPost,
  }
}
