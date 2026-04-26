//! Chat orchestration: agent registry, streaming pipeline, cancellation.

pub mod agent_registry;
pub mod cancel;
pub mod pipeline;
pub mod service;

pub use agent_registry::{
    claude_cli_agents, discover_ollama, openrouter_agents, AgentConfig, AgentDto, AgentType,
    Architecture, CostTier, Provider,
};
#[allow(deprecated)]
pub use agent_registry::placeholder_agents;
pub use cancel::CancellationRegistry;
pub use service::{run_chat, ChatRunOutcome};
