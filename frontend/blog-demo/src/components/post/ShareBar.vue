<script setup lang="ts">
import { computed } from 'vue'
import { useBlogStore } from '../../stores/blog'
import { useThemeStore } from '../../stores/theme'
import { Heart, Share2, Link2, MessageSquare, Check } from 'lucide-vue-next'
import { copyToClipboard } from '../../lib/utils'

const props = defineProps<{
  postId: string
  likes?: number
  commentCount?: number
}>()

const blogStore = useBlogStore()
const themeStore = useThemeStore()

const isLiked = computed(() => blogStore.likedPostIds.includes(props.postId))

const handleLike = () => {
  blogStore.toggleLike(props.postId)
}

const handleCopyLink = async () => {
  const success = await copyToClipboard(window.location.href)
  if (success) {
    themeStore.addToast({ type: 'success', message: '文章链接已复制到剪贴板！' })
  } else {
    themeStore.addToast({ type: 'error', message: '复制失败，请手动复制浏览器地址栏' })
  }
}

const scrollToComments = () => {
  const el = document.getElementById('comments-section')
  el?.scrollIntoView({ behavior: 'smooth' })
}
</script>

<template>
  <div class="flex flex-wrap items-center justify-between gap-4 py-6 border-y border-zinc-200/80 dark:border-zinc-800">
    <!-- 点赞按钮 -->
    <div class="flex items-center gap-3">
      <button
        @click="handleLike"
        class="inline-flex items-center gap-2 px-5 py-2.5 rounded-2xl transition-all duration-200 cursor-pointer text-sm font-bold shadow-sm"
        :class="isLiked
          ? 'bg-rose-500 hover:bg-rose-600 text-white shadow-rose-500/20 scale-105 border border-rose-600'
          : 'bg-zinc-100 dark:bg-zinc-800/90 hover:bg-zinc-200 dark:hover:bg-zinc-700 text-zinc-700 dark:text-zinc-200 border border-zinc-200/80 dark:border-zinc-700/80 hover:border-rose-300 dark:hover:border-rose-800/60 hover:text-rose-500 dark:hover:text-rose-400'"
      >
        <Heart class="w-4 h-4" :class="isLiked ? 'fill-white text-white' : 'text-rose-500'" />
        <span>{{ isLiked ? '已赞' : '点赞鼓励' }}</span>
        <span
          class="px-2 py-0.5 rounded-md text-xs font-semibold"
          :class="isLiked ? 'bg-rose-600 text-white' : 'bg-zinc-200 dark:bg-zinc-700 text-zinc-700 dark:text-zinc-300'"
        >
          {{ likes || 0 }}
        </span>
      </button>

      <!-- 评论锚点跳转 -->
      <button
        @click="scrollToComments"
        class="btn-secondary text-xs px-3.5 py-2.5"
      >
        <MessageSquare class="w-4 h-4 text-emerald-500" />
        <span>互动评论 ({{ commentCount || 0 }})</span>
      </button>
    </div>

    <!-- 分享操作 -->
    <div class="flex items-center gap-2">
      <button
        @click="handleCopyLink"
        class="btn-outline text-xs px-3.5 py-2"
        title="复制文章链接"
      >
        <Link2 class="w-4 h-4 text-zinc-400" />
        <span>复制链接</span>
      </button>
    </div>
  </div>
</template>
