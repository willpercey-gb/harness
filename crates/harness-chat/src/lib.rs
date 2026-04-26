//! Chat orchestration: agent registry, streaming pipeline, cancellation.

pub mod agent_registry;
pub mod cancel;
pub mod pipeline;
pub mod service;

pub use agent_registry::{
    discover_ollama, placeholder_agents, AgentConfig, AgentDto, AgentType, Architecture, CostTier,
    Provider,
};
pub use cancel::CancellationRegistry;
pub use service::{run_chat, ChatRunOutcome};
