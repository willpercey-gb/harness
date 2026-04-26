//! Shared context types for the multi-agent context window.

use harness_storage::{ContextCard, ConversationContext};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    /// Deepens or continues the existing goal.
    Expand,
    /// Refines or corrects within the goal.
    Revise,
    /// Changes the goal.
    Redirect,
    /// Tangential question that does not change the goal.
    Aside,
}

impl Intent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Intent::Expand => "expand",
            Intent::Revise => "revise",
            Intent::Redirect => "redirect",
            Intent::Aside => "aside",
        }
    }

    pub fn from_xml_text(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "expand" => Some(Intent::Expand),
            "revise" => Some(Intent::Revise),
            "redirect" => Some(Intent::Redirect),
            "aside" => Some(Intent::Aside),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntentSource {
    Auto,
    Manual,
}

/// Render a `ConversationContext` and `Intent` as the system-prompt
/// envelope that goes into the main agent. Same shape consumed by the
/// anchor agent's input prompt — round-trips through [`parse_envelope`].
pub fn xml_envelope(ctx: &ConversationContext, intent: Option<(Intent, IntentSource)>) -> String {
    let mut out = String::new();
    out.push_str("<context>\n");
    if let Some(anchor) = &ctx.anchor {
        out.push_str(&format!(
            "  <anchor>{}</anchor>\n",
            xml_escape(anchor)
        ));
    }
    for p in &ctx.priorities {
        out.push_str(&format!(
            "  <priority id=\"{}\"{}>{}</priority>\n",
            p.id,
            if p.edited_by_user { " edited=\"true\"" } else { "" },
            xml_escape(&p.text)
        ));
    }
    for a in &ctx.asides {
        out.push_str(&format!(
            "  <aside id=\"{}\"{}>{}</aside>\n",
            a.id,
            if a.edited_by_user { " edited=\"true\"" } else { "" },
            xml_escape(&a.text)
        ));
    }
    out.push_str("</context>\n");
    if let Some((intent, source)) = intent {
        out.push_str(&format!(
            "<intent source=\"{}\">{}</intent>\n",
            source_str(source),
            intent.as_str()
        ));
    }
    out
}

fn source_str(s: IntentSource) -> &'static str {
    match s {
        IntentSource::Auto => "auto",
        IntentSource::Manual => "manual",
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Tolerant parser for the XML envelope produced by the anchor agent.
/// Extracts the anchor string and any `<priority>` / `<aside>` tags into
/// a `ConversationContext`. Unknown tags and stray prose are ignored.
pub fn parse_envelope(text: &str) -> ConversationContext {
    let mut ctx = ConversationContext::default();

    if let Some(a) = extract_inner(text, "anchor") {
        let cleaned = a.trim();
        if !cleaned.is_empty() {
            ctx.anchor = Some(xml_unescape(cleaned));
        }
    }

    for (attrs, body) in extract_all(text, "priority") {
        let id = attr(&attrs, "id").unwrap_or_else(uuid_str);
        let edited = attr(&attrs, "edited").as_deref() == Some("true");
        let body = body.trim();
        if !body.is_empty() {
            ctx.priorities.push(ContextCard {
                id,
                text: xml_unescape(body),
                edited_by_user: edited,
            });
        }
    }

    for (attrs, body) in extract_all(text, "aside") {
        let id = attr(&attrs, "id").unwrap_or_else(uuid_str);
        let edited = attr(&attrs, "edited").as_deref() == Some("true");
        let body = body.trim();
        if !body.is_empty() {
            ctx.asides.push(ContextCard {
                id,
                text: xml_unescape(body),
                edited_by_user: edited,
            });
        }
    }

    ctx
}

fn uuid_str() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Extract the body of the FIRST occurrence of `<tag …>…</tag>`.
fn extract_inner(haystack: &str, tag: &str) -> Option<String> {
    extract_all(haystack, tag).into_iter().next().map(|(_, body)| body)
}

/// Extract every `<tag attrs>body</tag>` pair from the haystack. Returns
/// `(attrs, body)` per occurrence. Self-closing `<tag/>` not supported —
/// we only use paired forms.
fn extract_all(haystack: &str, tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let open_prefix = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut cursor = 0;
    while let Some(start) = haystack[cursor..].find(&open_prefix) {
        let abs_start = cursor + start;
        let after_open = &haystack[abs_start + open_prefix.len()..];
        // attrs run from after the tag name up to the first '>'
        let Some(gt) = after_open.find('>') else {
            break;
        };
        let attrs = after_open[..gt].trim().trim_end_matches('/').trim().to_string();
        let body_start = abs_start + open_prefix.len() + gt + 1;
        let Some(end_rel) = haystack[body_start..].find(&close) else {
            break;
        };
        let body = haystack[body_start..body_start + end_rel].to_string();
        out.push((attrs, body));
        cursor = body_start + end_rel + close.len();
    }
    out
}

fn attr(attrs: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = attrs.find(&needle)? + needle.len();
    let end_rel = attrs[start..].find('"')?;
    Some(attrs[start..start + end_rel].to_string())
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Extract the inner body of `<intent>…</intent>`, normalised.
pub fn parse_intent(text: &str) -> Option<Intent> {
    extract_inner(text, "intent").and_then(|body| Intent::from_xml_text(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_renders_full_context() {
        let ctx = ConversationContext {
            anchor: Some("plan a 4-day Lisbon trip".into()),
            priorities: vec![
                ContextCard::new("4-day duration"),
                ContextCard::edited("budget under £500"),
            ],
            asides: vec![ContextCard::new("note: GMT+0")],
            updated_at: None,
        };
        let s = xml_envelope(&ctx, Some((Intent::Expand, IntentSource::Auto)));
        assert!(s.contains("<anchor>plan a 4-day Lisbon trip</anchor>"));
        assert!(s.contains("<priority id=\""));
        assert!(s.contains("edited=\"true\""));
        assert!(s.contains("<intent source=\"auto\">expand</intent>"));
    }

    #[test]
    fn envelope_round_trips() {
        let ctx = ConversationContext {
            anchor: Some("ship harness".into()),
            priorities: vec![ContextCard::new("ollama support"), ContextCard::edited("low latency")],
            asides: vec![ContextCard::new("dark mode default")],
            updated_at: None,
        };
        let s = xml_envelope(&ctx, None);
        let parsed = parse_envelope(&s);
        assert_eq!(parsed.anchor, ctx.anchor);
        assert_eq!(parsed.priorities.len(), 2);
        assert_eq!(parsed.priorities[1].text, "low latency");
        assert!(parsed.priorities[1].edited_by_user);
        assert_eq!(parsed.asides.len(), 1);
    }

    #[test]
    fn parse_handles_loose_input() {
        // Models often emit prose around the XML; we should still pick out the tags.
        let raw = r#"
Sure, here is the updated context:
<context>
  <anchor>fix the import bug</anchor>
  <priority>regression test added</priority>
</context>
That's all I have."#;
        let ctx = parse_envelope(raw);
        assert_eq!(ctx.anchor.as_deref(), Some("fix the import bug"));
        assert_eq!(ctx.priorities.len(), 1);
        assert_eq!(ctx.priorities[0].text, "regression test added");
    }

    #[test]
    fn intent_parsing_handles_each_value() {
        for s in &["expand", "REVISE", "  redirect  ", "aside"] {
            let xml = format!("<intent>{s}</intent>");
            assert!(parse_intent(&xml).is_some(), "failed on {s}");
        }
        assert!(parse_intent("<intent>nonsense</intent>").is_none());
        assert!(parse_intent("no tags here").is_none());
    }

    #[test]
    fn xml_escape_handles_specials() {
        let ctx = ConversationContext {
            anchor: Some("compare a < b & a > c".into()),
            ..Default::default()
        };
        let s = xml_envelope(&ctx, None);
        assert!(s.contains("compare a &lt; b &amp; a &gt; c"));
        let back = parse_envelope(&s);
        assert_eq!(back.anchor.as_deref(), Some("compare a < b & a > c"));
    }
}
