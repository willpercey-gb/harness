<script lang="ts">
import { defineComponent, nextTick } from 'vue'
import {
  streamChat,
  getChatHistory,
  type StreamHandle,
} from '@/services/chat'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage, StreamEvent, ToolEvent } from '@/types/chat.types'
import MarkdownIt from 'markdown-it'
import hljs from 'highlight.js/lib/core'
import javascript from 'highlight.js/lib/languages/javascript'
import typescript from 'highlight.js/lib/languages/typescript'
import python from 'highlight.js/lib/languages/python'
import jsonLang from 'highlight.js/lib/languages/json'
import bash from 'highlight.js/lib/languages/bash'
import xml from 'highlight.js/lib/languages/xml'
import css from 'highlight.js/lib/languages/css'
import sql from 'highlight.js/lib/languages/sql'
import yaml from 'highlight.js/lib/languages/yaml'
import markdown from 'highlight.js/lib/languages/markdown'
import 'highlight.js/styles/atom-one-dark.css'

import AgentSelector from '@/components/AgentSelector.vue'

hljs.registerLanguage('javascript', javascript)
hljs.registerLanguage('typescript', typescript)
hljs.registerLanguage('python', python)
hljs.registerLanguage('json', jsonLang)
hljs.registerLanguage('bash', bash)
hljs.registerLanguage('xml', xml)
hljs.registerLanguage('html', xml)
hljs.registerLanguage('css', css)
hljs.registerLanguage('sql', sql)
hljs.registerLanguage('yaml', yaml)
hljs.registerLanguage('markdown', markdown)

const md = new MarkdownIt({
  highlight(str: string, lang: string | undefined | null): string {
    const language = lang && hljs.getLanguage(lang) ? lang : ''
    if (language) {
      try {
        return (
          '<pre class="hljs"><code>' +
          hljs.highlight(str, { language, ignoreIllegals: true }).value +
          '</code></pre>'
        )
      } catch (_) {}
    }
    return '<pre class="hljs"><code>' + md.utils.escapeHtml(str) + '</code></pre>'
  },
})

