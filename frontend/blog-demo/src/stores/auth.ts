import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { BlogUser } from '../types/blog'
import { getAuthClient, isHertaError } from '../lib/hb'
import { SEED_USERS } from '../lib/seed-data'
import { useThemeStore } from './theme'

export interface UserProfile {
  displayName?: string
  avatar?: string
  role?: string
}

export const useAuthStore = defineStore('auth', () => {
  const themeStore = useThemeStore()
  const user = ref<BlogUser | null>(null)
  const token = ref<string | null>(localStorage.getItem('herta_blog_token') || null)
  const loading = ref(false)

  // 从本地缓存恢复用户数据
  const cachedUser = localStorage.getItem('herta_blog_user')
  if (cachedUser) {
    try {
      user.value = JSON.parse(cachedUser)
    } catch {
      // ignore JSON parse error
    }
  }

  const isAuthenticated = computed(() => !!user.value)
  const isAdmin = computed(() => user.value?.role?.includes('站长') || user.value?.role?.includes('admin'))

  /**
   * 登录
   */
  const login = async (email: string, password: string): Promise<boolean> => {
    loading.value = true
    try {
      // 尝试调用 HertaBase SDK 登录
      const auth = getAuthClient()
      const res = await auth.login<UserProfile>({ email, password })
      user.value = {
        id: res.user.id,
        email: res.user.email,
        displayName: res.user.displayName || email.split('@')[0],
        role: res.user.role || '会员',
        avatar: res.user.avatar,
      }
      token.value = res.accessToken
      localStorage.setItem('herta_blog_token', res.accessToken)
      localStorage.setItem('herta_blog_user', JSON.stringify(user.value))
      themeStore.addToast({ type: 'success', message: `欢迎回来，${user.value.displayName}！` })
      return true
    } catch (err: any) {
      // 如果后端离线或未初始化集合，允许演示管理员账号登录
      if (email === 'admin@herta.ai' && password === '123456') {
        user.value = SEED_USERS[0]
        token.value = 'demo-admin-token'
        localStorage.setItem('herta_blog_token', token.value)
        localStorage.setItem('herta_blog_user', JSON.stringify(user.value))
        themeStore.addToast({ type: 'success', message: `演示管理员已登录：${user.value.displayName}` })
        return true
      }

      const msg = isHertaError(err) ? err.message : (err?.message || '登录失败，请检查账号密码')
      themeStore.addToast({ type: 'error', message: msg })
      return false
    } finally {
      loading.value = false
    }
  }

  /**
   * 注册
   */
  const register = async (email: string, password: string, displayName: string): Promise<boolean> => {
    loading.value = true
    try {
      const auth = getAuthClient()
      const res = await auth.register<UserProfile>({
        email,
        password,
        profile: { displayName, role: '创作者' },
      })
      user.value = {
        id: res.user.id,
        email: res.user.email,
        displayName: res.user.displayName || displayName,
        role: res.user.role || '创作者',
      }
      token.value = res.accessToken
      localStorage.setItem('herta_blog_token', res.accessToken)
      localStorage.setItem('herta_blog_user', JSON.stringify(user.value))
      themeStore.addToast({ type: 'success', message: `注册成功，欢迎加入 HertaBlog！` })
      return true
    } catch (err: any) {
      // 演示模式降级注册
      const newUser: BlogUser = {
        id: `blog_users:guest-${Date.now()}`,
        email,
        displayName: displayName || email.split('@')[0],
        role: '创作者',
        avatar: `https://api.dicebear.com/7.x/bottts/svg?seed=${encodeURIComponent(displayName)}`,
      }
      user.value = newUser
      token.value = `demo-token-${Date.now()}`
      localStorage.setItem('herta_blog_token', token.value)
      localStorage.setItem('herta_blog_user', JSON.stringify(user.value))
      themeStore.addToast({ type: 'success', message: `演示账号已创建并登录！` })
      return true
    } finally {
      loading.value = false
    }
  }

  /**
   * 注销
   */
  const logout = async () => {
    try {
      const auth = getAuthClient()
      await auth.logout()
    } catch {
      // ignore
    }
    user.value = null
    token.value = null
    localStorage.removeItem('herta_blog_token')
    localStorage.removeItem('herta_blog_user')
    themeStore.addToast({ type: 'info', message: '您已成功退出登录' })
  }

  return {
    user,
    token,
    loading,
    isAuthenticated,
    isAdmin,
    login,
    register,
    logout,
  }
})
