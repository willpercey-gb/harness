<script lang="ts">
import { defineComponent, nextTick } from 'vue'
import {
  streamChat,
  getChatHistory,
  type StreamHandle,
} from '@/services/chat'
import { useChatStore } from '@/stores/chat'
import { useContextStore } from '@/stores/context'
import { useMemoryStore } from '@/stores/memory'
import type { ChatMessage, StreamEvent, ToolEvent } from '@/types/chat.types'
import type { Intent } from '@/types/context.types'
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
import IntentDropdown from '@/components/composer/IntentDropdown.vue'

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
  components: { AgentSelector, IntentDropdown },
  setup() {
    return {
      chat: useChatStore(),
      context: useContextStore(),
      memory: useMemoryStore(),
    }
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
      intentOverride: 'auto' as Intent | 'auto',
    }
  },
  computed: {
    hasContent(): boolean {
      return this.messages.length > 0
    },
  },
  watch: {
    'chat.historyBumper'() {
      this.title = this.chat.pendingTitle ?? ''
      this.loadHistory()
      this.context.loadForSession(this.chat.currentSessionId)
    },
  },
  mounted() {
    if (this.chat.currentSessionId) this.loadHistory()
    this.context.loadForSession(this.chat.currentSessionId)
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

      if (!this.title) this.title = prompt.slice(0, 60)

      this.streaming = true
      this.error = ''
      this.thinking = false
      this.autoScroll = true

      const overrideForRequest =
        this.intentOverride === 'auto' ? null : this.intentOverride

      const handle = streamChat(
        this.chat.selectedAgent.id,
        prompt,
        this.chat.currentSessionId,
        (e: StreamEvent) => this.onStreamEvent(idx, e),
        overrideForRequest,
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
        // Reset intent override to Auto after each turn (per plan).
        this.intentOverride = 'auto'
      }
    },
    onStreamEvent(idx: number, e: StreamEvent) {
      // Mirror context-pipeline events into the context store first;
      // this is independent of the chat message state.
      this.context.applyStreamEvent(e)
      // Stage-4 (passive memory extractor) events feed the memory
      // store; the right-sidebar widget reads from there.
      this.memory.applyStreamEvent(e)

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
        case 'session_titled':
          this.chat.applySessionTitle(e.session_id, e.title)
          break
        case 'done':
        case 'cancelled':
        case 'context_started':
        case 'context_anchor':
        case 'context_priority':
        case 'context_aside':
        case 'context_done':
        case 'intent_classified':
        case 'memory_extraction_started':
        case 'entity_resolved':
        case 'relationship_created':
        case 'memory_stored':
        case 'memory_extraction_done':
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
    renderMarkdown(s: string): string {
      return md.render(s)
    },
  },
})
</script>

<template>
  <div class="page">
    <header v-if="chat.currentSessionId" class="session-alcove">
      <div class="session-pill">
        <span class="material-symbols-outlined dot">forum</span>
        <span v-if="title" class="title">{{ title }}</span>
        <span class="sep" v-if="title">·</span>
        <span class="sid">{{ chat.currentSessionId.slice(0, 8) }}</span>
      </div>
    </header>

    <div v-if="error" class="banner-error">
      <span>{{ error }}</span>
      <button @click="error = ''" aria-label="Dismiss">
        <span class="material-symbols-outlined">close</span>
      </button>
    </div>

    <section class="scroller" ref="scroller" @scroll="handleScroll">
      <div class="thread">
        <button
          v-if="hasMoreHistory && messages.length > 0"
          class="load-more"
          @click="loadMoreHistory"
          :disabled="loadingHistory"
        >
          {{ loadingHistory ? 'Loading…' : 'Load earlier messages' }}
        </button>

        <div v-if="!hasContent && !loadingHistory" class="empty">
          <h2 class="empty-title">What can I help with?</h2>
          <p class="empty-sub" v-if="chat.selectedAgent">Talking with {{ chat.selectedAgent.attributes.name }}</p>
          <p class="empty-sub" v-else>Pick an agent to begin.</p>
        </div>

        <template v-for="(m, i) in messages" :key="m.id">
          <div v-if="m.role === 'user'" class="row user">
            <div class="bubble">{{ m.content }}</div>
          </div>

          <div v-else class="row assistant">
            <div v-if="m.reasoning" class="reasoning">
              <details open>
                <summary>
                  <span class="material-symbols-outlined">psychology</span>
                  Thinking
                </summary>
                <div class="reasoning-body">{{ m.reasoning }}</div>
              </details>
            </div>

            <ul v-if="m.toolEvents && m.toolEvents.length" class="tools">
              <li
                v-for="(t, j) in m.toolEvents"
                :key="j"
                class="tool"
                :class="[t.kind, t.kind === 'tool_result' ? `status-${t.status}` : '']"
              >
                <span class="material-symbols-outlined tool-icon">
                  {{ t.kind === 'tool_use' ? 'bolt' : (t.status === 'success' ? 'check' : 'error') }}
                </span>
                <span class="tool-name">{{ t.name }}</span>
                <span v-if="t.kind === 'tool_result'" class="tool-status">{{ t.status }}</span>
              </li>
            </ul>

            <div
              v-if="m.content"
              class="prose"
              v-html="renderMarkdown(m.content)"
            ></div>
            <div v-else-if="streaming && i === messages.length - 1" class="prose pending">
              <span class="dots"><span></span><span></span><span></span></span>
            </div>
          </div>
        </template>
      </div>
    </section>

    <footer class="composer-wrap">
      <div class="composer" :class="{ disabled: !chat.selectedAgent }">
        <textarea
          v-model="currentMessage"
          rows="4"
          :placeholder="chat.selectedAgent ? `Message ${chat.selectedAgent.attributes.name}…` : 'Pick an agent below to begin.'"
          :disabled="streaming || !chat.selectedAgent"
          @keydown="handleKeydown"
          ref="composer"
          class="composer-input"
        ></textarea>
        <div class="composer-bar">
          <div class="composer-bar-left">
            <AgentSelector compact />
            <IntentDropdown v-model:value="intentOverride" compact />
          </div>
          <div class="composer-actions">
            <button
              v-if="streaming"
              class="action stop"
              @click="cancel"
              title="Stop generating"
            >
              <span class="material-symbols-outlined">stop</span>
            </button>
            <button
              v-else
              class="action send"
              :disabled="!currentMessage.trim() || !chat.selectedAgent"
              @click="sendMessage"
              title="Send"
            >
              <span class="material-symbols-outlined">arrow_upward</span>
            </button>
          </div>
        </div>
      </div>
    </footer>
  </div>
