<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'
import { useBlogStore } from '../stores/blog'
import { useThemeStore } from '../stores/theme'
import {
  performDatabaseInitialization,
  isDatabaseInitialized,
  isHertaError,
} from '../lib/hb'
import {
  Database,
  Server,
  ShieldCheck,
  User,
  Mail,
  Lock,
  Sparkles,
  CheckCircle2,
  AlertCircle,
  ArrowRight,
  RefreshCw,
  FileText,
  KeyRound,
} from 'lucide-vue-next'
import confetti from 'canvas-confetti'

const router = useRouter()
const authStore = useAuthStore()
const blogStore = useBlogStore()
const themeStore = useThemeStore()

const checking = ref(true)
const isAlreadyInitialized = ref(false)
const activeStep = ref<1 | 2 | 3>(1)
const loading = ref(false)
const progressMessage = ref('')
const errorMessage = ref('')

const form = reactive({
  // 1. HertaBase 后端管理员账号
  hbAdminEmail: 'admin@example.com',
  hbAdminPassword: 'correct horse battery staple',

  // 2. 博客超级管理员账号
  blogAdminEmail: 'admin@herta.ai',
  blogAdminPassword: 'password123',
  blogAdminName: '黑塔站长',
  blogAdminBio: '博客超级管理员，专注于系统架构与技术思考沉淀。',
})

onMounted(async () => {
  checking.value = true
  try {
    const ready = await isDatabaseInitialized()
    isAlreadyInitialized.value = ready
  } catch {
    isAlreadyInitialized.value = false
  } finally {
    checking.value = false
  }
})

