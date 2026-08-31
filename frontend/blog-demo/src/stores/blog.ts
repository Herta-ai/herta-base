import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { BlogPost, BlogCategory, BlogComment, BlogTag, PostFilter } from '../types/blog'
import { getPostsCollection, getCommentsCollection, checkServerStatus, isHertaError } from '../lib/hb'
import { SEED_POSTS, SEED_CATEGORIES, SEED_COMMENTS } from '../lib/seed-data'
import { useThemeStore } from './theme'
import { useAuthStore } from './auth'

function loadInitialPosts(): BlogPost[] {
  if (typeof window === 'undefined') return [...SEED_POSTS]
  const saved = localStorage.getItem('herta_blog_posts_local')
  if (saved) {
    try { return JSON.parse(saved) } catch {}
  }
  return [...SEED_POSTS]
}

function loadInitialComments(): BlogComment[] {
  if (typeof window === 'undefined') return [...SEED_COMMENTS]
  const saved = localStorage.getItem('herta_blog_comments_local')
  if (saved) {
    try { return JSON.parse(saved) } catch {}
  }
  return [...SEED_COMMENTS]
}

export const useBlogStore = defineStore('blog', () => {
  const themeStore = useThemeStore()
  const authStore = useAuthStore()

  // 文章与分类状态
  const posts = ref<BlogPost[]>(loadInitialPosts())
  const currentPost = ref<BlogPost | null>(null)
  const categories = ref<BlogCategory[]>([...SEED_CATEGORIES])
  const comments = ref<BlogComment[]>(loadInitialComments())

  // 点赞记录（本地存储）
  const likedPostIds = ref<string[]>(
    typeof window !== 'undefined'
      ? JSON.parse(localStorage.getItem('herta_liked_posts') || '[]')
      : []
  )

  // 服务端连接状态
  const isServerLive = ref(false)
  const isCollectionsReady = ref(false)
  const loading = ref(false)

  // 筛选与排序参数
  const filter = ref<PostFilter>({
    category: undefined,
    tag: undefined,
    search: '',
    status: 'published',
    sortBy: 'latest',
    page: 1,
    perPage: 6,
  })

  /**
   * 动态计算标签云
   */
  const tags = computed<BlogTag[]>(() => {
    const map = new Map<string, number>()
    posts.value.forEach(p => {
      p.tags?.forEach(t => {
        map.set(t, (map.get(t) || 0) + 1)
      })
    })
    return Array.from(map.entries())
      .map(([name, count]) => ({ name, count }))
      .sort((a, b) => b.count - a.count)
  })

  /**
   * 动态计算分类文章计数
   */
  const computedCategories = computed<BlogCategory[]>(() => {
    return categories.value.map(c => {
      const count = posts.value.filter(p => p.category === c.name && (p.is_public !== false)).length
      return { ...c, count }
    })
  })

  /**
   * 精选/置顶文章
   */
  const featuredPosts = computed<BlogPost[]>(() => {
    return posts.value.filter(p => p.featured && p.is_public !== false)
  })

  /**
   * 根据当前 Filter 过滤后的文章列表
   */
  const filteredPosts = computed<BlogPost[]>(() => {
    let result = [...posts.value]

    // 状态过滤
    if (filter.value.status === 'published') {
      result = result.filter(p => p.is_public !== false)
    } else if (filter.value.status === 'draft') {
      result = result.filter(p => p.is_public === false)
    }

    // 分类过滤
    if (filter.value.category) {
      result = result.filter(p => p.category === filter.value.category)
    }

    // 标签过滤
    if (filter.value.tag) {
      result = result.filter(p => p.tags?.includes(filter.value.tag!))
    }

    // 关键词搜索
    if (filter.value.search?.trim()) {
      const q = filter.value.search.toLowerCase().trim()
      result = result.filter(p =>
        p.title.toLowerCase().includes(q) ||
        p.excerpt?.toLowerCase().includes(q) ||
        p.content.toLowerCase().includes(q) ||
        p.tags?.some(t => t.toLowerCase().includes(q))
      )
    }

    // 排序
    if (filter.value.sortBy === 'latest') {
      result.sort((a, b) => new Date(b.created_at || '').getTime() - new Date(a.created_at || '').getTime())
    } else if (filter.value.sortBy === 'oldest') {
      result.sort((a, b) => new Date(a.created_at || '').getTime() - new Date(b.created_at || '').getTime())
    } else if (filter.value.sortBy === 'views') {
      result.sort((a, b) => (b.views || 0) - (a.views || 0))
    } else if (filter.value.sortBy === 'likes') {
      result.sort((a, b) => (b.likes || 0) - (a.likes || 0))
    }

    return result
  })

  /**
   * 分页数据
   */
  const paginatedPosts = computed<BlogPost[]>(() => {
    const page = filter.value.page || 1
    const perPage = filter.value.perPage || 6
    const start = (page - 1) * perPage
    return filteredPosts.value.slice(start, start + perPage)
  })

  const totalPages = computed(() => {
    const perPage = filter.value.perPage || 6
    return Math.max(1, Math.ceil(filteredPosts.value.length / perPage))
  })

  /**
   * 保存到本地备用存储
   */
  const persistLocal = () => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('herta_blog_posts_local', JSON.stringify(posts.value))
      localStorage.setItem('herta_blog_comments_local', JSON.stringify(comments.value))
    }
  }

  /**
   * 初始化并检查服务端状态与拉取文章
   */
  const init = async () => {
    loading.value = true
    try {
      const status = await checkServerStatus()
      isServerLive.value = status.connected
      isCollectionsReady.value = status.hasCollections

      if (status.connected && status.hasCollections) {
        await fetchRemotePosts()
      }
    } catch {
      // 保持演示数据
    } finally {
      loading.value = false
    }
  }

  /**
   * 从 HertaBase 服务端拉取数据
   */
  const fetchRemotePosts = async () => {
    try {
      const postsCol = getPostsCollection()
      const res = await postsCol.list({
        sort: '-created_at',
        expand: 'author',
        perPage: 50,
      })
      if (res.items && res.items.length > 0) {
        posts.value = res.items
      }
      persistLocal()
    } catch (err) {
      console.warn('拉取服务端文章失败，使用本地/演示缓存:', err)
    }
  }

  /**
   * 根据 Slug 或 ID 获取文章
   */
  const getPostBySlugOrId = async (identifier: string): Promise<BlogPost | null> => {
    // 优先从服务端精确获取
    try {
      const postsCol = getPostsCollection()
      let fetched: BlogPost | null = null
      try {
        fetched = await postsCol.get(identifier, { expand: 'author' })
      } catch {
        // 若以 ID 查询失败，尝试以 slug 检索
        const listRes = await postsCol.list({
          filter: `slug = "${identifier}"`,
          expand: 'author',
          perPage: 1,
        })
        if (listRes.items && listRes.items.length > 0) {
          fetched = listRes.items[0]
        }
      }

      if (fetched) {
        currentPost.value = fetched
        const idx = posts.value.findIndex(p => p.id === fetched!.id)
        if (idx !== -1) {
          posts.value[idx] = fetched
        } else {
          posts.value.unshift(fetched)
        }
        return fetched
      }
    } catch (err) {
      console.warn('从服务端获取文章详情失败:', err)
    }

    // 内存/本地后备查找
    const found = posts.value.find(p => p.id === identifier || p.slug === identifier)
    if (found) {
      found.views = (found.views || 0) + 1
      currentPost.value = found
      return found
    }

    return null
  }

  /**
   * 创建文章（直接调用 HertaBase 后端接口）
   */
  const createPost = async (postData: Partial<BlogPost>): Promise<BlogPost> => {
    const authorId = authStore.user?.id
    if (!authorId) {
      themeStore.openAuth('login')
      themeStore.addToast({ type: 'warning', message: '请先登录创作者账号后再发布文章' })
      throw new Error('请先登录创作者账号')
    }

    const postsCol = getPostsCollection()

    // 构造符合 API Rules 的新建 payload
    const serverPayload = {
      title: postData.title || '无标题文章',
      slug: postData.slug || `post-${Date.now()}`,
      content: postData.content || '',
      excerpt: postData.excerpt || '',
      cover_image: postData.cover_image || 'https://images.unsplash.com/photo-1517694712202-14dd9538aa97?w=1200&auto=format&fit=crop&q=80',
      is_public: postData.is_public !== false,
      featured: !!postData.featured,
      category: postData.category || '架构设计',
      tags: postData.tags || ['技术'],
      views: 1,
      likes: 0,
      author: authorId,
    }

    try {
      const created = await postsCol.create(serverPayload as any)
      posts.value.unshift(created)
      persistLocal()
      themeStore.addToast({ type: 'success', message: '文章已成功发布到 HertaBase！' })
      return created
    } catch (err: any) {
      const msg = isHertaError(err) ? err.message : (err?.message || '发布文章失败，请检查后端集合与权限')
      themeStore.addToast({ type: 'error', message: msg })
      throw err
    }
  }

  /**
   * 更新文章（直接调用 HertaBase 后端接口）
   */
  const updatePost = async (id: string, updateData: Partial<BlogPost>): Promise<boolean> => {
    const postsCol = getPostsCollection()
    const { id: _id, created_at: _ca, updated_at: _ua, deleted_at: _da, ...serverPayload } = updateData as any

    try {
      const updated = await postsCol.update(id, serverPayload as any)
      const idx = posts.value.findIndex(p => p.id === id)
      if (idx !== -1) {
        posts.value[idx] = updated
      }
      if (currentPost.value?.id === id) {
        currentPost.value = updated
      }
      persistLocal()
      themeStore.addToast({ type: 'success', message: '文章已更新并同步到 HertaBase！' })
      return true
    } catch (err: any) {
      const msg = isHertaError(err) ? err.message : (err?.message || '更新文章失败')
      themeStore.addToast({ type: 'error', message: msg })
      throw err
    }
  }

  /**
   * 删除文章（直接调用 HertaBase 后端接口）
   */
  const deletePost = async (id: string): Promise<boolean> => {
    try {
      const postsCol = getPostsCollection()
      await postsCol.delete(id)
      posts.value = posts.value.filter(p => p.id !== id)
      persistLocal()
      themeStore.addToast({ type: 'info', message: '文章已成功删除' })
      return true
    } catch (err: any) {
      const msg = isHertaError(err) ? err.message : (err?.message || '删除文章失败')
      themeStore.addToast({ type: 'error', message: msg })
      throw err
    }
  }

  /**
   * 点赞/取消点赞
   */
  const toggleLike = (postId: string) => {
    const post = posts.value.find(p => p.id === postId)
    if (!post) return

    const index = likedPostIds.value.indexOf(postId)
    if (index > -1) {
      likedPostIds.value.splice(index, 1)
      post.likes = Math.max(0, (post.likes || 1) - 1)
    } else {
      likedPostIds.value.push(postId)
      post.likes = (post.likes || 0) + 1
      themeStore.addToast({ type: 'success', message: '点赞成功，感谢鼓励！' })
    }
    if (typeof window !== 'undefined') {
      localStorage.setItem('herta_liked_posts', JSON.stringify(likedPostIds.value))
    }
    persistLocal()
  }

  /**
   * 获取文章评论
   */
  const getCommentsByPostId = (postId: string): BlogComment[] => {
    return comments.value.filter(c => c.post === postId)
  }

  /**
   * 发表评论
   */
  const addComment = async (commentData: {
    post: string
    content: string
    author_name?: string
    author_email?: string
  }): Promise<BlogComment> => {
    const newComment: BlogComment = {
      id: `comment-${Date.now()}`,
      post: commentData.post,
      content: commentData.content,
      author_name: authStore.user?.displayName || commentData.author_name || '匿名读者',
      author_email: authStore.user?.email || commentData.author_email,
      author: authStore.user?.id,
      created_at: new Date().toISOString(),
    }

    if (isServerLive.value && isCollectionsReady.value) {
      try {
        const commentsCol = getCommentsCollection()
        const { id: _id, created_at: _ca, updated_at: _ua, deleted_at: _da, ...serverPayload } = newComment as any
        const created = await commentsCol.create(serverPayload as any)
        comments.value.unshift(created)
        persistLocal()
        themeStore.addToast({ type: 'success', message: '评论发表成功！' })
        return created
      } catch (err: any) {
        console.warn('评论同步到服务端失败:', err)
      }
    }

    comments.value.unshift(newComment)
    persistLocal()
    themeStore.addToast({ type: 'success', message: '评论发表成功！' })
    return newComment
  }

  /**
   * 删除评论
   */
  const deleteComment = async (commentId: string): Promise<boolean> => {
    if (isServerLive.value && isCollectionsReady.value) {
      try {
        const commentsCol = getCommentsCollection()
        await commentsCol.delete(commentId)
      } catch (err) {
        console.warn('服务端删除评论失败:', err)
      }
    }

    comments.value = comments.value.filter(c => c.id !== commentId)
    persistLocal()
    themeStore.addToast({ type: 'info', message: '评论已删除' })
    return true
  }

  return {
    posts,
    currentPost,
    categories,
    computedCategories,
    tags,
    comments,
    likedPostIds,
    isServerLive,
    isCollectionsReady,
    loading,
    filter,
    featuredPosts,
    filteredPosts,
    paginatedPosts,
    totalPages,
    init,
    fetchRemotePosts,
    getPostBySlugOrId,
    createPost,
    updatePost,
    deletePost,
    toggleLike,
    getCommentsByPostId,
    addComment,
    deleteComment,
  }
})
