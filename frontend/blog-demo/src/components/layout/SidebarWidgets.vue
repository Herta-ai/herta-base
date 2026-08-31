<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useBlogStore } from '../../stores/blog'
import { useThemeStore } from '../../stores/theme'
import { Tag, Layers, Database, Sparkles, Send, CheckCircle2 } from 'lucide-vue-next'
import { getAuthorAvatar } from '../../lib/utils'

const router = useRouter()
const blogStore = useBlogStore()
const themeStore = useThemeStore()

const newsletterEmail = ref('')
const subscribed = ref(false)

const handleSubscribe = () => {
  if (!newsletterEmail.value) return
  subscribed.value = true
  themeStore.addToast({ type: 'success', message: '订阅成功！新文章发布时将第一时间通知您。' })
  setTimeout(() => {
    newsletterEmail.value = ''
    subscribed.value = false
  }, 2000)
}

const selectCategory = (categoryName: string) => {
  blogStore.filter.category = blogStore.filter.category === categoryName ? undefined : categoryName
  blogStore.filter.page = 1
}

const selectTag = (tagName: string) => {
  blogStore.filter.tag = blogStore.filter.tag === tagName ? undefined : tagName
  blogStore.filter.page = 1
}
</script>

<template>
  <aside class="space-y-6">
    <!-- 作者名片卡片 -->
    <div class="card-base p-6 text-center relative overflow-hidden group">
      <div class="absolute top-0 left-0 right-0 h-16 bg-gradient-to-r from-emerald-500/20 via-teal-500/20 to-blue-500/20"></div>

      <div class="relative pt-4 flex flex-col items-center">
        <img
          :src="getAuthorAvatar('黑塔空间站主管')"
          alt="作者头像"
          class="w-20 h-20 rounded-full border-4 border-white dark:border-zinc-900 shadow-lg object-cover mb-3"
        />
        <h3 class="text-base font-bold text-zinc-900 dark:text-zinc-100">黑塔空间站主管</h3>
        <p class="text-xs text-emerald-600 dark:text-emerald-400 font-semibold mb-2">站长 / 核心架构师</p>
        <p class="text-xs text-zinc-500 dark:text-zinc-400 leading-relaxed max-w-xs mb-4">
          致力于高性能分布式存储、Rust 后端框架演进与全场景 AI 赋能的探索者。
        </p>

        <div class="flex items-center gap-2">
          <router-link to="/about" class="btn-secondary text-xs px-3 py-1.5">
            了解更多
          </router-link>
          <button @click="themeStore.openAuth('login')" class="btn-primary text-xs px-3 py-1.5">
            加入社区
          </button>
        </div>
      </div>
    </div>

    <!-- 专栏分类小组件 -->
    <div class="card-base p-5">
      <div class="flex items-center gap-2 mb-4">
        <Layers class="w-4 h-4 text-emerald-500" />
        <h3 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">专栏分类</h3>
      </div>

      <div class="space-y-1.5">
        <button
          v-for="cat in blogStore.computedCategories"
          :key="cat.id"
          @click="selectCategory(cat.name)"
          class="w-full flex items-center justify-between px-3.5 py-2.5 rounded-xl text-xs font-medium transition cursor-pointer"
          :class="blogStore.filter.category === cat.name
            ? 'bg-emerald-500 text-white font-bold shadow-sm shadow-emerald-500/20'
            : 'bg-zinc-100/80 dark:bg-zinc-800/80 text-zinc-700 dark:text-zinc-200 hover:bg-zinc-200/80 dark:hover:bg-zinc-700/80 border border-zinc-200/60 dark:border-zinc-700/60'"
        >
          <span class="flex items-center gap-2">
            <span class="w-2 h-2 rounded-full" :class="blogStore.filter.category === cat.name ? 'bg-white' : 'bg-emerald-500'"></span>
            {{ cat.name }}
          </span>
          <span
            class="px-2 py-0.5 rounded-full text-[11px] font-semibold transition"
            :class="blogStore.filter.category === cat.name
              ? 'bg-white/20 text-white'
              : 'bg-zinc-200 dark:bg-zinc-700 text-zinc-600 dark:text-zinc-300'"
          >
            {{ cat.count || 0 }} 篇
          </span>
        </button>
      </div>
    </div>

    <!-- 热门标签云 -->
    <div class="card-base p-5">
      <div class="flex items-center gap-2 mb-4">
        <Tag class="w-4 h-4 text-emerald-500" />
        <h3 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">热门标签云</h3>
      </div>

      <div class="flex flex-wrap gap-1.5">
        <button
          v-for="tag in blogStore.tags"
          :key="tag.name"
          @click="selectTag(tag.name)"
          class="badge-base py-1 px-2.5 transition cursor-pointer"
          :class="blogStore.filter.tag === tag.name
            ? 'bg-emerald-500 text-white shadow-xs font-bold'
            : 'bg-zinc-100 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 hover:bg-zinc-200 dark:hover:bg-zinc-700 hover:text-zinc-900 dark:hover:text-zinc-100'"
        >
          #{{ tag.name }}
          <span class="opacity-60 text-[10px]">({{ tag.count }})</span>
        </button>
      </div>
    </div>

    <!-- 邮件周刊订阅 -->
    <div class="card-base p-5 bg-gradient-to-br from-emerald-50/70 to-teal-50/40 dark:from-zinc-900 dark:to-zinc-900/80 border-emerald-100/80 dark:border-zinc-800">
      <div class="flex items-center gap-2 mb-2 text-emerald-600 dark:text-emerald-400">
        <Sparkles class="w-4 h-4" />
        <h3 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">技术周刊订阅</h3>
      </div>
      <p class="text-xs text-zinc-500 dark:text-zinc-400 leading-relaxed mb-3">
        每月精选系统架构、Rust 与全栈技术深度好文，直达您的邮箱。
      </p>

      <form @submit.prevent="handleSubscribe" class="space-y-2 w-full">
        <input
          v-model="newsletterEmail"
          type="email"
          placeholder="your@email.com"
          class="input-base text-xs py-2 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-700"
          required
        />
        <button
          type="submit"
          :disabled="subscribed"
          class="w-full btn-primary text-xs py-2 shadow-xs"
        >
          <CheckCircle2 v-if="subscribed" class="w-3.5 h-3.5" />
          <Send v-else class="w-3.5 h-3.5" />
          {{ subscribed ? '已成功订阅' : '免费订阅' }}
        </button>
      </form>
    </div>

    <!-- HertaBase 状态快捷组件 -->
    <div class="card-base p-4 border-dashed border-zinc-200 dark:border-zinc-800 bg-zinc-50/60 dark:bg-zinc-900/60 flex items-center justify-between text-xs text-zinc-500 dark:text-zinc-400">
      <div class="flex items-center gap-2">
        <Database class="w-4 h-4 text-emerald-500" />
        <span class="font-medium text-zinc-700 dark:text-zinc-300">底座: HertaBase</span>
      </div>
      <button
        @click="themeStore.openSetup()"
        class="px-2.5 py-1 rounded-lg bg-emerald-50 dark:bg-emerald-950/60 text-emerald-600 dark:text-emerald-400 font-semibold hover:bg-emerald-100 dark:hover:bg-emerald-900/60 border border-emerald-200/50 dark:border-emerald-800/50 transition cursor-pointer shadow-2xs"
      >
        状态与设置 &rarr;
      </button>
    </div>
  </aside>
</template>
