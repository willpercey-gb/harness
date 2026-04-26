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
  padding: 28px 56px 18px;
  border-bottom: 1px solid var(--rule);
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.title-row { display: flex; align-items: baseline; gap: 16px; }
.title {
  margin: 0;
  font-family: var(--font-display);
  font-weight: 300;
  font-size: 30px;
  letter-spacing: -0.018em;
  line-height: 1.15;
  color: var(--ink);
  font-feature-settings: 'kern', 'liga', 'calt';
  &.untitled {
    font-style: italic;
    color: var(--ink-muted);
    font-weight: 300;
  }
  .muted { color: var(--ink-faint); font-style: italic; }
  .amp {
    font-style: italic;
    color: var(--accent);
    font-weight: 400;
    margin-right: 8px;
  }
}
.meta-row {
  display: flex;
  align-items: center;
  gap: 28px;
}
.timing {
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
  margin-left: auto;
  .eyebrow {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.18em;
    color: var(--ink-faint);
    text-transform: uppercase;
  }
  .hash {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--ink-muted);
    letter-spacing: 0.04em;
  }
}

// — Banner ————————————————————————————————————————————————
.banner-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 56px;
  background: var(--accent-soft);
  color: var(--accent);
  border-bottom: 1px solid var(--accent);
  font-family: var(--font-mono);
  font-size: 12px;
  letter-spacing: 0.03em;
  button {
    background: transparent;
    border: 0;
    color: var(--accent);
    text-transform: uppercase;
    letter-spacing: 0.16em;
    cursor: pointer;
    font-size: 10.5px;
    &:hover { text-decoration: underline; }
  }
}

// — Scroller / column ————————————————————————————————————————
.scroller {
  overflow-y: auto;
  padding: 28px 56px 60px;
}
.column {
  max-width: 720px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 36px;
}
.load-more {
  align-self: center;
  background: transparent;
  border: 0;
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.12em;
  color: var(--ink-faint);
  text-transform: uppercase;
  cursor: pointer;
  padding: 8px 12px;
  &:hover { color: var(--ink); }
}

// — Empty state ——————————————————————————————————————————————
.empty-state {
  margin-top: 12vh;
  text-align: center;
  .lead {
    margin: 0 0 8px;
    font-family: var(--font-display);
    font-size: 22px;
    font-weight: 300;
    color: var(--ink-muted);
    font-style: italic;
    letter-spacing: -0.005em;
  }
  .drop-cap {
    font-family: var(--font-display);
    font-size: 56px;
    line-height: 0.9;
    font-style: italic;
    color: var(--accent);
    float: left;
    margin: 4px 6px 0 0;
    font-weight: 400;
  }
  .hint {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--ink-faint);
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
}

// — Turn ————————————————————————————————————————————————————
.turn {
  display: flex;
  flex-direction: column;
  gap: 10px;
  position: relative;
}
.turn-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: var(--font-mono);
  font-size: 10.5px;
  letter-spacing: 0.18em;
  text-transform: uppercase;
  color: var(--ink-faint);
  &::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--rule);
    margin-left: 8px;
  }
  .speaker { color: var(--ink); font-weight: 500; }
  .time { color: var(--ink-faint); }
  .dot { color: var(--ink-faint); }
}
.turn.user .turn-head .speaker { color: var(--accent); }

