import { Store } from '@tanstack/react-store'

export interface AdminUser {
  id: string
  email: string
  collection?: string
  role?: string
  verified?: boolean
}

export interface AuthState {
  admin: AdminUser | null
  accessToken: string | null
  refreshToken: string | null
  isAuthenticated: boolean
}

const ACCESS_TOKEN_KEY = 'hb_access_token'
const REFRESH_TOKEN_KEY = 'hb_refresh_token'
const ADMIN_USER_KEY = 'hb_admin_user'

function loadInitialState(): AuthState {
  try {
    const accessToken = localStorage.getItem(ACCESS_TOKEN_KEY)
    const refreshToken = localStorage.getItem(REFRESH_TOKEN_KEY)
    const userStr = localStorage.getItem(ADMIN_USER_KEY)
    const admin = userStr ? JSON.parse(userStr) : null

    return {
      admin,
      accessToken,
      refreshToken,
      isAuthenticated: Boolean(accessToken && admin),
    }
  } catch {
    return {
      admin: null,
      accessToken: null,
      refreshToken: null,
      isAuthenticated: false,
    }
  }
}

export const authStore = new Store<AuthState>(loadInitialState())

export function setAuthSession(data: {
  accessToken: string
  refreshToken: string
  user: AdminUser
}) {
  localStorage.setItem(ACCESS_TOKEN_KEY, data.accessToken)
  localStorage.setItem(REFRESH_TOKEN_KEY, data.refreshToken)
  localStorage.setItem(ADMIN_USER_KEY, JSON.stringify(data.user))

  authStore.setState(() => ({
    accessToken: data.accessToken,
    refreshToken: data.refreshToken,
    admin: data.user,
    isAuthenticated: true,
  }))
}

export function updateTokens(accessToken: string, refreshToken: string) {
  localStorage.setItem(ACCESS_TOKEN_KEY, accessToken)
  localStorage.setItem(REFRESH_TOKEN_KEY, refreshToken)

  authStore.setState((state) => ({
    ...state,
    accessToken,
    refreshToken,
    isAuthenticated: true,
  }))
}

export function clearAuthSession() {
  localStorage.removeItem(ACCESS_TOKEN_KEY)
  localStorage.removeItem(REFRESH_TOKEN_KEY)
  localStorage.removeItem(ADMIN_USER_KEY)

  authStore.setState(() => ({
    admin: null,
    accessToken: null,
    refreshToken: null,
    isAuthenticated: false,
  }))
}

export function getAccessToken(): string | null {
  return authStore.state.accessToken || localStorage.getItem(ACCESS_TOKEN_KEY)
}

export function getRefreshToken(): string | null {
  return authStore.state.refreshToken || localStorage.getItem(REFRESH_TOKEN_KEY)
}
