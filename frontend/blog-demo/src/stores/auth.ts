import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { BlogUser } from '../types/blog'
import { getAuthClient, isHertaError, persistentAuthStore } from '../lib/hb'
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
  const token = ref<string | null>(null)
  const loading = ref(false)

  // 严格从 SDK 的持久化 AuthSession 中恢复登录态
  const initFromStorage = () => {
    try {
      const session = persistentAuthStore.get()
      if (session && session.accessToken && session.user) {
        user.value = {
          id: session.user.id,
          email: session.user.email,
          displayName: (session.user as any).displayName || session.user.email?.split('@')[0] || '创作者',
          role: (session.user as any).role || '创作者',
          avatar: (session.user as any).avatar,
        }
        token.value = session.accessToken
      } else {
        user.value = null
        token.value = null
      }
    } catch {
      user.value = null
      token.value = null
    }
  }

  initFromStorage()

  const isAuthenticated = computed(() => !!user.value && !!token.value)
  const isAdmin = computed(() => {
    const r = user.value?.role
    return r === 'admin' || r?.includes('admin') || r?.includes('站长') || r?.includes('超级管理员') || r?.includes('管理员')
  })

  /**
   * 登录
   */
  const login = async (email: string, password: string): Promise<boolean> => {
    loading.value = true
    try {
      // 调用 HertaBase SDK 真实认证接口
      const auth = getAuthClient()
      const res = await auth.login<UserProfile>({ email: email.trim(), password })
      user.value = {
        id: res.user.id,
        email: res.user.email,
        displayName: res.user.displayName || email.split('@')[0],
        role: res.user.role || '创作者',
        avatar: res.user.avatar,
      }
      token.value = res.accessToken
      themeStore.addToast({ type: 'success', message: `欢迎回来，${user.value.displayName}！` })
      return true
    } catch (err: any) {
      const msg = isHertaError(err) ? err.message : (err?.message || '登录失败，请检查账号密码或后端连接')
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
      themeStore.addToast({ type: 'success', message: `注册成功，欢迎加入 HertaBlog！` })
      return true
    } catch (err: any) {
      const msg = isHertaError(err) ? err.message : (err?.message || '注册失败，请检查邮箱是否已存在或后端连接')
      themeStore.addToast({ type: 'error', message: msg })
      return false
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
    persistentAuthStore.clear()
    user.value = null
    token.value = null
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
