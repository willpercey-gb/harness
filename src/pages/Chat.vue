<script lang="ts">
import { defineComponent, nextTick } from 'vue'
import {
  getAgents,
  streamChat,
  getChatHistory,
  getSessions,
  deleteSession,
  type StreamHandle,
} from '@/services/chat'
import type {
  Agent,
  ChatMessage,
  ChatSession,
  StreamEvent,
  ToolEvent,
} from '@/types/chat.types'
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
import 'highlight.js/styles/github-dark.css'

import ToolUseChip from '@/components/chat/ToolUseChip.vue'
import ToolResultChip from '@/components/chat/ToolResultChip.vue'
import ReasoningBlock from '@/components/chat/ReasoningBlock.vue'
import StreamingIndicator from '@/components/chat/StreamingIndicator.vue'

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
      } catch (_) {
        // fall through
      }
    }
    return '<pre class="hljs"><code>' + md.utils.escapeHtml(str) + '</code></pre>'
  },
})

export default defineComponent({
  name: 'ChatPage',
  components: { ToolUseChip, ToolResultChip, ReasoningBlock, StreamingIndicator },
  data() {
    return {
      agents: [] as Agent[],
      selectedAgent: null as Agent | null,
      currentSessionId: null as string | null,
      sessions: [] as ChatSession[],
      messages: [] as ChatMessage[],
      currentMessage: '',
      loading: false,
      streaming: false,
      error: '',
      messageIdCounter: 0,
      selectedTab: 'all',
      selectedProvider: 'all',
      autoScroll: true,
      thinking: false,
      loadingHistory: false,
      hasMoreHistory: false,
      historyOffset: 0,
      historyLimit: 50,
      showSessions: false,
      loadingSessions: false,
      currentStream: null as StreamHandle | null,
    }
  },
  computed: {
    filteredAgents(): Agent[] {
      let filtered = this.agents
      if (this.selectedTab !== 'all') {
        filtered = filtered.filter((a) => a.type === this.selectedTab)
      }
      if (this.selectedProvider !== 'all') {
        filtered = filtered.filter(
          (a) => a.attributes.provider.toLowerCase() === this.selectedProvider,
        )
      }
      return filtered
    },
    agentTypeCounts(): Record<string, number> {
      const counts: Record<string, number> = {
        all: this.agents.length,
        agent: 0,
        swarm: 0,
        graph: 0,
        a2a: 0,
        distributed: 0,
      }
      this.agents.forEach((a) => {
        if (counts[a.type] !== undefined) counts[a.type]++
      })
      return counts
    },
    providerCounts(): Record<string, number> {
      const counts: Record<string, number> = {
        all: this.agents.length,
        ollama: 0,
        openrouter: 0,
        bedrock: 0,
        vertex: 0,
        openai: 0,
      }
      this.agents.forEach((a) => {
        const p = a.attributes.provider.toLowerCase()
        if (counts[p] !== undefined) counts[p]++
      })
      return counts
    },
  },
  async mounted() {
    await this.loadAgents()
  },
  methods: {
    async loadAgents() {
      try {
        this.loading = true
        this.error = ''
        this.agents = await getAgents()
        if (this.agents.length > 0 && !this.selectedAgent) {
          await this.selectAgent(this.agents[0])
        }
      } catch (e: any) {
        this.error = e?.message || String(e) || 'Failed to load agents'
      } finally {
        this.loading = false
      }
    },
    async selectAgent(agent: Agent) {
      if (agent.attributes.disabled) return
      this.selectedAgent = agent
      this.currentSessionId = null
      this.messages = []
      this.hasMoreHistory = false
      this.historyOffset = 0
      await this.loadSessions()
      nextTick(() => this.scrollToBottom())
    },
    async loadSessions() {
      if (!this.selectedAgent) return
      try {
        this.loadingSessions = true
        const res = await getSessions(this.selectedAgent.id, 50, 0)
        this.sessions = res.data
      } catch {
        this.sessions = []
      } finally {
        this.loadingSessions = false
      }
    },
    async selectSession(session: ChatSession) {
      if (this.streaming || this.currentSessionId === session.sessionId) return
      this.currentSessionId = session.sessionId
      this.loadingHistory = true
      this.historyOffset = 0
      try {
        const res = await getChatHistory(session.sessionId, this.historyLimit, 0)
        this.messages = res.messages
        this.hasMoreHistory = res.hasMore
        this.historyOffset = res.messages.length
      } catch {
        this.messages = []
        this.hasMoreHistory = false
        this.historyOffset = 0
      } finally {
        this.loadingHistory = false
      }
      nextTick(() => this.scrollToBottom())
    },
    async loadMoreHistory() {
      if (!this.currentSessionId || this.loadingHistory || !this.hasMoreHistory) return
      this.loadingHistory = true
      try {
        const res = await getChatHistory(
          this.currentSessionId,
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
    async startNewChat() {
      if (!this.selectedAgent || this.streaming) return
      this.currentSessionId = null
      this.messages = []
      this.hasMoreHistory = false
      this.historyOffset = 0
      nextTick(() => this.scrollToBottom())
    },
    async deleteCurrentSession() {
      if (!this.currentSessionId || this.streaming) return
      try {
        await deleteSession(this.currentSessionId)
        this.currentSessionId = null
        this.messages = []
        await this.loadSessions()
      } catch (e: any) {
        this.error = e?.message || 'Failed to delete session'
      }
    },
    toggleSessions() {
      this.showSessions = !this.showSessions
    },
    formatSessionDate(dateString: string): string {
      const date = new Date(dateString)
      const now = new Date()
      const diffMins = Math.floor((now.getTime() - date.getTime()) / 60000)
      if (diffMins < 1) return 'Just now'
      if (diffMins < 60) return `${diffMins}m ago`
      const diffHours = Math.floor(diffMins / 60)
      if (diffHours < 24) return `${diffHours}h ago`
      const diffDays = Math.floor(diffHours / 24)
      if (diffDays < 7) return `${diffDays}d ago`
      return date.toLocaleDateString('en-GB', { month: 'short', day: 'numeric' })
    },
    async sendMessage() {
      if (!this.currentMessage.trim() || !this.selectedAgent || this.streaming) return

      const prompt = this.currentMessage
      this.currentMessage = ''

      const userMessage: ChatMessage = {
        id: `msg-${this.messageIdCounter++}`,
        role: 'user',
        content: prompt,
        timestamp: new Date(),
        agentId: this.selectedAgent.id,
      }
      this.messages.push(userMessage)

      const assistantIndex = this.messages.length
      const assistant: ChatMessage = {
        id: `msg-${this.messageIdCounter++}`,
        role: 'assistant',
        content: '',
        reasoning: '',
        toolEvents: [],
        timestamp: new Date(),
        agentId: this.selectedAgent.id,
      }
      this.messages.push(assistant)

      this.streaming = true
      this.error = ''
      this.thinking = false
      this.autoScroll = true

      const handle = streamChat(
        this.selectedAgent.id,
        prompt,
        this.currentSessionId,
        (e: StreamEvent) => this.onStreamEvent(assistantIndex, e),
      )
      this.currentStream = handle

      try {
        const { sessionId } = await handle.done
        if (sessionId && !this.currentSessionId) {
          this.currentSessionId = sessionId
          await this.loadSessions()
        }
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
          if (!this.currentSessionId) this.currentSessionId = e.session_id
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
            {
              kind: 'tool_result',
              name: e.name,
              id: e.id,
              status: e.status,
            } as ToolEvent,
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
          // terminal events handled by handle.done resolving
          break
      }
      nextTick(() => this.scrollToBottom())
    },
    async cancel() {
      if (!this.currentStream) return
      await this.currentStream.cancel()
    },
    scrollToBottom() {
      if (!this.autoScroll) return
      const container = this.$refs.messagesContainer as HTMLElement | undefined
      if (container) container.scrollTop = container.scrollHeight
    },
    handleScroll() {
      const container = this.$refs.messagesContainer as HTMLElement | undefined
      if (!container) return
      const threshold = 50
      this.autoScroll =
        container.scrollHeight - container.scrollTop - container.clientHeight < threshold
    },
    handleKeydown(event: KeyboardEvent) {
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault()
        this.sendMessage()
      }
    },
    formatTime(date: Date): string {
      return new Date(date).toLocaleTimeString('en-GB', {
        hour: '2-digit',
        minute: '2-digit',
      })
    },
    getCostLabel(cost: string): string {
      const labels: Record<string, string> = {
        free: 'Free',
        'very-low': 'Very Low',
        low: 'Low',
        medium: 'Med',
        high: 'High',
        'very-high': 'V.High',
        uncalculated: '?',
      }
      return labels[cost] || cost
    },
    getTypeLabel(type: string): string {
      const labels: Record<string, string> = {
        agent: 'Agent',
        swarm: 'Swarm',
        graph: 'Graph',
        a2a: 'A2A',
        distributed: 'Dist.',
      }
      return labels[type] || type
    },
    renderMarkdown(content: string): string {
      return md.render(content)
    },
  },
})
</script>

<template>
  <div class="chat-agents-page">
    <div v-if="error" class="error-message">
      {{ error }}
      <span
        class="material-symbols-outlined"
        style="cursor: pointer; margin-left: auto"
        @click="error = ''"
        >close</span
      >
    </div>

    <div v-if="loading" class="loading-message">Loading agents...</div>

    <div v-if="!loading && agents.length === 0" class="loading-message">
      No agents found. Is Ollama running locally?
    </div>

    <div v-if="!loading && agents.length > 0" class="chat-container">
      <!-- Agent Sidebar -->
      <div class="agents-sidebar">
        <div class="sidebar-header">
          <h3>Available Agents</h3>
        </div>

        <div class="filter-section">
          <label class="filter-label">Type:</label>
          <div class="agent-tabs">
            <button
              v-for="tab in ['all', 'agent', 'swarm', 'graph', 'a2a', 'distributed']"
              :key="tab"
              class="tab-button"
              :class="{ active: selectedTab === tab }"
              @click="selectedTab = tab"
            >
              {{
                tab === 'all'
                  ? `All (${agentTypeCounts.all})`
                  : `${getTypeLabel(tab)} (${agentTypeCounts[tab]})`
              }}
            </button>
          </div>
        </div>

        <div class="filter-section">
          <label class="filter-label">Provider:</label>
          <div class="provider-filters">
            <button
              v-for="prov in ['all', 'ollama', 'openrouter', 'bedrock', 'vertex', 'openai']"
              :key="prov"
              class="filter-button"
              :class="[`provider-${prov}`, { active: selectedProvider === prov }]"
              @click="selectedProvider = prov"
            >
              {{
                prov === 'all'
                  ? `All (${providerCounts.all})`
                  : `${prov.charAt(0).toUpperCase() + prov.slice(1)} (${providerCounts[prov]})`
              }}
            </button>
          </div>
        </div>

        <div class="agents-list">
          <div
            v-for="agent in filteredAgents"
            :key="agent.id"
            class="agent-card"
            :class="{
              selected: selectedAgent?.id === agent.id,
              disabled: agent.attributes.disabled,
            }"
            @click="selectAgent(agent)"
          >
            <div class="agent-name">
              {{ agent.attributes.name }}
              <span
                v-if="agent.attributes.disabled"
                class="disabled-indicator material-symbols-outlined"
                >block</span
              >
            </div>
            <div class="agent-badges">
              <span class="type-badge" :class="`type-${agent.type}`">
                {{ getTypeLabel(agent.type) }}
              </span>
              <span v-if="agent.attributes.parameters" class="power-badge">
                {{ agent.attributes.parameters }}B
              </span>
              <span class="cost-badge" :class="`cost-${agent.attributes.cost}`">
                <span class="cost-icon">$</span>
                {{ getCostLabel(agent.attributes.cost) }}
              </span>
            </div>
            <div class="agent-description">
              {{ agent.attributes.description }}
            </div>
            <div class="agent-footer">
              <span
                class="agent-provider"
                :class="`provider-${agent.attributes.provider.toLowerCase()}`"
                >{{ agent.attributes.provider }}</span
              >
            </div>
          </div>
        </div>
      </div>

      <!-- Chat Area -->
      <div class="chat-area">
        <div class="chat-header">
          <div v-if="selectedAgent" class="selected-agent-info">
            <h2>{{ selectedAgent.attributes.name }}</h2>
            <p>{{ selectedAgent.attributes.description }}</p>
          </div>
          <div class="header-actions">
            <button
              class="button button-sessions"
              @click="toggleSessions"
              :disabled="!selectedAgent"
            >
              <span class="material-symbols-outlined">history</span>
              {{ showSessions ? 'Hide' : 'Sessions' }}
            </button>
            <button
              class="button button-new-chat"
              @click="startNewChat"
              :disabled="!selectedAgent || streaming"
            >
              <span class="material-symbols-outlined">add_circle</span>
              New
            </button>
            <button
              v-if="currentSessionId"
              class="button button-clear"
              @click="deleteCurrentSession"
              :disabled="streaming"
            >
              <span class="material-symbols-outlined">delete</span>
              Delete
            </button>
          </div>
        </div>

        <div v-if="showSessions" class="sessions-panel">
          <div class="sessions-header">
            <h3>Sessions ({{ sessions.length }})</h3>
          </div>
          <div v-if="loadingSessions" class="sessions-empty">Loading...</div>
          <div v-else-if="sessions.length === 0" class="sessions-empty">
            No previous sessions.
          </div>
          <div v-else class="sessions-list">
            <div
              v-for="s in sessions"
              :key="s.sessionId"
              class="session-item"
              :class="{ active: currentSessionId === s.sessionId }"
              @click="selectSession(s)"
            >
              <div class="session-header-row">
                <span class="session-title">{{ s.title || 'Untitled' }}</span>
                <span class="session-date">{{ formatSessionDate(s.lastMessageAt) }}</span>
              </div>
              <div class="session-meta">
                <span class="session-messages">
                  <span class="material-symbols-outlined">chat</span>
                  {{ s.messageCount }} messages
                </span>
              </div>
            </div>
          </div>
        </div>

        <div class="messages-container" ref="messagesContainer" @scroll="handleScroll">
          <div v-if="hasMoreHistory && messages.length > 0" class="load-more-container">
            <button
              class="button button-load-more"
              @click="loadMoreHistory"
              :disabled="loadingHistory"
            >
              <span class="material-symbols-outlined">expand_less</span>
              {{ loadingHistory ? 'Loading...' : 'Load more history' }}
            </button>
          </div>

          <div v-if="messages.length === 0 && !loadingHistory" class="empty-state">
            <span class="material-symbols-outlined">chat</span>
            <p>Start a conversation with {{ selectedAgent?.attributes.name }}</p>
          </div>

          <div v-for="message in messages" :key="message.id" class="message" :class="message.role">
            <div class="message-meta">
              <span class="message-role">{{
                message.role === 'user' ? 'You' : selectedAgent?.attributes.name
              }}</span>
              <span class="message-time">{{ formatTime(message.timestamp) }}</span>
            </div>
            <div class="message-content">
              <ReasoningBlock v-if="message.reasoning" :content="message.reasoning" />
              <template v-for="(t, i) in message.toolEvents || []" :key="i">
                <ToolUseChip v-if="t.kind === 'tool_use'" :name="t.name" />
                <ToolResultChip
                  v-else-if="t.kind === 'tool_result'"
                  :name="t.name"
                  :status="t.status"
                />
              </template>
              <div
                v-if="message.role === 'user'"
                class="user-content"
              >{{ message.content }}</div>
              <div
                v-else
                class="assistant-content"
                v-html="renderMarkdown(message.content)"
              ></div>
            </div>
          </div>

          <StreamingIndicator v-if="streaming" :thinking="thinking" />
        </div>

        <div class="input-area">
          <textarea
            v-model="currentMessage"
            class="message-input"
            placeholder="Type your message... (Enter to send, Shift+Enter for new line)"
            rows="3"
            :disabled="streaming || !selectedAgent"
            @keydown="handleKeydown"
          ></textarea>
          <div class="input-actions">
            <button
              v-if="streaming"
              class="button button-cancel"
              @click="cancel"
            >
              <span class="material-symbols-outlined">stop_circle</span>
              Cancel
            </button>
            <button
              v-else
              class="button button-send"
              @click="sendMessage"
              :disabled="!currentMessage.trim() || !selectedAgent"
            >
              Send
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped lang="scss">
.chat-agents-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
}
.error-message {
  background-color: #f8d7da;
  color: #721c24;
  padding: 10px 14px;
  border-radius: 6px;
  margin-bottom: 12px;
  border: 1px solid #f5c6cb;
  display: flex;
  align-items: center;
  font-size: 13px;
}
.loading-message {
  color: var(--inactive-color);
  text-align: center;
  padding: 40px;
  font-size: 14px;
}

