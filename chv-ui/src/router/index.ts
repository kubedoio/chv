import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

import LoginView from '@/views/LoginView.vue'
import DashboardView from '@/views/DashboardView.vue'
import VMsView from '@/views/VMsView.vue'
import NodesView from '@/views/NodesView.vue'
import NetworksView from '@/views/NetworksView.vue'
import StorageView from '@/views/StorageView.vue'
import ImagesView from '@/views/ImagesView.vue'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/login',
      name: 'login',
      component: LoginView,
      meta: { public: true }
    },
    {
      path: '/',
      name: 'dashboard',
      component: DashboardView
    },
    {
      path: '/vms',
      name: 'vms',
      component: VMsView
    },
    {
      path: '/nodes',
      name: 'nodes',
      component: NodesView
    },
    {
      path: '/networks',
      name: 'networks',
      component: NetworksView
    },
    {
      path: '/storage',
      name: 'storage',
      component: StorageView
    },
    {
      path: '/images',
      name: 'images',
      component: ImagesView
    },
    {
      path: '/:pathMatch(.*)*',
      redirect: '/'
    }
  ]
})

// Navigation guard
router.beforeEach((to, from, next) => {
  const authStore = useAuthStore()
  
  if (!to.meta.public && !authStore.isAuthenticated) {
    next('/login')
  } else if (to.path === '/login' && authStore.isAuthenticated) {
    next('/')
  } else {
    next()
  }
})

export default router
