import type { BlogPost, BlogCategory, BlogComment, BlogUser } from '../types/blog'

export const SEED_USERS: BlogUser[] = [
  {
    id: 'blog_users:admin',
    email: 'admin@herta.ai',
    displayName: '黑塔空间站主管',
    avatar: 'https://images.unsplash.com/photo-1534528741775-53994a69daeb?w=150&auto=format&fit=crop&q=80',
    bio: '专注于现代分布式数据库、Rust 高性能架构与 AI Agent 全栈生态构建。',
    role: '站长 / 架构师',
    website: 'https://herta.ai',
  },
  {
    id: 'blog_users:rustacean',
    email: 'rust@herta.ai',
    displayName: '星际极客',
    avatar: 'https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?w=150&auto=format&fit=crop&q=80',
    bio: '热爱 Rust、Salvo 框架与极致内存效率探索者。',
    role: '资深后端研发',
  },
]

export const SEED_CATEGORIES: BlogCategory[] = [
  {
    id: 'arch',
    name: '架构设计',
    slug: 'arch',
    description: '分布式系统、存储引擎、高并发与高可用服务架构实践',
    icon: 'Layers',
    color: '#3b82f6',
    count: 3,
  },
  {
    id: 'ai',
    name: '人工智能',
    slug: 'ai',
    description: '大语言模型、AI Agent 协同、上下文工程与向量检索',
    icon: 'Sparkles',
    color: '#8b5cf6',
    count: 2,
  },
  {
    id: 'rust',
    name: 'Rust 实战',
    slug: 'rust',
    description: '异步编程、内存安全、零成本抽象与原生性能剖析',
    icon: 'Cpu',
    color: '#f97316',
    count: 2,
  },
  {
    id: 'frontend',
    name: '前端工程',
    slug: 'frontend',
    description: 'Vue 3、Vite 8、TypeScript、UnoCSS 与极速用户体验',
    icon: 'Palette',
    color: '#10b981',
    count: 2,
  },
  {
    id: 'thoughts',
    name: '思考随笔',
    slug: 'thoughts',
    description: '技术人的成长复盘、开源心路与数字游民探索',
    icon: 'BookOpen',
    color: '#ec4899',
    count: 1,
  },
]

