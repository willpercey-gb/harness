use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use strands_core::types::tools::ToolSpec;
use strands_core::{Tool, ToolContext, ToolOutput};
use strands_core::StrandsError;

/// Returns the current UTC and local time.
pub struct GetTime;

#[async_trait]
impl Tool for GetTime {
    fn name(&self) -> &str {
        "get_time"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "get_time".into(),
            description: "Return the current time as both UTC and local timezone.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn invoke(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let utc = Utc::now();
        let local = chrono::Local::now();
        Ok(ToolOutput::success(json!({
            "utc": utc.to_rfc3339(),
            "local": local.to_rfc3339(),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_utc_and_local() {
        let tool = GetTime;
        let out = tool
            .invoke(json!({}), &ToolContext::default())
            .await
            .unwrap();
        assert!(!out.is_error);
        let obj = out.content.as_object().expect("object");
        assert!(obj.contains_key("utc"));
        assert!(obj.contains_key("local"));
    }
}
