<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import type { BlogPost } from '../../types/blog'
import { useBlogStore } from '../../stores/blog'
import { Calendar, Clock, Eye, Heart, Tag } from 'lucide-vue-next'
import { formatDate, calculateReadingTime, getAuthorAvatar } from '../../lib/utils'

const props = defineProps<{
  post: BlogPost
  layout?: 'grid' | 'list'
}>()

const router = useRouter()
const blogStore = useBlogStore()

const isLiked = computed(() => blogStore.likedPostIds.includes(props.post.id))

const goToPost = () => {
  router.push(`/post/${props.post.slug || props.post.id}`)
}

const handleLike = (e: Event) => {
  e.stopPropagation()
  blogStore.toggleLike(props.post.id)
}
</script>

<template>
  <article
    @click="goToPost"
    class="card-base card-hover cursor-pointer overflow-hidden flex flex-col group transition duration-300"
    :class="layout === 'list' ? 'sm:flex-row gap-6 p-5' : ''"
  >
    <!-- 封面图 -->
    <div
      class="relative overflow-hidden bg-zinc-100 dark:bg-zinc-800"
      :class="layout === 'list' ? 'sm:w-64 h-48 sm:h-auto rounded-xl shrink-0' : 'h-48 w-full'"
    >
      <img
        :src="post.cover_image || 'https://images.unsplash.com/photo-1517694712202-14dd9538aa97?w=800&auto=format&fit=crop&q=80'"
        :alt="post.title"
        class="w-full h-full object-cover group-hover:scale-105 transition-transform duration-500"
        loading="lazy"
      />
      <div v-if="post.category" class="absolute top-3 left-3">
        <span class="px-2.5 py-1 rounded-lg text-xs font-bold bg-white/90 dark:bg-zinc-900/90 backdrop-blur-md text-emerald-600 dark:text-emerald-400 shadow-sm">
          {{ post.category }}
        </span>
      </div>
      <div v-if="post.is_public === false" class="absolute top-3 right-3">
        <span class="px-2 py-0.5 rounded-md text-[10px] font-bold bg-amber-500 text-white shadow-sm">
          草稿
        </span>
      </div>
    </div>

    <!-- 文章主体内容 -->
    <div class="p-5 flex-1 flex flex-col justify-between" :class="layout === 'list' ? 'p-0 sm:py-1' : ''">
      <div class="space-y-2.5">
        <!-- 元数据（日期与阅读时长） -->
        <div class="flex items-center gap-3 text-xs text-zinc-400">
          <div class="flex items-center gap-1">
            <Calendar class="w-3.5 h-3.5" />
            <span>{{ formatDate(post.created_at) }}</span>
          </div>
          <span>&bull;</span>
          <div class="flex items-center gap-1">
            <Clock class="w-3.5 h-3.5" />
            <span>{{ calculateReadingTime(post.content) }}</span>
          </div>
        </div>

        <!-- 标题 -->
        <h3 class="text-base sm:text-lg font-bold text-zinc-900 dark:text-zinc-100 group-hover:text-emerald-600 dark:group-hover:text-emerald-400 transition-colors line-clamp-2 leading-snug">
          {{ post.title }}
        </h3>

        <!-- 摘要 -->
        <p class="text-xs sm:text-sm text-zinc-500 dark:text-zinc-400 line-clamp-2 leading-relaxed">
          {{ post.excerpt || '暂无摘要描述...' }}
        </p>

        <!-- 标签列表 -->
        <div v-if="post.tags && post.tags.length" class="flex flex-wrap gap-1.5 pt-1">
          <span
            v-for="tag in post.tags.slice(0, 3)"
            :key="tag"
            class="badge-base bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-400 text-[11px]"
          >
            #{{ tag }}
          </span>
        </div>
      </div>

      <!-- 底部作者与互动信息 -->
      <div class="pt-4 mt-3 border-t border-zinc-100 dark:border-zinc-800/80 flex items-center justify-between">
        <div class="flex items-center gap-2">
          <img
            :src="getAuthorAvatar('作者')"
            alt="作者头像"
            class="w-6 h-6 rounded-full object-cover border border-zinc-200 dark:border-zinc-700"
          />
          <span class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">黑塔博客团队</span>
        </div>

        <div class="flex items-center gap-3 text-xs text-zinc-400">
          <span class="flex items-center gap-1">
            <Eye class="w-3.5 h-3.5" />
            {{ post.views || 0 }}
          </span>
          <button
            @click.stop="handleLike"
            class="flex items-center gap-1.5 px-2.5 py-1 rounded-full transition-all duration-200 cursor-pointer text-xs"
            :class="isLiked
              ? 'bg-rose-50 dark:bg-rose-950/60 text-rose-500 font-semibold border border-rose-200/60 dark:border-rose-900/60'
              : 'bg-zinc-100 dark:bg-zinc-800 text-zinc-500 dark:text-zinc-400 hover:text-rose-500 dark:hover:text-rose-400 hover:bg-rose-50/50 dark:hover:bg-rose-950/30 border border-zinc-200/50 dark:border-zinc-700/50'"
            title="喜欢/点赞"
          >
            <Heart class="w-3.5 h-3.5" :class="isLiked ? 'fill-rose-500 text-rose-500' : ''" />
            <span>{{ post.likes || 0 }}</span>
          </button>
        </div>
      </div>
    </div>
  </article>
</template>
