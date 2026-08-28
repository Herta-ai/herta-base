<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useBlogStore } from '../../stores/blog'
import { Sparkles, Clock, Calendar, ArrowRight } from 'lucide-vue-next'
import { formatDate, calculateReadingTime, getAuthorAvatar } from '../../lib/utils'

const router = useRouter()
const blogStore = useBlogStore()

const primaryPost = computed(() => {
  return blogStore.featuredPosts[0] || blogStore.posts[0]
})

const secondaryPosts = computed(() => {
  const list = blogStore.featuredPosts.slice(1)
  if (list.length >= 2) return list.slice(0, 2)
  return blogStore.posts.filter(p => p.id !== primaryPost.value?.id).slice(0, 2)
})

const goToPost = (post: any) => {
  if (!post) return
  router.push(`/post/${post.slug || post.id}`)
}
</script>

<template>
  <div v-if="primaryPost" class="mb-14">
    <div class="flex items-center gap-2 mb-4">
      <div class="p-1 rounded-md bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
        <Sparkles class="w-4 h-4" />
      </div>
      <h2 class="text-sm font-bold uppercase tracking-wider text-zinc-900 dark:text-zinc-100">
        编辑精选与头条
      </h2>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
      <!-- 主头条卡片 -->
      <div
        @click="goToPost(primaryPost)"
        class="lg:col-span-8 group relative rounded-3xl overflow-hidden card-base card-hover cursor-pointer min-h-[420px] flex flex-col justify-end p-6 sm:p-8 bg-zinc-900 border-zinc-700/50"
      >
        <!-- 背景图片与暗黑渐变遮罩 -->
        <img
          :src="primaryPost.cover_image"
          :alt="primaryPost.title"
          class="absolute inset-0 w-full h-full object-cover group-hover:scale-105 transition-transform duration-700 opacity-60 dark:opacity-40"
        />
        <div class="absolute inset-0 bg-gradient-to-t from-zinc-950 via-zinc-950/60 to-transparent"></div>

        <!-- 内容层 -->
        <div class="relative z-10 space-y-3.5">
          <div class="flex flex-wrap items-center gap-3">
            <span v-if="primaryPost.category" class="px-3 py-1 rounded-full text-xs font-bold bg-emerald-500 text-white shadow-md shadow-emerald-500/30">
              {{ primaryPost.category }}
            </span>
            <div class="flex items-center gap-1.5 text-xs font-medium text-zinc-300">
              <Calendar class="w-3.5 h-3.5" />
              <span>{{ formatDate(primaryPost.created_at) }}</span>
            </div>
            <div class="flex items-center gap-1.5 text-xs font-medium text-zinc-300">
              <Clock class="w-3.5 h-3.5" />
              <span>{{ calculateReadingTime(primaryPost.content) }}</span>
            </div>
          </div>

          <h3 class="text-xl sm:text-2xl lg:text-3xl font-extrabold text-white group-hover:text-emerald-300 transition-colors leading-tight">
            {{ primaryPost.title }}
          </h3>

          <p class="text-sm sm:text-base text-zinc-300 line-clamp-2 leading-relaxed max-w-2xl">
            {{ primaryPost.excerpt }}
          </p>

          <div class="pt-2 flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <img
                :src="getAuthorAvatar('黑塔主管')"
                alt="作者头像"
                class="w-7 h-7 rounded-full border border-white/30"
              />
              <span class="text-xs font-semibold text-zinc-200">黑塔空间站主管</span>
            </div>

            <span class="inline-flex items-center gap-1.5 text-xs font-bold text-emerald-400 group-hover:translate-x-1 transition">
              阅读全文 <ArrowRight class="w-3.5 h-3.5" />
            </span>
          </div>
        </div>
      </div>

      <!-- 次级精选卡片 (2 篇) -->
      <div class="lg:col-span-4 flex flex-col gap-6">
        <div
          v-for="subPost in secondaryPosts"
          :key="subPost.id"
          @click="goToPost(subPost)"
          class="flex-1 card-base card-hover p-5 flex flex-col justify-between cursor-pointer group relative overflow-hidden"
        >
          <div class="space-y-2.5">
            <div class="flex items-center justify-between">
              <span v-if="subPost.category" class="px-2.5 py-0.5 rounded-md text-[11px] font-bold bg-zinc-100 dark:bg-zinc-800 text-emerald-600 dark:text-emerald-400">
                {{ subPost.category }}
              </span>
              <span class="text-xs text-zinc-400">{{ formatDate(subPost.created_at) }}</span>
            </div>

            <h4 class="text-base font-bold text-zinc-900 dark:text-zinc-100 group-hover:text-emerald-600 dark:group-hover:text-emerald-400 transition-colors line-clamp-2">
              {{ subPost.title }}
            </h4>

            <p class="text-xs text-zinc-500 dark:text-zinc-400 line-clamp-2 leading-relaxed">
              {{ subPost.excerpt }}
            </p>
          </div>

          <div class="pt-3 border-t border-zinc-100 dark:border-zinc-800/80 flex items-center justify-between text-xs text-zinc-400">
            <span>{{ calculateReadingTime(subPost.content) }}</span>
            <span class="text-emerald-600 dark:text-emerald-400 font-semibold group-hover:translate-x-0.5 transition inline-flex items-center gap-1">
              查看 <ArrowRight class="w-3 h-3" />
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
