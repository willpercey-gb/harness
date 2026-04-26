//! Stage 1 of the multi-agent pipeline: extract / refine the
//! `ConversationContext` (anchor + priorities + asides) for the session.

use std::sync::Arc;

use futures::StreamExt;
use harness_storage::{ChatMessage, ConversationContext};
use strands_core::model::Model;
use strands_core::types::content::ContentBlock;
use strands_core::types::message::{Message, Role};
use strands_core::types::streaming::{DeltaContent, StreamEvent};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::context::{parse_envelope, xml_envelope};

const SYSTEM_PROMPT: &str = "You maintain a session-level context block for an LLM chat. \
Read the prior context (if any), the recent conversation, and the new user message; \
then output an UPDATED `<context>...</context>` block.\n\
\n\
The block contains exactly one <anchor> describing the user's overarching goal, plus \
zero or more <priority> tags for constraints / decisions that guide the goal, plus \
zero or more <aside> tags for tangential notes that should NOT redirect the goal.\n\
\n\
Rules:\n\
- Preserve any tag marked edited=\"true\" verbatim — the user wrote it themselves.\n\
- Use stable id attributes for priorities and asides (carry ids forward when re-emitting them).\n\
- Reword the anchor only if the user's recent message clearly changes the goal.\n\
- Output the <context>...</context> XML and NOTHING ELSE — no prose, no commentary, no code fences.";

pub struct AnchorRequest<'a> {
    pub model: Arc<dyn Model>,
    pub prior: &'a ConversationContext,
    pub history: &'a [ChatMessage],
    pub user_prompt: &'a str,
    pub cancel: CancellationToken,
}

/// Build the user-message body for the anchor agent. The model sees:
///   1. Existing context (or "<context></context>" when empty),
///   2. Recent conversation history as `User: ...` / `Assistant: ...`,
///   3. The new user prompt being sent this turn,
///   4. A reminder of the output contract.
fn render_user_message(req: &AnchorRequest<'_>) -> String {
    let mut s = String::new();
    s.push_str("Existing context:\n");
    s.push_str(&xml_envelope(req.prior, None));
    s.push_str("\nRecent conversation:\n");
    for m in req.history {
        let label = match m.role.as_str() {
            "assistant" => "Assistant",
            "system" => "System",
            _ => "User",
        };
        s.push_str(label);
        s.push_str(": ");
        s.push_str(&m.content);
        s.push('\n');
    }
    s.push_str("\nNew user message:\n");
    s.push_str(req.user_prompt);
    s.push_str("\n\nReturn the updated <context>...</context> block.");
    s
}

/// Run the anchor agent. Returns the parsed `ConversationContext`. On
/// any failure (model error, parse failure, cancel), falls back to the
/// prior context unchanged so the rest of the pipeline can keep going.
pub async fn extract_or_refine(req: AnchorRequest<'_>) -> ConversationContext {
    let user_msg = render_user_message(&req);
    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::Text { text: user_msg }],
    }];

    let stream_result = req.model.stream(&messages, Some(SYSTEM_PROMPT), &[]).await;
    let mut stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            warn!("anchor agent: open stream: {e}");
            return req.prior.clone();
        }
    };

    let mut accumulated = String::new();
    loop {
        tokio::select! {
            biased;
            _ = req.cancel.cancelled() => {
                return req.prior.clone();
            }
            next = stream.next() => {
                match next {
                    Some(Ok(StreamEvent::ContentBlockDelta {
                        delta: DeltaContent::TextDelta(t), ..
                    })) => accumulated.push_str(&t),
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        warn!("anchor agent: stream error: {e}");
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    let parsed = parse_envelope(&accumulated);
    if parsed.is_empty() && !req.prior.is_empty() {
        // Model produced nothing parseable; preserve prior rather than
        // wiping the user's existing cards.
        return req.prior.clone();
    }
    parsed
}
