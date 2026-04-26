import { defineStore } from 'pinia'
import type { ConversationContext, ContextCard, Intent, IntentSource } from '@/types/context.types'
import { emptyContext } from '@/types/context.types'
import type { StreamEvent } from '@/types/chat.types'
import { getContext, updateContext } from '@/services/context'

interface State {
  sessionId: string | null
  context: ConversationContext
  /** True while the anchor agent is mid-stream for this turn. */
  refreshing: boolean
  /** Most recently classified intent for the active turn (null between turns). */
  lastIntent: { intent: Intent; source: IntentSource } | null
  /** While true, accumulate context_* events into a draft and
   *  swap into `context` on context_done. */
  draftActive: boolean
  draft: ConversationContext
  saveError: string
}

export const useContextStore = defineStore('context', {
  state: (): State => ({
    sessionId: null,
    context: emptyContext(),
    refreshing: false,
    lastIntent: null,
    draftActive: false,
    draft: emptyContext(),
    saveError: '',
  }),
  actions: {
    /** Load (or clear) the context for a given session. */
    async loadForSession(sessionId: string | null) {
      this.sessionId = sessionId
      this.lastIntent = null
      if (!sessionId) {
        this.context = emptyContext()
        return
      }
      try {
        this.context = await getContext(sessionId)
      } catch {
        this.context = emptyContext()
      }
    },

    /** Apply one StreamEvent. Non-context events are ignored. */
    applyStreamEvent(ev: StreamEvent) {
      switch (ev.kind) {
        case 'context_started':
          this.refreshing = true
          this.draftActive = true
          this.draft = emptyContext()
          break
        case 'context_anchor':
          if (this.draftActive) this.draft.anchor = ev.text
          break
        case 'context_priority':
          if (this.draftActive) {
            this.draft.priorities.push({
              id: ev.id,
              text: ev.text,
              edited_by_user: ev.edited_by_user,
            })
          }
          break
        case 'context_aside':
          if (this.draftActive) {
            this.draft.asides.push({
              id: ev.id,
              text: ev.text,
              edited_by_user: ev.edited_by_user,
            })
          }
          break
        case 'context_done':
          this.context = this.draft
          this.draft = emptyContext()
          this.draftActive = false
          this.refreshing = false
          break
        case 'intent_classified':
          this.lastIntent = {
            intent: ev.intent as Intent,
            source: ev.source,
          }
          break
        case 'session_started':
          // session bind happens in chat store; we just need the id.
          if (!this.sessionId) this.sessionId = ev.session_id
          break
        default:
          break
      }
    },

    /** Save the current edited context to the backend. */
    async persist() {
      if (!this.sessionId) return
      this.saveError = ''
      try {
        await updateContext(this.sessionId, this.context)
      } catch (e: any) {
        this.saveError = e?.message ?? String(e)
      }
    },

    setAnchor(text: string) {
      this.context.anchor = text || null
      this.persist()
    },

    addPriority(text: string) {
      const card: ContextCard = {
        id: crypto.randomUUID(),
        text,
        edited_by_user: true,
      }
      this.context.priorities.push(card)
      this.persist()
    },

    editPriority(id: string, text: string) {
      const c = this.context.priorities.find((p) => p.id === id)
      if (!c) return
      c.text = text
      c.edited_by_user = true
      this.persist()
    },

    deletePriority(id: string) {
      this.context.priorities = this.context.priorities.filter((p) => p.id !== id)
      this.persist()
    },

    addAside(text: string) {
      const card: ContextCard = {
        id: crypto.randomUUID(),
        text,
        edited_by_user: true,
      }
      this.context.asides.push(card)
      this.persist()
    },

    editAside(id: string, text: string) {
      const c = this.context.asides.find((a) => a.id === id)
      if (!c) return
      c.text = text
      c.edited_by_user = true
      this.persist()
    },

    deleteAside(id: string) {
      this.context.asides = this.context.asides.filter((a) => a.id !== id)
      this.persist()
    },

    /** Reclassify a card by moving it from one bucket to the other.
     *  Marks the card as edited so the next refresh preserves it. */
    moveCard(fromKind: 'priority' | 'aside', id: string, toKind: 'priority' | 'aside') {
      if (fromKind === toKind) return
      if (fromKind === 'priority') {
        const idx = this.context.priorities.findIndex((p) => p.id === id)
        if (idx === -1) return
        const [card] = this.context.priorities.splice(idx, 1)
        card.edited_by_user = true
        this.context.asides.push(card)
      } else {
        const idx = this.context.asides.findIndex((a) => a.id === id)
        if (idx === -1) return
        const [card] = this.context.asides.splice(idx, 1)
        card.edited_by_user = true
        this.context.priorities.push(card)
      }
      this.persist()
    },
  },
})