export default defineComponent({
  name: 'ChatPage',
  components: { AgentSelector },
  setup() {
    return { chat: useChatStore() }
  },
  data() {
    return {
      messages: [] as ChatMessage[],
      currentMessage: '',
      streaming: false,
      thinking: false,
      error: '',
      messageIdCounter: 0,
      autoScroll: true,
      historyLimit: 50,
      hasMoreHistory: false,
      historyOffset: 0,
      loadingHistory: false,
      currentStream: null as StreamHandle | null,
      title: '' as string,
    }
  },
  computed: {
    agentName(): string {
      return this.chat.selectedAgent?.attributes.name ?? '—'
    },
    agentLabel(): string {
      return (this.chat.selectedAgent?.attributes.name ?? 'agent').replace(/[^a-z0-9]/gi, '').slice(0, 12).toUpperCase() || 'AGENT'
    },
  },
  watch: {
    'chat.historyBumper'() {
      this.title = this.chat.pendingTitle ?? ''
      this.loadHistory()
    },
  },
  mounted() {
    if (this.chat.currentSessionId) this.loadHistory()
  },
  methods: {
    async loadHistory() {
      this.messages = []
      this.historyOffset = 0
      this.hasMoreHistory = false
      this.error = ''
      if (!this.chat.currentSessionId) return
      this.loadingHistory = true
      try {
        const res = await getChatHistory(
          this.chat.currentSessionId,
          this.historyLimit,
          0,
        )
        this.messages = res.messages.map((m) => ({
          ...m,
          id: m.id || `m-${this.messageIdCounter++}`,
        }))
        this.hasMoreHistory = res.hasMore
        this.historyOffset = res.messages.length
      } catch {
        this.messages = []
      } finally {
        this.loadingHistory = false
        nextTick(() => this.scrollToBottom())
      }
    },
    async loadMoreHistory() {
      if (!this.chat.currentSessionId || this.loadingHistory || !this.hasMoreHistory) return
      this.loadingHistory = true
      try {
        const res = await getChatHistory(
          this.chat.currentSessionId,
          this.historyLimit,
          this.historyOffset,
        )
        this.messages = [...res.messages, ...this.messages]
        this.hasMoreHistory = res.hasMore
        this.historyOffset += res.messages.length
      } catch {
        // ignore
      } finally {
        this.loadingHistory = false
      }
    },
    async sendMessage() {
      if (!this.currentMessage.trim() || !this.chat.selectedAgent || this.streaming) return
      const prompt = this.currentMessage
      this.currentMessage = ''

      this.messages.push({
        id: `m-${this.messageIdCounter++}`,
        role: 'user',
        content: prompt,
        timestamp: new Date(),
        agentId: this.chat.selectedAgent.id,
      })

      const idx = this.messages.length
      this.messages.push({
        id: `m-${this.messageIdCounter++}`,
        role: 'assistant',
        content: '',
        reasoning: '',
        toolEvents: [],
        timestamp: new Date(),
        agentId: this.chat.selectedAgent.id,
      })

      // Title-from-first-prompt for in-flight conversations.
      if (!this.title) this.title = prompt.slice(0, 60)

      this.streaming = true
      this.error = ''
      this.thinking = false
      this.autoScroll = true

      const handle = streamChat(
        this.chat.selectedAgent.id,
        prompt,
        this.chat.currentSessionId,
        (e: StreamEvent) => this.onStreamEvent(idx, e),
      )
      this.currentStream = handle

      try {
        const { sessionId } = await handle.done
        if (sessionId) this.chat.setSessionFromStream(sessionId)
      } catch (err: any) {
        this.error = err?.message || String(err) || 'send failed'
      } finally {
        this.streaming = false
        this.thinking = false
        this.currentStream = null
      }
    },
    onStreamEvent(idx: number, e: StreamEvent) {
      const msg = this.messages[idx]
      if (!msg) return
      switch (e.kind) {
        case 'session_started':
          this.chat.setSessionFromStream(e.session_id)
          break
        case 'text_delta':
          msg.content += e.text
          break
        case 'reasoning_delta':
          msg.reasoning = (msg.reasoning ?? '') + e.text
          break
        case 'tool_use':
          msg.toolEvents = [
            ...(msg.toolEvents ?? []),
            { kind: 'tool_use', name: e.name, id: e.id } as ToolEvent,
          ]
          break
        case 'tool_result':
          msg.toolEvents = [
            ...(msg.toolEvents ?? []),
            { kind: 'tool_result', name: e.name, id: e.id, status: e.status } as ToolEvent,
          ]
          break
        case 'thinking':
          this.thinking = e.active
          break
        case 'error':
          this.error = e.message
          break
        case 'done':
        case 'cancelled':
          break
      }
      nextTick(() => this.scrollToBottom())
    },
    async cancel() {
      await this.currentStream?.cancel()
    },
    scrollToBottom() {
      if (!this.autoScroll) return
      const el = this.$refs.scroller as HTMLElement | undefined
      if (el) el.scrollTop = el.scrollHeight
    },
    handleScroll() {
      const el = this.$refs.scroller as HTMLElement | undefined
      if (!el) return
      this.autoScroll = el.scrollHeight - el.scrollTop - el.clientHeight < 80
    },
    handleKeydown(ev: KeyboardEvent) {
      if (ev.key === 'Enter' && !ev.shiftKey) {
        ev.preventDefault()
        this.sendMessage()
      }
    },
    formatTime(d: Date | string): string {
      return new Date(d).toLocaleTimeString('en-GB', {
        hour: '2-digit',
        minute: '2-digit',
      })
    },
    renderMarkdown(s: string): string {
      return md.render(s)
    },
    speakerLabel(role: string, agentName: string | undefined): string {
      if (role === 'user') return 'YOU'
      if (role === 'system') return 'SYSTEM'
      return (agentName ?? 'agent').replace(/[^a-z0-9]/gi, '').slice(0, 18).toUpperCase() || 'AGENT'
    },
  },
})
</script>

