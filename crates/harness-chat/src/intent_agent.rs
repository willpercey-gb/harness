//! Stage 2 of the multi-agent pipeline: classify the user's latest
//! message into one of `expand` / `revise` / `redirect` / `aside`.

use std::sync::Arc;

use futures::StreamExt;
use harness_storage::ConversationContext;
use strands_core::model::Model;
use strands_core::types::content::ContentBlock;
use strands_core::types::message::{Message, Role};
use strands_core::types::streaming::{DeltaContent, StreamEvent};
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::context::{parse_intent, xml_envelope, Intent};

const SYSTEM_PROMPT: &str = "Classify the user's latest message into EXACTLY ONE of:\n\
- expand: deepens or continues the existing goal.\n\
- revise: refines or corrects something within the goal.\n\
- redirect: changes the goal entirely.\n\
- aside: tangential question that does not change the goal.\n\
\n\
Output a single XML tag and nothing else: `<intent>expand</intent>` (or revise / redirect / aside).";

pub struct IntentRequest<'a> {
    pub model: Arc<dyn Model>,
    pub context: &'a ConversationContext,
    pub user_prompt: &'a str,
    pub cancel: CancellationToken,
}

fn render_user_message(req: &IntentRequest<'_>) -> String {
    let mut s = String::new();
    s.push_str("Current context:\n");
    s.push_str(&xml_envelope(req.context, None));
    s.push_str("\nUser's latest message:\n");
    s.push_str(req.user_prompt);
    s.push_str("\n\nClassify the intent. Return `<intent>...</intent>` only.");
    s
}

/// Run the intent classifier. Falls back to `Intent::Expand` on any
/// error or unparseable output — `Expand` is the safest no-op default
/// (the main agent treats it as "carry on").
pub async fn classify(req: IntentRequest<'_>) -> Intent {
    let user_msg = render_user_message(&req);
    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::Text { text: user_msg }],
    }];

    let stream_result = req.model.stream(&messages, Some(SYSTEM_PROMPT), &[]).await;
    let mut stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            warn!("intent agent: open stream: {e}");
            return Intent::Expand;
        }
    };

    let mut accumulated = String::new();
    loop {
        tokio::select! {
            biased;
            _ = req.cancel.cancelled() => return Intent::Expand,
            next = stream.next() => {
                match next {
                    Some(Ok(StreamEvent::ContentBlockDelta {
                        delta: DeltaContent::TextDelta(t), ..
                    })) => accumulated.push_str(&t),
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        warn!("intent agent: stream error: {e}");
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    parse_intent(&accumulated).unwrap_or_else(|| {
        // Tolerant fallback: scan raw for the words.
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
    })
}
