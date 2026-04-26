use super::events::{StreamEvent, ToolStatus};

/// Stateful classifier that splits an incoming raw text stream into typed
/// `StreamEvent`s. Recognises:
///
/// - `<response>...</response>`: stripped (markers ignored, content emits as `TextDelta`).
/// - `<reasoning>...</reasoning>` and `<thinking>...</thinking>`:
///    content emitted as `ReasoningDelta`.
/// - `<tool_use name="X" id="Y">` (paired close `</tool_use>` ignored):
///    emitted as `ToolUse { name, id }`.
/// - `<tool_result name="X" status="success" id="Y">`:
///    emitted as `ToolResult { name, status, id }`.
///
/// Tags can straddle delta boundaries — the unwrapper buffers a partial
/// tag until enough bytes have arrived to make a decision. Anything that
/// definitely isn't a recognised tag is flushed straight through as text.
pub struct XmlUnwrap {
    pending: String,
    /// Closing-tag literal we're waiting for when inside a reasoning block.
    inside_reasoning: Option<&'static str>,
}

impl XmlUnwrap {
    pub fn new() -> Self {
        Self {
            pending: String::new(),
            inside_reasoning: None,
        }
    }

    pub fn push(&mut self, chunk: &str, out: &mut Vec<StreamEvent>) {
        self.pending.push_str(chunk);
        loop {
            if let Some(closing) = self.inside_reasoning {
                if let Some(idx) = self.pending.find(closing) {
                    let body: String = self.pending.drain(..idx).collect();
                    if !body.is_empty() {
                        out.push(StreamEvent::ReasoningDelta { text: body });
                    }
                    self.pending.drain(..closing.len());
                    self.inside_reasoning = None;
                    continue;
                }
                let safe = safe_prefix_len(&self.pending, closing);
                if safe > 0 {
                    let to_emit: String = self.pending.drain(..safe).collect();
                    out.push(StreamEvent::ReasoningDelta { text: to_emit });
                }
                break;
            }

            match self.pending.find('<') {
                Some(0) => match try_parse_tag(&self.pending) {
                    TagParse::Recognised { event, consumed, enter_reasoning } => {
                        if let Some(e) = event {
                            out.push(e);
                        }
                        if let Some(close) = enter_reasoning {
                            self.inside_reasoning = Some(close);
                        }
                        self.pending.drain(..consumed);
                        continue;
                    }
                    TagParse::Unknown { consumed } => {
                        // Definitely not a recognised tag — emit as plain text.
                        let s: String = self.pending.drain(..consumed).collect();
                        out.push(StreamEvent::TextDelta { text: s });
                        continue;
                    }
                    TagParse::Incomplete => {
                        // Wait for more bytes.
                        break;
                    }
                },
                Some(idx) => {
                    let prefix: String = self.pending.drain(..idx).collect();
                    if !prefix.is_empty() {
                        out.push(StreamEvent::TextDelta { text: prefix });
                    }
                }
                None => {
                    if !self.pending.is_empty() {
                        let s = std::mem::take(&mut self.pending);
                        out.push(StreamEvent::TextDelta { text: s });
                    }
                    break;
                }
            }
        }
    }

    /// Flush remaining buffered text. Called once the upstream stream ends.
    pub fn flush(&mut self, out: &mut Vec<StreamEvent>) {
        if self.pending.is_empty() {
            return;
        }
        let s = std::mem::take(&mut self.pending);
        if self.inside_reasoning.is_some() {
            out.push(StreamEvent::ReasoningDelta { text: s });
        } else {
            out.push(StreamEvent::TextDelta { text: s });
        }
        self.inside_reasoning = None;
    }
}

impl Default for XmlUnwrap {
    fn default() -> Self {
        Self::new()
    }
}

/// Number of bytes from the start of `s` that definitely cannot be the
/// beginning of `needle`. Used to decide how much can be flushed when
/// hunting for a closing tag.
fn safe_prefix_len(s: &str, needle: &str) -> usize {
    let max_overlap = needle.len().min(s.len());
    for k in (1..=max_overlap).rev() {
        if s.is_char_boundary(s.len() - k) && s.ends_with(&needle[..k]) {
            return s.len() - k;
        }
    }
    s.len()
}

enum TagParse {
    Recognised {
        event: Option<StreamEvent>,
        enter_reasoning: Option<&'static str>,
        consumed: usize,
    },
    Unknown {
        consumed: usize,
    },
    Incomplete,
}

fn try_parse_tag(s: &str) -> TagParse {
    debug_assert!(s.starts_with('<'));
    let Some(close_idx) = s.find('>') else {
        return TagParse::Incomplete;
    };
    let raw = &s[..=close_idx];
    let consumed = close_idx + 1;

    if raw == "<response>" || raw == "</response>" {
        return TagParse::Recognised {
            event: None,
            enter_reasoning: None,
            consumed,
        };
    }

    if raw == "<reasoning>" {
        return TagParse::Recognised {
            event: None,
            enter_reasoning: Some("</reasoning>"),
            consumed,
        };
    }
    if raw == "<thinking>" {
        return TagParse::Recognised {
            event: None,
            enter_reasoning: Some("</thinking>"),
            consumed,
        };
    }

    if raw.starts_with("<tool_use") && !raw.starts_with("</tool_use") {
        let body = &raw[..raw.len() - 1]; // strip trailing '>'
        let name = attr_value(body, "name").unwrap_or_default();
        let id = attr_value(body, "id").unwrap_or_default();
        return TagParse::Recognised {
            event: Some(StreamEvent::ToolUse { name, id }),
            enter_reasoning: None,
            consumed,
        };
    }
    if raw == "</tool_use>" {
        return TagParse::Recognised {
            event: None,
            enter_reasoning: None,
            consumed,
        };
    }

    if raw.starts_with("<tool_result") && !raw.starts_with("</tool_result") {
        let body = &raw[..raw.len() - 1];
        let name = attr_value(body, "name").unwrap_or_default();
        let id = attr_value(body, "id").unwrap_or_default();
        let status = match attr_value(body, "status").as_deref() {
            Some("success") => ToolStatus::Success,
            _ => ToolStatus::Error,
        };
        return TagParse::Recognised {
            event: Some(StreamEvent::ToolResult { name, status, id }),
            enter_reasoning: None,
            consumed,
        };
    }
    if raw == "</tool_result>" {
        return TagParse::Recognised {
            event: None,
            enter_reasoning: None,
            consumed,
        };
    }

    TagParse::Unknown { consumed }
}

