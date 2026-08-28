import { createRouter, createWebHistory } from 'vue-router'
import HomeView from '../views/HomeView.vue'
import { isDatabaseInitialized } from '../lib/hb'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'home',
      component: HomeView,
    },
    {
      path: '/setup',
      name: 'setup',
      component: () => import('../views/SetupView.vue'),
    },
    {
      path: '/post/:slug',
      name: 'post-detail',
      component: () => import('../views/PostDetailView.vue'),
    },
    {
      path: '/category/:name',
      name: 'category',
      component: () => import('../views/CategoryView.vue'),
    },
    {
      path: '/categories',
      name: 'categories',
      component: () => import('../views/CategoryView.vue'),
    },
    {
      path: '/tag/:tag',
      name: 'tag',
      component: () => import('../views/TagView.vue'),
    },
    {
      path: '/archives',
      name: 'archives',
      component: () => import('../views/ArchivesView.vue'),
    },
    {
      path: '/editor',
      name: 'editor',
      component: () => import('../views/PostEditorView.vue'),
    },
    {
      path: '/admin',
      name: 'admin',
      component: () => import('../views/AdminPostsView.vue'),
    },
    {
      path: '/about',
      name: 'about',
      component: () => import('../views/AboutView.vue'),
    },
    {
      path: '/profile',
      name: 'profile',
      component: () => import('../views/ProfileView.vue'),
    },
    {
      path: '/:pathMatch(.*)*',
      redirect: '/',
    },
  ],
  scrollBehavior(_to, _from, savedPosition) {
    if (savedPosition) {
      return savedPosition
    } else {
      return { top: 0 }
    }
  },
})

// 路由守卫：检测数据库是否已初始化，若未初始化则自动引导进入 /setup 向导
let checkedInit = false

router.beforeEach(async (to, _from, next) => {
  if (to.path === '/setup' || to.path === '/about') {
    next()
    return
  }

  if (!checkedInit) {
    checkedInit = true
    try {
      const initialized = await isDatabaseInitialized()
      if (!initialized) {
        next('/setup')
        return
      }
    } catch {
      // 忽略检查异常
    }
  }

  next()
})

export default router
