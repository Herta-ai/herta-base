<script setup lang="ts">
import { onMounted } from 'vue'
import { useBlogStore } from '../stores/blog'
import HeroFeatured from '../components/layout/HeroFeatured.vue'
import PostList from '../components/post/PostList.vue'
import SidebarWidgets from '../components/layout/SidebarWidgets.vue'
import { Sparkles, Layers, BookOpen } from 'lucide-vue-next'

const blogStore = useBlogStore()

onMounted(() => {
  blogStore.init()
})
</script>

<template>
  <div class="space-y-10">
    <!-- 杂志风置顶头条区 -->
    <HeroFeatured />

    <!-- 分类快速筛选条 -->
    <div class="flex items-center gap-2 overflow-x-auto pb-2 scrollbar-none">
      <button
        @click="blogStore.filter.category = undefined; blogStore.filter.page = 1"
        class="px-4 py-2 rounded-2xl text-xs font-bold transition whitespace-nowrap"
        :class="!blogStore.filter.category ? 'bg-emerald-500 text-white shadow-md shadow-emerald-500/20' : 'bg-white dark:bg-zinc-900 text-zinc-600 dark:text-zinc-400 border border-zinc-200 dark:border-zinc-800 hover:border-emerald-500'"
      >
        🌟 全部文章 ({{ blogStore.posts.filter(p => p.is_public !== false).length }})
      </button>

      <button
        v-for="cat in blogStore.computedCategories"
        :key="cat.id"
        @click="blogStore.filter.category = cat.name; blogStore.filter.page = 1"
        class="px-4 py-2 rounded-2xl text-xs font-bold transition whitespace-nowrap"
        :class="blogStore.filter.category === cat.name ? 'bg-emerald-500 text-white shadow-md shadow-emerald-500/20' : 'bg-white dark:bg-zinc-900 text-zinc-600 dark:text-zinc-400 border border-zinc-200 dark:border-zinc-800 hover:border-emerald-500'"
      >
        {{ cat.name }} ({{ cat.count || 0 }})
      </button>
    </div>

    <!-- 首页双栏主体：文章流 + 侧边栏小组件 -->
    <div class="grid grid-cols-1 lg:grid-cols-12 gap-8 items-start">
      <!-- 左侧文章流 -->
      <div class="lg:col-span-8">
        <PostList />
      </div>

      <!-- 右侧侧边栏 -->
      <div class="lg:col-span-4">
        <SidebarWidgets />
      </div>
    </div>
  </div>
</template>
