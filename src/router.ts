import { createRouter, createWebHistory } from 'vue-router'
import Home from './pages/Home.vue'
import Chat from './pages/Chat.vue'
import Settings from './pages/Settings.vue'

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'home', component: Home },
    { path: '/chat', name: 'chat', component: Chat },
    { path: '/settings', name: 'settings', component: Settings },
  ],
})
