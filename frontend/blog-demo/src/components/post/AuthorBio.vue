<script setup lang="ts">
import { getAuthorAvatar } from '../../lib/utils'
import { Sparkles, Globe, Mail } from 'lucide-vue-next'

const props = withDefaults(
  defineProps<{
    name?: string
    avatar?: string
    role?: string
    bio?: string
  }>(),
  {
    name: '黑塔空间站主管',
    role: '站长 / 核心架构师',
    bio: '专注于现代分布式数据库、Rust 高性能架构与 AI Agent 全栈生态构建。与您共同探索更高效的软件工程世界。',
  }
)
</script>

<template>
  <div class="card-base p-6 sm:p-8 flex flex-col sm:flex-row items-center sm:items-start gap-5 bg-gradient-to-br from-zinc-50 to-emerald-50/20 dark:from-zinc-900 dark:to-zinc-900/60 border-zinc-200 dark:border-zinc-800">
    <img
      :src="getAuthorAvatar(name, avatar)"
      :alt="name"
      class="w-16 h-16 sm:w-20 sm:h-20 rounded-2xl object-cover shadow-md border-2 border-white dark:border-zinc-800 shrink-0"
    />

    <div class="space-y-2 text-center sm:text-left flex-1">
      <div class="flex flex-col sm:flex-row sm:items-center gap-1 sm:gap-3">
        <h4 class="text-base font-bold text-zinc-900 dark:text-zinc-100">{{ name }}</h4>
        <span class="badge-base bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300 w-fit mx-auto sm:mx-0">
          <Sparkles class="w-3 h-3" />
          {{ role }}
        </span>
      </div>

      <p class="text-xs sm:text-sm text-zinc-600 dark:text-zinc-400 leading-relaxed">
        {{ bio }}
      </p>

      <div class="pt-2 flex items-center justify-center sm:justify-start gap-3 text-xs text-zinc-500">
        <a href="https://herta.ai" target="_blank" class="flex items-center gap-1 hover:text-emerald-500 transition">
          <Globe class="w-3.5 h-3.5" />
          <span>个人主页</span>
        </a>
        <span>&bull;</span>
        <a href="mailto:admin@herta.ai" class="flex items-center gap-1 hover:text-emerald-500 transition">
          <Mail class="w-3.5 h-3.5" />
          <span>联系作者</span>
        </a>
      </div>
    </div>
  </div>
</template>