// — Prose blocks (no bubbles) ————————————————————————————————————
.prose {
  font-family: var(--font-body);
  font-size: 16.5px;
  line-height: 1.65;
  color: var(--ink);
  letter-spacing: -0.005em;
  white-space: normal;

  &.user-prose {
    white-space: pre-wrap;
    color: var(--ink);
    font-style: normal;
  }
  &.pending {
    color: var(--ink-faint);
    font-style: italic;
    .thinking-text { font-family: var(--font-mono); font-size: 12px; letter-spacing: 0.12em; text-transform: uppercase; }
  }

  :deep(p) { margin: 0 0 0.9em; }
  :deep(p:last-child) { margin-bottom: 0; }
  :deep(strong) { font-weight: 600; }
  :deep(em) { font-style: italic; }
  :deep(a) { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; text-decoration-thickness: 1px; }
  :deep(ul), :deep(ol) { padding-left: 1.4em; margin: 0.4em 0; }
  :deep(li) { margin: 0.2em 0; }
  :deep(blockquote) {
    border-left: 2px solid var(--accent);
    margin: 0.8em 0;
    padding: 0.1em 0 0.1em 1em;
    color: var(--ink-muted);
    font-style: italic;
  }
  :deep(h1), :deep(h2), :deep(h3) {
    font-family: var(--font-display);
    font-weight: 500;
    letter-spacing: -0.015em;
    margin: 0.8em 0 0.3em;
  }
  :deep(h1) { font-size: 1.5em; }
  :deep(h2) { font-size: 1.25em; }
  :deep(h3) { font-size: 1.1em; }
  :deep(pre.hljs) {
    margin: 1em 0;
    padding: 14px 16px;
    border: 1px solid var(--rule-strong);
    background: var(--bg-deep) !important;
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.55;
    overflow-x: auto;
    border-radius: 0;
  }
  :deep(code:not(pre code)) {
    font-family: var(--font-mono);
    font-size: 0.88em;
    padding: 1px 5px;
    background: var(--bg-deep);
    border: 1px solid var(--rule);
    border-radius: 0;
  }
  :deep(hr) {
    border: 0;
    height: 1px;
    background: var(--rule);
    margin: 1.4em 0;
  }
  :deep(table) {
    border-collapse: collapse;
    margin: 0.8em 0;
    font-family: var(--font-body);
    font-size: 0.95em;
  }
  :deep(th), :deep(td) {
    text-align: left;
    padding: 6px 10px;
    border-bottom: 1px solid var(--rule);
  }
  :deep(th) { font-family: var(--font-mono); font-size: 11px; letter-spacing: 0.12em; text-transform: uppercase; color: var(--ink-muted); }
}

// — Reasoning block (set off in left margin) ——
.reasoning {
  margin: 4px 0 8px;
  padding-left: 18px;
  border-left: 1px solid var(--accent);
}
.reasoning-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
  .eyebrow {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .rule { flex: 1; height: 1px; background: var(--rule); }
}
.reasoning-body {
  font-family: var(--font-body);
  font-style: italic;
  font-size: 14px;
  line-height: 1.6;
  color: var(--ink-muted);
  white-space: pre-wrap;
}

// — Tools (inline markers, not chips) ——
.tools {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--ink-muted);
}
.tool {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  letter-spacing: 0.04em;
  .arrow { color: var(--accent); }
  .t-name { color: var(--ink); }
  .t-status {
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.2em;
    color: var(--ink-faint);
  }
  &.status-success .t-status { color: #2f8f2f; }
  &.status-error .t-status { color: var(--accent); font-style: italic; }
}

// — Caret ——————————————————————————————————————————————————
.caret {
  display: inline-block;
  width: 9px;
  height: 1.05em;
  vertical-align: text-bottom;
  background: var(--accent);
  margin-left: 1px;
  animation: blink 1.05s steps(2, end) infinite;
}
@keyframes blink { 50% { opacity: 0; } }
.trailing-caret { padding-left: 0; }

// — Composer ————————————————————————————————————————————————
.composer {
  border-top: 1px solid var(--rule);
  background: var(--bg);
  padding: 16px 56px 24px;
}
.composer-inner {
  max-width: 720px;
  margin: 0 auto;
  display: grid;
  grid-template-columns: 24px 1fr auto;
  align-items: end;
  gap: 14px;
}
.composer-rail {
  padding-top: 6px;
  .prompt-glyph {
    font-family: var(--font-mono);
    color: var(--accent);
    font-size: 14px;
  }
}
.composer-input {
  font: inherit;
  font-family: var(--font-body);
  font-size: 16.5px;
  line-height: 1.55;
  color: var(--ink);
  background: transparent;
  border: 0;
  outline: none;
  resize: vertical;
  padding: 4px 0;
  min-height: 28px;
  max-height: 240px;
  &::placeholder {
    color: var(--ink-faint);
    font-style: italic;
  }
  &:disabled { cursor: not-allowed; opacity: 0.7; }
}
.composer-actions { padding-bottom: 4px; }
.action {
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  background: transparent;
  border: 1px solid var(--rule-strong);
  padding: 8px 14px;
  cursor: pointer;
  color: var(--ink);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  transition: all 0.16s;
  border-radius: 0;
  &:hover:not(:disabled) {
    background: var(--ink);
    color: var(--bg);
    border-color: var(--ink);
    .kbd { color: var(--bg); border-color: var(--bg); }
  }
  &:disabled { opacity: 0.4; cursor: not-allowed; }

  .kbd {
    font-size: 11px;
    color: var(--ink-faint);
    border: 1px solid var(--rule);
    padding: 0 4px;
    transition: all 0.16s;
  }
  .material-symbols-outlined { font-size: 16px; }

  &.cancel {
    color: var(--accent);
    border-color: var(--accent);
    &:hover {
      background: var(--accent);
      color: var(--bg);
      border-color: var(--accent);
    }
  }
}
</style>
