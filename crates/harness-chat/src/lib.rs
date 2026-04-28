//! Chat orchestration: agent registry, streaming pipeline, cancellation.

pub mod agent_registry;
pub mod cancel;
pub mod context;
pub mod context_agent;
pub mod memory_agent;
pub mod memory_resolver;
pub mod pipeline;
pub mod service;

pub use agent_registry::{
    claude_cli_agents, codex_cli_agents, discover_ollama, find_claude_cli, find_codex_cli,
    find_gemini_cli, gemini_cli_agents, openrouter_agents, AgentConfig, AgentDto, AgentType,
    Architecture, CostTier, Provider,
};
#[allow(deprecated)]
pub use agent_registry::placeholder_agents;
pub use cancel::CancellationRegistry;
pub use context::{parse_envelope, parse_intent, xml_envelope, Intent, IntentSource};
pub use service::{run_chat, ChatRunOutcome};
