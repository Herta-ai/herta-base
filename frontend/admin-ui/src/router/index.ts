import {
  createMemoryHistory,
  createRouter,
  createWebHashHistory,
  createWebHistory,

} from 'vue-router'
import { handleHotUpdate, routes } from 'vue-router/auto-routes'
import { setupLayouts } from 'virtual:generated-layouts'
import { createRouterGuard } from './guard'
import type { RouterHistory } from 'vue-router'
import type { App } from 'vue'

const { VITE_ROUTER_HISTORY_MODE = 'history', VITE_BASE_URL } = import.meta.env

const historyCreatorMap: Record<Env.RouterHistoryMode, (base?: string) => RouterHistory> = {
  hash: createWebHashHistory,
  history: createWebHistory,
  memory: createMemoryHistory,
}

export const router = createRouter({
  history: historyCreatorMap[VITE_ROUTER_HISTORY_MODE](VITE_BASE_URL),
  routes: setupLayouts(routes),
})

/** Setup Vue Router */
export async function setupRouter(app: App) {
  app.use(router)
  createRouterGuard(router)
  await router.isReady()
}

if (import.meta.hot) {
  handleHotUpdate(router)
}
