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

function loadInitialCategories(): BlogCategory[] {
  if (typeof window === 'undefined') return [...SEED_CATEGORIES]
  const saved = localStorage.getItem('herta_blog_categories_local')
  if (saved) {
    try {
      const parsed = JSON.parse(saved)
      if (Array.isArray(parsed) && parsed.length > 0) return parsed
    } catch {}
  }
  return [...SEED_CATEGORIES]
}

export const useBlogStore = defineStore('blog', () => {
  const themeStore = useThemeStore()
  const authStore = useAuthStore()

  // 文章与分类状态
  const posts = ref<BlogPost[]>(loadInitialPosts())
  const currentPost = ref<BlogPost | null>(null)
  const categories = ref<BlogCategory[]>(loadInitialCategories())
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
   * 动态计算分类文章计数（包含「未设置目录」等文章）
   */
  const computedCategories = computed<BlogCategory[]>(() => {
    const list = categories.value.map(c => {
      const count = posts.value.filter(p => p.category === c.name && (p.is_public !== false)).length
      return { ...c, count }
    })

    // 统计未设置目录或已被删除目录关联的文章
    const uncategorizedCount = posts.value.filter(p => {
      if (!p.is_public) return false
      return p.category === '未设置目录' || (!p.category && !categories.value.some(c => c.name === p.category))
    }).length

    if (uncategorizedCount > 0 && !categories.value.some(c => c.name === '未设置目录')) {
      list.push({
        id: 'cat-uncategorized',
        name: '未设置目录',
        slug: 'uncategorized',
        description: '暂未归属或原所属专栏目录已被删除的文章。',
        color: '#71717a',
        count: uncategorizedCount,
      })
    }

    return list
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
      if (filter.value.category === '未设置目录' || filter.value.category === 'uncategorized') {
        result = result.filter(p => !p.category || p.category === '未设置目录' || p.category === '' || !categories.value.some(c => c.name === p.category))
      } else {
        result = result.filter(p => p.category === filter.value.category)
      }
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
      localStorage.setItem('herta_blog_categories_local', JSON.stringify(categories.value))
    }
  }

  /**
   * 初始化并检查服务端状态与拉取文章和评论
   */
  const init = async () => {
    loading.value = true
    try {
      const status = await checkServerStatus()
      isServerLive.value = status.connected
      isCollectionsReady.value = status.hasCollections

      if (status.connected && status.hasCollections) {
        await Promise.all([fetchRemotePosts(), fetchRemoteComments()])
      }
    } catch {
      // 保持演示数据
    } finally {
      loading.value = false
    }
  }

  /**
   * 从 HertaBase 服务端拉取评论数据
   */
  const fetchRemoteComments = async () => {
    try {
      const commentsCol = getCommentsCollection()
      const res = await commentsCol.list({
        sort: '-created_at',
        perPage: 100,
      })
      if (res.items && res.items.length > 0) {
        comments.value = res.items
      }
      persistLocal()
    } catch (err) {
      console.warn('拉取服务端评论失败，使用本地/演示缓存:', err)
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

        // 异步向服务端上报浏览量 +1
        if (isServerLive.value && isCollectionsReady.value) {
          const nextViews = (fetched.views || 0) + 1
          fetched.views = nextViews
          postsCol.update(fetched.id, { views: nextViews }).catch(() => {})
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
      category: postData.category || '',
      tags: postData.tags || ['技术'],
      views: 1,
      likes: 0,
      author: authorId,
    }

    try {
      const created = await postsCol.create(serverPayload as any)
      const idx = posts.value.findIndex(p => p.id === created.id)
      if (idx === -1) {
        posts.value.unshift(created)
      } else {
        posts.value[idx] = { ...posts.value[idx], ...created }
      }
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
   * 点赞/取消点赞（实时向后端发送真实记录请求）
   */
  const toggleLike = async (postId: string) => {
    const post = posts.value.find(p => p.id === postId)
    if (!post) return

    const index = likedPostIds.value.indexOf(postId)
    let newLikes = post.likes || 0
    if (index > -1) {
      likedPostIds.value.splice(index, 1)
      newLikes = Math.max(0, newLikes - 1)
      post.likes = newLikes
    } else {
      likedPostIds.value.push(postId)
      newLikes = newLikes + 1
      post.likes = newLikes
      themeStore.addToast({ type: 'success', message: '点赞成功，感谢鼓励！' })
    }

    if (currentPost.value?.id === postId) {
      currentPost.value.likes = newLikes
    }

    if (typeof window !== 'undefined') {
      localStorage.setItem('herta_liked_posts', JSON.stringify(likedPostIds.value))
    }
    persistLocal()

    if (isServerLive.value && isCollectionsReady.value) {
      try {
        const postsCol = getPostsCollection()
        await postsCol.update(postId, { likes: newLikes })
      } catch (err: any) {
        console.warn('点赞数据同步到服务端失败:', err)
      }
    }
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
    const authorId = authStore.user?.id
    const payload = {
      post: commentData.post,
      content: commentData.content.trim(),
      author_name: authStore.user?.displayName || commentData.author_name?.trim() || '匿名读者',
      author_email: authStore.user?.email || commentData.author_email?.trim() || '',
      author: authorId || undefined,
    }

    if (isServerLive.value && isCollectionsReady.value) {
      try {
        const commentsCol = getCommentsCollection()
        const created = await commentsCol.create(payload as any)
        const idx = comments.value.findIndex(c => c.id === created.id)
        if (idx === -1) {
          comments.value.unshift(created)
        } else {
          comments.value[idx] = created
        }
        persistLocal()
        themeStore.addToast({ type: 'success', message: '评论发表成功！' })
        return created
      } catch (err: any) {
        const msg = isHertaError(err) ? err.message : (err?.message || '评论同步到服务端失败')
        themeStore.addToast({ type: 'error', message: msg })
        throw err
      }
    }

    // 离线备用
    const localComment: BlogComment = {
      id: `comment-${Date.now()}`,
      post: commentData.post,
      content: payload.content,
      author_name: payload.author_name,
      author_email: payload.author_email,
      author: authorId,
      created_at: new Date().toISOString(),
    }
    comments.value.unshift(localComment)
    persistLocal()
    themeStore.addToast({ type: 'success', message: '评论发表成功（本地离线）！' })
    return localComment
  }

  /**
   * 删除评论（按行级规则权限严格执行服务端删除）
   */
  const deleteComment = async (commentId: string): Promise<boolean> => {
    if (isServerLive.value && isCollectionsReady.value) {
      try {
        const commentsCol = getCommentsCollection()
        await commentsCol.delete(commentId)
      } catch (err: any) {
        const msg = isHertaError(err) ? err.message : (err?.message || '删除评论失败，无权操作该评论')
        themeStore.addToast({ type: 'error', message: msg })
        throw err
      }
    }

    comments.value = comments.value.filter(c => c.id !== commentId)
    persistLocal()
    themeStore.addToast({ type: 'info', message: '评论已成功删除' })
    return true
  }

  /**
   * 创建新专栏目录
   */
  const createCategory = (data: {
    name: string
    slug?: string
    description?: string
    color?: string
    icon?: string
  }): BlogCategory => {
    const trimmedName = data.name.trim()
    if (!trimmedName) {
      themeStore.addToast({ type: 'warning', message: '目录名称不能为空' })
      throw new Error('目录名称不能为空')
    }

    if (categories.value.some(c => c.name.toLowerCase() === trimmedName.toLowerCase())) {
      themeStore.addToast({ type: 'warning', message: '已存在同名目录' })
      throw new Error('已存在同名目录')
    }

    const newCat: BlogCategory = {
      id: `cat-${Date.now()}`,
      name: trimmedName,
      slug: data.slug?.trim() || trimmedName.toLowerCase().replace(/[\s/]+/g, '-'),
      description: data.description?.trim() || '',
      color: data.color || '#10b981',
      icon: data.icon || 'Folder',
    }

    categories.value.push(newCat)
    persistLocal()
    themeStore.addToast({ type: 'success', message: `目录「${trimmedName}」已成功创建` })
    return newCat
  }

  /**
   * 更新专栏目录
   */
  const updateCategory = async (id: string, updateData: Partial<BlogCategory>): Promise<boolean> => {
    const idx = categories.value.findIndex(c => c.id === id)
    if (idx === -1) return false

    const oldName = categories.value[idx].name
    const newName = updateData.name?.trim() || oldName

    // 如果重命名了目录名称，将归属该旧目录的所有文章全部同步更新为新目录名称
    if (newName !== oldName) {
      const affectedPosts = posts.value.filter(p => p.category === oldName)
      affectedPosts.forEach(p => {
        p.category = newName
      })
      if (isServerLive.value && isCollectionsReady.value) {
        const postsCol = getPostsCollection()
        affectedPosts.forEach(p => {
          postsCol.update(p.id, { category: newName }).catch(() => {})
        })
      }
    }

    categories.value[idx] = {
      ...categories.value[idx],
      ...updateData,
      name: newName,
      slug: updateData.slug?.trim() || categories.value[idx].slug,
    }
    persistLocal()
    themeStore.addToast({ type: 'success', message: '目录信息已更新' })
    return true
  }

  /**
   * 删除专栏目录，把管理的文章全部设置成「未设置目录」
   */
  const deleteCategory = async (idOrName: string): Promise<boolean> => {
    const target = categories.value.find(c => c.id === idOrName || c.name === idOrName)
    if (!target) return false

    const catName = target.name
    categories.value = categories.value.filter(c => c.id !== target.id)

    // 关键需求：把管理的文章全部设置成未设置目录
    const affectedPosts = posts.value.filter(p => p.category === catName || p.category === target.slug)
    affectedPosts.forEach(p => {
      p.category = '未设置目录'
    })

    if (currentPost.value && (currentPost.value.category === catName || currentPost.value.category === target.slug)) {
      currentPost.value.category = '未设置目录'
    }

    if (isServerLive.value && isCollectionsReady.value) {
      try {
        const postsCol = getPostsCollection()
        await Promise.all(
          affectedPosts.map(p => postsCol.update(p.id, { category: '未设置目录' }).catch(() => {}))
        )
      } catch (err) {
        console.warn('向服务端批量更新文章目录失败:', err)
      }
    }

    persistLocal()
    themeStore.addToast({
      type: 'info',
      message: `已删除目录「${catName}」，关联的 ${affectedPosts.length} 篇文章已归入「未设置目录」`,
    })
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
    createCategory,
    updateCategory,
    deleteCategory,
  }
})
