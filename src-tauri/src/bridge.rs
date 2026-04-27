//! Local TCP bridge — lets the `harness-mcp` proxy binary call into the
//! running harness app's already-open Memex DB without contending for
//! the RocksDB single-writer lock.
//!
//! Protocol: newline-delimited JSON over TCP on 127.0.0.1:{HARNESS_BRIDGE_PORT}.
//!   Request:  {"method": "tool_name", "params": {...}}
//!   Response: {"result": ...} or {"error": "..."}

use std::sync::Arc;

use harness_tools::memex_api::{entities, memories, query, relationships, types as mtypes};
use harness_tools::{EmbeddingService, MemexDb};
use serde_json::{json, Value};
use std::str::FromStr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

/// Different from Memex's 19850 so both apps can coexist.
pub const HARNESS_BRIDGE_PORT: u16 = 19851;

pub async fn start_bridge(db: Arc<MemexDb>, embedder: Option<Arc<EmbeddingService>>) {
    let addr = format!("127.0.0.1:{HARNESS_BRIDGE_PORT}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!("bridge: failed to bind {addr}: {e}");
            return;
        }
    };
    tracing::info!("bridge: listening on {addr}");

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("bridge: accept error: {e}");
                continue;
            }
        };
        tracing::debug!("bridge: connection from {peer}");
        let db = db.clone();
        let embedder = embedder.clone();
        tokio::spawn(handle_connection(stream, db, embedder));
    }
}

async fn handle_connection(
    stream: TcpStream,
    db: Arc<MemexDb>,
    embedder: Option<Arc<EmbeddingService>>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let resp = match serde_json::from_str::<Value>(&line) {
            Ok(req) => handle_request(&db, embedder.as_deref(), &req).await,
            Err(e) => json!({ "error": format!("invalid JSON: {e}") }),
        };
        let mut out = serde_json::to_string(&resp).unwrap_or_else(|_| r#"{"error":"serialize"}"#.into());
        out.push('\n');
        if writer.write_all(out.as_bytes()).await.is_err() {
            break;
        }
    }
}

fn require_embedder<'a>(embedder: Option<&'a EmbeddingService>) -> Result<&'a EmbeddingService, Value> {
    embedder.ok_or_else(|| json!({
        "error": "embedder unavailable — restart harness with internet to download the model"
    }))
}

