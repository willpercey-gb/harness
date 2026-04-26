use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::db::HarnessDb;
use crate::error::Result;

const SINGLETON_ID: &str = "singleton";

fn default_ollama_host() -> String {
    "http://localhost:11434".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub openrouter_api_key: Option<String>,
    #[serde(default)]
    pub openrouter_referrer: Option<String>,
    #[serde(default)]
    pub openrouter_app_title: Option<String>,
    #[serde(default = "default_ollama_host")]
    pub ollama_host: String,
    #[serde(default)]
    pub default_agent_id: Option<String>,
    #[serde(default)]
    pub http_fetch_allowlist: Vec<String>,
    #[serde(default)]
    pub read_file_sandbox_root: Option<PathBuf>,
    /// Absolute path to the `claude` binary. When `None`, AppState
    /// runs a startup discovery pass over common install locations
    /// and a login-shell `command -v` fallback. Overridable via
    /// Settings UI in a future phase.
    #[serde(default)]
    pub claude_cli_path: Option<PathBuf>,
    /// Override path for the Memex graph + vector store. When `None`,
    /// defaults to `~/.harness/memex-db`.
    #[serde(default)]
    pub memex_db_path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            openrouter_api_key: None,
            openrouter_referrer: None,
            openrouter_app_title: None,
            ollama_host: default_ollama_host(),
            default_agent_id: None,
            http_fetch_allowlist: Vec::new(),
            read_file_sandbox_root: None,
            claude_cli_path: None,
            memex_db_path: None,
        }
    }
}

impl Settings {
    /// True iff OpenRouter agents should be enabled.
    pub fn openrouter_enabled(&self) -> bool {
        self.openrouter_api_key
            .as_ref()
            .is_some_and(|k| !k.trim().is_empty())
    }

    /// Returns true when `host` is in the allowlist (case-insensitive,
    /// scheme-stripped). Used by the http_fetch tool to gate requests.
    pub fn http_fetch_allows(&self, host: &str) -> bool {
        let h = host.trim().to_lowercase();
        self.http_fetch_allowlist
            .iter()
            .any(|a| a.trim().to_lowercase() == h)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsRow {
    #[serde(default)]
    id: Option<Thing>,
    #[serde(flatten)]
    settings: Settings,
}

/// Load the singleton settings record. Returns `Settings::default()`
/// when no record exists.
pub async fn load(db: &HarnessDb) -> Result<Settings> {
    let row: Option<SettingsRow> = db.select(("settings", SINGLETON_ID)).await?;
    Ok(row.map(|r| r.settings).unwrap_or_default())
}

/// Persist the singleton settings record (upserts).
pub async fn save(db: &HarnessDb, settings: &Settings) -> Result<()> {
    let _: Option<SettingsRow> = db
        .upsert(("settings", SINGLETON_ID))
        .content(settings.clone())
        .await?;
    Ok(())
}
