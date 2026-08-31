<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'
import { useBlogStore } from '../stores/blog'
import { useThemeStore } from '../stores/theme'
import { User, Mail, Globe, Save, Lock, PenSquare, Eye, ShieldCheck, Sparkles } from 'lucide-vue-next'
import { getAuthorAvatar, formatDate } from '../lib/utils'

const router = useRouter()
const authStore = useAuthStore()
const blogStore = useBlogStore()
const themeStore = useThemeStore()

const profileForm = reactive({
  displayName: authStore.user?.displayName || '',
  bio: authStore.user?.bio || '热爱技术，乐于分享。',
  website: authStore.user?.website || 'https://herta.ai',
})

const myPosts = computed(() => {
  if (!authStore.user) return []
  return blogStore.posts.filter(p => p.author === authStore.user?.id || p.author?.includes(authStore.user?.displayName || ''))
})

const handleSaveProfile = () => {
  if (!authStore.user) return
  authStore.user.displayName = profileForm.displayName
  authStore.user.bio = profileForm.bio
  authStore.user.website = profileForm.website
  localStorage.setItem('herta_blog_user', JSON.stringify(authStore.user))
  themeStore.addToast({ type: 'success', message: '个人资料已更新！' })
}
</script>

<template>
  <div class="max-w-4xl mx-auto space-y-8 animate-fade-in">
    <!-- 未登录提示卡片 -->
    <div v-if="!authStore.isAuthenticated" class="card-base p-16 text-center space-y-4">
      <User class="w-12 h-12 mx-auto text-emerald-500 opacity-60" />
      <h2 class="text-xl font-bold text-zinc-800 dark:text-zinc-200">请先登录您的创作者账号</h2>
      <p class="text-xs text-zinc-500">登录后即可管理个人资料、查看我的文章与评论互动。</p>
      <button @click="themeStore.openAuth('login')" class="btn-primary text-xs px-5 py-2.5">
        立即登录账号
      </button>
    </div>

    <!-- 已登录个人中心 -->
    <div v-else class="space-y-8">
      <!-- 用户资料主卡片 -->
      <div class="card-base p-8 flex flex-col sm:flex-row items-center sm:items-start gap-6 bg-gradient-to-r from-emerald-500/10 via-teal-500/10 to-transparent dark:from-emerald-950/30 dark:via-zinc-900/50 dark:to-transparent">
        <img
          :src="getAuthorAvatar(authStore.user?.displayName, authStore.user?.avatar)"
          :alt="authStore.user?.displayName"
          class="w-20 h-20 rounded-full object-cover shadow-md border-2 border-white dark:border-zinc-800 shrink-0"
        />

        <div class="space-y-2 text-center sm:text-left flex-1">
          <div class="flex flex-col sm:flex-row sm:items-center gap-2">
            <h1 class="text-2xl font-bold text-zinc-900 dark:text-zinc-100">{{ authStore.user?.displayName }}</h1>
            <span class="badge-base bg-emerald-100 text-emerald-700 dark:bg-emerald-950 dark:text-emerald-300 w-fit mx-auto sm:mx-0">
              <ShieldCheck class="w-3.5 h-3.5" />
              {{ authStore.user?.role || '创作者' }}
            </span>
          </div>

          <div class="text-xs text-zinc-500 flex items-center justify-center sm:justify-start gap-1">
            <Mail class="w-3.5 h-3.5" />
            <span>{{ authStore.user?.email }}</span>
          </div>

          <p class="text-xs sm:text-sm text-zinc-600 dark:text-zinc-400">
            {{ profileForm.bio }}
          </p>
        </div>

        <router-link to="/editor" class="btn-primary text-xs px-4 py-2 shrink-0">
          <PenSquare class="w-3.5 h-3.5" />
          发布新文章
        </router-link>
      </div>

      <!-- 修改个人资料卡片 -->
      <div class="card-base p-6 space-y-4">
        <h3 class="text-sm font-bold text-zinc-900 dark:text-zinc-100 pb-2 border-b border-zinc-100 dark:border-zinc-800">
          编辑个人信息
        </h3>

        <form @submit.prevent="handleSaveProfile" class="space-y-4">
          <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div class="space-y-1">
              <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">显示昵称</label>
              <input v-model="profileForm.displayName" type="text" class="input-base text-xs" required />
            </div>

            <div class="space-y-1">
              <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">个人网站</label>
              <input v-model="profileForm.website" type="url" class="input-base text-xs" />
            </div>
          </div>

          <div class="space-y-1">
            <label class="text-xs font-semibold text-zinc-700 dark:text-zinc-300">个人简介 / Bio</label>
            <textarea v-model="profileForm.bio" rows="3" class="input-base text-xs resize-none"></textarea>
          </div>

          <div class="flex justify-end">
            <button type="submit" class="btn-primary text-xs px-4 py-2">
              <Save class="w-3.5 h-3.5" />
              保存更改
            </button>
          </div>
        </form>
      </div>

      <!-- 我的文章列表 -->
      <div class="card-base p-6 space-y-4">
        <div class="flex items-center justify-between pb-2 border-b border-zinc-100 dark:border-zinc-800">
          <h3 class="text-sm font-bold text-zinc-900 dark:text-zinc-100">
            我的创作文章 ({{ myPosts.length }})
          </h3>
          <router-link to="/admin" class="text-xs text-emerald-600 dark:text-emerald-400 font-semibold hover:text-emerald-500 transition">
            进入管理工作台 &rarr;
          </router-link>
        </div>

        <div v-if="myPosts.length > 0" class="divide-y divide-zinc-100 dark:divide-zinc-800">
          <div
            v-for="post in myPosts"
            :key="post.id"
            class="py-3 flex items-center justify-between gap-4"
          >
            <div class="min-w-0">
              <router-link :to="`/post/${post.slug || post.id}`" class="text-xs sm:text-sm font-bold text-zinc-800 dark:text-zinc-200 hover:text-emerald-500 transition line-clamp-1">
                {{ post.title }}
              </router-link>
              <div class="text-[11px] text-zinc-400 mt-0.5">
                {{ formatDate(post.created_at) }} &bull; {{ post.is_public ? '已公开发布' : '私密草稿' }}
              </div>
            </div>

            <div class="flex items-center gap-2 shrink-0">
              <router-link :to="`/editor?id=${post.id}`" class="btn-secondary text-[11px] px-2.5 py-1">
                编辑
              </router-link>
            </div>
          </div>
        </div>

        <div v-else class="py-8 text-center text-xs text-zinc-400">
          您尚未发布任何文章，快点击上方按钮开始创作吧！
        </div>
      </div>
    </div>
  </div>
</template>
