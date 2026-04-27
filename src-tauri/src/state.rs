use std::path::PathBuf;
use std::sync::Arc;

use harness_chat::{
    claude_cli_agents, codex_cli_agents, discover_ollama, find_claude_cli, find_codex_cli,
    find_gemini_cli, gemini_cli_agents, openrouter_agents, AgentConfig, CancellationRegistry,
};
use harness_storage::{init_db, settings, HarnessDb, Settings};
use harness_tools::{init_memex_db, EmbeddingService, MemexDb};
use tokio::sync::RwLock;

pub struct AppState {
    pub db: Arc<HarnessDb>,
    pub cancellations: CancellationRegistry,
    pub settings: Arc<RwLock<Settings>>,
    /// Per-harness Memex graph + vector store (separate file from the
    /// chat history db) — backs the remember/recall/note_entity/etc tools.
    pub memex_db: Arc<MemexDb>,
    /// In-process embedder used by the memex tools. None on first-run
    /// model-load failure; tools log a friendly "knowledge store
    /// unavailable" error in that case.
    pub embedder: Option<Arc<EmbeddingService>>,
}

fn default_memex_path() -> PathBuf {
    dirs::home_dir()
        .expect("home directory")
        .join(".harness")
        .join("memex-db")
}

impl AppState {
    pub async fn build() -> anyhow::Result<Self> {
        let db_path = harness_storage::default_db_path();
        let db = init_db(&db_path).await?;
        let mut loaded_settings = settings::load(&db).await.unwrap_or_default();

        // Resolve the Claude / Codex / Gemini CLI binaries if the user
        // hasn't set one — Tauri-spawned subprocesses don't see the
        // shell PATH. Lives in memory only; not persisted unless the
        // user later overrides it through Settings.
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
        if loaded_settings.codex_cli_path.is_none() {
            if let Some(p) = find_codex_cli() {
                tracing::info!("resolved codex CLI binary at {}", p.display());
                loaded_settings.codex_cli_path = Some(p);
            } else {
                tracing::info!("codex CLI binary not found; CodexCli agents will surface a spawn error");
            }
        }
        if loaded_settings.gemini_cli_path.is_none() {
            if let Some(p) = find_gemini_cli() {
                tracing::info!("resolved gemini CLI binary at {}", p.display());
                loaded_settings.gemini_cli_path = Some(p);
            } else {
                tracing::info!("gemini CLI binary not found; GeminiCli agents will surface a spawn error");
            }
        }

        // Initialise the Memex DB at the configured path (default
        // ~/.harness/memex-db) and warm up the embedder. Failure to
        // build the embedder is non-fatal — we record it as `None`
        // and the tools that depend on it surface a clean error.
        let memex_path = loaded_settings
            .memex_db_path
            .clone()
            .unwrap_or_else(default_memex_path);
        let memex_db = init_memex_db(&memex_path)
            .await
            .map_err(|e| anyhow::anyhow!("init memex db: {e}"))?;
        let embedder = match EmbeddingService::new() {
            Ok(e) => {
                tracing::info!("memex embedder ready");
                Some(Arc::new(e))
            }
            Err(e) => {
                tracing::warn!("memex embedder failed to load: {e} (memory tools will refuse calls)");
                None
            }
        };

        Ok(Self {
            db: Arc::new(db),
            cancellations: CancellationRegistry::new(),
            settings: Arc::new(RwLock::new(loaded_settings)),
            memex_db: Arc::new(memex_db),
            embedder,
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
        if fresh.codex_cli_path.is_none() {
            fresh.codex_cli_path = find_codex_cli();
        }
        if fresh.gemini_cli_path.is_none() {
            fresh.gemini_cli_path = find_gemini_cli();
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
        agents.extend(codex_cli_agents());
        agents.extend(gemini_cli_agents());
        agents
    }

    pub async fn agent_by_id(&self, id: &str) -> Option<AgentConfig> {
        self.current_agents().await.into_iter().find(|a| a.id == id)
    }
}