fn attr_value(s: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = s.find(&needle)? + needle.len();
    let end_rel = s[start..].find('"')?;
    Some(s[start..start + end_rel].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unwrap_all(input: &[&str]) -> Vec<StreamEvent> {
        let mut u = XmlUnwrap::new();
        let mut out = Vec::new();
        for chunk in input {
            u.push(chunk, &mut out);
        }
        u.flush(&mut out);
        out
    }

    #[track_caller]
    fn texts(events: &[StreamEvent]) -> Vec<String> {
        events
            .iter()
            .map(|e| match e {
                StreamEvent::TextDelta { text } => format!("text:{text}"),
                StreamEvent::ReasoningDelta { text } => format!("reason:{text}"),
                StreamEvent::ToolUse { name, .. } => format!("tool_use:{name}"),
                StreamEvent::ToolResult { name, status, .. } => {
                    format!("tool_result:{name}:{status:?}")
                }
                _ => "<other>".into(),
            })
            .collect()
    }

    #[test]
    fn plain_text_passes_through() {
        let evts = unwrap_all(&["hello ", "world"]);
        assert_eq!(texts(&evts), vec!["text:hello ", "text:world"]);
    }

    #[test]
    fn response_wrappers_are_stripped() {
        let evts = unwrap_all(&["<response>hi</response>"]);
        assert_eq!(texts(&evts), vec!["text:hi"]);
    }

    #[test]
    fn reasoning_extracted_intact() {
        let evts = unwrap_all(&["before <reasoning>thinking out loud</reasoning> after"]);
        assert_eq!(
            texts(&evts),
            vec![
                "text:before ",
                "reason:thinking out loud",
                "text: after",
            ]
        );
    }

    #[test]
    fn reasoning_split_across_chunks() {
        let evts = unwrap_all(&["<reas", "oning>part one ", "part two</reaso", "ning>tail"]);
        // Body is split into deltas as it arrives; tail emits afterwards.
        let mut reason = String::new();
        let mut tail = String::new();
        for e in &evts {
            match e {
                StreamEvent::ReasoningDelta { text } => reason.push_str(text),
                StreamEvent::TextDelta { text } => tail.push_str(text),
                _ => {}
            }
        }
        assert_eq!(reason, "part one part two");
        assert_eq!(tail, "tail");
    }

    #[test]
    fn thinking_alias_works() {
        let evts = unwrap_all(&["<thinking>hmm</thinking>ok"]);
        assert_eq!(texts(&evts), vec!["reason:hmm", "text:ok"]);
    }

    #[test]
    fn tool_use_emitted() {
        let evts = unwrap_all(&[r#"calling: <tool_use name="get_time" id="t1"></tool_use> done"#]);
        let collected = texts(&evts);
        assert!(collected.contains(&"tool_use:get_time".to_string()));
        assert_eq!(collected.first().unwrap(), "text:calling: ");
        assert_eq!(collected.last().unwrap(), "text: done");
    }

    #[test]
    fn tool_result_success_and_error() {
        let evts = unwrap_all(&[
            r#"<tool_result name="get_time" status="success" id="t1"></tool_result>"#,
            r#"<tool_result name="http_fetch" status="error" id="h1"></tool_result>"#,
        ]);
        let collected = texts(&evts);
        assert_eq!(
            collected,
            vec![
                "tool_result:get_time:Success",
                "tool_result:http_fetch:Error",
            ]
        );
    }

    #[test]
    fn unknown_tag_passes_through() {
        let evts = unwrap_all(&["<unknown>x</unknown>"]);
        // Unknown tags + their bodies emit as plain text (the literal angle
        // brackets are preserved so the user sees what the model produced).
        let joined: String = evts
            .iter()
            .map(|e| match e {
                StreamEvent::TextDelta { text } => text.as_str(),
                _ => "",
            })
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(joined, "<unknown>x</unknown>");
    }

    #[test]
    fn safe_prefix_holds_back_partial_close_tag() {
        let mut u = XmlUnwrap::new();
        let mut out = Vec::new();
        u.push("<reasoning>abc</reaso", &mut out);
        // The partial "</reaso" should NOT have been emitted yet.
        let so_far: String = out
            .iter()
            .map(|e| match e {
                StreamEvent::ReasoningDelta { text } => text.clone(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(so_far, "abc");
        u.push("ning>done", &mut out);
        u.flush(&mut out);
        let reason: String = out
            .iter()
            .map(|e| match e {
                StreamEvent::ReasoningDelta { text } => text.clone(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("");
        let tail: String = out
            .iter()
            .map(|e| match e {
                StreamEvent::TextDelta { text } => text.clone(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("");
        assert_eq!(reason, "abc");
        assert_eq!(tail, "done");
    }
}
