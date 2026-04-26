//! Tools that wrap the local memex-core graph + vector store. Lets the
//! main agent remember facts, recall them later, note entities, and
//! draw typed relationships between them — all backed by the
//! per-harness Memex DB at `~/.harness/memex-db` (override via Settings).

use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use memex_core::{
    entities, memories, query, relationships, types as mtypes, EmbeddingService, EntityType,
    MemexDb, MemoryChunk, RelationType,
};
use serde_json::{json, Value};
use strands_core::types::tools::ToolSpec;
use strands_core::{StrandsError, Tool, ToolContext, ToolOutput};

const ENTITY_TYPES: &str = "person, organization, project, technology, topic, location, component";
const RELATION_TYPES: &str =
    "works_at, part_of, works_on, uses_tech, knows_about, related_to, mentions";

// ---------------------------------------------------------------------------
// remember
// ---------------------------------------------------------------------------

pub struct Remember {
    pub db: Arc<MemexDb>,
    pub embedder: Arc<EmbeddingService>,
    /// The active session id, baked in at agent build time. None when
    /// the chat is happening outside a persisted session.
    pub session_id: Option<String>,
}

#[async_trait]
impl Tool for Remember {
    fn name(&self) -> &str {
        "remember"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "remember".into(),
            description: "Save a free-form memory about the user, the project, or anything else \
                          worth recalling later. Memories are searchable by semantic similarity \
                          via the `recall` tool. Use sparingly — one fact per call."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The fact or note to save. Self-contained, one sentence to a paragraph."
                    },
                    "summary": {
                        "type": "string",
                        "description": "Optional shorter summary used in recall snippets."
                    }
                },
                "required": ["content"]
            }),
        }
    }

    async fn invoke(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let content = match input.get("content").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'content' string")),
        };
        let summary = input
            .get("summary")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let chunk = MemoryChunk {
            id: None,
            content,
            summary,
            source_type: "chat".into(),
            source_id: self.session_id.clone(),
            source_path: None,
            timestamp: Utc::now(),
            metadata: Default::default(),
        };
        match memories::insert_memory(&self.db, &self.embedder, &chunk).await {
            Ok(id) => Ok(ToolOutput::success(json!({ "id": id }))),
            Err(e) => Ok(ToolOutput::error(format!("remember failed: {e}"))),
        }
    }
}

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
// note_entity
// ---------------------------------------------------------------------------

pub struct NoteEntity {
    pub db: Arc<MemexDb>,
    pub embedder: Arc<EmbeddingService>,
}

#[async_trait]
impl Tool for NoteEntity {
    fn name(&self) -> &str {
        "note_entity"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "note_entity".into(),
            description: format!(
                "Create or update a structured entity in the knowledge graph. Use for stable \
                 nouns the user mentions repeatedly (their employer, a project they're working \
                 on, a tech they're using). entity_type must be one of: {ENTITY_TYPES}. \
                 Existing entities with the same name are merged; their description is replaced \
                 and content is appended."
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "entity_type": {
                        "type": "string",
                        "description": format!("One of: {ENTITY_TYPES}.")
                    },
                    "name": { "type": "string" },
                    "description": {
                        "type": "string",
                        "description": "Short one-line description. Replaces any existing one."
                    },
                    "content": {
                        "type": "string",
                        "description": "Longer body of knowledge about this entity. Appended to existing content."
                    }
                },
                "required": ["entity_type", "name"]
            }),
        }
    }

    async fn invoke(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let type_str = match input.get("entity_type").and_then(Value::as_str) {
            Some(s) => s.to_string(),
            None => return Ok(ToolOutput::error("missing 'entity_type'")),
        };
        let entity_type = match EntityType::from_str(&type_str) {
            Ok(t) => t,
            Err(e) => return Ok(ToolOutput::error(format!("{e}. valid: {ENTITY_TYPES}"))),
        };
        let name = match input.get("name").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'name'")),
        };
        let description = input
            .get("description")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        let content = input
            .get("content")
            .and_then(Value::as_str)
            .map(|s| s.to_string());

        let entity = mtypes::Entity {
            id: None,
            name,
            aliases: Vec::new(),
            description,
            content,
            metadata: Default::default(),
            created_at: None,
            updated_at: None,
        };

        match entities::upsert_entity(&self.db, &self.embedder, &entity_type, &entity).await {
            Ok(id) => Ok(ToolOutput::success(
                json!({ "id": id, "type": entity_type.to_string() }),
            )),
            Err(e) => Ok(ToolOutput::error(format!("note_entity failed: {e}"))),
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

// ---------------------------------------------------------------------------
// link_entities
// ---------------------------------------------------------------------------

pub struct LinkEntities {
    pub db: Arc<MemexDb>,
}

#[async_trait]
impl Tool for LinkEntities {
    fn name(&self) -> &str {
        "link_entities"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "link_entities".into(),
            description: format!(
                "Create a typed relationship between two entities. from_id and to_id are the \
                 ids returned by note_entity / lookup_entity (format: 'table:id'). Valid \
                 relations: {RELATION_TYPES}. Schema constraints may remap the relation if \
                 the entity types don't match the canonical pairing."
            ),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from_id": { "type": "string" },
                    "to_id": { "type": "string" },
                    "relation": {
                        "type": "string",
                        "description": format!("One of: {RELATION_TYPES}.")
                    },
                    "metadata": {
                        "type": "object",
                        "description": "Optional free-form metadata (e.g. role, since)."
                    }
                },
                "required": ["from_id", "to_id", "relation"]
            }),
        }
    }

    async fn invoke(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput, StrandsError> {
        let from_id = match input.get("from_id").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'from_id'")),
        };
        let to_id = match input.get("to_id").and_then(Value::as_str) {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return Ok(ToolOutput::error("missing 'to_id'")),
        };
        let rel_str = match input.get("relation").and_then(Value::as_str) {
            Some(s) => s.to_string(),
            None => return Ok(ToolOutput::error("missing 'relation'")),
        };
        let rel_type = match RelationType::from_str(&rel_str) {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutput::error(format!("{e}. valid: {RELATION_TYPES}"))),
        };
        let metadata = input
            .get("metadata")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default();

        match relationships::create_relationship(&self.db, &rel_type, &from_id, &to_id, &metadata)
            .await
        {
            Ok(id) => Ok(ToolOutput::success(json!({ "id": id }))),
            Err(e) => Ok(ToolOutput::error(format!("link_entities failed: {e}"))),
        }
    }
}
