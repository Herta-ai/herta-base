import type { Router } from 'vue-router'

export function createProgressGuard(router: Router) {
  router.beforeEach((_to) => {
    window.NProgress?.start?.()
  })
  router.afterEach((_to) => {
    window.NProgress?.done?.()
  })
}