</template>

<style scoped lang="scss">
.page {
  display: flex;
  flex-direction: column;
  height: 100%;
  flex: 1;
  min-height: 0;
  background-color: var(--bg);
  padding-top: 8px;
}

// — Banner —————————————————————————————————————————
.banner-error {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 20px;
  background: #fee2e2;
  color: #991b1b;
  font-size: 13px;
  border-bottom: 1px solid #fecaca;
  button {
    background: transparent;
    border: 0;
    cursor: pointer;
    color: #991b1b;
    display: inline-flex;
    align-items: center;
    .material-symbols-outlined { font-size: 18px; }
  }
}
:global(html.dark) .banner-error {
  background: #2a1212;
  color: #fca5a5;
  border-color: #422525;
  button { color: #fca5a5; }
}

// — Scroll —————————————————————————————————————————
.scroller {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 24px 24px 16px;
}
.thread {
  max-width: 760px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.load-more {
  align-self: center;
  background: transparent;
  border: 1px solid var(--rule);
  color: var(--ink-muted);
  font-size: 12.5px;
  padding: 6px 14px;
  border-radius: 999px;
  cursor: pointer;
  transition: all 0.12s;
  &:hover { background: var(--bg-soft); color: var(--ink); }
}

// — Empty state ——————————————————————————————————
.empty {
  text-align: center;
  margin-top: 18vh;
  .empty-title {
    margin: 0 0 6px;
    font-size: 26px;
    font-weight: 600;
    color: var(--ink);
    letter-spacing: -0.015em;
  }
  .empty-sub {
    margin: 0;
    color: var(--ink-faint);
    font-size: 14px;
  }
}

// — User row ——————————————————————————————————————
.row {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.row.user {
  align-items: flex-end;
  .bubble {
    background: var(--user-bubble);
    color: var(--ink);
    padding: 10px 14px;
    border-radius: var(--radius-lg);
    max-width: min(80%, 540px);
    white-space: pre-wrap;
    line-height: 1.5;
    font-size: 14.5px;
  }
}

// — Assistant row ————————————————————————————————
.row.assistant {
  align-items: stretch;
}

.reasoning {
  margin-bottom: 6px;
  details {
    border: 1px solid var(--rule);
    border-radius: var(--radius-md);
    background: var(--bg-soft);
    overflow: hidden;
    summary {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 7px 12px;
      cursor: pointer;
      font-size: 12.5px;
      color: var(--ink-muted);
      list-style: none;
      &::-webkit-details-marker { display: none; }
      .material-symbols-outlined { font-size: 16px; }
    }
    .reasoning-body {
      padding: 10px 14px 14px;
      font-size: 13px;
      color: var(--ink-muted);
      line-height: 1.6;
      white-space: pre-wrap;
      border-top: 1px solid var(--rule);
    }
  }
}

.tools {
  list-style: none;
  margin: 0 0 6px;
  padding: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.tool {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--bg-soft);
  border: 1px solid var(--rule);
  font-size: 12px;
  color: var(--ink-muted);
  .tool-icon { font-size: 14px; color: var(--ink-faint); }
  .tool-name { color: var(--ink); font-weight: 500; }
  .tool-status {
    text-transform: lowercase;
    font-size: 11px;
    color: var(--ink-faint);
  }
  &.status-success {
    .tool-icon { color: #16a34a; }
    .tool-status { color: #16a34a; }
  }
  &.status-error {
    .tool-icon { color: #dc2626; }
    .tool-status { color: #dc2626; }
  }
}

// — Prose ————————————————————————————————————————
.prose {
  font-size: 14.5px;
  line-height: 1.65;
  color: var(--ink);
  &.pending { color: var(--ink-faint); }
  :deep(p) { margin: 0 0 0.9em; }
  :deep(p:last-child) { margin-bottom: 0; }
  :deep(strong) { font-weight: 600; }
  :deep(em) { font-style: italic; }
  :deep(a) { color: #2563eb; text-decoration: underline; text-underline-offset: 2px; }
  :deep(ul), :deep(ol) { padding-left: 1.4em; margin: 0.4em 0 0.9em; }
  :deep(li) { margin: 0.2em 0; }
  :deep(blockquote) {
    border-left: 3px solid var(--rule-strong);
    margin: 0.6em 0;
    padding: 0.1em 0 0.1em 0.9em;
    color: var(--ink-muted);
  }
  :deep(h1), :deep(h2), :deep(h3) {
    font-weight: 600;
    letter-spacing: -0.012em;
    margin: 0.9em 0 0.4em;
  }
  :deep(h1) { font-size: 1.4em; }
  :deep(h2) { font-size: 1.22em; }
  :deep(h3) { font-size: 1.08em; }
  :deep(pre.hljs) {
    margin: 0.8em 0;
    padding: 12px 14px;
    background: var(--bg-deep) !important;
    color: var(--ink);
    font-family: ui-monospace, SFMono-Regular, 'JetBrains Mono', monospace;
    font-size: 12.5px;
    line-height: 1.55;
    overflow-x: auto;
    border-radius: var(--radius-md);
    border: 1px solid var(--rule);
  }
  :deep(code:not(pre code)) {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 0.9em;
    padding: 1px 5px;
    background: var(--bg-deep);
    border-radius: 4px;
  }
  :deep(table) {
    border-collapse: collapse;
    margin: 0.6em 0;
    font-size: 0.95em;
  }
  :deep(th), :deep(td) {
    text-align: left;
    padding: 6px 10px;
    border-bottom: 1px solid var(--rule);
  }
  :deep(th) { font-weight: 600; }
  :deep(hr) { border: 0; height: 1px; background: var(--rule); margin: 1em 0; }
}

// — Streaming dots ————————————————————————————————
.dots {
  display: inline-flex;
  gap: 4px;
  align-items: center;
  span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ink-faint);
    animation: blink 1.2s infinite;
    &:nth-child(2) { animation-delay: 0.2s; }
    &:nth-child(3) { animation-delay: 0.4s; }
  }
}
@keyframes blink {
  0%, 80%, 100% { opacity: 0.3; transform: scale(0.85); }
  40% { opacity: 1; transform: scale(1); }
}

// — Composer ——————————————————————————————————————
.composer-wrap {
  flex-shrink: 0;
  padding: 16px 24px 32px;
  background: var(--bg);
}
.composer {
  max-width: 760px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  background: var(--bg-soft);
  border: 1px solid var(--rule);
  border-radius: var(--radius-xl);
  padding: 12px 14px 8px;
  transition: border-color 0.12s;
  &:focus-within { border-color: var(--rule-strong); }
  &.disabled { opacity: 0.7; }
}
.composer-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 6px;
}
.composer-bar-left {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  flex: 1;
}
// — Session alcove ————————————————————————————————
.session-alcove {
  flex-shrink: 0;
  display: flex;
  justify-content: center;
  padding: 6px 24px 4px;
  background: var(--bg);
}
.session-pill {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  max-width: min(560px, 80%);
  padding: 5px 14px;
  background: var(--bg-deep);
  border: 1px solid var(--rule);
  border-radius: 999px;
  box-shadow: inset 0 1px 0 rgba(0, 0, 0, 0.04);
  font-size: 12px;
  color: var(--ink-muted);

  .dot {
    font-size: 14px;
    color: var(--ink-faint);
    flex-shrink: 0;
  }
  .title {
    color: var(--ink);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .sep { color: var(--ink-faint); flex-shrink: 0; }
  .sid {
    font-family: ui-monospace, SFMono-Regular, 'JetBrains Mono', monospace;
    font-size: 11.5px;
    color: var(--ink-faint);
    flex-shrink: 0;
  }
}
.composer-input {
  border: 0;
  outline: 0;
  background: transparent;
  font: inherit;
  font-size: 14.5px;
  line-height: 1.55;
  color: var(--ink);
  resize: none;
  padding: 0;
  min-height: 84px;
  max-height: 320px;
  width: 100%;
  &::placeholder { color: var(--ink-faint); }
  &:disabled { cursor: not-allowed; }
}
.composer-actions { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }
.action {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: 0;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: all 0.12s;
  background: var(--ink);
  color: var(--bg);
  &:disabled {
    background: var(--rule-strong);
    color: var(--ink-faint);
    cursor: not-allowed;
  }
  &:hover:not(:disabled) { transform: scale(1.05); }
  &.stop { background: var(--ink); }
  .material-symbols-outlined { font-size: 18px; }
}
</style>
