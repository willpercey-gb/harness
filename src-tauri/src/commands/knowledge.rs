//! Read-only Tauri commands over the harness-owned Memex graph + vector
//! store. Backs the /knowledge page in the frontend (graph explorer +
//! memory timeline + search).

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use harness_tools::ingest::{ingest_folder, IngestProgress};
use harness_tools::memex_api::{entities, memories, query, relationships, types as mtypes};
use harness_tools::provisional;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
pub struct FullGraphEntityDto {
    pub id: String,
    pub entity_type: String,
    pub name: String,
    pub description: Option<String>,
    pub content: Option<String>,
    pub aliases: Vec<String>,
}

#[derive(Serialize)]
pub struct GraphEdgeDto {
    pub from_id: String,
    pub to_id: String,
    pub relation_type: String,
}

#[derive(Serialize)]
pub struct FullGraphResponse {
    pub entities: Vec<FullGraphEntityDto>,
    pub edges: Vec<GraphEdgeDto>,
}

#[tauri::command]
pub async fn get_full_graph(state: State<'_, AppState>) -> Result<FullGraphResponse, String> {
    let entities = entities::get_all_entities(&state.memex_db, 500)
        .await
        .map_err(|e| e.to_string())?;
    let edges_raw = relationships::get_all_relationships(&state.memex_db)
        .await
        .map_err(|e| e.to_string())?;

    let entities = entities
        .into_iter()
        .map(|(et, e)| FullGraphEntityDto {
            id: e.id.unwrap_or_default(),
            entity_type: et.to_string(),
            name: e.name,
            description: e.description,
            content: e.content,
            aliases: e.aliases,
        })
        .collect();

    let edges = edges_raw
        .into_iter()
        .map(|e| GraphEdgeDto {
            from_id: e.from_id,
            to_id: e.to_id,
            relation_type: e.relation_type,
        })
        .collect();

    Ok(FullGraphResponse { entities, edges })
}

