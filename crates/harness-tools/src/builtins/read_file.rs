use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};
use strands_core::types::tools::ToolSpec;
use strands_core::{StrandsError, Tool, ToolContext, ToolOutput};

const MAX_BYTES: usize = 64 * 1024;

/// Read a UTF-8 text file. Refuses any path that resolves outside the
/// configured sandbox root, and refuses any path entirely if no root
/// has been configured.
pub struct ReadFile {
    /// Canonicalised root directory; `None` disables the tool.
    pub sandbox_root: Option<PathBuf>,
}

impl ReadFile {
    pub fn new(sandbox_root: Option<PathBuf>) -> Self {
        let canonical = sandbox_root.and_then(|p| std::fs::canonicalize(&p).ok());
        Self {
            sandbox_root: canonical,
        }
    }

    fn resolve(&self, requested: &Path) -> Result<PathBuf, &'static str> {
        let root = match &self.sandbox_root {
            Some(r) => r,
            None => return Err("no sandbox root configured"),
        };
        let absolute = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            root.join(requested)
        };
        let canonical = std::fs::canonicalize(&absolute).map_err(|_| "path does not exist")?;
        if !canonical.starts_with(root) {
            return Err("path outside sandbox");
        }
        Ok(canonical)
    }
}

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".into(),
            description: match &self.sandbox_root {
                Some(r) => format!(
                    "Read a UTF-8 text file. Paths are resolved relative to the configured \
                     sandbox root ({}). Files larger than 64KB are truncated.",
                    r.display()
                ),
                None => "Read a UTF-8 text file. Disabled — no sandbox root configured in \
                         Settings; the tool will refuse every call."
                    .into(),
            },
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path inside the sandbox root."
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn invoke(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let raw = match input.get("path").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'path' string")),
        };
        let resolved = match self.resolve(Path::new(&raw)) {
            Ok(p) => p,
            Err(reason) => return Ok(ToolOutput::error(reason)),
        };
        let bytes = match std::fs::read(&resolved) {
            Ok(b) => b,
            Err(e) => return Ok(ToolOutput::error(format!("read failed: {e}"))),
        };
        let truncated = bytes.len() > MAX_BYTES;
        let slice = if truncated { &bytes[..MAX_BYTES] } else { &bytes[..] };
        let content = String::from_utf8_lossy(slice).to_string();
        Ok(ToolOutput::success(json!({
            "path": resolved.to_string_lossy(),
            "bytes": bytes.len(),
            "content": content,
            "truncated": truncated,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[tokio::test]
    async fn no_root_refuses() {
        let tool = ReadFile::new(None);
        let out = tool
            .invoke(json!({ "path": "/etc/passwd" }), &ToolContext::default())
            .await
            .unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn reads_file_inside_sandbox() {
        let dir = tempdir();
        let p = dir.path().join("hello.txt");
        let mut f = fs::File::create(&p).unwrap();
        writeln!(f, "world").unwrap();
        let tool = ReadFile::new(Some(dir.path().to_path_buf()));
        let out = tool
            .invoke(json!({ "path": "hello.txt" }), &ToolContext::default())
            .await
            .unwrap();
        assert!(!out.is_error);
        assert!(out.content["content"].as_str().unwrap().contains("world"));
    }

    #[tokio::test]
    async fn refuses_escape_outside_sandbox() {
        let dir = tempdir();
        let outer = dir.path().parent().unwrap().to_path_buf();
        // Try to escape the sandbox via ..
        let tool = ReadFile::new(Some(dir.path().to_path_buf()));
        let target = outer.join("etc.txt");
        let _ = fs::write(&target, "x");
        let out = tool
            .invoke(json!({ "path": "../etc.txt" }), &ToolContext::default())
            .await
            .unwrap();
        assert!(out.is_error);
    }

    #[tokio::test]
    async fn missing_path_is_error() {
        let tool = ReadFile::new(Some(std::env::temp_dir()));
        let out = tool.invoke(json!({}), &ToolContext::default()).await.unwrap();
        assert!(out.is_error);
    }
}
