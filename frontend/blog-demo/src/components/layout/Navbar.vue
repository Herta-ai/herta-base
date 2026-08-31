<script setup lang="ts">
import { ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useThemeStore } from '../../stores/theme'
import { useAuthStore } from '../../stores/auth'
import {
  Search,
  Sun,
  Moon,
  PenSquare,
  User,
  LogOut,
  Sliders,
  Menu,
  X,
  BookOpen,
  Sparkles,
  Layers,
  Archive,
  Info,
} from 'lucide-vue-next'
import { getAuthorAvatar } from '../../lib/utils'

const router = useRouter()
const route = useRoute()
const themeStore = useThemeStore()
const authStore = useAuthStore()

const mobileMenuOpen = ref(false)
const userMenuOpen = ref(false)

const navLinks = [
  { name: '首页', path: '/', icon: BookOpen },
  { name: '专栏分类', path: '/categories', icon: Layers },
  { name: '文章归档', path: '/archives', icon: Archive },
  { name: '关于本站', path: '/about', icon: Info },
]

const handleLogout = async () => {
  userMenuOpen.value = false
  await authStore.logout()
}
</script>

<template>
  <header class="sticky top-0 z-40 w-full bg-white/80 dark:bg-zinc-950/80 backdrop-blur-md border-b border-zinc-200/80 dark:border-zinc-800/80 transition-colors">
    <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between gap-4">
      <!-- 品牌 Logo -->
      <router-link to="/" class="flex items-center gap-3 group shrink-0">
        <div class="w-9 h-9 rounded-xl bg-gradient-to-tr from-emerald-500 to-teal-600 flex-center text-white shadow-md shadow-emerald-500/20 group-hover:scale-105 transition">
          <Sparkles class="w-5 h-5" />
        </div>
        <div>
          <div class="text-base font-extrabold tracking-tight bg-gradient-to-r from-zinc-900 via-zinc-800 to-emerald-600 dark:from-white dark:via-zinc-100 dark:to-emerald-400 bg-clip-text text-transparent">
            HertaBlog
          </div>
          <div class="text-[10px] font-medium text-zinc-400 tracking-wider -mt-1 hidden sm:block">
            现代技术与思维空间
          </div>
        </div>
      </router-link>

      <!-- 桌面端导航链接 -->
      <nav class="hidden md:flex items-center gap-1">
        <router-link
          v-for="link in navLinks"
          :key="link.path"
          :to="link.path"
          class="px-3.5 py-2 rounded-xl text-sm font-medium transition flex items-center gap-1.5"
          :class="route.path === link.path
            ? 'bg-emerald-50 dark:bg-emerald-950/70 text-emerald-700 dark:text-emerald-300 font-semibold border border-emerald-200/60 dark:border-emerald-800/60'
            : 'text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-900 hover:text-zinc-900 dark:hover:text-white border border-transparent'"
        >
          <component :is="link.icon" class="w-4 h-4" />
          {{ link.name }}
        </router-link>
      </nav>

      <!-- 右侧工具栏 -->
      <div class="flex items-center gap-2">
        <!-- 全局搜索触发按钮 -->
        <button
          @click="themeStore.openSearch()"
          class="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-zinc-100 hover:bg-zinc-200 dark:bg-zinc-900 dark:hover:bg-zinc-800 text-zinc-500 dark:text-zinc-400 text-xs font-medium transition"
        >
          <Search class="w-4 h-4 text-zinc-400" />
          <span class="hidden sm:inline">快速搜索...</span>
          <kbd class="hidden sm:inline-block px-1.5 py-0.5 text-[10px] font-bold bg-white dark:bg-zinc-800 border border-zinc-200 dark:border-zinc-700 rounded text-zinc-400 shadow-xs">Ctrl K</kbd>
        </button>

        <!-- 暗黑模式切换 -->
        <button
          @click="themeStore.toggleDark()"
          class="w-9 h-9 rounded-xl bg-zinc-100 dark:bg-zinc-800 border border-zinc-200/80 dark:border-zinc-700/80 flex-center text-zinc-600 dark:text-amber-400 hover:bg-zinc-200 dark:hover:bg-zinc-700 transition cursor-pointer shadow-xs"
          title="切换深色/浅色模式"
          aria-label="切换深色/浅色模式"
        >
          <Sun v-if="themeStore.isDark" class="w-4.5 h-4.5 text-amber-400" />
          <Moon v-else class="w-4.5 h-4.5 text-zinc-600" />
        </button>

        <!-- 写文章按钮 -->
        <router-link
          to="/editor"
          class="hidden sm:inline-flex btn-primary text-xs px-3.5 py-2 shadow-xs"
        >
          <PenSquare class="w-3.5 h-3.5" />
          <span>写文章</span>
        </router-link>

        <!-- 用户头像 / 登录按钮 -->
        <div class="relative">
          <button
            v-if="authStore.isAuthenticated"
            @click="userMenuOpen = !userMenuOpen"
            class="flex items-center gap-2 p-1 rounded-full hover:ring-2 hover:ring-emerald-500/40 transition"
          >
            <img
              :src="getAuthorAvatar(authStore.user?.displayName, authStore.user?.avatar)"
              :alt="authStore.user?.displayName"
              class="w-8 h-8 rounded-full object-cover border border-zinc-200 dark:border-zinc-700"
            />
          </button>
          <button
            v-else
            @click="themeStore.openAuth('login')"
            class="btn-secondary text-xs px-3 py-2"
          >
            <User class="w-3.5 h-3.5" />
            <span>登录</span>
          </button>

          <!-- 用户下拉菜单 -->
          <div
            v-if="userMenuOpen"
            class="absolute right-0 mt-2 w-48 card-base shadow-xl p-1.5 z-50 text-xs text-zinc-700 dark:text-zinc-300"
            @click="userMenuOpen = false"
          >
            <div class="px-3 py-2 border-b border-zinc-100 dark:border-zinc-800">
              <div class="font-bold text-zinc-900 dark:text-zinc-100 truncate">{{ authStore.user?.displayName }}</div>
              <div class="text-[11px] text-zinc-400 truncate">{{ authStore.user?.email }}</div>
            </div>

            <router-link to="/admin" class="flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-zinc-100 dark:hover:bg-zinc-800 transition">
              <Sliders class="w-4 h-4 text-emerald-500" />
              文章管理后台
            </router-link>

            <router-link to="/profile" class="flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-zinc-100 dark:hover:bg-zinc-800 transition">
              <User class="w-4 h-4 text-blue-500" />
              个人资料设置
            </router-link>

            <button
              @click="handleLogout"
              class="w-full flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-rose-50 dark:hover:bg-rose-950/40 text-rose-600 dark:text-rose-400 transition"
            >
              <LogOut class="w-4 h-4" />
              退出当前账号
            </button>
          </div>
        </div>

        <!-- 移动端汉堡菜单按钮 -->
        <button
          @click="mobileMenuOpen = !mobileMenuOpen"
          class="md:hidden p-2 rounded-xl text-zinc-600 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-900 transition"
        >
          <X v-if="mobileMenuOpen" class="w-5 h-5" />
          <Menu v-else class="w-5 h-5" />
        </button>
      </div>
    </div>

    <!-- 移动端折叠导航 -->
    <div
      v-if="mobileMenuOpen"
      class="md:hidden border-t border-zinc-200 dark:border-zinc-800 px-4 py-3 bg-white dark:bg-zinc-950 space-y-1"
    >
      <router-link
        v-for="link in navLinks"
        :key="link.path"
        :to="link.path"
        @click="mobileMenuOpen = false"
        class="flex items-center gap-2 px-3 py-2.5 rounded-xl text-sm font-medium transition"
        :class="route.path === link.path ? 'bg-emerald-50 text-emerald-600 dark:bg-emerald-950/40 dark:text-emerald-400' : 'text-zinc-600 dark:text-zinc-300'"
      >
        <component :is="link.icon" class="w-4 h-4" />
        {{ link.name }}
      </router-link>

      <router-link
        to="/editor"
        @click="mobileMenuOpen = false"
        class="w-full btn-primary text-xs py-2.5 mt-2 flex-center"
      >
        <PenSquare class="w-4 h-4" />
        开始写文章
      </router-link>
    </div>
  </header>
</template>
