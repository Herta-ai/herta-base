import { localStg } from '@/utils/storage'
import type {
  Router,
} from 'vue-router'

/**
 * create route guard
 *
 * @param router router instance
 */
export function createRouteGuard(router: Router) {
  router.beforeEach(async (to) => {
    const isLogin = Boolean(localStg.get('token'))
    const needLogin = !to.meta.unAuth

    // if it is login route when logged in, then switch to the root page
    if (to.name === 'login' && isLogin) {
      return '/home'
    }

    // if the route does not need login, then it is allowed to access directly
    if (!needLogin) {
      return
    }

    // the route need login but the user is not logged in, then switch to the login page
    if (!isLogin && to.name !== 'login') {
      return ({ name: 'login', query: { redirect: to.fullPath } })
    }

    if (!to.matched.length) {
      return '/not-found'
    }
  })
}