<template>
  <div class="page">
    <header class="masthead">
      <div class="title-row">
        <h1 class="title" v-if="title || chat.currentSessionId">
          <span v-if="title">{{ title }}</span>
          <span v-else class="muted">Untitled</span>
        </h1>
        <h1 class="title untitled" v-else>
          <span class="amp">¶</span>
          new conversation
        </h1>
      </div>
      <div class="meta-row">
        <AgentSelector />
        <div class="timing" v-if="chat.currentSessionId">
          <span class="eyebrow">session</span>
          <span class="hash">{{ chat.currentSessionId.slice(0, 8) }}</span>
        </div>
      </div>
    </header>

    <div v-if="error" class="banner-error">
      <span>{{ error }}</span>
      <button @click="error = ''">dismiss</button>
    </div>

    <section class="scroller" ref="scroller" @scroll="handleScroll">
      <div class="column">
        <button
          v-if="hasMoreHistory && messages.length > 0"
          class="load-more"
          @click="loadMoreHistory"
          :disabled="loadingHistory"
        >
          {{ loadingHistory ? 'loading…' : '↑ earlier in this conversation' }}
        </button>

        <div v-if="messages.length === 0 && !loadingHistory" class="empty-state">
          <p class="lead">
            <span class="drop-cap">A</span> blank page — type below to begin.
          </p>
          <p class="hint">
            Conversations are saved automatically and live in the rail to the left.
          </p>
        </div>

        <article
          v-for="(m, i) in messages"
          :key="m.id"
          class="turn"
          :class="m.role"
        >
          <header class="turn-head">
            <span class="speaker">{{ speakerLabel(m.role, chat.selectedAgent?.attributes.name) }}</span>
            <span class="dot">·</span>
            <span class="time">{{ formatTime(m.timestamp) }}</span>
          </header>

          <div v-if="m.reasoning" class="reasoning">
            <div class="reasoning-head">
              <span class="eyebrow">reasoning</span>
              <span class="rule"></span>
            </div>
            <div class="reasoning-body">{{ m.reasoning }}</div>
          </div>

          <ul v-if="m.toolEvents && m.toolEvents.length" class="tools">
            <li
              v-for="(t, j) in m.toolEvents"
              :key="j"
              :class="['tool', t.kind, t.kind === 'tool_result' ? `status-${t.status}` : '']"
            >
              <span class="arrow" v-if="t.kind === 'tool_use'">→</span>
              <span class="arrow" v-else-if="t.kind === 'tool_result'">←</span>
              <span class="t-name">{{ t.name }}</span>
              <span v-if="t.kind === 'tool_result'" class="t-status">{{ t.status }}</span>
            </li>
          </ul>

          <div
            v-if="m.role === 'user'"
            class="prose user-prose"
          >{{ m.content }}</div>
          <div
            v-else-if="m.content"
            class="prose assistant-prose"
            v-html="renderMarkdown(m.content)"
          ></div>
          <div
            v-else-if="streaming && i === messages.length - 1"
            class="prose pending"
          >
            <span v-if="thinking" class="thinking-text">— thinking</span>
            <span v-else class="caret"></span>
          </div>
        </article>

        <div v-if="streaming && messages[messages.length-1]?.role === 'assistant' && messages[messages.length-1]?.content" class="trailing-caret">
          <span class="caret"></span>
        </div>
      </div>
    </section>

    <footer class="composer">
      <div class="composer-inner">
        <div class="composer-rail">
          <span class="prompt-glyph">▷</span>
        </div>
        <textarea
          v-model="currentMessage"
          class="composer-input"
          rows="1"
          :placeholder="chat.selectedAgent ? 'Type a message — Enter to send, Shift+Enter for newline.' : 'Pick an agent above to begin.'"
          :disabled="streaming || !chat.selectedAgent"
          @keydown="handleKeydown"
        ></textarea>
        <div class="composer-actions">
          <button
            v-if="streaming"
            class="action cancel"
            @click="cancel"
            title="Cancel stream"
          >
            <span class="material-symbols-outlined">stop_circle</span>
            cancel
          </button>
          <button
            v-else
            class="action send"
            :disabled="!currentMessage.trim() || !chat.selectedAgent"
            @click="sendMessage"
          >
            send
            <span class="kbd">⏎</span>
          </button>
        </div>
      </div>
    </footer>
  </div>
</template>

<style scoped lang="scss">
.page {
  display: grid;
  grid-template-rows: auto auto 1fr auto;
  height: 100%;
  background-color: var(--bg);
}