const handleStartInitialization = async () => {
  if (!form.hbAdminEmail || !form.hbAdminPassword) {
    themeStore.addToast({ type: 'warning', message: '请填写后端管理员邮箱与密码' })
    activeStep.value = 1
    return
  }

  if (!form.blogAdminEmail || !form.blogAdminPassword || !form.blogAdminName) {
    themeStore.addToast({ type: 'warning', message: '请完整填写博客超管账号信息' })
    activeStep.value = 2
    return
  }

  loading.value = true
  errorMessage.value = ''
  activeStep.value = 3

  try {
    const result = await performDatabaseInitialization({
      hbAdminEmail: form.hbAdminEmail,
      hbAdminPassword: form.hbAdminPassword,
      blogAdminEmail: form.blogAdminEmail,
      blogAdminPassword: form.blogAdminPassword,
      blogAdminName: form.blogAdminName,
      blogAdminBio: form.blogAdminBio,
      onProgress: (msg) => {
        progressMessage.value = msg
      },
    })

    progressMessage.value = '🎉 全部初始化完成！正在为您自动登录...'

    // 自动更新 auth store 用户状态
    authStore.user = result.superAdminUser
    localStorage.setItem('herta_blog_user', JSON.stringify(result.superAdminUser))

    // 刷新文章列表
    await blogStore.fetchRemotePosts()

    // 庆祝纸屑动效
    try {
      confetti({
        particleCount: 100,
        spread: 80,
        origin: { y: 0.55 },
      })
    } catch {}

    themeStore.addToast({
      type: 'success',
      message: 'HertaBase 数据库与博客超级管理员已初始化成功！',
    })

    setTimeout(() => {
      router.push('/')
    }, 1200)
  } catch (err: any) {
    const msg = isHertaError(err) ? err.message : (err?.message || '初始化失败，请检查账号密码或后端连接')
    errorMessage.value = msg
    themeStore.addToast({ type: 'error', message: msg })
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="max-w-3xl mx-auto py-6 space-y-8 animate-fade-in">
    <!-- 向导顶部 Banner -->
    <div class="text-center space-y-3">
      <div class="inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full text-xs font-bold bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20 shadow-xs">
        <Sparkles class="w-4 h-4" />
        HertaBase 数据库与博客系统安装向导
      </div>
      <h1 class="text-3xl sm:text-4xl font-extrabold text-zinc-900 dark:text-zinc-100 tracking-tight">
        欢迎使用 HertaBlog
      </h1>
      <p class="text-xs sm:text-sm text-zinc-500 dark:text-zinc-400 max-w-lg mx-auto leading-relaxed">
        只需简单两步，即可自动在 HertaBase 后端创建所需数据集合、配置行级安全规则、创建博客超级管理员账号并发布首篇欢迎文章。
      </p>
    </div>

    <!-- 加载中检测 -->
    <div v-if="checking" class="card-base p-12 text-center space-y-3">
      <RefreshCw class="w-8 h-8 mx-auto text-emerald-500 animate-spin" />
      <p class="text-xs text-zinc-400">正在检测 HertaBase 后端数据库状态...</p>
    </div>

    <!-- 已初始化状态卡片 -->
    <div v-else-if="isAlreadyInitialized && !loading && activeStep !== 3" class="card-base p-8 border-emerald-200 dark:border-emerald-800 bg-emerald-50/40 dark:bg-emerald-950/20 space-y-4">
      <div class="flex items-center gap-3">
        <div class="p-2.5 rounded-xl bg-emerald-500 text-white">
          <CheckCircle2 class="w-6 h-6" />
        </div>
        <div>
          <h3 class="text-base font-bold text-emerald-900 dark:text-emerald-200">
            HertaBase 后端数据库已初始化完毕
          </h3>
          <p class="text-xs text-emerald-700 dark:text-emerald-400 mt-0.5">
            所有博客数据集合 (blog_users, blog_posts, blog_comments) 已就绪，您可以直接进入博客或重新执行覆盖初始化。
          </p>
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-3 pt-2">
        <router-link to="/" class="btn-primary text-xs px-4 py-2">
          进入博客首页
        </router-link>
        <button
          @click="isAlreadyInitialized = false"
          class="btn-secondary text-xs px-3.5 py-2"
        >
          重新配置与初始化
        </button>
      </div>
    </div>

    <!-- 主表单向导卡片 -->
    <div v-else class="card-base p-6 sm:p-10 space-y-8">
      <!-- 步骤指示器 -->
      <div class="flex items-center justify-between border-b border-zinc-100 dark:border-zinc-800 pb-5">
        <button
          @click="activeStep = 1"
          class="flex items-center gap-2 text-xs font-bold transition"
          :class="activeStep === 1 ? 'text-emerald-600 dark:text-emerald-400' : 'text-zinc-400'"
        >
          <span class="w-6 h-6 rounded-full flex-center text-xs" :class="activeStep === 1 ? 'bg-emerald-500 text-white shadow-xs' : 'bg-zinc-200 dark:bg-zinc-800'">1</span>
          <span>后端管理员授权</span>
        </button>

        <span class="w-8 h-px bg-zinc-200 dark:bg-zinc-800"></span>

        <button
          @click="activeStep = 2"
          class="flex items-center gap-2 text-xs font-bold transition"
          :class="activeStep === 2 ? 'text-emerald-600 dark:text-emerald-400' : 'text-zinc-400'"
        >
          <span class="w-6 h-6 rounded-full flex-center text-xs" :class="activeStep === 2 ? 'bg-emerald-500 text-white shadow-xs' : 'bg-zinc-200 dark:bg-zinc-800'">2</span>
          <span>博客超管账号</span>
        </button>

        <span class="w-8 h-px bg-zinc-200 dark:bg-zinc-800"></span>

        <div
          class="flex items-center gap-2 text-xs font-bold"
          :class="activeStep === 3 ? 'text-emerald-600 dark:text-emerald-400' : 'text-zinc-400'"
        >
          <span class="w-6 h-6 rounded-full flex-center text-xs" :class="activeStep === 3 ? 'bg-emerald-500 text-white shadow-xs' : 'bg-zinc-200 dark:bg-zinc-800'">3</span>
          <span>执行初始化</span>
        </div>
      </div>

      <!-- 步骤 1：HertaBase 后端管理员账号 -->
      <div v-show="activeStep === 1" class="space-y-5">
        <div class="space-y-1">
          <div class="flex items-center gap-2 text-sm font-bold text-zinc-900 dark:text-zinc-100">
            <Server class="w-4 h-4 text-emerald-500" />
            第一步：输入 HertaBase 后端管理员凭证
          </div>
          <p class="text-xs text-zinc-500 leading-relaxed">
            用于调用 HertaBase 管理接口（<code>/_/collections</code>）自动创建数据库表和 API 安全规则。
          </p>
        </div>

        <div class="space-y-4 pt-1">
          <div class="space-y-1.5">
            <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">后端管理员邮箱 (Admin Email)</label>
            <div class="relative">
              <Mail class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
              <input
                v-model="form.hbAdminEmail"
                type="email"
                placeholder="admin@example.com"
                class="input-base pl-10 text-xs sm:text-sm font-mono"
                required
              />
            </div>
          </div>

          <div class="space-y-1.5">
            <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">后端管理员密码 (Admin Password)</label>
            <div class="relative">
              <KeyRound class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
              <input
                v-model="form.hbAdminPassword"
                type="password"
                placeholder="••••••••••••"
                class="input-base pl-10 text-xs sm:text-sm font-mono"
                required
              />
            </div>
          </div>
        </div>

        <div class="flex justify-end pt-4">
          <button
            @click="activeStep = 2"
            type="button"
            class="btn-primary text-xs px-5 py-2.5"
          >
            <span>下一步：配置博客超管</span>
            <ArrowRight class="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      <!-- 步骤 2：博客超级管理员账号 -->
      <div v-show="activeStep === 2" class="space-y-5">
        <div class="space-y-1">
          <div class="flex items-center gap-2 text-sm font-bold text-zinc-900 dark:text-zinc-100">
            <ShieldCheck class="w-4 h-4 text-emerald-500" />
            第二步：设置博客超级管理员 (站长) 账号
          </div>
          <p class="text-xs text-zinc-500 leading-relaxed">
            该账号将注册到 <code>blog_users</code> 集合，具备前台文章写作、编辑、全量管理与删除权限。
          </p>
        </div>

        <div class="space-y-4 pt-1">
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div class="space-y-1.5">
              <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">博客超管邮箱 (用于登录)</label>
              <div class="relative">
                <Mail class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
                <input
                  v-model="form.blogAdminEmail"
                  type="email"
                  placeholder="admin@herta.ai"
                  class="input-base pl-10 text-xs sm:text-sm font-mono"
                  required
                />
              </div>
            </div>

            <div class="space-y-1.5">
              <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">博客超管密码</label>
              <div class="relative">
                <Lock class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
                <input
                  v-model="form.blogAdminPassword"
                  type="password"
                  placeholder="password123"
                  class="input-base pl-10 text-xs sm:text-sm font-mono"
                  required
                />
              </div>
            </div>
          </div>

          <div class="space-y-1.5">
            <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">站长显示昵称</label>
            <div class="relative">
              <User class="w-4 h-4 text-zinc-400 absolute left-3.5 top-3" />
              <input
                v-model="form.blogAdminName"
                type="text"
                placeholder="黑塔空间站主管"
                class="input-base pl-10 text-xs sm:text-sm"
                required
              />
            </div>
          </div>

          <div class="space-y-1.5">
            <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">站长个人简介</label>
            <textarea
              v-model="form.blogAdminBio"
              rows="2"
              placeholder="记录技术演进与架构思考..."
              class="input-base text-xs resize-none"
            ></textarea>
          </div>
        </div>

        <div class="flex items-center justify-between pt-4">
          <button
            @click="activeStep = 1"
            type="button"
            class="btn-secondary text-xs px-4 py-2"
          >
            返回上一步
          </button>

          <button
            @click="handleStartInitialization"
            type="button"
            :disabled="loading"
            class="btn-primary text-xs px-6 py-2.5 shadow-md"
          >
            <Sparkles class="w-4 h-4" />
            <span>一键初始化并发布首篇文章</span>
          </button>
        </div>
      </div>

      <!-- 步骤 3：初始化进度与结果 -->
      <div v-show="activeStep === 3" class="space-y-6 text-center py-6">
        <div v-if="loading" class="space-y-4">
          <RefreshCw class="w-12 h-12 mx-auto text-emerald-500 animate-spin" />
          <h3 class="text-base font-bold text-zinc-900 dark:text-zinc-100">
            正在初始化 HertaBase 数据底座...
          </h3>
          <p class="text-xs font-mono text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-950/40 py-2 px-4 rounded-xl inline-block">
            {{ progressMessage }}
          </p>
        </div>

        <div v-else-if="errorMessage" class="space-y-4">
          <AlertCircle class="w-12 h-12 mx-auto text-rose-500" />
          <h3 class="text-base font-bold text-rose-600 dark:text-rose-400">
            初始化过程中遇到错误
          </h3>
          <div class="p-4 rounded-xl bg-rose-50 dark:bg-rose-950/40 border border-rose-200 dark:border-rose-800 text-xs text-rose-700 dark:text-rose-300 font-mono text-left max-w-md mx-auto">
            {{ errorMessage }}
          </div>
          <div class="pt-2 flex justify-center gap-3">
            <button @click="activeStep = 1" class="btn-secondary text-xs px-4 py-2">
              返回检查管理员配置
            </button>
            <button @click="handleStartInitialization" class="btn-primary text-xs px-4 py-2">
              重新尝试初始化
            </button>
          </div>
        </div>

        <div v-else class="space-y-4">
          <CheckCircle2 class="w-12 h-12 mx-auto text-emerald-500" />
          <h3 class="text-lg font-bold text-zinc-900 dark:text-zinc-100">
            全部初始化完成！
          </h3>
          <p class="text-xs text-zinc-500">
            已成功创建 <code>blog_users</code>, <code>blog_posts</code>, <code>blog_comments</code> 集合并发布《世界，你好！》文章。
          </p>
          <router-link to="/" class="btn-primary text-xs px-6 py-2.5 inline-flex">
            立即进入博客首页 &rarr;
          </router-link>
        </div>
      </div>
    </div>
  </div>
</template>
