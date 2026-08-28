<script setup lang="ts">
import { ref } from 'vue'
import { useBlogStore } from '../../stores/blog'
import PostCard from './PostCard.vue'
import { LayoutGrid, List, SlidersHorizontal, X, ArrowLeft, ArrowRight, Inbox } from 'lucide-vue-next'

const blogStore = useBlogStore()
const layout = ref<'grid' | 'list'>('grid')

const clearFilter = (type: 'category' | 'tag' | 'search') => {
  if (type === 'category') blogStore.filter.category = undefined
  if (type === 'tag') blogStore.filter.tag = undefined
  if (type === 'search') blogStore.filter.search = ''
  blogStore.filter.page = 1
}

const clearAllFilters = () => {
  blogStore.filter.category = undefined
  blogStore.filter.tag = undefined
  blogStore.filter.search = ''
  blogStore.filter.status = 'published'
  blogStore.filter.sortBy = 'latest'
  blogStore.filter.page = 1
}

const changePage = (page: number) => {
  if (page < 1 || page > blogStore.totalPages) return
  blogStore.filter.page = page
  window.scrollTo({ top: 400, behavior: 'smooth' })
}
</script>

<template>
  <div class="space-y-6">
    <!-- 顶部工具栏：筛选标签与视图切换 -->
    <div class="flex flex-wrap items-center justify-between gap-4 p-4 rounded-2xl bg-white dark:bg-zinc-900 border border-zinc-200/80 dark:border-zinc-800">
      <!-- 当前筛选状态 -->
      <div class="flex flex-wrap items-center gap-2 text-xs">
        <span class="text-zinc-400 font-medium">共 {{ blogStore.filteredPosts.length }} 篇文章</span>

        <span
          v-if="blogStore.filter.category"
          class="badge-base bg-emerald-50 text-emerald-700 dark:bg-emerald-950/60 dark:text-emerald-300 border border-emerald-200 dark:border-emerald-800"
        >
          分类: {{ blogStore.filter.category }}
          <button @click="clearFilter('category')" class="hover:text-emerald-900 ml-1"><X class="w-3 h-3" /></button>
        </span>

        <span
          v-if="blogStore.filter.tag"
          class="badge-base bg-blue-50 text-blue-700 dark:bg-blue-950/60 dark:text-blue-300 border border-blue-200 dark:border-blue-800"
        >
          标签: #{{ blogStore.filter.tag }}
          <button @click="clearFilter('tag')" class="hover:text-blue-900 ml-1"><X class="w-3 h-3" /></button>
        </span>

        <span
          v-if="blogStore.filter.search"
          class="badge-base bg-amber-50 text-amber-700 dark:bg-amber-950/60 dark:text-amber-300 border border-amber-200 dark:border-amber-800"
        >
          关键词: {{ blogStore.filter.search }}
          <button @click="clearFilter('search')" class="hover:text-amber-900 ml-1"><X class="w-3 h-3" /></button>
        </span>

        <button
          v-if="blogStore.filter.category || blogStore.filter.tag || blogStore.filter.search"
          @click="clearAllFilters"
          class="text-zinc-400 hover:text-rose-500 transition text-[11px] underline ml-1"
        >
          重置全部
        </button>
      </div>

      <!-- 排序与布局切换 -->
      <div class="flex items-center gap-3">
        <select
          v-model="blogStore.filter.sortBy"
          class="px-3 py-1.5 rounded-xl bg-zinc-50 dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 text-xs font-medium text-zinc-700 dark:text-zinc-300 focus:outline-none"
        >
          <option value="latest">最新发布</option>
          <option value="views">最多浏览</option>
          <option value="likes">最多点赞</option>
          <option value="oldest">最早发布</option>
        </select>

        <div class="flex items-center p-1 rounded-xl bg-zinc-100 dark:bg-zinc-800">
          <button
            @click="layout = 'grid'"
            class="p-1.5 rounded-lg transition"
            :class="layout === 'grid' ? 'bg-white dark:bg-zinc-700 text-emerald-600 dark:text-emerald-400 shadow-xs' : 'text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200'"
            title="网格视图"
          >
            <LayoutGrid class="w-4 h-4" />
          </button>
          <button
            @click="layout = 'list'"
            class="p-1.5 rounded-lg transition"
            :class="layout === 'list' ? 'bg-white dark:bg-zinc-700 text-emerald-600 dark:text-emerald-400 shadow-xs' : 'text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200'"
            title="列表视图"
          >
            <List class="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>

    <!-- 文章空状态 -->
    <div
      v-if="blogStore.paginatedPosts.length === 0"
      class="card-base p-16 text-center text-zinc-400"
    >
      <Inbox class="w-12 h-12 mx-auto mb-3 opacity-40 text-emerald-500" />
      <h3 class="text-base font-bold text-zinc-700 dark:text-zinc-300 mb-1">未找到符合条件的文章</h3>
      <p class="text-xs text-zinc-500 mb-4">尝试更换筛选条件或清除搜索关键字</p>
      <button @click="clearAllFilters" class="btn-primary text-xs px-4 py-2">
        重置所有筛选
      </button>
    </div>

    <!-- 文章列表流 -->
    <div
      v-else
      :class="layout === 'grid' ? 'grid grid-cols-1 md:grid-cols-2 gap-6' : 'flex flex-col gap-6'"
    >
      <PostCard
        v-for="post in blogStore.paginatedPosts"
        :key="post.id"
        :post="post"
        :layout="layout"
      />
    </div>

    <!-- 分页控制器 -->
    <div
      v-if="blogStore.totalPages > 1"
      class="pt-6 flex items-center justify-center gap-2"
    >
      <button
        @click="changePage((blogStore.filter.page || 1) - 1)"
        :disabled="(blogStore.filter.page || 1) <= 1"
        class="btn-secondary text-xs px-3 py-2 disabled:opacity-40 disabled:cursor-not-allowed"
      >
        <ArrowLeft class="w-3.5 h-3.5" />
        上一页
      </button>

      <div class="flex items-center gap-1">
        <button
          v-for="p in blogStore.totalPages"
          :key="p"
          @click="changePage(p)"
          class="w-8 h-8 rounded-xl text-xs font-bold transition flex-center"
          :class="(blogStore.filter.page || 1) === p ? 'bg-emerald-500 text-white shadow-sm' : 'text-zinc-600 dark:text-zinc-400 hover:bg-zinc-100 dark:hover:bg-zinc-800'"
        >
          {{ p }}
        </button>
      </div>

      <button
        @click="changePage((blogStore.filter.page || 1) + 1)"
        :disabled="(blogStore.filter.page || 1) >= blogStore.totalPages"
        class="btn-secondary text-xs px-3 py-2 disabled:opacity-40 disabled:cursor-not-allowed"
      >
        下一页
        <ArrowRight class="w-3.5 h-3.5" />
      </button>
    </div>
  </div>
</template>
