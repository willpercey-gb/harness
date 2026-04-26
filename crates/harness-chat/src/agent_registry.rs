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
        Provider::Bedrock => "Bedrock",
        Provider::Vertex => "Vertex",
        Provider::OpenAi => "OpenAI",
    }
}

/// Static placeholder agents for providers we don't run yet — they
/// surface in the UI provider chips with `disabled = true` so the user
/// can see what's coming.
pub fn placeholder_agents() -> Vec<AgentConfig> {
    vec![
        AgentConfig {
            id: "openrouter:placeholder".into(),
            agent_type: AgentType::Agent,
            name: "OpenRouter (coming soon)".into(),
            description: "OpenRouter provider lands in a follow-up phase.".into(),
            provider: Provider::OpenRouter,
            model_id: "tbd".into(),
            parameters: None,
            architecture: None,
            cost: CostTier::Uncalculated,
            supports_tools: false,
            disabled: true,
            disabled_message: Some("OpenRouter integration not yet enabled".into()),
        },
    ]
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
    fn placeholder_agents_disabled() {
        let p = placeholder_agents();
        assert!(p.iter().all(|a| a.disabled));
    }

    #[tokio::test]
    async fn discover_ollama_unreachable_returns_empty() {
        let agents = discover_ollama("http://127.0.0.1:1").await;
        assert!(agents.is_empty());
    }
}
