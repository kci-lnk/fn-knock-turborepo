import { createRouter, createWebHashHistory } from 'vue-router'

import Login from '../views/Login.vue'
import Home from '../views/Home.vue' // We will create this

const router = createRouter({
  history: createWebHashHistory(),
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
