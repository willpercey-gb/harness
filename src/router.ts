import { createRouter, createWebHistory } from 'vue-router'
import Home from './pages/Home.vue'
import Chat from './pages/Chat.vue'
import Settings from './pages/Settings.vue'
import Knowledge from './pages/Knowledge.vue'

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', name: 'home', component: Home, meta: { rightSidebar: true } },
    { path: '/chat', name: 'chat', component: Chat, meta: { rightSidebar: true } },
    { path: '/knowledge', name: 'knowledge', component: Knowledge },
    { path: '/settings', name: 'settings', component: Settings },
  ],
})
