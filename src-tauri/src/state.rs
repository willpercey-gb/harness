use std::sync::Arc;

use harness_chat::{discover_ollama, openrouter_agents, AgentConfig, CancellationRegistry};
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
        let loaded_settings = settings::load(&db).await.unwrap_or_default();
        Ok(Self {
            db: Arc::new(db),
            cancellations: CancellationRegistry::new(),
            settings: Arc::new(RwLock::new(loaded_settings)),
        })
    }

    /// Reload settings from the database into the cached RwLock.
    pub async fn refresh_settings(&self) -> anyhow::Result<()> {
        let fresh = settings::load(&self.db).await?;
        *self.settings.write().await = fresh;
        Ok(())
    }

    /// Live agent list: dynamic Ollama discovery + curated OpenRouter
    /// list. The OpenRouter entries are gated on `Settings.openrouter_enabled()`.
    pub async fn current_agents(&self) -> Vec<AgentConfig> {
        let settings = self.settings.read().await.clone();
        let mut agents = discover_ollama(&settings.ollama_host).await;
        agents.extend(openrouter_agents(settings.openrouter_enabled()));
        agents
    }

    pub async fn agent_by_id(&self, id: &str) -> Option<AgentConfig> {
        self.current_agents().await.into_iter().find(|a| a.id == id)
    }
}
