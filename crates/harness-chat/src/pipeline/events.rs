use serde::Serialize;

/// Frontend-facing stream event. Serialised to the Tauri Channel as JSON
/// with a `kind` discriminator for ergonomic TS unions.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StreamEvent {
    /// First event after a new session is created. Sent before any token
    /// deltas so the frontend can capture the id without parsing headers.
    SessionStarted { session_id: String },

    /// Plain assistant text. Coalesced upstream (~16ms ticks) to avoid
    /// flooding the IPC channel.
    TextDelta { text: String },

    /// Reasoning text (from <reasoning> or <thinking> blocks emitted by
    /// thinking-mode models like DeepSeek-R1, Qwen-3-thinking).
    ReasoningDelta { text: String },

    /// A tool call has started. Frontend renders a chip with the tool name.
    ToolUse { name: String, id: String },

    /// A tool call has completed. Frontend renders the success/error pill.
    ToolResult {
        name: String,
        status: ToolStatus,
        id: String,
    },

    /// True when no token has arrived for >1s mid-stream.
    Thinking { active: bool },

    /// Mid-stream error, followed by a `Done` event with `Error` stop reason.
    Error { message: String },

    /// Terminal event for a successful or filtered run.
    Done { stop_reason: StopReason, usage: Usage },

    /// Terminal event when the user cancelled mid-stream.
    Cancelled,

    // ---- Multi-agent context pipeline events ----
    /// Stage 1 (anchor agent) is running.
    ContextStarted,
    /// Final anchor extracted by the anchor agent.
    ContextAnchor { text: String },
    /// One priority card. Multiple of these may arrive per turn.
    ContextPriority {
        id: String,
        text: String,
        edited_by_user: bool,
    },
    /// One aside card.
    ContextAside {
        id: String,
        text: String,
        edited_by_user: bool,
    },
    /// Stage 1 finished; the cards above are the full new state.
    ContextDone,
    /// Stage 2: intent classified (or pulled from manual override).
    IntentClassified {
        intent: String,
        source: String, // "auto" | "manual"
    },
    /// A background task generated a summary title for the session.
    /// Emitted asynchronously; may arrive after `Done`.
    SessionTitled {
        session_id: String,
        title: String,
    },

    // ---- Stage 4: passive memory extractor (fires after `Done`) ----
    /// The detached extractor task has started running for this turn.
    MemoryExtractionStarted { session_id: String },
    /// One entity processed by the extractor — either matched against an
    /// existing graph node or created fresh.
    EntityResolved {
        name: String,
        entity_type: String,
        status: EntityResolutionStatus,
    },
    /// One typed relationship written to the graph.
    RelationshipCreated {
        from_name: String,
        to_name: String,
        relation: String,
    },
    /// One memory chunk persisted to the vector store.
    MemoryStored { content_preview: String },
    /// Extractor finished; counts summarise what was written.
    MemoryExtractionDone {
        entities: u32,
        relationships: u32,
        memories: u32,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityResolutionStatus {
    Matched,
    Created,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Success,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_duration_ms: Option<u64>,
}

impl From<strands_core::Usage> for Usage {
    fn from(u: strands_core::Usage) -> Self {
        Self {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            total_duration_ms: u.total_duration_ns.map(|ns| ns / 1_000_000),
        }
    }
}

impl From<strands_core::StopReason> for StopReason {
    fn from(s: strands_core::StopReason) -> Self {
        match s {
            strands_core::StopReason::EndTurn => Self::EndTurn,
            strands_core::StopReason::ToolUse => Self::ToolUse,
            strands_core::StopReason::MaxTokens => Self::MaxTokens,
            strands_core::StopReason::Cancelled => Self::Cancelled,
            strands_core::StopReason::ContentFiltered
            | strands_core::StopReason::GuardrailIntervention => Self::Error,
        }
    }
}