export const SEED_POSTS: BlogPost[] = [
  {
    id: 'blog_posts:hb-intro',
    title: '深入理解 HertaBase：下一代轻量级自托管 BaaS 后端架构设计',
    slug: 'deep-dive-into-hertabase-architecture',
    excerpt: '探讨 HertaBase 如何利用 Rust 的极致性能与内存安全，提供集数据存储、用户鉴权、动态 API Rules 与静态前端部署于一体的现代化开发底座。',
    cover_image: 'https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?w=1200&auto=format&fit=crop&q=80',
    is_public: true,
    featured: true,
    category: '架构设计',
    tags: ['HertaBase', 'Rust', '架构设计', '数据库', 'BaaS'],
    views: 3420,
    likes: 128,
    author: 'blog_users:admin',
    created_at: '2026-03-01T10:30:00Z',
    updated_at: '2026-03-02T14:15:00Z',
    content: `
# 深入理解 HertaBase：下一代轻量级自托管 BaaS 后端架构设计

在现代全栈应用开发中，开发者常常面临在重量级云服务（如 Firebase、Supabase）与从零手写增删改查之间权衡的困境。**HertaBase** 正是在这一背景下诞生——它是一款基于 **Rust + Salvo** 构建的高性能、自托管一站式后端服务平台。

> “让全栈开发者专注于产品业务逻辑，把鉴权、存储、实时通信与前端部署交给极致高效的 Rust 单二进制执行体。”

---

## 核心架构概览

HertaBase 采用模块化分层架构，具备以下三大核心能力：

1. **动态 Schema 与混合集合管理**：支持严格 Schema 与 Schema-less 混合模式，字段变更无需复杂迁移。
2. **表达式驱动的 API Rules**：通过类似 \`is_public = true OR author = $auth.record\` 的安全规则，细粒度控制行级数据可见性。
3. **原生 Web 静态资源托管**：内置版本回滚、多路由别名与 SPA Fallback 机制。

\`\`\`mermaid
graph LR
    Client[Web/App 客户端] -->|SDK 请求| Server[HertaBase 核心服务]
    Server --> Auth[用户鉴权与 JWT]
    Server --> Engine[数据存储引擎]
    Server --> Hosting[静态 Web 托管]
    Engine --> Storage[(本地 RocksDB / Memory)]
\`\`\`

---

## 极简的 SDK 集成体验

通过官方 TypeScript SDK \`@hb/sdk\`，前端仅需数行代码即可完成类型安全的数据查询与过滤：

\`\`\`ts
import { HertaBaseClient } from '@hb/sdk'

interface Post {
  id: string
  title: string
  content: string
  is_public: boolean
}

// 初始化客户端，部署在 HertaBase 上时 baseUrl 直接使用 '/'
const hb = new HertaBaseClient({ baseUrl: '/' })

// 查询公开文章列表并展开作者详情
const posts = await hb.collection<Post>('blog_posts').list({
  filter: 'is_public = true',
  expand: 'author',
  sort: '-created_at',
})

console.log('文章列表:', posts.items)
\`\`\`

---

## 性能基准与内存占用

在标准 2 核心 4GB 内存机器上测试，HertaBase 的基准表现如下：

| 指标 | HertaBase (Rust) | 传统 Node.js BaaS |
|---|---|---|
| **冷启动时间** | < 15 ms | > 1200 ms |
| **空载内存占用** | ~ 18 MB | ~ 140 MB |
| **单机并发 QPS** | 35,000+ | 6,200 |
| **P99 响应延迟** | 0.8 ms | 12.4 ms |

---

## 总结与未来展望

HertaBase 不仅提供了开箱即用的后端能力，其极小的系统开销更使得单机低成本部署成为可能。下一阶段将引入更强大的实时协同通信与插件机制，敬请期待！
    `,
  },
  {
    id: 'blog_posts:vue3-vite8',
    title: 'Vue 3.5 + Vite 8 全新特性解析与企业级现代博客实战',
    slug: 'vue3-vite8-modern-blog-engineering',
    excerpt: '全面拆解 Vue 3.5 响应式系统重构、Props 解构与 Vite 8 极速构建优化，分享如何构建一套具备极致流畅体验的现代化 WordPress 级前端。',
    cover_image: 'https://images.unsplash.com/photo-1555066931-4365d14bab8c?w=1200&auto=format&fit=crop&q=80',
    is_public: true,
    featured: true,
    category: '前端工程',
    tags: ['Vue3', 'Vite', 'TypeScript', 'UnoCSS', '前端工程'],
    views: 2890,
    likes: 95,
    author: 'blog_users:admin',
    created_at: '2026-03-03T16:00:00Z',
    updated_at: '2026-03-03T16:00:00Z',
    content: `
# Vue 3.5 + Vite 8 全新特性解析与企业级现代博客实战

随着 Vue 3.5 与 Vite 8 的正式发布，前端开发体验与运行时性能迈入了一个新的里程碑。本文将结合本项目（**HertaBlog**）的开发实践，探讨最新特性的应用。

---

## Vue 3.5 核心提升

### 1. 响应式系统重构
Vue 3.5 重构了内部响应式引擎，内存占用减少了 **56%**，在频繁触发的数组变异与大型对象追踪场景下提升尤为显著。

### 2. 响应式 Props 解构正式稳定
告别繁琐的 \`toRefs\` 或计算属性包装：

\`\`\`vue
<script setup lang="ts">
const { title, viewCount = 0 } = defineProps<{
  title: string
  viewCount?: number
}>()

// title 和 viewCount 保持完整的响应式追踪！
</script>
\`\`\`

### 3. useTemplateRef 简洁类型安全
\`\`\`vue
<script setup lang="ts">
import { useTemplateRef, onMounted } from 'vue'

const inputRef = useTemplateRef<HTMLInputElement>('searchInput')

onMounted(() => {
  inputRef.value?.focus()
})
</script>
\`\`\`

---

## 为什么选择 UnoCSS 作为样式底座？

UnoCSS 作为即时原子化 CSS 引擎，具备以下优势：

- **极速热更新**：按需生成，零冗余 CSS。
- **丰富的预设集成**：\`presetWind3\` 提供 Tailwind v3 完整类名，\`presetTypography\` 带来沉浸式 Markdown 排版。
- **纯 CSS 图标支持**：借助 \`presetIcons\`，无需引入庞大字体图标包。

\`\`\`ts
// uno.config.ts 核心配置
import { defineConfig, presetWind3, presetTypography } from 'unocss'

export default defineConfig({
  presets: [
    presetWind3(),
    presetTypography(),
  ],
})
\`\`\`

---

## 打造极致的阅读排版

在博客详情页中，我们使用 \`prose prose-zinc dark:prose-invert\` 配合自定义中文字体栈，保证在中英文混排时的行高、字间距与段落舒适度。
    `,
  },
  {
    id: 'blog_posts:ai-agent-era',
    title: 'AI Agent 驱动的多智能体协作系统：从提示词到自律工作流',
    slug: 'ai-agents-autonomous-workflow-evolution',
    excerpt: '从单一 LLM 对话走向自主规划、工具调用、消息总线与多智能体分工，探索现代 Agentic 系统的演进路径与工程实践。',
    cover_image: 'https://images.unsplash.com/photo-1677442136019-21780efad99a?w=1200&auto=format&fit=crop&q=80',
    is_public: true,
    featured: false,
    category: '人工智能',
    tags: ['AI', 'LLM', 'Agent', '自动化', '架构设计'],
    views: 1980,
    likes: 82,
    author: 'blog_users:admin',
    created_at: '2026-03-05T09:20:00Z',
    content: `
# AI Agent 驱动的多智能体协作系统：从提示词到自律工作流

2026 年，大语言模型的竞争焦点已经从单纯的参数规模转向了**端到端复杂任务解决能力**。AI Agent 正在深刻改变软件开发与自动化流水线。

---

## 智能体协作的三个阶段

1. **单轮指令提示（Prompt-based）**：依赖上下文一次性生成结果，无法反思与纠错。
2. **ReAct 工具链（Reasoning + Action）**：引入环境感知与工具调用，支持循环迭代。
3. **多智能体自治协作（Multi-Agent Swarm）**：由架构规划师、研究员、编码员、质检员等多角色智能体协同完成大型项目。

\`\`\`
[用户目标] 
    │
    ▼
[Planner 规划智能体] ──制定任务清单──> [Subagent 1: 编码智能体]
    │                                     │
    ▼                                     ▼
[Reviewer 质检智能体] <───代码产出及测试验证───┘
\`\`\`

---

## 关键技术：记忆系统与工具执行

- **短时记忆**：动态上下文压缩与分层滑动窗口。
- **长时记忆**：结构化知识图谱与语义向量数据库。
- **工具安全沙箱**：基于权限隔离的命令与文件系统操作。

未来每一个开发者都将是一个“智能体团队的指挥官”。
    `,
  },
  {
    id: 'blog_posts:rust-async-mastery',
    title: 'Rust 异步运行时深度剖析：Tokio 调度器与零成本并发模型',
    slug: 'rust-tokio-runtime-internals',
    excerpt: '深入分析 Rust 状态机编译、Pinning 语义、Tokio 工作窃取（Work-Stealing）调度机制，以及高并发网络服务的调优策略。',
    cover_image: 'https://images.unsplash.com/photo-1526374965328-7f61d4dc18c5?w=1200&auto=format&fit=crop&q=80',
    is_public: true,
    featured: false,
    category: 'Rust 实战',
    tags: ['Rust', 'Tokio', '并发编程', '性能优化'],
    views: 1640,
    likes: 74,
    author: 'blog_users:rustacean',
    created_at: '2026-03-06T11:45:00Z',
    content: `
# Rust 异步运行时深度剖析：Tokio 调度器与零成本并发模型

Rust 的异步模型具有“无运行时开销（Zero-cost Abstractions）”与“惰性执行（Lazy Futures）”的鲜明特点。

---

## Future Trait 的本质

在 Rust 中，\`Future\` 本质上是一个拉取式（Pull-based）状态机：

\`\`\`rust
pub trait Future {
    type Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}
\`\`\`

只有当执行器（如 Tokio）调用 \`poll\` 时，状态机才会推进；当遇到 I/O 阻塞时，通过 \`Waker\` 注册唤醒回调，避免了不必要的 CPU 轮询开销。

---

## Tokio 工作窃取调度策略

- 每个工作线程拥有独立的本地队列（Local Run Queue）。
- 当本地任务耗尽时，主动从全局队列或其他线程的队列末尾“窃取”一半任务，确保多核心负载均衡。

这种设计使得在超高并发请求下依然保持稳定的 P99 延迟。
    `,
  },
]

export const SEED_COMMENTS: BlogComment[] = [
  {
    id: 'comment-1',
    post: 'blog_posts:hb-intro',
    author_name: '李知远',
    author_email: 'zhiyuan@example.com',
    content: '这篇文章讲得很透彻！HertaBase 单文件二进制部署体验太爽了，配合前端这个博客系统直接秒级上线！',
    created_at: '2026-03-02T16:20:00Z',
  },
  {
    id: 'comment-2',
    post: 'blog_posts:hb-intro',
    author_name: '星际漫步者',
    author_email: 'walker@example.com',
    content: 'API Rules 的语法非常直观，行级权限控制解决了大多数自建系统的鉴权痛点。',
    created_at: '2026-03-02T18:40:00Z',
  },
  {
    id: 'comment-3',
    post: 'blog_posts:vue3-vite8',
    author_name: '前端小师弟',
    author_email: 'fe@example.com',
    content: 'Vue 3.5 的 Props 解构终于原生支持响应式了，再也不用写一堆 toRefs，代码清爽多了。',
    created_at: '2026-03-04T09:10:00Z',
  },
]
