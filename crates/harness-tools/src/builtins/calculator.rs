use async_trait::async_trait;
use serde_json::{json, Value};
use strands_core::types::tools::ToolSpec;
use strands_core::{StrandsError, Tool, ToolContext, ToolOutput};

/// Evaluates a mathematical expression like "2 + 2 * 5" using `evalexpr`.
pub struct Calculator;

#[async_trait]
impl Tool for Calculator {
    fn name(&self) -> &str {
        "calculator"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "calculator".into(),
            description:
                "Evaluate a mathematical expression. Supports +, -, *, /, %, ^, parentheses, and \
                 functions like sqrt, sin, cos, log."
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The expression to evaluate, e.g. '2 + 2' or 'sqrt(144)'."
                    }
                },
                "required": ["expression"]
            }),
        }
    }

    async fn invoke(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let expr = match input.get("expression").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'expression' string")),
        };
        match evalexpr::eval(&expr) {
            Ok(value) => Ok(ToolOutput::success(json!({
                "expression": expr,
                "result": value.to_string(),
            }))),
            Err(e) => Ok(ToolOutput::error(format!("evaluation error: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn evaluates_arithmetic() {
        let out = Calculator
            .invoke(json!({ "expression": "2 + 2 * 5" }), &ToolContext::default())
            .await
            .unwrap();
        assert!(!out.is_error);
        assert_eq!(out.content["result"], "12");
    }

    #[tokio::test]
    async fn missing_expression_is_error() {
        let out = Calculator.invoke(json!({}), &ToolContext::default()).await.unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn invalid_expression_is_error() {
        let out = Calculator
            .invoke(json!({ "expression": "2 + " }), &ToolContext::default())
            .await
            .unwrap();
        assert!(out.is_error);
    }
}
