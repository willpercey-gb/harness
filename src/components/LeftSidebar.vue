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
      <div class="brand-actions">
        <RouterLink to="/settings" class="theme" title="Settings" aria-label="Settings">
          <span class="material-symbols-outlined">settings</span>
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
      </div>
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
  font-weight: 300;
  font-size: 24px;
  letter-spacing: -0.03em;
  color: var(--ink);
  display: inline-flex;
  align-items: baseline;

  .wordmark-letter {
    font-weight: 400;
    font-style: italic;
    color: var(--accent);
    margin-right: -1px;
    font-variation-settings: 'opsz' 36;
  }
  .wordmark-rest {
    font-feature-settings: 'kern', 'liga';
    opacity: 0.9;
  }
}
.brand-actions {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}
.theme {
  background: transparent;
  border: 0;
  cursor: pointer;
  color: var(--ink-faint);
  padding: 4px;
  display: inline-flex;
  align-items: center;
  text-decoration: none;
  transition: color 0.18s;
  &:hover { color: var(--ink); }
  .material-symbols-outlined { font-size: 18px; }
}

// New chat -----------------------------------------------------------
.new-chat {
  margin: 18px 16px 14px;
  padding: 10px 14px;
  background: var(--bg);
  border: 1px solid var(--rule-strong);
  border-radius: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
  text-align: left;
  box-shadow: var(--shadow-sm);

  .plus {
    font-family: var(--font-mono);
    font-weight: 400;
    font-size: 16px;
    color: var(--accent);
    width: 12px;
  }
  .label {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink);
    opacity: 0.8;
  }
  .kbd {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-faint);
    background: transparent;
    padding: 1px 4px;
    border: 1px solid var(--rule);
    opacity: 0.6;
  }
  &:hover {
    background: var(--bg-deep);
    border-color: var(--ink-faint);
    transform: translateY(-1px);
    box-shadow: var(--shadow-md);
    .kbd { color: var(--ink-muted); border-color: var(--rule-strong); opacity: 1; }
  }
  &:active {
    transform: translateY(0);
    box-shadow: var(--shadow-sm);
  }
}

// Section rail ------------------------------------------------------
.section-rail {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 12px 22px 8px;
  .eyebrow {
    font-size: 9.5px;
    opacity: 0.7;
  }
  .count {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--ink-faint);
    letter-spacing: 0.05em;
  }
}

// Session list ------------------------------------------------------
.sessions {
  overflow-y: auto;
  padding: 0 8px 24px;
  display: flex;
  flex-direction: column;
}
.empty {
  padding: 32px 14px;
  font-family: var(--font-body);
  font-style: italic;
  color: var(--ink-faint);
  font-size: 14px;
  text-align: center;
}
.session {
  display: grid;
  grid-template-columns: 1fr auto;
  grid-template-rows: auto auto;
  grid-template-areas:
    "title del"
    "meta del";
  gap: 2px 12px;
  padding: 12px 14px;
  background: transparent;
  border: 0;
  border-radius: 4px;
  cursor: pointer;
  text-align: left;
  position: relative;
  transition: all 0.18s ease;
  margin-bottom: 2px;

  .title {
    grid-area: title;
    font-family: var(--font-display);
    font-weight: 400;
    font-size: 15px;
    letter-spacing: -0.01em;
    color: var(--ink-muted);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    line-height: 1.3;
  }
  .meta {
    grid-area: meta;
    display: flex;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-faint);
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .delete {
    grid-area: del;
    align-self: center;
    color: var(--ink-faint);
    opacity: 0;
    transition: all 0.16s;
    font-size: 16px;
    &:hover { color: var(--accent); transform: scale(1.1); }
  }

  &:hover {
    background: var(--bg);
    .title { color: var(--ink); }
    .delete { opacity: 1; }
  }
  &.active {
    background: var(--bg);
    box-shadow: var(--shadow-sm);
    .title { color: var(--ink); font-weight: 500; }
    .meta { color: var(--ink-muted); }
  }
}
</style>
