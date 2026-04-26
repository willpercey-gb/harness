// Mirrors crates/harness-storage/src/context_store.rs and
// crates/harness-chat/src/context.rs.

export type Intent = 'expand' | 'revise' | 'redirect' | 'aside'
export type IntentSource = 'auto' | 'manual'

export interface ContextCard {
  id: string
  text: string
  edited_by_user: boolean
}

export interface ConversationContext {
  anchor: string | null
  priorities: ContextCard[]
  asides: ContextCard[]
  updated_at?: string | null
}

export function emptyContext(): ConversationContext {
  return { anchor: null, priorities: [], asides: [] }
}
