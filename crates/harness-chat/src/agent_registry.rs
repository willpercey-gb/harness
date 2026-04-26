use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Agent,
    Swarm,
    Graph,
    A2a,
    Distributed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Ollama,
    OpenRouter,
    /// Local Claude Code CLI (`claude -p`) — uses the user's logged-in
    /// session, no API key managed by harness.
    ClaudeCli,
    Bedrock,
    Vertex,
    OpenAi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CostTier {
    Free,
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
    Uncalculated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    Moe,
    Mod,
    Dense,
    Sparse,
    Hybrid,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: String,
    pub agent_type: AgentType,
    pub name: String,
    pub description: String,
    pub provider: Provider,
    pub model_id: String,
    pub parameters: Option<u32>,
    pub architecture: Option<Architecture>,
    pub cost: CostTier,
    pub supports_tools: bool,
    pub disabled: bool,
    pub disabled_message: Option<String>,
}

/// JSON-serialised, frontend-friendly view of an agent. Mirrors the
/// `Agent` type in `../portfolio/src/types/chat.types.ts`.
#[derive(Debug, Clone, Serialize)]
pub struct AgentDto {
    pub id: String,
    #[serde(rename = "type")]
    pub agent_type: AgentType,
    pub attributes: AgentAttributes,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentAttributes {
    pub name: String,
    pub description: String,
    pub model_id: Option<String>,
    pub provider: String,
    pub region: Option<String>,
    pub cost: CostTier,
    pub input_cost: Option<f64>,
    pub output_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub parameters: Option<u32>,
    pub total_parameters: Option<u32>,
    pub architecture: Option<Architecture>,
    pub supports_tools: bool,
    pub supports_session_manager: bool,
    pub disabled: bool,
    pub disabled_message: Option<String>,
}

impl From<AgentConfig> for AgentDto {
    fn from(cfg: AgentConfig) -> Self {
        AgentDto {
            id: cfg.id,
            agent_type: cfg.agent_type,
            attributes: AgentAttributes {
                name: cfg.name,
                description: cfg.description,
                model_id: Some(cfg.model_id),
                provider: provider_label(cfg.provider).to_string(),
                region: None,
                cost: cfg.cost,
                input_cost: None,
                output_cost: None,
                total_cost: None,
                parameters: cfg.parameters,
                total_parameters: cfg.parameters,
                architecture: cfg.architecture,
                supports_tools: cfg.supports_tools,
                supports_session_manager: true,
                disabled: cfg.disabled,
                disabled_message: cfg.disabled_message,
            },
        }
    }
}

fn provider_label(p: Provider) -> &'static str {
    match p {
        Provider::Ollama => "Ollama",
        Provider::OpenRouter => "OpenRouter",
        Provider::ClaudeCli => "ClaudeCLI",
        Provider::Bedrock => "Bedrock",
        Provider::Vertex => "Vertex",
        Provider::OpenAi => "OpenAI",
    }
}

