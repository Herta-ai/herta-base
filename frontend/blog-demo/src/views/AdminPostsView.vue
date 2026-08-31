<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useBlogStore } from '../stores/blog'
import { useThemeStore } from '../stores/theme'
import type { BlogCategory } from '../types/blog'
import {
  Sliders,
  PenSquare,
  Eye,
  Trash2,
  Lock,
  Globe,
  Search,
  CheckCircle2,
  FileText,
  Heart,
  Plus,
  Folder,
  FolderPlus,
  Edit3,
  X,
  Sparkles,
  Layers,
} from 'lucide-vue-next'
import { formatDate } from '../lib/utils'

const router = useRouter()
const blogStore = useBlogStore()
const themeStore = useThemeStore()

// 顶级视图模式：文章管理 vs 专栏目录管理
const viewMode = ref<'posts' | 'categories'>('posts')

// 文章筛选
const currentTab = ref<'all' | 'published' | 'draft'>('all')
const searchQuery = ref('')
const selectedCategoryFilter = ref<string>('all')

// 目录管理状态
const catSearchQuery = ref('')
const catModalOpen = ref(false)
const editingCatId = ref<string | null>(null)
const catForm = ref({
  name: '',
  slug: '',
  description: '',
  color: '#10b981',
})

const PRESET_COLORS = [
  { name: '翡翠绿', value: '#10b981' },
  { name: '青碧蓝', value: '#06b6d4' },
  { name: '天蓝色', value: '#3b82f6' },
  { name: '紫罗兰', value: '#8b5cf6' },
  { name: '日落橙', value: '#f97316' },
  { name: '玫瑰红', value: '#f43f5e' },
  { name: '石墨灰', value: '#71717a' },
]

const displayedPosts = computed(() => {
  let list = [...blogStore.posts]

  if (currentTab.value === 'published') {
    list = list.filter(p => p.is_public !== false)
  } else if (currentTab.value === 'draft') {
    list = list.filter(p => p.is_public === false)
  }

  if (selectedCategoryFilter.value !== 'all') {
    list = list.filter(p => (p.category || '未设置目录') === selectedCategoryFilter.value)
  }

  if (searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase().trim()
    list = list.filter(p =>
      p.title.toLowerCase().includes(q) ||
      p.category?.toLowerCase().includes(q) ||
      p.tags?.some(t => t.toLowerCase().includes(q))
    )
  }

  return list
})

const displayedCategories = computed(() => {
  let list = blogStore.computedCategories
  if (catSearchQuery.value.trim()) {
    const q = catSearchQuery.value.toLowerCase().trim()
    list = list.filter(c =>
      c.name.toLowerCase().includes(q) ||
      c.slug.toLowerCase().includes(q) ||
      c.description?.toLowerCase().includes(q)
    )
  }
  return list
})

const stats = computed(() => {
  const total = blogStore.posts.length
  const published = blogStore.posts.filter(p => p.is_public !== false).length
  const draft = blogStore.posts.filter(p => p.is_public === false).length
  const totalViews = blogStore.posts.reduce((sum, p) => sum + (p.views || 0), 0)
  const totalLikes = blogStore.posts.reduce((sum, p) => sum + (p.likes || 0), 0)
  const totalCategories = blogStore.categories.length
  const uncategorizedCount = blogStore.posts.filter(p => !p.category || p.category === '未设置目录').length

  return { total, published, draft, totalViews, totalLikes, totalCategories, uncategorizedCount }
})

const togglePublishStatus = async (post: any) => {
  const newStatus = !post.is_public
  await blogStore.updatePost(post.id, { is_public: newStatus })
}

const deletePost = async (post: any) => {
  if (confirm(`确定要删除文章《${post.title}》吗？`)) {
    await blogStore.deletePost(post.id)
  }
}

const openCreateCatModal = () => {
  editingCatId.value = null
  catForm.value = {
    name: '',
    slug: '',
    description: '',
    color: '#10b981',
  }
  catModalOpen.value = true
}