.chat-container {
  display: flex;
  gap: 16px;
  flex: 1;
  overflow: hidden;
  min-height: 0;
}

// Sidebar -----------------------------------------------------------------
.agents-sidebar {
  width: 260px;
  background-color: var(--projects-section);
  border: 1px solid var(--message-box-border);
  border-radius: 10px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow: hidden;

  h3 {
    margin: 0;
    color: var(--secondary-color);
    font-size: 14px;
    font-weight: 700;
  }
}
.filter-label {
  display: block;
  font-size: 10px;
  font-weight: 700;
  color: var(--inactive-color);
  text-transform: uppercase;
  letter-spacing: 0.3px;
  margin-bottom: 5px;
}
.agent-tabs,
.provider-filters {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.tab-button,
.filter-button {
  padding: 4px 8px;
  font-size: 11px;
  font-weight: 600;
  background-color: var(--projects-section);
  color: var(--inactive-color);
  border: 1px solid var(--message-box-border);
  border-radius: 4px;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.15s;
  &:hover {
    background-color: var(--hover-menu-bg);
  }
  &.active {
    background-color: var(--link-color);
    color: white;
    border-color: var(--link-color);
  }
  &.provider-ollama.active { background-color: #8B5CF6; border-color: #8B5CF6; }
  &.provider-openrouter.active { background-color: #16a34a; border-color: #16a34a; }
  &.provider-bedrock.active { background-color: #FF9900; color: #232F3E; border-color: #FF9900; }
  &.provider-vertex.active { background-color: #4285F4; border-color: #4285F4; }
  &.provider-openai.active { background-color: #10a37f; border-color: #10a37f; }
}
.agents-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow-y: auto;
  padding-right: 6px;
  flex: 1;
  min-height: 0;
}
.agent-card {
  background-color: var(--projects-section);
  border: 1px solid var(--message-box-border);
  border-radius: 6px;
  padding: 8px;
  cursor: pointer;
  transition: all 0.15s;
  &:hover:not(.disabled) {
    border-color: var(--link-color);
    box-shadow: 0 2px 6px rgba(0, 123, 255, 0.12);
  }
  &.selected {
    border-color: #28a745;
    background-color: rgba(40, 167, 69, 0.07);
  }
  &.disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .agent-name {
    font-weight: 700;
    font-size: 13px;
    color: var(--main-color);
    line-height: 1.2;
    margin-bottom: 4px;
    display: flex;
    align-items: center;
    gap: 4px;
    .disabled-indicator { font-size: 14px; color: #dc3545; }
  }
  .agent-badges {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    margin-bottom: 4px;
  }
  .agent-description {
    font-size: 10px;
    color: var(--inactive-color);
    margin-bottom: 4px;
    line-height: 1.3;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }
}
.type-badge {
  font-size: 9px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 8px;
  text-transform: uppercase;
  letter-spacing: 0.3px;
  &.type-agent { background-color: #e3f2fd; color: #1565c0; }
  &.type-swarm { background-color: #f3e5f5; color: #6a1b9a; }
  &.type-graph { background-color: #e8f5e9; color: #2e7d32; }
  &.type-a2a { background-color: #fff3e0; color: #e65100; }
  &.type-distributed { background-color: #e0f2f1; color: #00695c; }
}
.power-badge {
  font-size: 9px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 8px;
  background-color: #e7f1ff;
  color: #004085;
}
.cost-badge {
  font-size: 9px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 8px;
  display: inline-flex;
  align-items: center;
  gap: 2px;
  &.cost-free { background-color: #d4edda; color: #155724; }
  &.cost-low { background-color: #fff3cd; color: #856404; }
  &.cost-medium { background-color: #ffe8cc; color: #cc6600; }
  &.cost-high { background-color: #f8d7da; color: #721c24; }
  &.cost-uncalculated { background-color: #e0e0e0; color: #616161; }
  .cost-icon { font-size: 9px; opacity: 0.85; }
}
.agent-provider {
  font-size: 9px;
  color: white;
  font-family: ui-monospace, monospace;
  background-color: #6c757d;
  padding: 2px 6px;
  border-radius: 3px;
  display: inline-block;
  text-transform: uppercase;
  font-weight: 700;
  &.provider-ollama { background-color: #8B5CF6; }
  &.provider-openrouter { background-color: #16a34a; }
  &.provider-bedrock { background-color: #FF9900; color: #232F3E; }
  &.provider-vertex { background-color: #4285F4; }
  &.provider-openai { background-color: #10a37f; }
}

// Chat area --------------------------------------------------------------
.chat-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  background-color: var(--projects-section);
  border-radius: 10px;
  border: 1px solid var(--message-box-border);
  overflow: hidden;
  min-width: 0;
}
.chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 14px;
  border-bottom: 1px solid var(--message-box-border);
  background-color: var(--hover-menu-bg);
  .selected-agent-info {
    flex: 1;
    h2 { margin: 0 0 2px 0; font-size: 15px; color: var(--main-color); }
    p { margin: 0; font-size: 12px; color: var(--inactive-color); }
  }
  .header-actions { display: flex; gap: 6px; }
}
.sessions-panel {
  border-bottom: 1px solid var(--message-box-border);
  background-color: var(--hover-menu-bg);
  max-height: 200px;
  overflow-y: auto;
}
.sessions-header {
  padding: 8px 14px;
  border-bottom: 1px solid var(--message-box-border);
  h3 { margin: 0; font-size: 12px; color: var(--secondary-color); }
}
.sessions-empty {
  padding: 16px;
  font-size: 13px;
  color: var(--inactive-color);
  text-align: center;
}
.sessions-list { padding: 4px; }
.session-item {
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  &:hover { background-color: var(--hover-menu-bg); }
  &.active {
    background-color: rgba(40, 167, 69, 0.1);
    border-left: 3px solid #28a745;
  }
}
.session-header-row { display: flex; justify-content: space-between; }
.session-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--main-color);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  max-width: 70%;
}
.session-date {
  font-size: 11px;
  color: var(--inactive-color);
}
.session-meta {
  font-size: 11px;
  color: var(--inactive-color);
  display: flex;
  align-items: center;
  gap: 4px;
  .material-symbols-outlined { font-size: 14px; }
}
.messages-container {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.load-more-container { text-align: center; padding-bottom: 8px; }
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--inactive-color);
  .material-symbols-outlined { font-size: 64px; opacity: 0.4; margin-bottom: 12px; }
}
.message {
  display: flex;
  flex-direction: column;
  max-width: 92%;
  &.user {
    align-self: flex-end;
    .message-content {
      background-color: var(--link-color);
      color: white;
      border-radius: 16px 16px 4px 16px;
      padding: 10px 14px;
      .user-content { white-space: pre-wrap; }
    }
    .message-meta { align-self: flex-end; }
  }
  &.assistant {
    align-self: flex-start;
    .message-content {
      background-color: var(--hover-menu-bg);
      color: var(--main-color);
      border-radius: 16px 16px 16px 4px;
      padding: 10px 14px;
      line-height: 1.55;
      :deep(pre.hljs) {
        border-radius: 6px;
        padding: 10px 14px;
        margin: 8px 0;
        font-size: 12.5px;
        overflow-x: auto;
      }
      :deep(code:not(pre code)) {
        background: rgba(0, 0, 0, 0.08);
        padding: 1px 4px;
        border-radius: 4px;
        font-size: 12.5px;
      }
    }
  }
}
.message-meta {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 4px;
  font-size: 11px;
  color: var(--inactive-color);
  .message-role { font-weight: 600; }
  .message-time { opacity: 0.7; }
}
.input-area {
  display: flex;
  gap: 10px;
  padding: 10px 14px;
  border-top: 1px solid var(--message-box-border);
  background-color: var(--hover-menu-bg);
}
.message-input {
  flex: 1;
  padding: 8px 10px;
  border: 1px solid var(--message-box-border);
  border-radius: 6px;
  font-size: 13px;
  font-family: inherit;
  resize: none;
  background-color: var(--search-area-bg);
  color: var(--main-color);
  &:focus {
    outline: none;
    border-color: var(--link-color);
  }
  &:disabled { opacity: 0.7; }
}
.input-actions {
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  gap: 6px;
}
.button {
  padding: 6px 12px;
  border: none;
  border-radius: 4px;
  background-color: var(--link-color);
  color: white;
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  &:disabled {
    background-color: var(--inactive-color);
    cursor: not-allowed;
    opacity: 0.6;
  }
  .material-symbols-outlined { font-size: 14px; }
}
.button-send { background-color: #28a745; }
.button-cancel { background-color: #dc3545; }
.button-sessions { background-color: #fd7e14; }
.button-new-chat { background-color: #28a745; }
.button-clear { background-color: #dc3545; }
.button-load-more { background-color: #17a2b8; }
</style>
