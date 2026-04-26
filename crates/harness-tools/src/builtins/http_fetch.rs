use async_trait::async_trait;
use serde_json::{json, Value};
use strands_core::types::tools::ToolSpec;
use strands_core::{StrandsError, Tool, ToolContext, ToolOutput};

const MAX_BODY_BYTES: usize = 64 * 1024;

/// Fetch a URL via HTTP GET. Refuses any host outside the configured
/// allowlist (so the user must opt in per-host before the model can
/// reach the open internet through this tool).
pub struct HttpFetch {
    /// Host strings, lowercased and stripped of scheme. Empty disables
    /// the tool effectively (every call refuses).
    pub allowlist: Vec<String>,
}

impl HttpFetch {
    pub fn new(allowlist: Vec<String>) -> Self {
        let allowlist = allowlist
            .into_iter()
            .map(|s| {
                let lowered = s.trim().to_lowercase();
                lowered
                    .trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();
        Self { allowlist }
    }

    fn allowed(&self, host: &str) -> bool {
        let h = host.trim().to_lowercase();
        self.allowlist.iter().any(|a| *a == h)
    }
}

#[async_trait]
impl Tool for HttpFetch {
    fn name(&self) -> &str {
        "http_fetch"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "http_fetch".into(),
            description: format!(
                "HTTP GET a URL and return the response body as text. Only hosts on the user's \
                 allowlist are reachable; the current allowlist is {}.",
                if self.allowlist.is_empty() {
                    "empty (the tool will refuse every URL until the user adds hosts in Settings)".to_string()
                } else {
                    self.allowlist.join(", ")
                }
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The full URL to fetch (must include scheme)."
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn invoke(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let url_str = match input.get("url").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'url' string")),
        };
        let parsed = match reqwest::Url::parse(&url_str) {
            Ok(u) => u,
            Err(e) => return Ok(ToolOutput::error(format!("invalid url: {e}"))),
        };
        let host = match parsed.host_str() {
            Some(h) => h.to_string(),
            None => return Ok(ToolOutput::error("url has no host")),
        };
        if !self.allowed(&host) {
            return Ok(ToolOutput::error(format!("host not allowed: {host}")));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| StrandsError::Other(format!("http client: {e}")))?;
        let resp = match client.get(parsed).send().await {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutput::error(format!("request failed: {e}"))),
        };
        let status = resp.status();
        let bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => return Ok(ToolOutput::error(format!("read body: {e}"))),
        };
        let truncated = bytes.len() > MAX_BODY_BYTES;
        let slice = if truncated { &bytes[..MAX_BODY_BYTES] } else { &bytes[..] };
        let body = String::from_utf8_lossy(slice).to_string();
        Ok(ToolOutput::success(json!({
            "status": status.as_u16(),
            "body": body,
            "truncated": truncated,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_allowlist_refuses() {
        let tool = HttpFetch::new(vec![]);
        let out = tool
            .invoke(json!({ "url": "https://example.com" }), &ToolContext::default())
            .await
            .unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn unallowed_host_refused() {
        let tool = HttpFetch::new(vec!["example.com".into()]);
        let out = tool
            .invoke(json!({ "url": "https://evil.com" }), &ToolContext::default())
            .await
            .unwrap();
        assert!(out.is_error);
        let msg = out.content.as_str().unwrap_or_default().to_string();
        assert!(msg.contains("not allowed"));
    }

    #[tokio::test]
    async fn allowlist_strips_scheme_and_lowercases() {
        let tool = HttpFetch::new(vec!["HTTPS://Example.com".into()]);
        assert!(tool.allowed("example.com"));
        assert!(tool.allowed("EXAMPLE.com"));
        assert!(!tool.allowed("api.example.com"));
    }

    #[tokio::test]
    async fn missing_url_is_error() {
        let tool = HttpFetch::new(vec!["example.com".into()]);
        let out = tool.invoke(json!({}), &ToolContext::default()).await.unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn invalid_url_is_error() {
        let tool = HttpFetch::new(vec!["example.com".into()]);
        let out = tool
            .invoke(json!({ "url": "not a url" }), &ToolContext::default())
            .await
            .unwrap();
        assert!(out.is_error);
    }
}
