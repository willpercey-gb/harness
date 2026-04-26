use super::events::StreamEvent;

/// Batches consecutive `TextDelta` events together, and consecutive
/// `ReasoningDelta` events together, so that high-frequency token bursts
/// (one event per token from Ollama) collapse into one IPC message per
/// frame. Other event variants pass through untouched.
///
/// The coalescer is stateless across calls — feed it one batch at a time.
pub fn coalesce_batch(events: Vec<StreamEvent>) -> Vec<StreamEvent> {
    let mut out: Vec<StreamEvent> = Vec::with_capacity(events.len());
    for e in events {
        match (&e, out.last_mut()) {
            (
                StreamEvent::TextDelta { text },
                Some(StreamEvent::TextDelta { text: prev }),
            ) => {
                prev.push_str(text);
            }
            (
                StreamEvent::ReasoningDelta { text },
                Some(StreamEvent::ReasoningDelta { text: prev }),
            ) => {
                prev.push_str(text);
            }
            _ => out.push(e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::events::{StopReason, ToolStatus, Usage};
    use super::*;

    fn td(s: &str) -> StreamEvent {
        StreamEvent::TextDelta { text: s.into() }
    }
    fn rd(s: &str) -> StreamEvent {
        StreamEvent::ReasoningDelta { text: s.into() }
    }

    #[test]
    fn merges_consecutive_text_deltas() {
        let merged = coalesce_batch(vec![td("a"), td("bc"), td("d")]);
        assert_eq!(merged.len(), 1);
        if let StreamEvent::TextDelta { text } = &merged[0] {
            assert_eq!(text, "abcd");
        } else {
            panic!("expected TextDelta");
        }
    }

    #[test]
    fn merges_consecutive_reasoning_deltas() {
        let merged = coalesce_batch(vec![rd("a"), rd("b")]);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn does_not_merge_across_other_events() {
        let merged = coalesce_batch(vec![
            td("hello "),
            StreamEvent::ToolUse {
                name: "x".into(),
                id: "1".into(),
            },
            td("world"),
        ]);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn does_not_merge_text_with_reasoning() {
        let merged = coalesce_batch(vec![td("a"), rd("b"), td("c")]);
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn passes_through_terminal_events() {
        let merged = coalesce_batch(vec![
            td("hi"),
            StreamEvent::Done {
                stop_reason: StopReason::EndTurn,
                usage: Usage::default(),
            },
        ]);
        assert_eq!(merged.len(), 2);
        // suppress unused
        let _ = ToolStatus::Success;
    }
}
