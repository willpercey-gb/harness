use std::sync::Arc;

use harness_chat::{discover_ollama, placeholder_agents, AgentConfig, CancellationRegistry};
use harness_storage::{init_db, HarnessDb};

pub struct AppState {
    pub db: Arc<HarnessDb>,
    pub cancellations: CancellationRegistry,
    pub ollama_host: String,
    /// Snapshot of placeholder/static agents loaded once at startup.
    pub static_agents: Vec<AgentConfig>,
}

impl AppState {
    pub async fn build() -> anyhow::Result<Self> {
        let db_path = harness_storage::default_db_path();
        let db = init_db(&db_path).await?;
        let ollama_host = std::env::var("HARNESS_OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".into());
        let static_agents = placeholder_agents();
        Ok(Self {
            db: Arc::new(db),
            cancellations: CancellationRegistry::new(),
            ollama_host,
            static_agents,
        })
    }

    /// Combined live agent list: dynamic Ollama discovery + static
    /// placeholders. Called per `list_agents` invocation so newly-pulled
    /// Ollama models appear without an app restart.
    pub async fn current_agents(&self) -> Vec<AgentConfig> {
        let mut agents = discover_ollama(&self.ollama_host).await;
        agents.extend(self.static_agents.clone());
        agents
    }

    pub async fn agent_by_id(&self, id: &str) -> Option<AgentConfig> {
        self.current_agents()
            .await
            .into_iter()
            .find(|a| a.id == id)
    }
}