const openEditCatModal = (cat: BlogCategory) => {
  if (cat.name === '未设置目录') {
    themeStore.addToast({ type: 'warning', message: '「未设置目录」为系统默认分类，无需修改' })
    return
  }
  editingCatId.value = cat.id
  catForm.value = {
    name: cat.name,
    slug: cat.slug,
    description: cat.description || '',
    color: cat.color || '#10b981',
  }
  catModalOpen.value = true
}

const handleSaveCategory = async () => {
  if (!catForm.value.name.trim()) {
    themeStore.addToast({ type: 'warning', message: '请输入目录名称' })
    return
  }

  try {
    if (editingCatId.value) {
      await blogStore.updateCategory(editingCatId.value, {
        name: catForm.value.name.trim(),
        slug: catForm.value.slug.trim() || undefined,
        description: catForm.value.description.trim(),
        color: catForm.value.color,
      })
    } else {
      blogStore.createCategory({
        name: catForm.value.name.trim(),
        slug: catForm.value.slug.trim() || undefined,
        description: catForm.value.description.trim(),
        color: catForm.value.color,
      })
    }
    catModalOpen.value = false
  } catch {
    // Toast 已捕获
  }
}

const handleDeleteCategory = async (cat: BlogCategory) => {
  if (cat.name === '未设置目录') {
    themeStore.addToast({ type: 'warning', message: '「未设置目录」为系统预留分类，不可删除' })
    return
  }

  const postCount = cat.count || 0
  const confirmMsg = postCount > 0
    ? `确定要删除目录「${cat.name}」吗？\n\n【注意】：归属于该目录的 ${postCount} 篇文章将全部自动转为「未设置目录」！`
    : `确定要删除目录「${cat.name}」吗？`

  if (confirm(confirmMsg)) {
    await blogStore.deleteCategory(cat.id)
  }
}

const filterByThisCategory = (catName: string) => {
  viewMode.value = 'posts'
  selectedCategoryFilter.value = catName
}
</script>

