import { createRouter, createWebHistory } from 'vue-router'
import { canonicalAuthHistoryTarget } from '../lib/auth-route-canonicalization'

const detectAuthBasePrefix = () => {
  if (typeof window === 'undefined') return '/'
  const pathname = window.location.pathname || '/'

  if (pathname === '/__auth__' || pathname.startsWith('/__auth__/')) {
    return '/__auth__/'
  }

  if (pathname === '/auth' || pathname.startsWith('/auth/')) {
    return '/auth/'
  }

  return '/'
}

const canonicalizeAuthPath = () => {
  if (typeof window === 'undefined') return

  const target = canonicalAuthHistoryTarget(window.location)
  if (target) {
    window.history.replaceState(window.history.state, '', target)
  }
}

canonicalizeAuthPath()

const router = createRouter({
  history: createWebHistory(detectAuthBasePrefix()),
  routes: [
    {
      path: '/',
      name: 'Home',
      component: () => import('../views/Home.vue')
    },
    {
      path: '/login',
      name: 'Login',
      component: () => import('../views/Login.vue')
    },
    {
      path: '/oidc/bind',
      name: 'OidcBind',
      component: () => import('../views/OidcBind.vue')
    },
    {
      path: '/ldap/bind',
      name: 'LdapBind',
      component: () => import('../views/LdapBind.vue')
    },
    {
      path: '/:pathMatch(.*)*',
      name: 'NotFound',
      component: () => import('../views/NotFound.vue')
    }
  ]
})

export default router
