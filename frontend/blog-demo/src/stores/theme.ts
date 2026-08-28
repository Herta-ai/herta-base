import { defineStore } from 'pinia'
import { ref } from 'vue'

export interface ToastItem {
  id: string
  type: 'success' | 'error' | 'info' | 'warning'
  message: string
  duration?: number
}

export const useThemeStore = defineStore('theme', () => {
  // 暗黑模式检测与存储
  const isDark = ref<boolean>(
    localStorage.getItem('herta_blog_theme') === 'dark' ||
    (!localStorage.getItem('herta_blog_theme') && window.matchMedia('(prefers-color-scheme: dark)').matches)
  )

  // 弹窗状态
  const searchModalOpen = ref(false)
  const setupModalOpen = ref(false)
  const authModalOpen = ref(false)
  const authMode = ref<'login' | 'register'>('login')

  // Toast 消息
  const toasts = ref<ToastItem[]>([])

  const applyTheme = () => {
    if (isDark.value) {
      document.documentElement.classList.add('dark')
      localStorage.setItem('herta_blog_theme', 'dark')
    } else {
      document.documentElement.classList.remove('dark')
      localStorage.setItem('herta_blog_theme', 'light')
    }
  }

  // 初始化主题
  applyTheme()

  const toggleDark = () => {
    isDark.value = !isDark.value
    applyTheme()
  }

  const openSearch = () => { searchModalOpen.value = true }
  const closeSearch = () => { searchModalOpen.value = false }

  const openSetup = () => { setupModalOpen.value = true }
  const closeSetup = () => { setupModalOpen.value = false }

  const openAuth = (mode: 'login' | 'register' = 'login') => {
    authMode.value = mode
    authModalOpen.value = true
  }
  const closeAuth = () => { authModalOpen.value = false }

  const addToast = (toast: Omit<ToastItem, 'id'>) => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`
    const item: ToastItem = {
      id,
      duration: 3500,
      ...toast,
    }
    toasts.value.push(item)

    if (item.duration && item.duration > 0) {
      setTimeout(() => {
        removeToast(id)
      }, item.duration)
    }
  }

  const removeToast = (id: string) => {
    toasts.value = toasts.value.filter(t => t.id !== id)
  }

  return {
    isDark,
    searchModalOpen,
    setupModalOpen,
    authModalOpen,
    authMode,
    toasts,
    toggleDark,
    openSearch,
    closeSearch,
    openSetup,
    closeSetup,
    openAuth,
    closeAuth,
    addToast,
    removeToast,
  }
})
