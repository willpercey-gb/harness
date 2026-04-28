//! Tools that expose READ access to the local memex-core graph + vector
//! store to the main chat agent: `recall` for hybrid search over saved
//! memories and entities, and `lookup_entity` for direct name lookups.
//!
//! Memory WRITES are no longer tool-driven — the passive
//! `crate::memory_agent` extractor (stage 4 of the per-turn pipeline,
//! defined in harness-chat) observes each turn and updates the graph
//! itself. The main agent never has to consciously decide to remember.

use std::sync::Arc;

use async_trait::async_trait;
use memex_core::{entities, query, EmbeddingService, MemexDb};
use serde_json::{json, Value};
use strands_core::types::tools::ToolSpec;
use strands_core::{StrandsError, Tool, ToolContext, ToolOutput};

// ---------------------------------------------------------------------------
// recall
// ---------------------------------------------------------------------------

pub struct Recall {
    pub db: Arc<MemexDb>,
    pub embedder: Arc<EmbeddingService>,
}

#[async_trait]
impl Tool for Recall {
    fn name(&self) -> &str {
        "recall"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "recall".into(),
            description: "Search saved memories and entities for content relevant to a query. \
                          Returns ranked hits with their content, summary, source, and any \
                          linked entities. Use whenever the user asks about something they \
                          may have told you before."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "What to look for. Natural language; semantic + keyword search."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max number of hits. Default 5.",
                        "minimum": 1,
                        "maximum": 25
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn invoke(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let q = match input.get("query").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'query' string")),
        };
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .map(|n| n.clamp(1, 25) as usize)
            .unwrap_or(5);
        match query::hybrid_query(&self.db, &self.embedder, &q, None, limit).await {
            Ok(results) => {
                let payload: Vec<Value> = results
                    .into_iter()
                    .map(|r| {
                        json!({
                            "content": r.content,
                            "summary": r.summary,
                            "source_type": r.source_type,
                            "source_id": r.source_id,
                            "timestamp": r.timestamp,
                            "score": r.score,
                            "linked_entities": r.linked_entities.iter().map(|e| {
                                json!({ "type": e.entity_type, "name": e.name })
                            }).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                Ok(ToolOutput::success(json!({ "hits": payload })))
            }
            Err(e) => Ok(ToolOutput::error(format!("recall failed: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// lookup_entity
// ---------------------------------------------------------------------------

pub struct LookupEntity {
    pub db: Arc<MemexDb>,
}

#[async_trait]
impl Tool for LookupEntity {
    fn name(&self) -> &str {
        "lookup_entity"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "lookup_entity".into(),
            description: "Find an entity by name (case-insensitive, any type). Returns its id, \
                          type, description, and content body if it exists; null otherwise."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"]
            }),
        }
    }

    async fn invoke(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let name = match input.get("name").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'name'")),
        };
        match entities::find_entity_any_type(&self.db, &name).await {
            Ok(Some((entity_type, entity))) => Ok(ToolOutput::success(json!({
                "id": entity.id,
                "type": entity_type.to_string(),
                "name": entity.name,
                "aliases": entity.aliases,
                "description": entity.description,
                "content": entity.content,
            }))),
            Ok(None) => Ok(ToolOutput::success(Value::Null)),
            Err(e) => Ok(ToolOutput::error(format!("lookup_entity failed: {e}"))),
        }
    }
}

