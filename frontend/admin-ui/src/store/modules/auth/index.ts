import { computed, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { defineStore } from 'pinia'
import { useLoading } from '@/hooks'
import { localStg } from '@/utils/storage'
import { SetupStoreId } from '@/const'
import { $t } from '@/locales'
import { clearAuthStorage, getToken } from './shared'

export const useAuthStore = defineStore(SetupStoreId.Auth, () => {
  const route = useRoute()
  const router = useRouter()
  const authStore = useAuthStore()
  const { loading: loginLoading, startLoading, endLoading } = useLoading()

  const token = ref(getToken())

  const userInfo: Api.User.UserInfo = reactive({
    userId: '',
    username: '',
  })

  /** Is login */
  const isLogin = computed(() => Boolean(token.value))

  /** Reset auth store */
  async function resetStore() {
    clearAuthStorage()

    authStore.$reset()

    if (!route.meta.constant) {
      await router.push('/login')
    }
  }

  /**
   * Login
   *
   * @param username User name
   * @param password Password
   * @param [redirect] Whether to redirect after login. Default is `true`
   */
  async function login(username: string, password: string, redirect = true) {
    startLoading()

    // mock login
    const token = await new Promise<Api.User.LoginToken>(res => setTimeout(res, 1000, {
      token: `${username}-${password}`,
    }))
    const error = false

    if (!error) {
      const pass = await loginByToken(token)

      if (pass) {
        if (redirect) {
          await router.push((route.query?.redirect || '/home') as string)
        }
        await router.push('/home')
      }
    }
    else {
      await resetStore()
    }
    window.$notification?.success({
      title: $t('page.login.loginSuccess'),
      content: $t('page.login.welcomeBack', { userName: userInfo.username }),
      duration: 4500,
    })
    endLoading()
  }

  async function loginByToken(loginToken: Api.User.LoginToken) {
    // 1. stored in the localStorage, the later requests need it in headers
    localStg.set('token', loginToken.token)

    // 2. get user info
    const pass = await getUserInfo()

    if (pass) {
      token.value = loginToken.token

      return true
    }

    return false
  }

  async function getUserInfo() {
    // mock user info
    const info = await new Promise<Api.User.UserInfo>(
      res => setTimeout(
        () => res({
          userId: '123',
          username: localStg.get('token')!.split('-')[0]!,
        }),
        1000,
      ),
    )
    const error = false

    if (!error) {
      // update store
      Object.assign(userInfo, info)

      return true
    }

    return false
  }

  async function initUserInfo() {
    const hasToken = getToken()

    if (hasToken) {
      const pass = await getUserInfo()

      if (!pass) {
        resetStore()
      }
    }
  }

  return {
    token,
    userInfo,
    isLogin,
    loginLoading,
    resetStore,
    login,
    initUserInfo,
  }
}, {
  persist: true,
})
