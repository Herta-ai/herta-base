<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import type { BlogPost } from '../../types/blog'
import { useBlogStore } from '../../stores/blog'
import { Sparkles, ArrowRight, Clock } from 'lucide-vue-next'
import { calculateReadingTime, formatDate } from '../../lib/utils'

const props = defineProps<{
  currentPostId: string
  category?: string
  tags?: string[]
}>()

const router = useRouter()
const blogStore = useBlogStore()

const relatedPosts = computed<BlogPost[]>(() => {
  return blogStore.posts
    .filter(p => p.id !== props.currentPostId && p.is_public !== false)
    .filter(p => p.category === props.category || p.tags?.some(t => props.tags?.includes(t)))
    .slice(0, 3)
})

const goToPost = (post: BlogPost) => {
  router.push(`/post/${post.slug || post.id}`)
}
</script>

<template>
  <div v-if="relatedPosts.length > 0" class="pt-8 space-y-4">
    <div class="flex items-center gap-2">
      <Sparkles class="w-4 h-4 text-emerald-500" />
      <h3 class="text-sm font-bold uppercase tracking-wider text-zinc-900 dark:text-zinc-100">
        相关推荐阅读
      </h3>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
      <div
        v-for="post in relatedPosts"
        :key="post.id"
        @click="goToPost(post)"
        class="card-base card-hover p-4 cursor-pointer flex flex-col justify-between group overflow-hidden"
      >
        <div class="space-y-2">
          <div class="h-32 rounded-xl overflow-hidden bg-zinc-100 dark:bg-zinc-800 mb-2">
            <img
              :src="post.cover_image"
              :alt="post.title"
              class="w-full h-full object-cover group-hover:scale-105 transition duration-500"
            />
          </div>
          <span v-if="post.category" class="badge-base bg-emerald-50 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-400 text-[10px]">
            {{ post.category }}
          </span>
          <h4 class="text-xs sm:text-sm font-bold text-zinc-800 dark:text-zinc-200 group-hover:text-emerald-600 dark:group-hover:text-emerald-400 line-clamp-2 leading-snug transition">
            {{ post.title }}
          </h4>
        </div>

        <div class="pt-3 mt-2 border-t border-zinc-100 dark:border-zinc-800 flex items-center justify-between text-[11px] text-zinc-400">
          <span>{{ calculateReadingTime(post.content) }}</span>
          <ArrowRight class="w-3.5 h-3.5 text-zinc-400 group-hover:text-emerald-500 group-hover:translate-x-0.5 transition" />
        </div>
      </div>
    </div>
  </div>
</template>
