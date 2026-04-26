use std::sync::Arc;

use harness_chat::{discover_ollama, placeholder_agents, AgentConfig, CancellationRegistry};
use harness_storage::{init_db, settings, HarnessDb, Settings};
use tokio::sync::RwLock;

pub struct AppState {
    pub db: Arc<HarnessDb>,
    pub cancellations: CancellationRegistry,
    pub settings: Arc<RwLock<Settings>>,
    /// Snapshot of placeholder/static agents loaded once at startup.
    pub static_agents: Vec<AgentConfig>,
}

impl AppState {
    pub async fn build() -> anyhow::Result<Self> {
        let db_path = harness_storage::default_db_path();
        let db = init_db(&db_path).await?;
        let loaded_settings = settings::load(&db).await.unwrap_or_default();
        let static_agents = placeholder_agents();
        Ok(Self {
            db: Arc::new(db),
            cancellations: CancellationRegistry::new(),
            settings: Arc::new(RwLock::new(loaded_settings)),
            static_agents,
        })
    }

    /// Reload settings from the database into the cached RwLock.
    pub async fn refresh_settings(&self) -> anyhow::Result<()> {
        let fresh = settings::load(&self.db).await?;
        *self.settings.write().await = fresh;
        Ok(())
    }

    /// Combined live agent list: dynamic Ollama discovery + static
    /// placeholders. Called per `list_agents` invocation so newly-pulled
    /// Ollama models appear without an app restart.
    pub async fn current_agents(&self) -> Vec<AgentConfig> {
        let settings = self.settings.read().await.clone();
        let mut agents = discover_ollama(&settings.ollama_host).await;
        agents.extend(self.static_agents.clone());
        agents
    }

    pub async fn agent_by_id(&self, id: &str) -> Option<AgentConfig> {
        self.current_agents().await.into_iter().find(|a| a.id == id)
    }
}
