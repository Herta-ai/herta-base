<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useBlogStore } from '../stores/blog'
import { useThemeStore } from '../stores/theme'
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
} from 'lucide-vue-next'
import { formatDate } from '../lib/utils'

const router = useRouter()
const blogStore = useBlogStore()
const themeStore = useThemeStore()

const currentTab = ref<'all' | 'published' | 'draft'>('all')
const searchQuery = ref('')

const displayedPosts = computed(() => {
  let list = [...blogStore.posts]

  if (currentTab.value === 'published') {
    list = list.filter(p => p.is_public !== false)
  } else if (currentTab.value === 'draft') {
    list = list.filter(p => p.is_public === false)
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

const stats = computed(() => {
  const total = blogStore.posts.length
  const published = blogStore.posts.filter(p => p.is_public !== false).length
  const draft = blogStore.posts.filter(p => p.is_public === false).length
  const totalViews = blogStore.posts.reduce((sum, p) => sum + (p.views || 0), 0)
  const totalLikes = blogStore.posts.reduce((sum, p) => sum + (p.likes || 0), 0)

  return { total, published, draft, totalViews, totalLikes }
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
</script>

<template>
  <div class="space-y-8 animate-fade-in max-w-7xl mx-auto">
    <!-- 头部信息与快捷新建 -->
    <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
      <div>
        <h1 class="text-2xl sm:text-3xl font-extrabold text-zinc-900 dark:text-zinc-100 flex items-center gap-2.5">
          <Sliders class="w-7 h-7 text-emerald-500" />
          文章管理工作台
        </h1>
        <p class="text-xs sm:text-sm text-zinc-500 dark:text-zinc-400 mt-1">
          管理、审阅、编辑与发布 HertaBlog 全站博客内容。
        </p>
      </div>

      <router-link to="/editor" class="btn-primary text-xs px-4 py-2.5 shadow-sm">
        <Plus class="w-4 h-4" />
        <span>撰写新文章</span>
      </router-link>
    </div>

    <!-- 数据概览指标卡片 -->
    <div class="grid grid-cols-2 sm:grid-cols-5 gap-4">
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

    <!-- 管理表格卡片 -->
    <div class="card-base overflow-hidden">
      <!-- 表格上方过滤与搜索栏 -->
      <div class="p-4 border-b border-zinc-200 dark:border-zinc-800 flex flex-col sm:flex-row items-center justify-between gap-4">
        <!-- 状态选项卡 -->
        <div class="flex p-1 rounded-xl bg-zinc-100 dark:bg-zinc-800/90 border border-zinc-200 dark:border-zinc-700/80 w-full sm:w-auto">
          <button
            @click="currentTab = 'all'"
            class="flex-1 sm:flex-initial px-3.5 py-1.5 text-xs font-bold rounded-lg transition cursor-pointer"
            :class="currentTab === 'all'
              ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
              : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-200/60 dark:hover:bg-zinc-700/60'"
          >
            全部 ({{ stats.total }})
          </button>
          <button
            @click="currentTab = 'published'"
            class="flex-1 sm:flex-initial px-3.5 py-1.5 text-xs font-bold rounded-lg transition cursor-pointer"
            :class="currentTab === 'published'
              ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
              : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-200/60 dark:hover:bg-zinc-700/60'"
          >
            已公开发布 ({{ stats.published }})
          </button>
          <button
            @click="currentTab = 'draft'"
            class="flex-1 sm:flex-initial px-3.5 py-1.5 text-xs font-bold rounded-lg transition cursor-pointer"
            :class="currentTab === 'draft'
              ? 'bg-emerald-500 text-white shadow-sm shadow-emerald-500/20'
              : 'text-zinc-600 dark:text-zinc-400 hover:text-zinc-900 dark:hover:text-zinc-100 hover:bg-zinc-200/60 dark:hover:bg-zinc-700/60'"
          >
            草稿箱 ({{ stats.draft }})
          </button>
        </div>

        <!-- 搜索输入框 -->
        <div class="relative w-full sm:w-72">
          <Search class="w-4 h-4 text-zinc-400 absolute left-3 top-2.5" />
          <input
            v-model="searchQuery"
            type="text"
            placeholder="搜索文章标题或分类..."
            class="input-base pl-9 py-1.5 text-xs"
          />
        </div>
      </div>

      <!-- 表格主体 -->
      <div class="overflow-x-auto">
        <table class="w-full text-left text-xs">
          <thead class="bg-zinc-50 dark:bg-zinc-800/50 border-b border-zinc-200 dark:border-zinc-800 text-zinc-500 uppercase tracking-wider font-semibold">
            <tr>
              <th class="px-6 py-3.5">文章标题与摘要</th>
              <th class="px-4 py-3.5">专栏分类</th>
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
                <span v-if="post.category" class="px-2 py-0.5 rounded text-[11px] font-bold bg-zinc-100 dark:bg-zinc-800 text-emerald-600 dark:text-emerald-400">
                  {{ post.category }}
                </span>
                <span v-else class="text-zinc-400">-</span>
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
                    class="p-1.5 rounded-lg text-zinc-500 hover:text-amber-600 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition"
                    :title="post.is_public ? '转为草稿' : '公开发布'"
                  >
                    <Lock v-if="post.is_public" class="w-4 h-4" />
                    <Globe v-else class="w-4 h-4" />
                  </button>

                  <button
                    @click="deletePost(post)"
                    class="p-1.5 rounded-lg text-zinc-500 hover:text-rose-600 hover:bg-zinc-100 dark:hover:bg-zinc-800 transition"
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
  </div>
</template>
