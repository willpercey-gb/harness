import { defineStore } from 'pinia'
import type { Agent, ChatSession } from '@/types/chat.types'

export const useChatStore = defineStore('chat', {
  state: () => ({
    selectedAgentId: null as string | null,
    selectedAgent: null as Agent | null,
    currentSessionId: null as string | null,
    pendingTitle: null as string | null,
    /** Bumped whenever the session list should be re-fetched. */
    sessionsBumper: 0,
    /** Bumped whenever the chat page should reload current history. */
    historyBumper: 0,
  }),
  actions: {
    selectAgent(agent: Agent) {
      this.selectedAgent = agent
      this.selectedAgentId = agent.id
      // Sessions are no longer scoped to an agent — keep the active
      // session intact so the user can switch models mid-conversation
      // and pick up where they left off.
    },
    openSession(s: ChatSession) {
      this.currentSessionId = s.sessionId
      this.pendingTitle = s.title
      this.historyBumper++
    },
    newChat() {
      this.currentSessionId = null
      this.pendingTitle = null
      this.historyBumper++
    },
    bumpSessions() { this.sessionsBumper++ },
    setSessionFromStream(sessionId: string) {
      if (!this.currentSessionId) {
        this.currentSessionId = sessionId
        this.bumpSessions()
      }
    },
    /** Background title generator finished — refresh the sidebar and
     *  update pendingTitle if this is still the active session. */
    applySessionTitle(sessionId: string, title: string) {
      if (this.currentSessionId === sessionId) {
        this.pendingTitle = title
      }
      this.bumpSessions()
    },
  },
})
