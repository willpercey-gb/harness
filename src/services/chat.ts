import { invoke, Channel } from '@tauri-apps/api/core'
import type {
  Agent,
  ChatMessage,
  ChatSession,
  StreamEvent,
} from '@/types/chat.types'
import { parseAssistantContent } from '@/services/parseStored'

export async function getAgents(): Promise<Agent[]> {
  return await invoke<Agent[]>('list_agents')
}

interface SessionsPage {
  data: ChatSession[]
  meta: { pagination: { total: number; limit: number; offset: number; hasMore: boolean } }
}

export async function getSessions(
  agentId: string,
  limit = 50,
  offset = 0,
): Promise<{ data: ChatSession[]; hasMore: boolean; total: number }> {
  const page = await invoke<SessionsPage>('list_sessions', {
    agent: agentId,
    limit,
    offset,
  })
  return {
    data: page.data,
    hasMore: page.meta.pagination.hasMore,
    total: page.meta.pagination.total,
  }
}

interface HistoryPage {
  data: Array<{
    id: string
    role: 'user' | 'assistant' | 'system'
    content: string
    createdAt: string
    agentId: string | null
  }>
  meta: { pagination: { total: number; limit: number; offset: number; hasMore: boolean } }
}

export async function getChatHistory(
  sessionId: string,
  limit = 50,
  offset = 0,
): Promise<{ messages: ChatMessage[]; hasMore: boolean; total: number }> {
  const page = await invoke<HistoryPage>('get_history', {
    sessionId,
    limit,
    offset,
  })
  // backend returns newest-first; reverse for display order
  const messages: ChatMessage[] = page.data
    .map((m) => {
      if (m.role === 'assistant') {
        const parsed = parseAssistantContent(m.content)
        return {
          id: m.id,
          role: m.role,
          content: parsed.content,
          reasoning: parsed.reasoning || undefined,
          toolEvents: parsed.toolEvents.length ? parsed.toolEvents : undefined,
          timestamp: new Date(m.createdAt),
          agentId: m.agentId ?? undefined,
        }
      }
      return {
        id: m.id,
        role: m.role,
        content: m.content,
        timestamp: new Date(m.createdAt),
        agentId: m.agentId ?? undefined,
      }
    })
    .reverse()
  return {
    messages,
    hasMore: page.meta.pagination.hasMore,
    total: page.meta.pagination.total,
  }
}

export async function deleteSession(sessionId: string): Promise<void> {
  await invoke('delete_session', { sessionId })
}

export interface StreamHandle {
  /** Resolves when the Rust task has run to completion (or emitted Cancelled). */
  done: Promise<{ sessionId: string }>
  /** Cancel the in-flight stream. Idempotent. */
  cancel: () => Promise<void>
}

/**
 * Open a streaming chat turn. Calls the `chat_send` command, passing a
 * Tauri Channel for the typed event stream. Each event is delivered to
 * the supplied `onEvent` callback synchronously as it arrives.
 */
export function streamChat(
  agentId: string,
  prompt: string,
  sessionId: string | null,
  onEvent: (e: StreamEvent) => void,
  intentOverride: string | null = null,
): StreamHandle {
  const channel = new Channel<StreamEvent>()
  channel.onmessage = (e) => onEvent(e)
  const channelId = (channel as unknown as { id: number }).id
  const done = invoke<string>('chat_send', {
    agent: agentId,
    prompt,
    sessionId,
    intentOverride,
    onEvent: channel,
  }).then((sessionId) => ({ sessionId }))
  const cancel = async () => {
    await invoke('chat_cancel', { channelId })
  }
  return { done, cancel }
}