#[derive(Serialize)]
pub struct GraphNodeDto {
    pub id: String,
    pub entity_type: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct EntityGraphDto {
    pub center: GraphNodeDto,
    pub nodes: Vec<GraphNodeDto>,
    pub edges: Vec<GraphEdgeDto>,
}

#[tauri::command]
pub async fn get_entity_graph(
    entity_id: String,
    depth: Option<usize>,
    state: State<'_, AppState>,
) -> Result<EntityGraphDto, String> {
    let g = relationships::get_entity_graph(&state.memex_db, &entity_id, depth.unwrap_or(2))
        .await
        .map_err(|e| e.to_string())?;
    Ok(EntityGraphDto {
        center: GraphNodeDto {
            id: g.center.id,
            entity_type: g.center.entity_type,
            name: g.center.name,
            description: g.center.description,
        },
        nodes: g
            .nodes
            .into_iter()
            .map(|n| GraphNodeDto {
                id: n.id,
                entity_type: n.entity_type,
                name: n.name,
                description: n.description,
            })
            .collect(),
        edges: g
            .edges
            .into_iter()
            .map(|e| GraphEdgeDto {
                from_id: e.from_id,
                to_id: e.to_id,
                relation_type: e.relation_type,
            })
            .collect(),
    })
}

#[derive(Serialize)]
pub struct MemoryChunkDto {
    pub id: Option<String>,
    pub content: String,
    pub summary: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[tauri::command]
pub async fn get_recent_memories(
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryChunkDto>, String> {
    let limit = limit.unwrap_or(50).clamp(1, 500) as i64;
    let mut res = state
        .memex_db
        .query(
            "SELECT id, content, summary, source_type, source_id, timestamp \
             FROM memory_chunk ORDER BY timestamp DESC LIMIT $limit",
        )
        .bind(("limit", limit))
        .await
        .map_err(|e| e.to_string())?;
    #[derive(serde::Deserialize)]
    struct Row {
        id: Option<surrealdb::sql::Thing>,
        content: String,
        summary: Option<String>,
        source_type: String,
        source_id: Option<String>,
        timestamp: DateTime<Utc>,
    }
    let rows: Vec<Row> = res.take(0).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| MemoryChunkDto {
            id: r.id.map(|t| t.to_string()),
            content: r.content,
            summary: r.summary,
            source_type: r.source_type,
            source_id: r.source_id,
            timestamp: r.timestamp,
        })
        .collect())
}

#[derive(Serialize)]
pub struct EntityRefDto {
    pub entity_type: String,
    pub name: String,
    pub id: String,
}

#[derive(Serialize)]
pub struct QueryResultDto {
    pub content: String,
    pub summary: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub score: f64,
    pub linked_entities: Vec<EntityRefDto>,
}

#[tauri::command]
pub async fn query_knowledge(
    query_text: String,
    entity_types: Option<Vec<String>>,
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<QueryResultDto>, String> {
    let embedder = state
        .embedder
        .as_ref()
        .ok_or_else(|| "embedder unavailable — restart with internet to download the model".to_string())?;
    let limit = limit.unwrap_or(10).clamp(1, 50) as usize;

    let parsed_types: Option<Vec<mtypes::EntityType>> = entity_types.map(|ts| {
        ts.into_iter()
            .filter_map(|s| s.parse::<mtypes::EntityType>().ok())
            .collect()
    });
    let type_slice = parsed_types.as_deref();

    let results = query::hybrid_query(&state.memex_db, embedder, &query_text, type_slice, limit)
        .await
        .map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .map(|r| QueryResultDto {
            content: r.content,
            summary: r.summary,
            source_type: r.source_type,
            source_id: r.source_id,
            timestamp: r.timestamp,
            score: r.score,
            linked_entities: r
                .linked_entities
                .into_iter()
                .map(|e| EntityRefDto {
                    entity_type: e.entity_type,
                    name: e.name,
                    id: e.id,
                })
                .collect(),
        })
        .collect())
}

#[derive(Serialize)]
pub struct KnowledgeStatsDto {
    pub memory_chunks: i64,
    pub entities_total: i64,
    pub entities_by_type: std::collections::HashMap<String, i64>,
    pub relationships: i64,
}

#[tauri::command]
pub async fn get_knowledge_stats(state: State<'_, AppState>) -> Result<KnowledgeStatsDto, String> {
    let memory_chunks = memories::count_memories(&state.memex_db)
        .await
        .map_err(|e| e.to_string())?;

    let mut entities_by_type = std::collections::HashMap::new();
    let mut entities_total = 0i64;
    for et in mtypes::EntityType::all() {
        let q = format!("SELECT count() AS c FROM {} GROUP ALL", et.table_name());
        let mut res = state.memex_db.query(&q).await.map_err(|e| e.to_string())?;
        #[derive(serde::Deserialize)]
        struct R {
            c: i64,
        }
        let rows: Vec<R> = res.take(0).unwrap_or_default();
        let count = rows.into_iter().next().map(|r| r.c).unwrap_or(0);
        entities_total += count;
        entities_by_type.insert(et.to_string(), count);
    }

    // Relationships = sum of all relation table counts.
    let mut relationships_total = 0i64;
    for table in [
        "works_at",
        "part_of",
        "works_on",
        "uses_tech",
        "knows_about",
        "related_to",
        "mentions",
    ] {
        let q = format!("SELECT count() AS c FROM {table} GROUP ALL");
        let mut res = state.memex_db.query(&q).await.map_err(|e| e.to_string())?;
        #[derive(serde::Deserialize)]
        struct R {
            c: i64,
        }
        let rows: Vec<R> = res.take(0).unwrap_or_default();
        relationships_total += rows.into_iter().next().map(|r| r.c).unwrap_or(0);
    }

    Ok(KnowledgeStatsDto {
        memory_chunks,
        entities_total,
        entities_by_type,
        relationships: relationships_total,
    })
}

// ===========================================================================
// Provisional-extraction surface (Phase 2 — uncertain-band drawer)
// ===========================================================================

#[derive(Serialize)]
pub struct ProvisionalCandidateDto {
    pub id: String,
    pub canonical_name: String,
    pub entity_type: String,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct ProvisionalDto {
    pub id: String,
    pub entity_name: String,
    pub entity_type: String,
    pub seen_count: i64,
    pub top_score: Option<f64>,
    pub session_id: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub candidates: Vec<ProvisionalCandidateDto>,
}

#[tauri::command]
pub async fn list_provisional(
    limit: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<ProvisionalDto>, String> {
    let limit = limit.unwrap_or(100).clamp(1, 500) as usize;
    let rows = provisional::list_pending(&state.memex_db, limit)
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut cands = Vec::new();
        for cid in &row.candidate_ids {
            // The id is "table:uuid"; the table tells us the entity
            // type. Rather than parse manually, query directly.
            let cand: Option<EntitySummary> = match state
                .memex_db
                .query("SELECT id, name, description, meta::tb(id) AS et FROM type::thing($id)")
                .bind(("id", cid.clone()))
                .await
            {
                Ok(mut res) => res.take(0).ok().and_then(|v: Vec<EntitySummary>| v.into_iter().next()),
                Err(_) => None,
            };
            if let Some(c) = cand {
                cands.push(ProvisionalCandidateDto {
                    id: c.id.to_string(),
                    canonical_name: c.name,
                    entity_type: c.et,
                    description: c.description,
                });
            }
        }
        out.push(ProvisionalDto {
            id: row.id.unwrap_or_default(),
            entity_name: row.entity_name,
            entity_type: row.entity_type,
            seen_count: row.seen_count,
            top_score: row.top_score,
            session_id: row.session_id,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
            candidates: cands,
        });
    }
    Ok(out)
}

#[derive(serde::Deserialize)]
struct EntitySummary {
    id: surrealdb::sql::Thing,
    name: String,
    #[serde(default)]
    description: Option<String>,
    et: String,
}

/// Promote a parked row to one of its candidates. The row gets marked
/// `promoted` and its `entity_name` is appended as an alias on the
/// resolved entity (so future mentions short-circuit).
#[tauri::command]
pub async fn promote_provisional(
    provisional_id: String,
    resolved_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Append the parked entity_name onto the resolved entity's aliases.
    // Cheap: SurrealQL array::union dedupes for us.
    state
        .memex_db
        .query(
            "LET $row = (SELECT entity_name FROM type::thing($pid))[0]; \
             UPDATE type::thing($rid) SET aliases = array::union(aliases, [$row.entity_name]), updated_at = time::now()",
        )
        .bind(("pid", provisional_id.clone()))
        .bind(("rid", resolved_id.clone()))
        .await
        .map_err(|e| e.to_string())?;
    provisional::mark_promoted(&state.memex_db, &provisional_id, &resolved_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn discard_provisional(
    provisional_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    provisional::mark_discarded(&state.memex_db, &provisional_id)
        .await
        .map_err(|e| e.to_string())
}

/// Promote a parked row by *creating* a fresh entity from its name +
/// type rather than matching against a candidate. Used when none of
/// the candidates is right.
#[tauri::command]
pub async fn promote_provisional_as_new(
    provisional_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use std::str::FromStr;
    let embedder = state
        .embedder
        .clone()
        .ok_or_else(|| "embedder unavailable".to_string())?;

    #[derive(serde::Deserialize)]
    struct Row {
        entity_name: String,
        entity_type: String,
    }
    let mut res = state
        .memex_db
        .query("SELECT entity_name, entity_type FROM type::thing($id)")
        .bind(("id", provisional_id.clone()))
        .await
        .map_err(|e| e.to_string())?;
    let rows: Vec<Row> = res.take(0).map_err(|e| e.to_string())?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| "provisional row not found".to_string())?;

    let et = mtypes::EntityType::from_str(&row.entity_type)
        .map_err(|e| format!("unknown entity type: {e}"))?;
    let entity = mtypes::Entity {
        id: None,
        name: row.entity_name.clone(),
        aliases: Vec::new(),
        description: None,
        content: None,
        metadata: Default::default(),
        created_at: None,
        updated_at: None,
    };
    let id = entities::upsert_entity(&state.memex_db, &embedder, &et, &entity)
        .await
        .map_err(|e| e.to_string())?;
    provisional::mark_promoted(&state.memex_db, &provisional_id, &id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn ingest_markdown_folder(
    path: Option<String>,
    state: State<'_, AppState>,
) -> Result<IngestProgress, String> {
    let embedder = state
        .embedder
        .clone()
        .ok_or_else(|| "embedder unavailable — restart with internet to download the model".to_string())?;
    let root = match path.as_deref().map(str::trim) {
        Some("") | None | Some("~") | Some("~/") => dirs::home_dir()
            .ok_or_else(|| "could not resolve home directory".to_string())?,
        Some(p) if p.starts_with("~/") => dirs::home_dir()
            .ok_or_else(|| "could not resolve home directory".to_string())?
            .join(&p[2..]),
        Some(p) => PathBuf::from(p),
    };
    if !root.is_dir() {
        return Err(format!("not a directory: {}", root.display()));
    }
    ingest_folder(state.memex_db.clone(), embedder, root, |_| {}).await
}
