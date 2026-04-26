import { invoke } from '@tauri-apps/api/core'
import type { ConversationContext } from '@/types/context.types'

export async function getContext(sessionId: string): Promise<ConversationContext> {
  return await invoke<ConversationContext>('get_context', { sessionId })
}

export async function updateContext(
  sessionId: string,
  context: ConversationContext,
): Promise<void> {
  await invoke('update_context', { sessionId, context })
}
