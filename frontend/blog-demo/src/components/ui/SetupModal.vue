<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useThemeStore } from '../../stores/theme'
import { useBlogStore } from '../../stores/blog'
import Modal from './Modal.vue'
import { Database, CheckCircle2, AlertCircle, RefreshCw, Sparkles, Server, ArrowRight } from 'lucide-vue-next'

const router = useRouter()
const themeStore = useThemeStore()
const blogStore = useBlogStore()

const checking = ref(false)

const handleCheckStatus = async () => {
  checking.value = true
  await blogStore.init()
  checking.value = false
  themeStore.addToast({
    type: blogStore.isServerLive ? 'success' : 'info',
    message: blogStore.isServerLive ? '已成功连接到 HertaBase 实例！' : '当前运行在本地离线演示模式',
  })
}

const goToSetupPage = () => {
  themeStore.closeSetup()
  router.push('/setup')
}
</script>

<template>
  <Modal
    v-model="themeStore.setupModalOpen"
    title="HertaBase 数据底座状态与初始化"
    max-width="max-w-xl"
  >
    <div class="space-y-5">
      <!-- 状态概览卡片 -->
      <div class="p-4 rounded-2xl bg-zinc-50 dark:bg-zinc-800/60 border border-zinc-200 dark:border-zinc-700 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="p-2.5 rounded-xl bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
            <Server class="w-6 h-6" />
          </div>
          <div>
            <div class="text-sm font-semibold text-zinc-900 dark:text-zinc-100 flex items-center gap-2">
              后端连接状态:
              <span v-if="blogStore.isServerLive && blogStore.isCollectionsReady" class="badge-base bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300">
                <CheckCircle2 class="w-3.5 h-3.5" /> 已就绪
              </span>
              <span v-else-if="blogStore.isServerLive" class="badge-base bg-blue-100 text-blue-700 dark:bg-blue-950 dark:text-blue-300">
                <CheckCircle2 class="w-3.5 h-3.5" /> 服务在线 (待初始化集合)
              </span>
              <span v-else class="badge-base bg-amber-100 text-amber-700 dark:bg-amber-950 dark:text-amber-300">
                <AlertCircle class="w-3.5 h-3.5" /> 离线演示模式
              </span>
            </div>
            <div class="text-xs text-zinc-500 dark:text-zinc-400 mt-0.5">
              API Base URL: <code class="px-1.5 py-0.5 rounded bg-zinc-200 dark:bg-zinc-700 text-zinc-700 dark:text-zinc-300">/</code>
            </div>
          </div>
        </div>

        <button
          @click="handleCheckStatus"
          :disabled="checking"
          class="btn-secondary text-xs px-3 py-2"
        >
          <RefreshCw class="w-3.5 h-3.5" :class="{ 'animate-spin': checking }" />
          重新检测
        </button>
      </div>

      <!-- 集合配置说明 -->
      <div class="text-xs text-zinc-600 dark:text-zinc-400 space-y-2 leading-relaxed">
        <p class="font-medium text-zinc-800 dark:text-zinc-200">HertaBase 集合架构：</p>
        <ul class="list-disc pl-5 space-y-1">
          <li><code class="text-emerald-600 font-mono">blog_users</code>: 博客用户与管理员鉴权集合。</li>
          <li><code class="text-emerald-600 font-mono">blog_posts</code>: 文章标题、内容、分类、标签与行级 API Rules。</li>
          <li><code class="text-emerald-600 font-mono">blog_comments</code>: 互动评论与作者关联。</li>
        </ul>
      </div>

      <!-- 快捷操作区 -->
      <div class="pt-2 flex flex-col sm:flex-row gap-3">
        <button
          @click="goToSetupPage"
          class="flex-1 btn-primary text-sm py-2.5"
        >
          <Sparkles class="w-4 h-4" />
          <span>进入安装初始化向导</span>
          <ArrowRight class="w-3.5 h-3.5" />
        </button>

        <button
          @click="themeStore.closeSetup()"
          class="btn-secondary text-sm py-2.5"
        >
          关闭
        </button>
      </div>
    </div>
  </Modal>
</template>
