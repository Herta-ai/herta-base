<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useBlogStore } from '../stores/blog'
import { Archive, Calendar, Layers, Tag, ArrowRight, Clock } from 'lucide-vue-next'
import { formatDate, calculateReadingTime } from '../lib/utils'
import dayjs from 'dayjs'

const router = useRouter()
const blogStore = useBlogStore()

interface ArchiveGroup {
  year: string
  months: {
    month: string
    posts: any[]
  }[]
}

const archiveGroups = computed<ArchiveGroup[]>(() => {
  const map = new Map<string, Map<string, any[]>>()

  const publicPosts = blogStore.posts
    .filter(p => p.is_public !== false)
    .sort((a, b) => new Date(b.created_at || '').getTime() - new Date(a.created_at || '').getTime())

  publicPosts.forEach(post => {
    const d = dayjs(post.created_at)
    const year = d.format('YYYY年')
    const month = d.format('M月')

    if (!map.has(year)) {
      map.set(year, new Map())
    }
    const monthMap = map.get(year)!
    if (!monthMap.has(month)) {
      monthMap.set(month, [])
    }
    monthMap.get(month)!.push(post)
  })

  const result: ArchiveGroup[] = []
  map.forEach((monthMap, year) => {
    const months: { month: string; posts: any[] }[] = []
    monthMap.forEach((posts, month) => {
      months.push({ month, posts })
    })
    result.push({ year, months })
  })

  return result
})

const goToPost = (post: any) => {
  router.push(`/post/${post.slug || post.id}`)
}
</script>

<template>
  <div class="max-w-4xl mx-auto space-y-10 animate-fade-in">
    <!-- 头部统计卡片 -->
    <div class="card-base p-8 bg-gradient-to-r from-emerald-500/10 via-teal-500/10 to-indigo-500/10 dark:from-emerald-950/30 dark:via-zinc-900 dark:to-zinc-900 border-emerald-200/50 dark:border-emerald-900/50">
      <div class="space-y-4">
        <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full text-xs font-bold bg-emerald-500 text-white shadow-sm">
          <Archive class="w-3.5 h-3.5" />
          时间轴归档
        </div>

        <h1 class="text-2xl sm:text-3xl font-extrabold text-zinc-900 dark:text-zinc-100">
          全站文章历史归档
        </h1>

        <p class="text-xs sm:text-sm text-zinc-600 dark:text-zinc-400">
          记录每一个技术灵感与思考沉淀的轨迹。
        </p>

        <div class="pt-3 flex flex-wrap items-center gap-6 text-xs text-zinc-600 dark:text-zinc-400">
          <div><strong class="text-emerald-600 dark:text-emerald-400 text-base">{{ blogStore.posts.filter(p => p.is_public !== false).length }}</strong> 篇公开发布</div>
          <div><strong class="text-blue-600 dark:text-blue-400 text-base">{{ blogStore.categories.length }}</strong> 个专栏</div>
          <div><strong class="text-purple-600 dark:text-purple-400 text-base">{{ blogStore.tags.length }}</strong> 个标签</div>
        </div>
      </div>
    </div>

    <!-- 年月时间轴列表 -->
    <div class="space-y-12 pl-4 sm:pl-8 border-l-2 border-zinc-200 dark:border-zinc-800 ml-4 sm:ml-6">
      <div
        v-for="group in archiveGroups"
        :key="group.year"
        class="space-y-8 relative"
      >
        <!-- 年份标记点 -->
        <div class="flex items-center gap-3 -ml-[25px] sm:-ml-[41px]">
          <div class="w-5 h-5 rounded-full bg-emerald-500 ring-4 ring-white dark:ring-zinc-950"></div>
          <h2 class="text-xl sm:text-2xl font-black text-zinc-900 dark:text-zinc-100">
            {{ group.year }}
          </h2>
        </div>

        <!-- 月份分组 -->
        <div
          v-for="monthGroup in group.months"
          :key="monthGroup.month"
          class="space-y-4 ml-2 sm:ml-4"
        >
          <h3 class="text-sm font-bold text-emerald-600 dark:text-emerald-400">
            {{ monthGroup.month }}
          </h3>

          <div class="space-y-3">
            <div
              v-for="post in monthGroup.posts"
              :key="post.id"
              @click="goToPost(post)"
              class="card-base card-hover p-4 cursor-pointer flex flex-col sm:flex-row sm:items-center justify-between gap-3 group"
            >
              <div class="flex items-center gap-3">
                <span class="text-xs font-mono text-zinc-400 shrink-0">
                  {{ dayjs(post.created_at).format('DD日') }}
                </span>
                <span v-if="post.category" class="px-2 py-0.5 rounded text-[11px] font-bold bg-zinc-100 dark:bg-zinc-800 text-emerald-600 dark:text-emerald-400 shrink-0">
                  {{ post.category }}
                </span>
                <h4 class="text-sm font-semibold text-zinc-800 dark:text-zinc-200 group-hover:text-emerald-600 dark:group-hover:text-emerald-400 transition leading-snug">
                  {{ post.title }}
                </h4>
              </div>

              <div class="flex items-center gap-3 text-xs text-zinc-400 shrink-0 self-end sm:self-center">
                <span>{{ calculateReadingTime(post.content) }}</span>
                <ArrowRight class="w-4 h-4 text-zinc-400 group-hover:text-emerald-500 group-hover:translate-x-1 transition" />
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
