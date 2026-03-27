import { createRouter, createWebHistory } from 'vue-router'

import Login from '../views/Login.vue'
import Home from '../views/Home.vue' // We will create this

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

  let pathname = window.location.pathname || '/'
  const replacements: Array<[string, string]> = [
    ['/__auth__/__auth__', '/__auth__'],
    ['/auth/auth', '/auth'],
  ]

  for (const [duplicatedPrefix, canonicalPrefix] of replacements) {
    if (
      pathname === duplicatedPrefix ||
      pathname.startsWith(`${duplicatedPrefix}/`)
    ) {
      pathname = `${canonicalPrefix}${pathname.slice(duplicatedPrefix.length)}`
      break
    }
  }

  const hash = window.location.hash || ''
  if (hash.startsWith('#/')) {
    const basePrefix = detectAuthBasePrefix().replace(/\/+$/, '')
    const historyPath = hash.slice(1)
    pathname = basePrefix
      ? `${basePrefix}${historyPath}`
      : historyPath
  }

  if (pathname !== window.location.pathname || hash.startsWith('#/')) {
    window.history.replaceState(
      window.history.state,
      '',
      `${pathname}${window.location.search}`,
    )
  }
}

canonicalizeAuthPath()

const router = createRouter({
  history: createWebHistory(detectAuthBasePrefix()),
  routes: [
    {
      path: '/',
      name: 'Home',
      component: Home
    },
    {
      path: '/login',
      name: 'Login',
      component: Login
    }
  ]
})

export default router
