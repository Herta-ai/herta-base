<script setup lang="ts">
import { computed, watch, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useBlogStore } from '../stores/blog'
import PostList from '../components/post/PostList.vue'
import { Layers, Sparkles, Folder } from 'lucide-vue-next'

const route = useRoute()
const blogStore = useBlogStore()

const currentCategory = computed(() => {
  const nameOrSlug = route.params.name as string
  return blogStore.categories.find(c => c.name === nameOrSlug || c.slug === nameOrSlug)
})

const updateFilter = () => {
  const nameOrSlug = route.params.name as string
  if (nameOrSlug) {
    const found = blogStore.categories.find(c => c.name === nameOrSlug || c.slug === nameOrSlug)
    blogStore.filter.category = found ? found.name : nameOrSlug
  } else {
    blogStore.filter.category = undefined
  }
  blogStore.filter.page = 1
}

watch(() => route.params.name, updateFilter)
onMounted(() => {
  updateFilter()
})
</script>

<template>
  <div class="space-y-8 animate-fade-in">
    <!-- 专栏头部 Hero -->
    <div class="card-base p-8 bg-gradient-to-r from-emerald-500/10 via-teal-500/10 to-blue-500/10 dark:from-emerald-950/30 dark:via-zinc-900 dark:to-zinc-900 border-emerald-200/50 dark:border-emerald-900/50 relative overflow-hidden">
      <div class="relative z-10 space-y-3">
        <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-bold bg-emerald-500 text-white shadow-sm">
          <Layers class="w-3.5 h-3.5" />
          专栏分类
        </div>

        <h1 class="text-2xl sm:text-3xl font-extrabold text-zinc-900 dark:text-zinc-100">
          {{ currentCategory?.name || '专栏分类精选' }}
        </h1>

        <p class="text-xs sm:text-sm text-zinc-600 dark:text-zinc-400 max-w-2xl leading-relaxed">
          {{ currentCategory?.description || '汇聚全部专栏核心技术解析、分布式系统设计、Rust 深度实践与全栈架构好文。' }}
        </p>
      </div>
    </div>

    <!-- 分类切换标签 -->
    <div class="flex items-center gap-2 overflow-x-auto pb-2 scrollbar-none">
      <router-link
        to="/categories"
        class="px-4 py-2 rounded-2xl text-xs font-bold transition whitespace-nowrap cursor-pointer shadow-xs"
        :class="!route.params.name
          ? 'bg-emerald-500 text-white shadow-md shadow-emerald-500/20'
          : 'bg-zinc-100 dark:bg-zinc-800/90 text-zinc-700 dark:text-zinc-200 border border-zinc-200 dark:border-zinc-700 hover:border-emerald-500 dark:hover:border-emerald-500 hover:text-emerald-600 dark:hover:text-emerald-400'"
      >
        🌟 全部专栏
      </router-link>

      <router-link
        v-for="cat in blogStore.computedCategories"
        :key="cat.id"
        :to="`/category/${cat.slug || cat.name}`"
        class="px-4 py-2 rounded-2xl text-xs font-bold transition whitespace-nowrap cursor-pointer shadow-xs"
        :class="(currentCategory?.name === cat.name || route.params.name === cat.slug)
          ? 'bg-emerald-500 text-white shadow-md shadow-emerald-500/20'
          : 'bg-zinc-100 dark:bg-zinc-800/90 text-zinc-700 dark:text-zinc-200 border border-zinc-200 dark:border-zinc-700 hover:border-emerald-500 dark:hover:border-emerald-500 hover:text-emerald-600 dark:hover:text-emerald-400'"
      >
        {{ cat.name }} ({{ cat.count || 0 }})
      </router-link>
    </div>

    <!-- 文章列表 -->
    <PostList />
  </div>
</template>