<template>
  <div class="space-y-8 animate-fade-in max-w-7xl mx-auto">
    <!-- 头部信息与顶级工作台导航 -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
      <div>
        <h1 class="text-2xl sm:text-3xl font-extrabold text-zinc-900 dark:text-zinc-100 flex items-center gap-2.5">
          <Sliders class="w-7 h-7 text-emerald-500" />
          博客综合管理控制台
        </h1>
        <p class="text-xs sm:text-sm text-zinc-500 dark:text-zinc-400 mt-1">
          管理文章发布、专栏目录体系，支持目录删除与文章归档自动流转。
        </p>
      </div>

      <div class="flex items-center gap-2">
        <button
          v-if="viewMode === 'categories'"
          @click="openCreateCatModal"
          class="btn-primary text-xs px-4 py-2.5 shadow-sm cursor-pointer flex items-center gap-1.5"
        >
          <FolderPlus class="w-4 h-4" />
          <span>新建专栏目录</span>
        </button>

        <router-link
          v-else
          to="/editor"
          class="btn-primary text-xs px-4 py-2.5 shadow-sm cursor-pointer flex items-center gap-1.5"
        >
          <Plus class="w-4 h-4" />
          <span>撰写新文章</span>
        </router-link>
      </div>
    </div>

    <!-- 顶部工作台视图切换 Tab -->
    <div class="flex items-center gap-2 border-b border-zinc-200 dark:border-zinc-800 pb-1">
      <button
        @click="viewMode = 'posts'"
        class="flex items-center gap-2 px-5 py-2.5 text-xs sm:text-sm font-bold border-b-2 transition cursor-pointer"
        :class="viewMode === 'posts'
          ? 'border-emerald-500 text-emerald-600 dark:text-emerald-400 font-extrabold'
          : 'border-transparent text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-200'"
      >
        <FileText class="w-4 h-4" />
        <span>文章管理 ({{ stats.total }})</span>
      </button>

      <button
        @click="viewMode = 'categories'"
        class="flex items-center gap-2 px-5 py-2.5 text-xs sm:text-sm font-bold border-b-2 transition cursor-pointer"
        :class="viewMode === 'categories'
          ? 'border-emerald-500 text-emerald-600 dark:text-emerald-400 font-extrabold'
          : 'border-transparent text-zinc-500 hover:text-zinc-900 dark:hover:text-zinc-200'"
      >
        <Folder class="w-4 h-4" />
        <span>专栏目录管理 ({{ stats.totalCategories }})</span>
      </button>
    </div>

    <!-- 数据概览指标卡片 -->
    <div v-if="viewMode === 'posts'" class="grid grid-cols-2 sm:grid-cols-5 gap-4">
      <div class="card-base p-4">
        <div class="text-xs text-zinc-400">全部文章</div>
        <div class="text-xl sm:text-2xl font-bold text-zinc-900 dark:text-zinc-100 mt-1">{{ stats.total }}</div>
      </div>
      <div class="card-base p-4">
        <div class="text-xs text-emerald-600 dark:text-emerald-400">已发布</div>
        <div class="text-xl sm:text-2xl font-bold text-emerald-600 dark:text-emerald-400 mt-1">{{ stats.published }}</div>
      </div>
      <div class="card-base p-4">
        <div class="text-xs text-amber-500">草稿箱</div>
        <div class="text-xl sm:text-2xl font-bold text-amber-500 mt-1">{{ stats.draft }}</div>
      </div>
      <div class="card-base p-4">
        <div class="text-xs text-blue-500">总浏览量</div>
        <div class="text-xl sm:text-2xl font-bold text-blue-500 mt-1">{{ stats.totalViews }}</div>
      </div>
      <div class="card-base p-4 col-span-2 sm:col-span-1">
        <div class="text-xs text-rose-500">总收获赞同</div>
        <div class="text-xl sm:text-2xl font-bold text-rose-500 mt-1">{{ stats.totalLikes }}</div>
      </div>
    </div>

    <div v-else class="grid grid-cols-1 sm:grid-cols-3 gap-4">
      <div class="card-base p-4">
        <div class="text-xs text-emerald-600 dark:text-emerald-400">活跃专栏目录数</div>
        <div class="text-xl sm:text-2xl font-bold text-emerald-600 dark:text-emerald-400 mt-1">{{ stats.totalCategories }}</div>
      </div>
      <div class="card-base p-4">
        <div class="text-xs text-blue-500">已设置目录文章数</div>
        <div class="text-xl sm:text-2xl font-bold text-blue-500 mt-1">{{ stats.total - stats.uncategorizedCount }}</div>
      </div>
      <div class="card-base p-4">
        <div class="text-xs text-zinc-500 dark:text-zinc-400">未设置目录文章数</div>
        <div class="text-xl sm:text-2xl font-bold text-zinc-700 dark:text-zinc-300 mt-1">{{ stats.uncategorizedCount }}</div>
      </div>
    </div>

    <!-- ==================== 1. 文章管理视图 ==================== -->
    <div v-if="viewMode === 'posts'" class="card-base overflow-hidden">
      <!-- 表格上方过滤与搜索栏 -->
      <div class="p-4 border-b border-zinc-200 dark:border-zinc-800 flex flex-col md:flex-row items-center justify-between gap-4">
        <!-- 状态与分类筛选 -->
        <div class="flex flex-wrap items-center gap-2.5 w-full md:w-auto">
          <!-- 状态选项卡 -->
          <div class="flex p-1 rounded-xl bg-zinc-100 dark:bg-zinc-800/90 border border-zinc-200 dark:border-zinc-700/80">
            <button
              @click="currentTab = 'all'"
              class="px-3 py-1.5 text-xs font-bold rounded-lg transition cursor-pointer"
              :class="currentTab === 'all'
                ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
                : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-200/60 dark:hover:bg-zinc-700/60'"
            >
              全部 ({{ stats.total }})
            </button>
            <button
              @click="currentTab = 'published'"
              class="px-3 py-1.5 text-xs font-bold rounded-lg transition cursor-pointer"
              :class="currentTab === 'published'
                ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
                : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-200/60 dark:hover:bg-zinc-700/60'"
            >
              已公开 ({{ stats.published }})
            </button>
            <button
              @click="currentTab = 'draft'"
              class="px-3 py-1.5 text-xs font-bold rounded-lg transition cursor-pointer"
              :class="currentTab === 'draft'
                ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
                : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-200/60 dark:hover:bg-zinc-700/60'"
            >
              草稿 ({{ stats.draft }})
            </button>
          </div>

          <!-- 专栏分类下拉筛选 -->
          <div class="flex items-center gap-1.5 text-xs">
            <select
              v-model="selectedCategoryFilter"
              class="input-base text-xs py-1.5 px-3 rounded-xl"
            >
              <option value="all">全部目录专栏</option>
              <option value="未设置目录">未设置目录 ({{ stats.uncategorizedCount }})</option>
              <option
                v-for="cat in blogStore.categories"
                :key="cat.id"
                :value="cat.name"
              >
                {{ cat.name }} ({{ cat.count || 0 }})
              </option>
            </select>
          </div>
        </div>

        <!-- 搜索输入框 -->
        <div class="relative w-full md:w-72">
          <Search class="w-4 h-4 text-zinc-400 absolute left-3 top-2.5" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="搜索文章标题或分类..."
            class="input-base pl-9 py-1.5 text-xs w-full"
          />
        </div>
      </div>

      <!-- 文章表格主体 -->
      <div class="overflow-x-auto">
        <table class="w-full text-left text-xs">
          <thead class="bg-zinc-50 dark:bg-zinc-800/50 border-b border-zinc-200 dark:border-zinc-800 text-zinc-500 uppercase tracking-wider font-semibold">
            <tr>
              <th class="px-6 py-3.5">文章标题与摘要</th>
              <th class="px-4 py-3.5">所属目录</th>
              <th class="px-4 py-3.5">状态</th>
              <th class="px-4 py-3.5">统计 (看/赞)</th>
              <th class="px-4 py-3.5">发布日期</th>
              <th class="px-6 py-3.5 text-right">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-zinc-100 dark:divide-zinc-800/60">
            <tr
              v-for="post in displayedPosts"
              :key="post.id"
              class="hover:bg-zinc-50/70 dark:hover:bg-zinc-800/40 transition"
            >
              <td class="px-6 py-4 max-w-sm">
                <div class="font-bold text-zinc-900 dark:text-zinc-100 hover:text-emerald-500 transition line-clamp-1">
                  {{ post.title }}
                </div>
                <div class="text-zinc-400 line-clamp-1 mt-0.5 text-[11px]">
                  {{ post.excerpt || '暂无摘要' }}
                </div>
              </td>

              <td class="px-4 py-4">
                <span
                  v-if="post.category && post.category !== '未设置目录'"
                  class="px-2.5 py-1 rounded-lg text-[11px] font-bold bg-emerald-50 text-emerald-700 dark:bg-emerald-950/60 dark:text-emerald-300 border border-emerald-200/60 dark:border-emerald-800/60"
                >
                  {{ post.category }}
                </span>
                <span
                  v-else
                  class="px-2.5 py-1 rounded-lg text-[11px] font-medium bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-400 border border-zinc-200/80 dark:border-zinc-700/80"
                >
                  未设置目录
                </span>
              </td>

              <td class="px-4 py-4">
                <span
                  class="badge-base text-[10px]"
                  :class="post.is_public !== false ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300' : 'bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300'"
                >
                  <Globe v-if="post.is_public !== false" class="w-3 h-3" />
                  <Lock v-else class="w-3 h-3" />
                  {{ post.is_public !== false ? '公开' : '草稿' }}
                </span>
              </td>

              <td class="px-4 py-4 text-zinc-500 whitespace-nowrap">
                <span class="font-mono">{{ post.views || 0 }}</span> / <span class="font-mono text-rose-500">{{ post.likes || 0 }}</span>
              </td>

              <td class="px-4 py-4 text-zinc-400 whitespace-nowrap">
                {{ formatDate(post.created_at) }}
              </td>

              <td class="px-6 py-4 text-right whitespace-nowrap">
                <div class="flex items-center justify-end gap-2">
                  <router-link
                    :to="`/post/${post.slug || post.id}`"
                    class="p-1.5 rounded-lg text-zinc-500 hover:text-emerald-600 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition"
                    title="查看前台文章"
                  >
                    <Eye class="w-4 h-4" />
                  </router-link>

                  <router-link
                    :to="`/editor?id=${post.id}`"
                    class="p-1.5 rounded-lg text-zinc-500 hover:text-blue-600 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition"
                    title="编辑文章"
                  >
                    <PenSquare class="w-4 h-4" />
                  </router-link>

                  <button
                    @click="togglePublishStatus(post)"
                    class="p-1.5 rounded-lg text-zinc-500 hover:text-amber-600 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition cursor-pointer"
                    :title="post.is_public ? '转为草稿' : '公开发布'"
                  >
                    <Lock v-if="post.is_public" class="w-4 h-4" />
                    <Globe v-else class="w-4 h-4" />
                  </button>

                  <button
                    @click="deletePost(post)"
                    class="p-1.5 rounded-lg text-zinc-500 hover:text-rose-600 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition cursor-pointer"
                    title="删除文章"
                  >
                    <Trash2 class="w-4 h-4" />
                  </button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div v-if="displayedPosts.length === 0" class="p-12 text-center text-zinc-400">
        未找到匹配的文章记录
      </div>
    </div>

    <!-- ==================== 2. 专栏目录管理视图 ==================== -->
    <div v-else class="card-base overflow-hidden">
      <!-- 目录搜索与操作条 -->
      <div class="p-4 border-b border-zinc-200 dark:border-zinc-800 flex flex-col sm:flex-row items-center justify-between gap-4">
        <div class="text-xs text-zinc-500 dark:text-zinc-400">
          共收录 <strong>{{ displayedCategories.length }}</strong> 个目录专栏（删除目录后，相关文章将自动保留并归入「未设置目录」）
        </div>

        <div class="relative w-full sm:w-72">
          <Search class="w-4 h-4 text-zinc-400 absolute left-3 top-2.5" />
          <input
            v-model="catSearchQuery"
            type="text"
            placeholder="搜索专栏目录名称或别名..."
            class="input-base pl-9 py-1.5 text-xs w-full"
          />
        </div>
      </div>

      <!-- 目录表格 -->
      <div class="overflow-x-auto">
        <table class="w-full text-left text-xs">
          <thead class="bg-zinc-50 dark:bg-zinc-800/50 border-b border-zinc-200 dark:border-zinc-800 text-zinc-500 uppercase tracking-wider font-semibold">
            <tr>
              <th class="px-6 py-3.5">专栏目录名称</th>
              <th class="px-4 py-3.5">别名 (Slug)</th>
              <th class="px-4 py-3.5">描述说明</th>
              <th class="px-4 py-3.5">关联文章数</th>
              <th class="px-6 py-3.5 text-right">操作</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-zinc-100 dark:divide-zinc-800/60">
            <tr
              v-for="cat in displayedCategories"
              :key="cat.id"
              class="hover:bg-zinc-50/70 dark:hover:bg-zinc-800/40 transition"
            >
              <td class="px-6 py-4">
                <div class="flex items-center gap-2.5">
                  <span
                    class="w-3 h-3 rounded-full shrink-0"
                    :style="{ backgroundColor: cat.color || '#10b981' }"
                  />
                  <span class="font-bold text-zinc-900 dark:text-zinc-100 text-sm">
                    {{ cat.name }}
                  </span>
                  <span
                    v-if="cat.name === '未设置目录'"
                    class="badge-base bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400 text-[10px]"
                  >
                    系统预留
                  </span>
                </div>
              </td>

              <td class="px-4 py-4 font-mono text-zinc-500">
                {{ cat.slug }}
              </td>

              <td class="px-4 py-4 text-zinc-500 dark:text-zinc-400 max-w-xs truncate">
                {{ cat.description || '暂无描述' }}
              </td>

              <td class="px-4 py-4">
                <button
                  @click="filterByThisCategory(cat.name)"
                  class="px-2.5 py-1 rounded-full text-xs font-bold bg-zinc-100 hover:bg-emerald-100 dark:bg-zinc-800 dark:hover:bg-emerald-950/60 text-zinc-700 hover:text-emerald-700 dark:text-zinc-300 dark:hover:text-emerald-300 transition cursor-pointer"
                  title="点击筛选查看该目录下的文章"
                >
                  {{ cat.count || 0 }} 篇
                </button>
              </td>

              <td class="px-6 py-4 text-right whitespace-nowrap">
                <div v-if="cat.name !== '未设置目录'" class="flex items-center justify-end gap-2">
                  <button
                    @click="openEditCatModal(cat)"
                    class="p-1.5 rounded-lg text-zinc-500 hover:text-blue-600 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition cursor-pointer"
                    title="编辑目录"
                  >
                    <Edit3 class="w-4 h-4" />
                  </button>

                  <button
                    @click="handleDeleteCategory(cat)"
                    class="p-1.5 rounded-lg text-zinc-500 hover:text-rose-600 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition cursor-pointer"
                    title="删除目录（文章自动转为未设置目录）"
                  >
                    <Trash2 class="w-4 h-4" />
                  </button>
                </div>
                <div v-else class="text-zinc-400 text-[11px]">
                  预置保护
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- ==================== 3. 新建 / 编辑专栏目录 Modal 对话框 ==================== -->
    <div
      v-if="catModalOpen"
      class="fixed inset-0 z-50 bg-black/60 backdrop-blur-xs flex items-center justify-center p-4"
      @click.self="catModalOpen = false"
    >
      <div class="card-base w-full max-w-md p-6 space-y-5 animate-scale-in shadow-2xl">
        <div class="flex items-center justify-between border-b border-zinc-100 dark:border-zinc-800 pb-3">
          <h3 class="text-base font-bold text-zinc-900 dark:text-zinc-100 flex items-center gap-2">
            <Folder class="w-5 h-5 text-emerald-500" />
            {{ editingCatId ? '编辑专栏目录' : '新建专栏目录' }}
          </h3>
          <button
            @click="catModalOpen = false"
            class="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 cursor-pointer"
          >
            <X class="w-5 h-5" />
          </button>
        </div>

        <div class="space-y-4 text-xs">
          <!-- 目录名称 -->
          <div class="space-y-1.5">
            <label class="font-bold text-zinc-700 dark:text-zinc-300">
              目录名称 <span class="text-rose-500">*</span>
            </label>
            <input
              v-model="catForm.name"
              type="text"
              placeholder="例如：云原生架构、前端前沿等"
              class="input-base w-full"
              required
            />
          </div>

          <!-- 目录 Slug -->
          <div class="space-y-1.5">
            <label class="font-bold text-zinc-700 dark:text-zinc-300">
              别名 Slug (用于 URL 路由)
            </label>
            <input
              v-model="catForm.slug"
              type="text"
              placeholder="例如：cloud-native（留空将自动生成）"
              class="input-base w-full font-mono"
            />
          </div>

          <!-- 目录简介描述 -->
          <div class="space-y-1.5">
            <label class="font-bold text-zinc-700 dark:text-zinc-300">
              目录简介说明
            </label>
            <textarea
              v-model="catForm.description"
              rows="3"
              placeholder="一句话介绍该专栏目录的主题内容..."
              class="input-base w-full resize-none"
            ></textarea>
          </div>

          <!-- 主题色彩选择 -->
          <div class="space-y-1.5">
            <label class="font-bold text-zinc-700 dark:text-zinc-300 flex items-center justify-between">
              <span>标牌色彩</span>
              <span class="font-mono text-zinc-400">{{ catForm.color }}</span>
            </label>
            <div class="flex items-center gap-2 pt-1">
              <button
                v-for="color in PRESET_COLORS"
                :key="color.value"
                type="button"
                @click="catForm.color = color.value"
                class="w-7 h-7 rounded-full flex items-center justify-center transition cursor-pointer border-2"
                :style="{ backgroundColor: color.value }"
                :class="catForm.color === color.value ? 'border-zinc-900 dark:border-white scale-110 shadow-md' : 'border-transparent opacity-80 hover:opacity-100'"
                :title="color.name"
              >
                <CheckCircle2 v-if="catForm.color === color.value" class="w-4 h-4 text-white" />
              </button>
            </div>
          </div>
        </div>

        <div class="flex items-center justify-end gap-3 pt-3 border-t border-zinc-100 dark:border-zinc-800">
          <button
            type="button"
            @click="catModalOpen = false"
            class="btn-secondary text-xs px-4 py-2 cursor-pointer"
          >
            取消
          </button>
          <button
            type="button"
            @click="handleSaveCategory"
            class="btn-primary text-xs px-5 py-2 cursor-pointer"
          >
            {{ editingCatId ? '保存修改' : '确认创建' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
