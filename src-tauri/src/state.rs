use std::sync::Arc;

use harness_chat::{
    claude_cli_agents, discover_ollama, find_claude_cli, openrouter_agents, AgentConfig,
    CancellationRegistry,
};
use harness_storage::{init_db, settings, HarnessDb, Settings};
use tokio::sync::RwLock;

pub struct AppState {
    pub db: Arc<HarnessDb>,
    pub cancellations: CancellationRegistry,
    pub settings: Arc<RwLock<Settings>>,
}

impl AppState {
    pub async fn build() -> anyhow::Result<Self> {
        let db_path = harness_storage::default_db_path();
        let db = init_db(&db_path).await?;
        let mut loaded_settings = settings::load(&db).await.unwrap_or_default();

        // Resolve the Claude CLI binary path if the user hasn't set
        // one — Tauri-spawned subprocesses don't see the shell PATH.
        // Lives in memory only; not persisted unless the user later
        // overrides it through Settings.
        if loaded_settings.claude_cli_path.is_none() {
            if let Some(p) = find_claude_cli() {
                tracing::info!("resolved claude CLI binary at {}", p.display());
                loaded_settings.claude_cli_path = Some(p);
            } else {
                tracing::warn!(
                    "claude CLI binary not found; ClaudeCli agents will fail until installed or \
                     a path is configured in Settings"
                );
            }
        }

        Ok(Self {
            db: Arc::new(db),
            cancellations: CancellationRegistry::new(),
            settings: Arc::new(RwLock::new(loaded_settings)),
        })
    }

    /// Reload settings from the database into the cached RwLock,
    /// re-running CLI-path discovery if the user hasn't persisted an
    /// override. Without this, a `settings_set` round-trip would
    /// blank out the in-memory `claude_cli_path` discovered at boot.
    pub async fn refresh_settings(&self) -> anyhow::Result<()> {
        let mut fresh = settings::load(&self.db).await?;
        if fresh.claude_cli_path.is_none() {
            fresh.claude_cli_path = find_claude_cli();
        }
        *self.settings.write().await = fresh;
        Ok(())
    }

    /// Live agent list: dynamic Ollama discovery + curated OpenRouter
    /// list (gated on `Settings.openrouter_enabled()`) + curated
    /// Claude-CLI list (always shown — actual availability is checked
    /// at chat_send time when the subprocess is spawned).
    pub async fn current_agents(&self) -> Vec<AgentConfig> {
        let settings = self.settings.read().await.clone();
        let mut agents = discover_ollama(&settings.ollama_host).await;
        agents.extend(openrouter_agents(settings.openrouter_enabled()));
        agents.extend(claude_cli_agents());
        agents
    }

    pub async fn agent_by_id(&self, id: &str) -> Option<AgentConfig> {
        self.current_agents().await.into_iter().find(|a| a.id == id)
    }
}