/// Try to locate the `claude` binary the user has installed. Tauri-
/// bundled apps don't inherit the shell's PATH, so we have to look in
/// well-known locations + fall back to a login-shell probe.
///
/// Returns the first absolute path that exists. `None` if not found —
/// the ChatService will then return a friendly "is the Claude CLI
/// installed?" error from the spawn attempt.
pub fn find_claude_cli() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local/bin/claude"));
        candidates.push(home.join(".claude/local/claude"));
        candidates.push(home.join("bin/claude"));
    }
    candidates.push(PathBuf::from("/usr/local/bin/claude"));
    candidates.push(PathBuf::from("/opt/homebrew/bin/claude"));

    for c in &candidates {
        if c.exists() {
            return Some(c.clone());
        }
    }

    // Last resort: ask a login shell. Picks up nvm / volta / asdf etc.
    if let Ok(output) = std::process::Command::new("/bin/bash")
        .args(["-lc", "command -v claude"])
        .output()
    {
        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout);
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let p = PathBuf::from(trimmed);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Curated agents backed by the local `claude -p` CLI. Always enabled
/// when the binary is available — discovery is a cheap probe that
/// caches across calls.
pub fn claude_cli_agents() -> Vec<AgentConfig> {
    let entries: &[(&str, &str, &str, CostTier)] = &[
        (
            "haiku",
            "Claude Haiku 4.5 (CLI)",
            "Fast, cheap Claude via local CLI. Uses your logged-in session.",
            CostTier::Low,
        ),
        (
            "sonnet",
            "Claude Sonnet 4.6 (CLI)",
            "Balanced Claude via local CLI. Uses your logged-in session.",
            CostTier::Medium,
        ),
        (
            "opus",
            "Claude Opus 4.7 (CLI)",
            "Most capable Claude via local CLI. Slow, premium-priced. Uses your logged-in session.",
            CostTier::High,
        ),
    ];
    entries
        .iter()
        .map(|(alias, name, desc, cost)| AgentConfig {
            id: format!("claude-cli:{alias}"),
            agent_type: AgentType::Agent,
            name: (*name).to_string(),
            description: (*desc).to_string(),
            provider: Provider::ClaudeCli,
            model_id: (*alias).to_string(),
            parameters: None,
            architecture: None,
            cost: *cost,
            supports_tools: true,
            disabled: false,
            disabled_message: None,
        })
        .collect()
}

/// Curated list of OpenRouter models surfaced in the agent picker.
/// When `enabled` is false (no API key configured), every entry is
/// returned with `disabled: true` and a "configure API key" message,
/// so the provider chip still renders with non-zero count.
pub fn openrouter_agents(enabled: bool) -> Vec<AgentConfig> {
    let disabled_msg = if enabled {
        None
    } else {
        Some("Configure OpenRouter API key in Settings".to_string())
    };

    let entries: &[(&str, &str, &str, CostTier, Option<u32>, Architecture)] = &[
        // ── Free tier ──
        (
            "meta-llama/llama-3.3-70b-instruct:free",
            "Llama 3.3 70B (free)",
            "Meta's flagship 70B instruct model on the free tier.",
            CostTier::Free,
            Some(70),
            Architecture::Dense,
        ),
        (
            "qwen/qwen-2.5-72b-instruct:free",
            "Qwen 2.5 72B (free)",
            "Alibaba's general-purpose 72B model on the free tier.",
            CostTier::Free,
            Some(72),
            Architecture::Dense,
        ),
        // ── Cheap-and-good ──
        (
            "anthropic/claude-haiku-4-5",
            "Claude Haiku 4.5",
            "Fast, cheap, capable Anthropic model. Good for everyday chats.",
            CostTier::Low,
            None,
            Architecture::Unknown,
        ),
        (
            "openai/gpt-4o-mini",
            "GPT-4o mini",
            "OpenAI's affordable multimodal model.",
            CostTier::Low,
            None,
            Architecture::Unknown,
        ),
        // ── Premium ──
        (
            "anthropic/claude-opus-4-7",
            "Claude Opus 4.7",
            "Anthropic's most capable model. High reasoning, slow, premium-priced.",
            CostTier::High,
            None,
            Architecture::Unknown,
        ),
        (
            "openai/o3-mini",
            "OpenAI o3-mini",
            "OpenAI reasoning-focused model with extended thinking.",
            CostTier::Medium,
            None,
            Architecture::Unknown,
        ),
    ];

    entries
        .iter()
        .map(|(model_id, name, desc, cost, params, arch)| AgentConfig {
            id: format!("openrouter:{model_id}"),
            agent_type: AgentType::Agent,
            name: (*name).to_string(),
            description: (*desc).to_string(),
            provider: Provider::OpenRouter,
            model_id: (*model_id).to_string(),
            parameters: *params,
            architecture: Some(*arch),
            cost: *cost,
            supports_tools: true,
            disabled: !enabled,
            disabled_message: disabled_msg.clone(),
        })
        .collect()
}

/// Backwards-compatible alias — phase-1 callers asked for "placeholder
/// agents". Delegates to the curated OpenRouter list with key absent.
#[deprecated(note = "use openrouter_agents(enabled) directly")]
pub fn placeholder_agents() -> Vec<AgentConfig> {
    openrouter_agents(false)
}