async fn handle_request(
    db: &MemexDb,
    embedder: Option<&EmbeddingService>,
    req: &Value,
) -> Value {
    let method = req["method"].as_str().unwrap_or("");
    let params = &req["params"];
    match method {
        // -------- Query --------
        "query_knowledge" => {
            let emb = match require_embedder(embedder) { Ok(e) => e, Err(v) => return v };
            let q = params["query"].as_str().unwrap_or("");
            let limit = params["limit"].as_u64().unwrap_or(10).clamp(1, 50) as usize;
            let types: Option<Vec<mtypes::EntityType>> = params["entity_types"]
                .as_array()
                .map(|v| v.iter().filter_map(|s| s.as_str()?.parse().ok()).collect());
            match query::hybrid_query(db, emb, q, types.as_deref(), limit).await {
                Ok(r) => json!({ "result": r }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "search_memories" => {
            let emb = match require_embedder(embedder) { Ok(e) => e, Err(v) => return v };
            let q = params["query"].as_str().unwrap_or("");
            let source = params["source_type"].as_str();
            let limit = params["limit"].as_u64().unwrap_or(10).clamp(1, 50) as usize;
            match memories::search_memories_vector(db, emb, q, source, None, limit).await {
                Ok(r) => json!({ "result": r }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }

        // -------- Entities --------
        "get_entity_by_name" => {
            let name = params["name"].as_str().unwrap_or("");
            match entities::find_entity_any_type(db, name).await {
                Ok(Some((et, e))) => json!({ "result": {
                    "entity_type": et.table_name(),
                    "entity": e,
                }}),
                Ok(None) => json!({ "result": Value::Null }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "get_entity" => {
            let et = match params["entity_type"].as_str().and_then(|s| mtypes::EntityType::from_str(s).ok()) {
                Some(t) => t,
                None => return json!({ "error": "entity_type required" }),
            };
            let name = params["name"].as_str().unwrap_or("");
            match entities::get_entity(db, &et, name).await {
                Ok(Some(e)) => json!({ "result": e }),
                Ok(None) => json!({ "result": Value::Null }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "list_entities" => {
            let et = match params["entity_type"].as_str().and_then(|s| mtypes::EntityType::from_str(s).ok()) {
                Some(t) => t,
                None => return json!({ "error": "entity_type required" }),
            };
            let filter = params["filter"].as_str();
            let limit = params["limit"].as_u64().unwrap_or(50).clamp(1, 500) as usize;
            match entities::list_entities(db, &et, filter, limit).await {
                Ok(r) => json!({ "result": r }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "add_entity" => {
            let emb = match require_embedder(embedder) { Ok(e) => e, Err(v) => return v };
            let et = match params["entity_type"].as_str().and_then(|s| mtypes::EntityType::from_str(s).ok()) {
                Some(t) => t,
                None => return json!({ "error": "entity_type required" }),
            };
            let name = params["name"].as_str().unwrap_or("").to_string();
            if name.trim().is_empty() {
                return json!({ "error": "name required" });
            }
            let aliases: Vec<String> = params["aliases"]
                .as_array()
                .map(|v| v.iter().filter_map(|s| s.as_str()).map(|s| s.to_string()).collect())
                .unwrap_or_default();
            let entity = mtypes::Entity {
                id: None,
                name,
                aliases,
                description: params["description"].as_str().map(|s| s.to_string()),
                content: params["content"].as_str().map(|s| s.to_string()),
                metadata: Default::default(),
                created_at: None,
                updated_at: None,
            };
            match entities::upsert_entity(db, emb, &et, &entity).await {
                Ok(id) => json!({ "result": { "id": id, "type": et.table_name() } }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "update_entity" => {
            let name = params["name"].as_str().unwrap_or("");
            if name.is_empty() {
                return json!({ "error": "name required" });
            }
            let description = params["description"].as_str();
            let content = params["content"].as_str();
            if description.is_none() && content.is_none() {
                return json!({ "error": "at least one of description or content required" });
            }
            match entities::update_entity_by_name(db, name, description, content).await {
                Ok(Some(e)) => json!({ "result": e }),
                Ok(None) => json!({ "error": format!("entity '{name}' not found") }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }
        "get_entity_graph" => {
            let name = params["name"].as_str().unwrap_or("");
            let depth = params["depth"].as_u64().unwrap_or(2).clamp(1, 3) as usize;
            // Resolve entity → id, then traverse.
            let id = if let Some(et_str) = params["entity_type"].as_str() {
                let et = match mtypes::EntityType::from_str(et_str) {
                    Ok(t) => t,
                    Err(e) => return json!({ "error": e }),
                };
                match entities::get_entity(db, &et, name).await {
                    Ok(Some(e)) => e.id,
                    Ok(None) => return json!({ "error": format!("entity '{name}' not found") }),
                    Err(e) => return json!({ "error": e.to_string() }),
                }
            } else {
                match entities::find_entity_any_type(db, name).await {
                    Ok(Some((_, e))) => e.id,
                    Ok(None) => return json!({ "error": format!("entity '{name}' not found") }),
                    Err(e) => return json!({ "error": e.to_string() }),
                }
            };
            let id = match id {
                Some(i) => i,
                None => return json!({ "error": "entity has no id" }),
            };
            match relationships::get_entity_graph(db, &id, depth).await {
                Ok(g) => json!({ "result": g }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }

        // -------- Relationships --------
        "add_relationship" => {
            // Resolve from_/to_ entities by name (and optional type).
            let from_name = params["from_name"].as_str().unwrap_or("");
            let to_name = params["to_name"].as_str().unwrap_or("");
            let rel_type = match params["rel_type"]
                .as_str()
                .and_then(|s| mtypes::RelationType::from_str(s).ok())
            {
                Some(r) => r,
                None => return json!({ "error": "rel_type required (works_at|part_of|works_on|uses_tech|knows_about|related_to|mentions)" }),
            };
            let from_id = match resolve_id(db, from_name, params["from_type"].as_str()).await {
                Ok(id) => id,
                Err(v) => return v,
            };
            let to_id = match resolve_id(db, to_name, params["to_type"].as_str()).await {
                Ok(id) => id,
                Err(v) => return v,
            };
            let metadata = params["metadata"]
                .as_object()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();
            match relationships::create_relationship(db, &rel_type, &from_id, &to_id, &metadata).await {
                Ok(id) => json!({ "result": { "id": id } }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }

        // -------- Memories --------
        "add_memory" => {
            let emb = match require_embedder(embedder) { Ok(e) => e, Err(v) => return v };
            let content = params["content"].as_str().unwrap_or("").to_string();
            if content.trim().is_empty() {
                return json!({ "error": "content required" });
            }
            let chunk = mtypes::MemoryChunk {
                id: None,
                content,
                summary: params["summary"].as_str().map(|s| s.to_string()),
                source_type: params["source_type"].as_str().unwrap_or("manual").to_string(),
                source_id: params["source_id"].as_str().map(|s| s.to_string()),
                source_path: params["source_path"].as_str().map(|s| s.to_string()),
                timestamp: chrono::Utc::now(),
                metadata: Default::default(),
            };
            match memories::insert_memory(db, emb, &chunk).await {
                Ok(id) => json!({ "result": { "id": id } }),
                Err(e) => json!({ "error": e.to_string() }),
            }
        }

        unknown => json!({ "error": format!("unknown method: {unknown}") }),
    }
}

async fn resolve_id(db: &MemexDb, name: &str, opt_type: Option<&str>) -> Result<String, Value> {
    if let Some(t) = opt_type {
        let et = mtypes::EntityType::from_str(t).map_err(|e| json!({ "error": e }))?;
        match entities::get_entity(db, &et, name).await {
            Ok(Some(e)) => e
                .id
                .ok_or_else(|| json!({ "error": format!("entity '{name}' has no id") })),
            Ok(None) => Err(json!({ "error": format!("entity '{name}' not found") })),
            Err(e) => Err(json!({ "error": e.to_string() })),
        }
    } else {
        match entities::find_entity_any_type(db, name).await {
            Ok(Some((_, e))) => e
                .id
                .ok_or_else(|| json!({ "error": format!("entity '{name}' has no id") })),
            Ok(None) => Err(json!({ "error": format!("entity '{name}' not found") })),
            Err(e) => Err(json!({ "error": e.to_string() })),
        }
    }
}
