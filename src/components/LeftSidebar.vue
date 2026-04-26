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
    <header class="brand">
      <RouterLink to="/" class="wordmark" aria-label="Harness home">
        <span class="wordmark-letter">H</span>
        <span class="wordmark-rest">arness</span>
      </RouterLink>
      <button
        class="theme"
        :title="darkMode ? 'Light mode' : 'Dark mode'"
        @click="$emit('toggle-dark')"
      >
        <span class="material-symbols-outlined">
          {{ darkMode ? 'wb_sunny' : 'dark_mode' }}
        </span>
      </button>
    </header>

    <button class="new-chat" @click="startNew">
      <span class="plus">+</span>
      <span class="label">New conversation</span>
      <span class="kbd">⏎</span>
    </button>

    <div class="section-rail">
      <span class="eyebrow">Sessions</span>
      <span class="count" v-if="sessions.length">·&nbsp;{{ sessions.length }}</span>
    </div>

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
        <span class="meta">
          <span class="time">{{ relativeDate(s.lastMessageAt) }}</span>
          <span class="dot">·</span>
          <span class="msgs">{{ s.messageCount }}</span>
        </span>
        <span
          class="delete material-symbols-outlined"
          title="Delete session"
          @click="remove(s, $event)"
        >close</span>
      </button>
    </nav>
  </aside>
</template>

<style scoped lang="scss">
.left {
  display: grid;
  grid-template-rows: auto auto auto 1fr;
  height: 100vh;
  background-color: var(--bg-soft);
  padding: 22px 0 0 0;
  overflow: hidden;
}

// Brand row -----------------------------------------------------------
.brand {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  padding: 0 22px 18px;
  border-bottom: 1px solid var(--rule);
}
.wordmark {
  font-family: var(--font-display);
  font-weight: 400;
  font-size: 22px;
  letter-spacing: -0.02em;
  color: var(--ink);
  display: inline-flex;
  align-items: baseline;

  .wordmark-letter {
    font-weight: 600;
    font-style: italic;
    color: var(--accent);
    margin-right: -2px;
  }
  .wordmark-rest {
    font-feature-settings: 'kern', 'liga';
  }
}
.theme {
  background: transparent;
  border: 0;
  cursor: pointer;
  color: var(--ink-faint);
  padding: 4px;
  display: inline-flex;
  align-items: center;
  transition: color 0.18s;
  &:hover { color: var(--ink); }
  .material-symbols-outlined { font-size: 18px; }
}

// New chat -----------------------------------------------------------
.new-chat {
  margin: 18px 18px 14px;
  padding: 12px 14px;
  background: var(--bg);
  border: 1px solid var(--rule-strong);
  border-radius: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
  transition: all 0.18s;
  text-align: left;

  .plus {
    font-family: var(--font-mono);
    font-weight: 400;
    font-size: 15px;
    color: var(--accent);
    width: 12px;
  }
  .label {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 12px;
    letter-spacing: 0.04em;
    color: var(--ink);
  }
  .kbd {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-faint);
    background: transparent;
    padding: 2px 5px;
    border: 1px solid var(--rule);
  }
  &:hover {
    background: var(--bg-deep);
    .kbd { color: var(--ink-muted); border-color: var(--rule-strong); }
  }
}

// Section rail ------------------------------------------------------
.section-rail {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 22px 10px;
  .count {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink-faint);
    letter-spacing: 0.05em;
  }
}

// Session list ------------------------------------------------------
.sessions {
  overflow-y: auto;
  padding: 0 8px 16px;
  display: flex;
  flex-direction: column;
}
.empty {
  padding: 20px 14px;
  font-family: var(--font-body);
  font-style: italic;
  color: var(--ink-faint);
  font-size: 13.5px;
}
.session {
  display: grid;
  grid-template-columns: 1fr auto;
  grid-template-rows: auto auto;
  grid-template-areas:
    "title del"
    "meta del";
  gap: 4px 12px;
  padding: 11px 14px 11px 14px;
  background: transparent;
  border: 0;
  border-left: 2px solid transparent;
  cursor: pointer;
  text-align: left;
  position: relative;
  transition: background 0.16s, border-color 0.16s;

  .title {
    grid-area: title;
    font-family: var(--font-display);
    font-weight: 400;
    font-size: 14.5px;
    letter-spacing: -0.005em;
    color: var(--ink);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    line-height: 1.25;
  }
  .meta {
    grid-area: meta;
    display: flex;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink-faint);
    letter-spacing: 0.04em;
  }
  .delete {
    grid-area: del;
    align-self: center;
    color: var(--ink-faint);
    opacity: 0;
    transition: opacity 0.16s, color 0.16s;
    font-size: 16px;
    &:hover { color: var(--accent); }
  }

  &:hover {
    background: var(--bg);
    .delete { opacity: 1; }
  }
  &.active {
    background: var(--bg);
    border-left-color: var(--accent);
    .title { color: var(--ink); }
  }
}
</style>