// — Masthead —————————————————————————————————————————————————
.masthead {
  padding: 24px 40px;
  border-bottom: 1px solid var(--rule-strong);
  display: flex;
  flex-direction: column;
  gap: 16px;
  background: var(--bg);
}
.title-row { display: flex; align-items: baseline; gap: 16px; }
.title {
  margin: 0;
  font-family: var(--font-display);
  font-weight: 400;
  font-size: 24px;
  letter-spacing: -0.02em;
  line-height: 1.2;
  color: var(--ink);
  &.untitled {
    color: var(--ink-faint);
  }
  .amp {
    font-style: italic;
    color: var(--accent);
    margin-right: 6px;
  }
}
.meta-row {
  display: flex;
  align-items: center;
  gap: 24px;
}

// — Banner ————————————————————————————————————————————————
.banner-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 40px;
  background: var(--accent);
  color: var(--bg);
  font-family: var(--font-mono);
  font-size: 11px;
  button {
    background: var(--bg);
    border: 0;
    color: var(--accent);
    text-transform: uppercase;
    font-size: 9px;
    padding: 2px 6px;
    cursor: pointer;
  }
}

// — Scroller / column ————————————————————————————————————————
.scroller {
  overflow-y: auto;
  padding: 32px 40px 60px;
}
.column {
  max-width: 640px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 32px;
}

// — Empty state ——————————————————————————————————————————————
.empty-state {
  margin-top: 8vh;
  .lead {
    margin: 0 0 8px;
    font-family: var(--font-display);
    font-size: 22px;
    color: var(--ink-muted);
    font-style: italic;
  }
  .drop-cap {
    font-family: var(--font-display);
    font-size: 48px;
    color: var(--accent);
    float: left;
    margin: 4px 8px 0 0;
  }
}

// — Turn ————————————————————————————————————————————————————
.turn {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.turn-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: var(--font-mono);
  font-size: 10px;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--ink-faint);
  border-bottom: 1px solid var(--rule);
  padding-bottom: 4px;
  .speaker { color: var(--ink); font-weight: 600; }
}
.turn.user .turn-head { border-bottom-color: var(--accent-soft); }
.turn.user .turn-head .speaker { color: var(--accent); }

// — Prose blocks ————————————————————————————————————————————————
.prose {
  font-family: var(--font-body);
  font-size: 16px;
  line-height: 1.6;
  color: var(--ink);

  &.user-prose { white-space: pre-wrap; }
  &.pending {
    color: var(--ink-faint);
    .thinking-text { font-family: var(--font-mono); font-size: 10px; text-transform: uppercase; }
  }

  :deep(p) { margin: 0 0 1em; }
  :deep(pre.hljs) {
    margin: 1em 0;
    padding: 12px 16px;
    background: var(--bg-soft) !important;
    border: 1px solid var(--rule);
    font-family: var(--font-mono);
    font-size: 13px;
  }
  :deep(code:not(pre code)) {
    font-family: var(--font-mono);
    background: var(--bg-soft);
    padding: 1px 4px;
  }
}

// — Reasoning block ——————————————————————————————————————————
.reasoning {
  margin: 4px 0 8px;
  padding: 12px 16px;
  background: var(--bg-soft);
  border: 1px solid var(--rule);
}
.reasoning-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
  .eyebrow {
    font-family: var(--font-mono);
    font-size: 9px;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .rule { flex: 1; height: 1px; background: var(--rule); }
}
.reasoning-body {
  font-family: var(--font-body);
  font-style: italic;
  font-size: 14px;
  color: var(--ink-muted);
}

// — Composer ————————————————————————————————————————————————
.composer {
  border-top: 1px solid var(--rule-strong);
  background: var(--bg);
  padding: 16px 40px 24px;
}
.composer-inner {
  max-width: 640px;
  margin: 0 auto;
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: end;
  gap: 12px;
  border: 1px solid var(--rule-strong);
  padding: 8px 12px;
}
.composer-rail { padding-bottom: 6px; .prompt-glyph { font-family: var(--font-mono); color: var(--accent); } }
.composer-input {
  font: inherit;
  font-family: var(--font-body);
  font-size: 16px;
  background: transparent;
  border: 0;
  outline: none;
  padding: 4px 0;
  min-height: 24px;
}
.action {
  font-family: var(--font-mono);
  font-size: 10px;
  text-transform: uppercase;
  background: var(--bg-soft);
  border: 1px solid var(--rule);
  padding: 4px 10px;
  cursor: pointer;
  &:hover { background: var(--ink); color: var(--bg); }
  .kbd { opacity: 0.5; margin-left: 4px; }
}
</style>
