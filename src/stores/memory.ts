import { defineStore } from 'pinia'
import type { StreamEvent } from '@/types/chat.types'

export interface ExtractionEntry {
  kind: 'entity' | 'relationship' | 'memory'
  label: string
  detail?: string
}

interface State {
  /** True while stage 4 is running for the current turn. */
  active: boolean
  /** The session this run belongs to, if any. */
  sessionId: string | null
  /** Live counts during the run; the final values are echoed by `done`. */
  entities: number
  relationships: number
  memories: number
  /** A small rolling log of the last extraction's writes. */
  recent: ExtractionEntry[]
  /** Set when the most recent extraction finished. */
  finishedAt: string | null
}

export const useMemoryStore = defineStore('memory', {
  state: (): State => ({
    active: false,
    sessionId: null,
    entities: 0,
    relationships: 0,
    memories: 0,
    recent: [],
    finishedAt: null,
  }),
  actions: {
    applyStreamEvent(ev: StreamEvent) {
      switch (ev.kind) {
        case 'memory_extraction_started':
          this.active = true
          this.sessionId = ev.session_id
          this.entities = 0
          this.relationships = 0
          this.memories = 0
          this.recent = []
          this.finishedAt = null
          break
        case 'entity_resolved':
          this.entities += 1
          this.recent.push({
            kind: 'entity',
            label: ev.name,
            detail: `${ev.entity_type} • ${ev.status}`,
          })
          this.trim()
          break
        case 'relationship_created':
          this.relationships += 1
          this.recent.push({
            kind: 'relationship',
            label: `${ev.from_name} → ${ev.to_name}`,
            detail: ev.relation,
          })
          this.trim()
          break
        case 'memory_stored':
          this.memories += 1
          this.recent.push({
            kind: 'memory',
            label: ev.content_preview,
          })
          this.trim()
          break
        case 'memory_extraction_done':
          this.active = false
          this.entities = ev.entities
          this.relationships = ev.relationships
          this.memories = ev.memories
          this.finishedAt = new Date().toISOString()
          break
      }
    },
    trim() {
      const MAX = 12
      if (this.recent.length > MAX) {
        this.recent = this.recent.slice(this.recent.length - MAX)
      }
    },
    reset() {
      this.active = false
      this.sessionId = null
      this.entities = 0
      this.relationships = 0
      this.memories = 0
      this.recent = []
      this.finishedAt = null
    },
  },
})
