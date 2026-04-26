// Mirrors the AgentDto / AgentAttributes shape returned by the Rust
// `list_agents` command. Keep in sync with crates/harness-chat/src/agent_registry.rs.

export type Architecture = 'moe' | 'mod' | 'dense' | 'sparse' | 'hybrid' | 'unknown'
export type CostTier =
  | 'free'
  | 'very-low'
  | 'low'
  | 'medium'
  | 'high'
  | 'very-high'
  | 'uncalculated'
export type AgentType = 'agent' | 'swarm' | 'graph' | 'a2a' | 'distributed'

export interface AgentAttributes {
  name: string
  description: string
  modelId: string | null
  provider: string
  region: string | null
  cost: CostTier
  inputCost: number | null
  outputCost: number | null
  totalCost: number | null
  parameters: number | null
  totalParameters?: number | null
  architecture?: Architecture
  supportsTools: boolean
  supportsSessionManager: boolean
  disabled?: boolean
  disabledMessage?: string | null
}

export interface Agent {
  id: string
  type: AgentType
  attributes: AgentAttributes
}

export interface ChatSession {
  sessionId: string
  title: string
  agentId: string | null
  messageCount: number
  createdAt: string
  lastMessageAt: string
}

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: Date
  agentId?: string
  reasoning?: string
  toolEvents?: ToolEvent[]
}

export type ToolEvent =
  | { kind: 'tool_use'; name: string; id: string }
  | { kind: 'tool_result'; name: string; id: string; status: 'success' | 'error' }

// ---------------------------------------------------------------------------
// StreamEvent — typed IPC payload from the Rust core. Mirrors
// crates/harness-chat/src/pipeline/events.rs.
// ---------------------------------------------------------------------------

export type StopReason = 'end_turn' | 'tool_use' | 'max_tokens' | 'cancelled' | 'error'

export interface Usage {
  input_tokens: number | null
  output_tokens: number | null
  total_duration_ms: number | null
}

export type StreamEvent =
  | { kind: 'session_started'; session_id: string }
  | { kind: 'text_delta'; text: string }
  | { kind: 'reasoning_delta'; text: string }
  | { kind: 'tool_use'; name: string; id: string }
  | {
      kind: 'tool_result'
      name: string
      id: string
      status: 'success' | 'error'
    }
  | { kind: 'thinking'; active: boolean }
  | { kind: 'error'; message: string }
  | { kind: 'done'; stop_reason: StopReason; usage: Usage }
  | { kind: 'cancelled' }
  // Multi-agent context pipeline events:
  | { kind: 'context_started' }
  | { kind: 'context_anchor'; text: string }
  | { kind: 'context_priority'; id: string; text: string; edited_by_user: boolean }
  | { kind: 'context_aside'; id: string; text: string; edited_by_user: boolean }
  | { kind: 'context_done' }
  | { kind: 'intent_classified'; intent: string; source: 'auto' | 'manual' }