/// Probe an Ollama daemon at `host` for installed models and return one
/// `AgentConfig` per model. Returns an empty Vec if the daemon is
/// unreachable — UI shows the existing empty state, not an error toast.
pub async fn discover_ollama(host: &str) -> Vec<AgentConfig> {
    #[derive(Deserialize)]
    struct TagsResponse {
        models: Vec<TagsModel>,
    }
    #[derive(Deserialize)]
    struct TagsModel {
        name: String,
        #[serde(default)]
        details: Option<TagsDetails>,
    }
    #[derive(Deserialize)]
    struct TagsDetails {
        #[serde(default)]
        parameter_size: Option<String>,
        #[serde(default)]
        family: Option<String>,
    }

    let url = format!("{}/api/tags", host.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build();
    let client = match client {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("ollama: failed to build client: {e}");
            return Vec::new();
        }
    };
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::info!("ollama unreachable at {host}: {e}");
            return Vec::new();
        }
    };
    if !resp.status().is_success() {
        tracing::warn!("ollama tags returned {}", resp.status());
        return Vec::new();
    }
    let tags: TagsResponse = match resp.json().await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("ollama tags decode: {e}");
            return Vec::new();
        }
    };

    tags.models
        .into_iter()
        .map(|m| {
            let parameters = m
                .details
                .as_ref()
                .and_then(|d| d.parameter_size.as_deref())
                .and_then(parse_parameter_size_to_billions);
            let family = m.details.as_ref().and_then(|d| d.family.clone());
            AgentConfig {
                id: format!("ollama:{}", m.name),
                agent_type: AgentType::Agent,
                name: m.name.clone(),
                description: family
                    .map(|f| format!("Ollama-served {f} model"))
                    .unwrap_or_else(|| "Local Ollama model".into()),
                provider: Provider::Ollama,
                model_id: m.name,
                parameters,
                architecture: Some(Architecture::Dense),
                cost: CostTier::Free,
                supports_tools: true,
                disabled: false,
                disabled_message: None,
            }
        })
        .collect()
}

/// Parse strings like "8.0B", "70B", "1.5B" into integer billions.
fn parse_parameter_size_to_billions(s: &str) -> Option<u32> {
    let trimmed = s.trim();
    let (num, suffix) = trimmed.split_at(trimmed.find(|c: char| !c.is_ascii_digit() && c != '.')?);
    let n: f64 = num.parse().ok()?;
    let scale = match suffix.trim().to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "M" => 0.001,
        _ => return None,
    };
    Some((n * scale).round() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_parameter_sizes() {
        assert_eq!(parse_parameter_size_to_billions("8.0B"), Some(8));
        assert_eq!(parse_parameter_size_to_billions("70B"), Some(70));
        assert_eq!(parse_parameter_size_to_billions("1.5B"), Some(2));
        assert_eq!(parse_parameter_size_to_billions("500M"), Some(1));
        assert_eq!(parse_parameter_size_to_billions(""), None);
        assert_eq!(parse_parameter_size_to_billions("garbage"), None);
    }

    #[test]
    fn openrouter_disabled_when_no_key() {
        let p = openrouter_agents(false);
        assert!(!p.is_empty());
        assert!(p.iter().all(|a| a.disabled));
        assert!(
            p.iter().all(|a| a.disabled_message.is_some()),
            "every disabled entry has a hint message"
        );
    }

    #[test]
    fn openrouter_enabled_when_key_set() {
        let p = openrouter_agents(true);
        assert!(p.iter().all(|a| !a.disabled));
        assert!(p.iter().all(|a| a.disabled_message.is_none()));
        // At least one free, one premium, all marked OpenRouter provider.
        assert!(p.iter().any(|a| matches!(a.cost, CostTier::Free)));
        assert!(p.iter().any(|a| matches!(a.cost, CostTier::High)));
        assert!(p.iter().all(|a| matches!(a.provider, Provider::OpenRouter)));
    }

    #[tokio::test]
    async fn discover_ollama_unreachable_returns_empty() {
        let agents = discover_ollama("http://127.0.0.1:1").await;
        assert!(agents.is_empty());
    }
}
