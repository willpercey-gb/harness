<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { getSessions, deleteSession } from '@/services/chat'
import { useChatStore } from '@/stores/chat'
import type { ChatSession } from '@/types/chat.types'

defineProps<{ darkMode: boolean }>()
defineEmits<{ (e: 'toggle-dark'): void }>()

const router = useRouter()
const route = useRoute()
const chat = useChatStore()

const sessions = ref<ChatSession[]>([])
const loading = ref(false)

const filterAgentId = computed(() => chat.selectedAgentId)

async function reload() {
  if (!filterAgentId.value) {
    sessions.value = []
    return
  }
  loading.value = true
  try {
    const res = await getSessions(filterAgentId.value, 100, 0)
    sessions.value = res.data
  } catch {
    sessions.value = []
  } finally {
    loading.value = false
  }
}

onMounted(reload)
watch(filterAgentId, reload)
watch(() => chat.sessionsBumper, reload)

function open(session: ChatSession) {
  if (route.path !== '/chat') router.push('/chat')
  chat.openSession(session)
}

async function remove(session: ChatSession, ev: Event) {
  ev.stopPropagation()
  await deleteSession(session.sessionId)
  if (chat.currentSessionId === session.sessionId) chat.newChat()
  reload()
}

function startNew() {
  chat.newChat()
  if (route.path !== '/chat') router.push('/chat')
}

function relativeDate(s: string): string {
  const d = new Date(s)
  const mins = Math.floor((Date.now() - d.getTime()) / 60000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m`
  const h = Math.floor(mins / 60)
  if (h < 24) return `${h}h`
  const days = Math.floor(h / 24)
  if (days < 7) return `${days}d`
  return d.toLocaleDateString('en-GB', { month: 'short', day: 'numeric' })
}
</script>

<template>
  <aside class="left">
    <button class="new-row" @click="startNew">
      <span class="material-symbols-outlined">add</span>
      <span>New conversation</span>
    </button>

    <nav class="sessions">
      <div v-if="loading" class="empty">Loading…</div>
      <div v-else-if="!filterAgentId" class="empty">Pick an agent to begin.</div>
      <div v-else-if="sessions.length === 0" class="empty">No conversations yet.</div>

      <button
        v-for="s in sessions"
        :key="s.sessionId"
        class="session"
        :class="{ active: chat.currentSessionId === s.sessionId }"
        @click="open(s)"
      >
        <span class="title">{{ s.title || 'Untitled' }}</span>
        <span class="meta">{{ relativeDate(s.lastMessageAt) }}</span>
        <span
          class="delete material-symbols-outlined"
          title="Delete"
          @click="remove(s, $event)"
        >close</span>
      </button>
    </nav>

    <footer class="bottom">
      <RouterLink to="/settings" class="footer-btn" :class="{ active: route.path === '/settings' }" title="Settings">
        <span class="material-symbols-outlined">settings</span>
        <span>Settings</span>
      </RouterLink>
      <button class="footer-btn icon-only" :title="darkMode ? 'Light mode' : 'Dark mode'" @click="$emit('toggle-dark')">
        <span class="material-symbols-outlined">{{ darkMode ? 'light_mode' : 'dark_mode' }}</span>
      </button>
    </footer>
  </aside>
</template>

<style scoped lang="scss">
.left {
  display: grid;
  grid-template-rows: auto 1fr auto;
  height: 100vh;
  background-color: var(--bg-soft);
  border-right: 1px solid var(--rule);
}

.new-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 14px 8px 8px;
  padding: 10px 12px;
  background: transparent;
  border: 0;
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--ink);
  font-size: 14px;
  font-weight: 500;
  text-align: left;
  transition: background 0.12s;
  &:hover { background: var(--bg-hover); }
  .material-symbols-outlined { font-size: 18px; color: var(--ink-muted); }
}

.sessions {
  overflow-y: auto;
  padding: 4px 8px 8px;
  display: flex;
  flex-direction: column;
}
.empty {
  padding: 14px 12px;
  font-size: 13px;
  color: var(--ink-faint);
}
.session {
  display: grid;
  grid-template-columns: 1fr auto auto;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: transparent;
  border: 0;
  border-radius: var(--radius-md);
  cursor: pointer;
  text-align: left;
  position: relative;
  transition: background 0.12s;

  .title {
    font-size: 13.5px;
    color: var(--ink);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .meta {
    font-size: 11.5px;
    color: var(--ink-faint);
  }
  .delete {
    color: var(--ink-faint);
    opacity: 0;
    font-size: 16px;
    border-radius: var(--radius-sm);
    padding: 2px;
    &:hover { color: var(--ink); background: var(--rule-strong); }
  }

  &:hover {
    background: var(--bg-hover);
    .delete { opacity: 1; }
    .meta { opacity: 0; }
  }
  &.active {
    background: var(--bg-hover);
    .title { font-weight: 500; }
  }
}

.bottom {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 4px;
  padding: 8px 8px 12px;
  border-top: 1px solid var(--rule);
}
.footer-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  font-size: 13px;
  color: var(--ink-muted);
  background: transparent;
  border: 0;
  border-radius: var(--radius-md);
  cursor: pointer;
  text-decoration: none;
  flex: 1;
  transition: background 0.12s, color 0.12s;
  &:hover { background: var(--bg-hover); color: var(--ink); }
  &.active { background: var(--bg-hover); color: var(--ink); }
  &.icon-only { flex: 0; }
  .material-symbols-outlined { font-size: 17px; }
}
</style>
