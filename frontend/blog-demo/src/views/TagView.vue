<script setup lang="ts">
import { watch, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { useBlogStore } from '../stores/blog'
import PostList from '../components/post/PostList.vue'
import { Tag } from 'lucide-vue-next'

const route = useRoute()
const blogStore = useBlogStore()

const updateFilter = () => {
  const tag = route.params.tag as string
  if (tag) {
    blogStore.filter.tag = tag
    blogStore.filter.page = 1
  }
}

watch(() => route.params.tag, updateFilter)
onMounted(() => {
  updateFilter()
})
</script>

<template>
  <div class="space-y-8 animate-fade-in">
    <!-- 标签头部 Hero -->
    <div class="card-base p-8 bg-gradient-to-r from-blue-500/10 via-indigo-500/10 to-purple-500/10 border-blue-200/50 dark:border-blue-900/50">
      <div class="space-y-3">
        <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-bold bg-blue-500 text-white shadow-sm">
          <Tag class="w-3.5 h-3.5" />
          标签聚合
        </div>

        <h1 class="text-2xl sm:text-3xl font-extrabold text-zinc-900 dark:text-zinc-100">
          #{{ route.params.tag }}
        </h1>

        <p class="text-xs sm:text-sm text-zinc-600 dark:text-zinc-400 leading-relaxed">
          包含当前标签的主题文章索引与技术讨论。
        </p>
      </div>
    </div>

    <!-- 标签云快捷切换 -->
    <div class="flex flex-wrap gap-2">
      <router-link
        v-for="t in blogStore.tags"
        :key="t.name"
        :to="`/tag/${t.name}`"
        class="badge-base py-1.5 px-3 transition"
        :class="blogStore.filter.tag === t.name ? 'bg-blue-500 text-white font-bold shadow-sm' : 'bg-white dark:bg-zinc-900 text-zinc-600 dark:text-zinc-400 border border-zinc-200 dark:border-zinc-800 hover:border-blue-500'"
      >
        #{{ t.name }}
        <span class="opacity-60 text-[10px]">({{ t.count }})</span>
      </router-link>
    </div>

    <!-- 文章列表 -->
    <PostList />
  </div>
</template>
