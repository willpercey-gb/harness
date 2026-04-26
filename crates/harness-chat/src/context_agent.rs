//! The single small agent that owns the multi-agent context window.
//! One model call emits both the refreshed `ConversationContext`
//! (anchor + priorities + asides) AND the intent classification for
//! the user's latest message — see [`refresh`] for the contract.

use std::sync::Arc;

use futures::StreamExt;
use harness_storage::{ChatMessage, ConversationContext};
use strands_core::model::Model;
use strands_core::types::content::ContentBlock;
use strands_core::types::message::{Message, Role};
use strands_core::types::streaming::{DeltaContent, StreamEvent};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::context::{parse_envelope, parse_intent, xml_envelope, Intent};

const SYSTEM_PROMPT: &str = "You maintain a session-level context block AND classify the user's latest message in a single response.\n\
\n\
Output two XML elements in order, and NOTHING ELSE — no prose, no commentary, no code fences:\n\
\n\
1. A <context>...</context> block containing exactly one <anchor> describing the user's overarching \
goal, plus zero or more <priority> tags for constraints / decisions that guide the goal, plus zero \
or more <aside> tags for tangential notes that should NOT redirect the goal.\n\
\n\
2. A single <intent>...</intent> tag whose value is one of:\n\
   - expand: the user's latest message deepens or continues the existing goal.\n\
   - revise: the user is refining or correcting within the same goal.\n\
   - redirect: the user is changing the goal entirely.\n\
   - aside: the user asked something tangential that does NOT change the goal.\n\
\n\
Rules:\n\
- Preserve any <priority> or <aside> marked edited=\"true\" verbatim — the user wrote it themselves.\n\
- Carry id attributes forward when re-emitting existing priorities / asides.\n\
- Reword the <anchor> ONLY if the user's recent message clearly changes the goal.\n";

pub struct ContextRequest<'a> {
    pub model: Arc<dyn Model>,
    pub prior: &'a ConversationContext,
    pub history: &'a [ChatMessage],
    pub user_prompt: &'a str,
    pub cancel: CancellationToken,
}

pub struct ContextOutcome {
    pub context: ConversationContext,
    pub intent: Intent,
}

fn render_user_message(req: &ContextRequest<'_>) -> String {
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
    s.push_str(
        "\n\nReturn the updated <context>...</context> block followed by a single \
         <intent>...</intent> tag.",
    );
    s
}

/// Run the combined context+intent agent for a single turn.
///
/// On any failure (model error, parse failure, cancel), falls back to
/// the prior context unchanged + `Intent::Expand` so the caller can keep
/// the chat moving without dropping the message.
pub async fn refresh(req: ContextRequest<'_>) -> ContextOutcome {
    let user_msg = render_user_message(&req);
    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::Text { text: user_msg }],
    }];

    let stream_result = req.model.stream(&messages, Some(SYSTEM_PROMPT), &[]).await;
    let mut stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            warn!("context agent: open stream: {e}");
            return ContextOutcome {
                context: req.prior.clone(),
                intent: Intent::Expand,
            };
        }
    };

    let mut accumulated = String::new();
    loop {
        tokio::select! {
            biased;
            _ = req.cancel.cancelled() => {
                return ContextOutcome {
                    context: req.prior.clone(),
                    intent: Intent::Expand,
                };
            }
            next = stream.next() => {
                match next {
                    Some(Ok(StreamEvent::ContentBlockDelta {
                        delta: DeltaContent::TextDelta(t), ..
                    })) => accumulated.push_str(&t),
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        warn!("context agent: stream error: {e}");
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    let parsed_ctx = parse_envelope(&accumulated);
    let context = if parsed_ctx.is_empty() && !req.prior.is_empty() {
        // Model produced nothing parseable; preserve the prior cards
        // rather than wiping them.
        req.prior.clone()
    } else {
        parsed_ctx
    };

    let intent = parse_intent(&accumulated).unwrap_or_else(|| {
        // Tolerant fallback: scan the raw output for the words.
        let lower = accumulated.to_lowercase();
        if lower.contains("redirect") {
            Intent::Redirect
        } else if lower.contains("revise") {
            Intent::Revise
        } else if lower.contains("aside") {
            Intent::Aside
        } else {
            Intent::Expand
        }
    });

    ContextOutcome { context, intent }
}
