//! Stage 4 of the per-turn pipeline — passive memory extractor.
//!
//! Runs detached, AFTER the main agent's response has finished
//! streaming. Reads the user turn + assistant turn + the current
//! `ConversationContext` (anchor + priorities + asides), pre-fetches
//! likely-relevant entities from the graph, and asks a small Ollama
//! model to emit structured JSON describing entities, relationships,
//! and memories present in the turn. The deterministic
//! [`memory_resolver`] decides whether each entity matches an existing
//! row or warrants a new one. Writes back into the Memex graph + memory
//! store; emits [`StreamEvent`]s as it goes so the UI can show
//! progress.
//!
//! The main agent is no longer given `remember` / `note_entity` /
//! `link_entities` tools — memory is now a side effect of conversation
//! rather than something the main agent has to remember to do.

use std::sync::Arc;

use chrono::Utc;
use futures::StreamExt;
use harness_storage::ConversationContext;
use harness_tools::provisional::{self, ParkRequest};
use memex_core::{
    entities, memories, relationships, EmbeddingService, Entity, EntityType, MemexDb, MemoryChunk,
    RelationType,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::str::FromStr;
use strands_core::model::Model;
use strands_core::types::content::ContentBlock;
use strands_core::types::message::{Message, Role};
use strands_core::types::streaming::{DeltaContent, StreamEvent as CoreStream};
use tracing::{debug, info, warn};

use crate::memory_resolver::{self, Resolution};
use crate::pipeline::events::{EntityResolutionStatus, StreamEvent};

const MAX_PREFETCH_PER_TYPE: usize = 6;
const MAX_TURN_CHARS: usize = 8000;

const SYSTEM_PROMPT: &str = "You extract structured knowledge from a single conversation turn so it can be saved to a personal knowledge graph. The MAIN agent is busy responding to the user; you observe what was said and emit JSON describing entities, relationships, and atomic memories worth keeping.\n\
\n\
Output ONE JSON object and NOTHING ELSE — no prose, no markdown, no code fences. Schema:\n\
\n\
{\n\
  \"entities\": [\n\
    { \"mentioned_as\": <string, the form that appeared in the turn>,\n\
      \"type\": <one of: person, organization, project, technology, topic, location, component>,\n\
      \"description\": <optional one-line description, omitted if not clear from turn> }\n\
  ],\n\
  \"relationships\": [\n\
    { \"from\": <mentioned_as of an entity above OR an existing-graph name>,\n\
      \"to\":   <same>,\n\
      \"type\": <one of: works_at, part_of, works_on, uses_tech, knows_about, related_to, mentions> }\n\
  ],\n\
  \"inferred_relationships\": [\n\
    { \"from\": <name>, \"to\": <name>, \"type\": <relation>,\n\
      \"reason\": <short why-string — e.g. \"Person P works at Org O and works on Project X, so Project X likely belongs to Org O\"> }\n\
  ],\n\
  \"memories\": [\n\
    { \"content\": <atomic fact, one or two sentences>, \"summary\": <optional short label> }\n\
  ]\n\
}\n\
\n\
Rules:\n\
- Prefer matching `mentioned_as` to an entity in the EXISTING ENTITIES list when it clearly refers to the same thing — don't invent variant spellings.\n\
- Type-pick the most specific category that fits. Use `component` for sub-products inside a parent project.\n\
- `relationships` are facts EXPLICITLY stated or directly implied by the turn (\"Person P works at Org O\").\n\
- `inferred_relationships` are reasonable structural deductions across what's already known + what's just been said. Examples: someone working AT an org and working ON a project usually means the project belongs to the org. A project using a technology that's part of a vendor — link the vendor too. Be conservative — only emit inferences with clear evidential support.\n\
- Memories should be atomic — one fact per object, salient enough to be worth recalling later. Skip greetings, banter, the model's own reasoning.\n\
- If nothing in the turn is worth extracting, return empty arrays.\n\
- NEVER wrap the JSON in markdown or commentary.\n";

pub struct ExtractRequest {
    pub model: Arc<dyn Model>,
    pub memex_db: Arc<MemexDb>,
    pub embedder: Arc<EmbeddingService>,
    pub conv_context: ConversationContext,
    pub user_turn: String,
    pub assistant_turn: String,
    pub session_id: String,
    pub emit: Arc<dyn Fn(StreamEvent) + Send + Sync>,
}

#[derive(Debug, Default)]
pub struct ExtractOutcome {
    pub entities: u32,
    pub relationships: u32,
    pub memories: u32,
}

pub async fn extract(req: ExtractRequest) -> Result<ExtractOutcome, String> {
    (req.emit)(StreamEvent::MemoryExtractionStarted {
        session_id: req.session_id.clone(),
    });

    let candidates = prefetch_candidates(&req).await;
    let prompt = render_user_message(&req, &candidates);

    let messages = vec![Message {
        role: Role::User,
        content: vec![ContentBlock::Text { text: prompt }],
    }];

    let mut stream = match req.model.stream(&messages, Some(SYSTEM_PROMPT), &[]).await {
        Ok(s) => s,
        Err(e) => {
            warn!("memory extractor: open stream: {e}");
            (req.emit)(StreamEvent::MemoryExtractionDone {
                entities: 0,
                relationships: 0,
                memories: 0,
            });
            return Err(e.to_string());
        }
    };

    let mut accumulated = String::new();
    while let Some(next) = stream.next().await {
        match next {
            Ok(CoreStream::ContentBlockDelta {
                delta: DeltaContent::TextDelta(t),
                ..
            }) => accumulated.push_str(&t),
            Ok(_) => {}
            Err(e) => {
                warn!("memory extractor: stream error: {e}");
                break;
            }
        }
    }

    let parsed = match parse_extraction(&accumulated) {
        Some(p) => p,
        None => {
            warn!(
                "memory extractor: failed to parse JSON output. Raw: {}",
                truncate_for_log(&accumulated)
            );
            (req.emit)(StreamEvent::MemoryExtractionDone {
                entities: 0,
                relationships: 0,
                memories: 0,
            });
            return Err("extractor returned unparseable output".into());
        }
    };

    debug!(
        "memory extractor: parsed {} entities, {} relationships, {} memories",
        parsed.entities.len(),
        parsed.relationships.len(),
        parsed.memories.len()
    );

    let outcome = commit(&req, parsed).await;
    (req.emit)(StreamEvent::MemoryExtractionDone {
        entities: outcome.entities,
        relationships: outcome.relationships,
        memories: outcome.memories,
    });
    info!(
        "memory extractor done for session {} ({} entities, {} relationships, {} memories)",
        req.session_id, outcome.entities, outcome.relationships, outcome.memories
    );
    Ok(outcome)
}

// ---------------------------------------------------------------------------
// Pre-fetch — give the extractor visibility into the graph it might match
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct CandidateEntity {
    name: String,
    aliases: Vec<String>,
    entity_type: EntityType,
    description: Option<String>,
}

async fn prefetch_candidates(req: &ExtractRequest) -> Vec<CandidateEntity> {
    let mut out = Vec::new();

    // 1. KNN over the combined turn embedding — pulls semantically
    //    related entities even when the name doesn't appear verbatim.
    let combined = format!("{}\n{}", req.user_turn, req.assistant_turn);
    if let Ok(target_vec) = req.embedder.embed_text(&combined).await {
        for et in EntityType::all() {
            match knn_for_type(&req.memex_db, et, &target_vec, MAX_PREFETCH_PER_TYPE).await {
                Ok(rows) => out.extend(rows),
                Err(e) => warn!("memory extractor: KNN for {} failed: {e}", et.table_name()),
            }
        }
    }

    // 2. Anchor + priorities + asides — high-value targets the user has
    //    already curated (or that the anchor agent chose to pin).
    let mut named: Vec<String> = Vec::new();
    if let Some(a) = req.conv_context.anchor.as_ref() {
        named.push(a.clone());
    }
    for p in &req.conv_context.priorities {
        named.push(p.text.clone());
    }
    for a in &req.conv_context.asides {
        named.push(a.text.clone());
    }
    for label in named {
        // For each context card, pick out token-y words and look them up
        // against the graph so the extractor sees those rows too. Cheap.
        for chunk in label.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-') {
            let word = chunk.trim();
            if word.len() < 3 {
                continue;
            }
            if let Ok(Some((et, ent))) = entities::find_entity_any_type(&req.memex_db, word).await {
                if !out.iter().any(|c| c.name == ent.name) {
                    out.push(CandidateEntity {
                        name: ent.name.clone(),
                        aliases: ent.aliases.clone(),
                        entity_type: et,
                        description: ent.description.clone(),
                    });
                }
            }
        }
    }

    out
}

async fn knn_for_type(
    db: &MemexDb,
    et: &EntityType,
    target_vec: &[f32],
    limit: usize,
) -> Result<Vec<CandidateEntity>, memex_core::Error> {
    #[derive(Deserialize)]
    struct Row {
        name: String,
        #[serde(default)]
        aliases: Vec<String>,
        #[serde(default)]
        description: Option<String>,
    }
    // KNN K must be a literal in SurrealDB 2.x.
    let table = et.table_name();
    let q = format!(
        "SELECT name, aliases, description \
         FROM {table} WHERE archived != true AND embedding != NONE AND embedding <|{limit}|> $emb"
    );
    let mut res = db
        .query(&q)
        .bind(("emb", target_vec.to_vec()))
        .await
        .map_err(|e| memex_core::Error::Db(e.to_string()))?;
    let rows: Vec<Row> = res
        .take(0)
        .map_err(|e| memex_core::Error::Db(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| CandidateEntity {
            name: r.name,
            aliases: r.aliases,
            entity_type: et.clone(),
            description: r.description,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Prompt rendering
// ---------------------------------------------------------------------------

fn render_user_message(req: &ExtractRequest, candidates: &[CandidateEntity]) -> String {
    let mut s = String::new();
    s.push_str("EXISTING ENTITIES (match `mentioned_as` to one of these when it refers to the same thing — don't create duplicates):\n");
    if candidates.is_empty() {
        s.push_str("(graph is empty — every entity below will be new)\n");
    } else {
        for c in candidates {
            s.push_str(&format!(
                "- [{}] {}{}{}\n",
                c.entity_type.table_name(),
                c.name,
                if c.aliases.is_empty() {
                    String::new()
                } else {
                    format!(" (aka: {})", c.aliases.join(", "))
                },
                c.description
                    .as_ref()
                    .map(|d| format!(" — {d}"))
                    .unwrap_or_default()
            ));
        }
    }

    s.push_str("\nCONVERSATION CONTEXT:\n");
    if let Some(a) = req.conv_context.anchor.as_ref() {
        s.push_str(&format!("anchor: {a}\n"));
    }
    for p in &req.conv_context.priorities {
        s.push_str(&format!("priority: {}\n", p.text));
    }

    s.push_str("\nUSER TURN:\n");
    s.push_str(truncate(&req.user_turn, MAX_TURN_CHARS));
    s.push_str("\n\nASSISTANT TURN:\n");
    s.push_str(truncate(&req.assistant_turn, MAX_TURN_CHARS));
    s.push_str("\n\nReturn the JSON object.\n");
    s
}

// ---------------------------------------------------------------------------
// JSON parsing (tolerant — strips ```json fences if the model sneaks them in)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct ParsedExtraction {
    #[serde(default)]
    entities: Vec<ParsedEntity>,
    #[serde(default)]
    relationships: Vec<ParsedRelationship>,
    #[serde(default)]
    inferred_relationships: Vec<ParsedRelationship>,
    #[serde(default)]
    memories: Vec<ParsedMemory>,
}

#[derive(Debug, Deserialize)]
struct ParsedEntity {
    mentioned_as: String,
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ParsedRelationship {
    from: String,
    to: String,
    #[serde(rename = "type")]
    rel_type: String,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ParsedMemory {
    content: String,
    #[serde(default)]
    summary: Option<String>,
}

fn parse_extraction(raw: &str) -> Option<ParsedExtraction> {
    let trimmed = raw.trim();
    let stripped = strip_fences(trimmed);
    // Some models add a preamble — find the first `{` and the matching `}`.
    let start = stripped.find('{')?;
    let end = stripped.rfind('}')?;
    if end <= start {
        return None;
    }
    let json = &stripped[start..=end];
    serde_json::from_str(json).ok()
}

fn strip_fences(s: &str) -> &str {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```json") {
        let r = rest.trim_start_matches('\n');
        return r.strip_suffix("```").map(str::trim).unwrap_or(r);
    }
    if let Some(rest) = t.strip_prefix("```") {
        let r = rest.trim_start_matches('\n');
        return r.strip_suffix("```").map(str::trim).unwrap_or(r);
    }
    t
}

// ---------------------------------------------------------------------------
// Commit — resolve, write, emit
// ---------------------------------------------------------------------------

async fn commit(req: &ExtractRequest, parsed: ParsedExtraction) -> ExtractOutcome {
    let mut outcome = ExtractOutcome::default();

    // mentioned_as -> resolved (table:id, canonical name) — used by the
    // relationship pass to look up ids without a second model call.
    let mut resolved_map: HashMap<String, (String, String)> = HashMap::new();

    for ent in parsed.entities {
        let entity_type = match EntityType::from_str(&ent.entity_type) {
            Ok(t) => t,
            Err(_) => {
                warn!(
                    "memory extractor: skipping entity '{}' with unknown type '{}'",
                    ent.mentioned_as, ent.entity_type
                );
                continue;
            }
        };
        let name = ent.mentioned_as.trim();
        if name.is_empty() {
            continue;
        }

        let resolution = match memory_resolver::resolve(
            &req.memex_db,
            Some(&req.embedder),
            name,
            &entity_type,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("memory extractor: resolve '{name}' failed: {e}");
                continue;
            }
        };

        match resolution {
            Resolution::Existing {
                id,
                canonical_name,
                ..
            } => {
                // Index the resolved id under both the input form (so
                // relationships referencing the model's spelling work)
                // and the canonical name (so relationships referencing
                // the existing graph spelling work).
                resolved_map.insert(name.to_string(), (id.clone(), canonical_name.clone()));
                resolved_map.insert(canonical_name.clone(), (id.clone(), canonical_name.clone()));
                // Promote any pending provisional row that was waiting
                // for evidence on this name+type — we just confidently
                // resolved the same name elsewhere, so the wait is over.
                if let Err(e) = promote_pending_for(
                    &req.memex_db,
                    name,
                    entity_type.table_name(),
                    &id,
                )
                .await
                {
                    warn!("memory extractor: promote pending failed: {e}");
                }
                (req.emit)(StreamEvent::EntityResolved {
                    name: canonical_name,
                    entity_type: entity_type.table_name().to_string(),
                    status: EntityResolutionStatus::Matched,
                });
                outcome.entities += 1;
            }
            Resolution::Uncertain {
                candidates,
                top_score,
            } => {
                // Park the extraction. If the same (name, type) has
                // already been parked once before, `park` bumps
                // seen_count → and the post-pass below will promote.
                let candidate_ids: Vec<String> =
                    candidates.iter().map(|c| c.id.clone()).collect();
                let req_park = ParkRequest {
                    entity_name: name,
                    entity_type: entity_type.table_name(),
                    candidate_ids: candidate_ids.clone(),
                    context_signature: None,
                    session_id: &req.session_id,
                    top_score: Some(top_score as f64),
                };
                match provisional::park(&req.memex_db, req_park).await {
                    Ok(_) => {
                        debug!(
                            "memory extractor: parked '{name}' as uncertain (top={:.3}, candidates={})",
                            top_score,
                            candidate_ids.len()
                        );
                    }
                    Err(e) => warn!("memory extractor: park '{name}' failed: {e}"),
                }
                // Don't index this name in resolved_map — relationships
                // referencing it will fall through to the per-name DB
                // lookup, which won't find anything either, so they'll
                // be skipped. That's the desired behaviour: don't link
                // to a candidate we're not confident about.
            }
            Resolution::New => {
                let entity = Entity {
                    id: None,
                    name: name.to_string(),
                    aliases: Vec::new(),
                    description: ent.description.clone(),
                    content: None,
                    metadata: HashMap::new(),
                    created_at: None,
                    updated_at: None,
                };
                match entities::upsert_entity(&req.memex_db, &req.embedder, &entity_type, &entity)
                    .await
                {
                    Ok(id) => {
                        resolved_map.insert(name.to_string(), (id, name.to_string()));
                        (req.emit)(StreamEvent::EntityResolved {
                            name: name.to_string(),
                            entity_type: entity_type.table_name().to_string(),
                            status: EntityResolutionStatus::Created,
                        });
                        outcome.entities += 1;
                    }
                    Err(e) => warn!("memory extractor: upsert '{name}' failed: {e}"),
                }
            }
        }
    }

    // Post-pass: any provisional rows that hit seen_count >= 2 (and
    // weren't already promoted above by name match) get promoted now
    // to their top candidate. The "second mention" rule is the v1
    // signal — refined to context-overlap matching in a later phase.
    if let Err(e) = promote_repeat_offenders(&req.memex_db).await {
        warn!("memory extractor: promote-repeat pass failed: {e}");
    }
    // Populate a context_signature on the still-pending rows from this
    // turn, so the next turn's logic has something to overlap against.
    if let Err(e) = stamp_signature_for_session(
        &req.memex_db,
        &req.session_id,
        &collect_signature(&resolved_map),
    )
    .await
    {
        warn!("memory extractor: stamp signature failed: {e}");
    }

    for rel in parsed.relationships {
        if commit_relationship(req, &resolved_map, &rel, false).await {
            outcome.relationships += 1;
        }
    }
    for rel in parsed.inferred_relationships {
        if commit_relationship(req, &resolved_map, &rel, true).await {
            outcome.relationships += 1;
        }
    }

    for mem in parsed.memories {
        let content = mem.content.trim();
        if content.is_empty() {
            continue;
        }
        let chunk = MemoryChunk {
            id: None,
            content: content.to_string(),
            summary: mem.summary.clone(),
            source_type: "chat".to_string(),
            source_id: Some(req.session_id.clone()),
            source_path: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };
        match memories::insert_memory(&req.memex_db, &req.embedder, &chunk).await {
            Ok(_) => {
                (req.emit)(StreamEvent::MemoryStored {
                    content_preview: preview(content, 80),
                });
                outcome.memories += 1;
            }
            Err(e) => warn!("memory extractor: insert memory failed: {e}"),
        }
    }

    outcome
}

/// Commit one relationship row. Returns true on success. `inferred`
/// rows get `metadata.inferred = true` + `metadata.reason = <model
/// reason>`; the Knowledge UI can render those greyed-out.
async fn commit_relationship(
    req: &ExtractRequest,
    resolved_map: &HashMap<String, (String, String)>,
    rel: &ParsedRelationship,
    inferred: bool,
) -> bool {
    let rel_type = match RelationType::from_str(&rel.rel_type) {
        Ok(r) => r,
        Err(_) => {
            warn!(
                "memory extractor: skipping relationship with unknown type '{}'",
                rel.rel_type
            );
            return false;
        }
    };

    let from_id = match resolve_id_by_name(&req.memex_db, resolved_map, &rel.from).await {
        Some(id) => id,
        None => {
            warn!(
                "memory extractor: relationship from '{}' not resolvable; skipping",
                rel.from
            );
            return false;
        }
    };
    let to_id = match resolve_id_by_name(&req.memex_db, resolved_map, &rel.to).await {
        Some(id) => id,
        None => {
            warn!(
                "memory extractor: relationship to '{}' not resolvable; skipping",
                rel.to
            );
            return false;
        }
    };

    let mut metadata: HashMap<String, Value> = HashMap::new();
    if inferred {
        metadata.insert("inferred".to_string(), Value::Bool(true));
        if let Some(reason) = rel.reason.as_ref() {
            metadata.insert("reason".to_string(), Value::String(reason.clone()));
        }
    }

    match relationships::create_relationship(&req.memex_db, &rel_type, &from_id, &to_id, &metadata)
        .await
    {
        Ok(_) => {
            (req.emit)(StreamEvent::RelationshipCreated {
                from_name: rel.from.clone(),
                to_name: rel.to.clone(),
                relation: rel_type.table_name().to_string(),
            });
            true
        }
        Err(e) => {
            warn!(
                "memory extractor: link {} -> {} ({}) failed: {e}",
                rel.from,
                rel.to,
                rel_type.table_name()
            );
            false
        }
    }
}

/// Build a turn-level signature: a sorted, deduplicated, comma-joined
/// list of every confidently-resolved entity id this turn produced.
/// Used to disambiguate uncertain extractions across turns — two
/// mentions of the same name are more likely the same entity if they
/// share other entities in their respective turns.
fn collect_signature(resolved_map: &HashMap<String, (String, String)>) -> String {
    let unique: BTreeSet<&str> = resolved_map.values().map(|(id, _)| id.as_str()).collect();
    unique.into_iter().collect::<Vec<_>>().join(",")
}

/// If a pending provisional row exists for `(name, entity_type)`,
/// promote it to `resolved_id`. We just confidently resolved that
/// name in this turn so the wait is over.
async fn promote_pending_for(
    db: &Arc<MemexDb>,
    name: &str,
    entity_type: &str,
    resolved_id: &str,
) -> Result<(), memex_core::Error> {
    let rows = provisional::find_pending_for_name(db, name, entity_type).await?;
    for row in rows {
        if let Some(id) = row.id.as_ref() {
            provisional::mark_promoted(db, id, resolved_id).await?;
            tracing::info!(
                "memory extractor: promoted provisional '{}' ({}) -> {}",
                row.entity_name,
                entity_type,
                resolved_id
            );
        }
    }
    Ok(())
}

/// Promote any pending provisional row whose `seen_count` has hit 2 or
/// more (i.e. the same name+type has been parked twice) by linking it
/// to its top candidate. The second mention is treated as enough
/// evidence to commit; finer context-overlap heuristics live in a
/// later phase.
async fn promote_repeat_offenders(db: &Arc<MemexDb>) -> Result<(), memex_core::Error> {
    let pending = provisional::list_pending(db, 200).await?;
    for row in pending {
        if row.seen_count < 2 {
            continue;
        }
        let Some(top) = row.candidate_ids.first().cloned() else {
            continue;
        };
        if let Some(id) = row.id.as_ref() {
            provisional::mark_promoted(db, id, &top).await?;
            tracing::info!(
                "memory extractor: promoted repeat-offender '{}' ({}) -> {} after {} mentions",
                row.entity_name,
                row.entity_type,
                top,
                row.seen_count
            );
        }
    }
    Ok(())
}

/// Stamp this turn's signature onto every still-pending row that
/// belongs to the same session, so the *next* turn that lands here can
/// make a context-overlap decision.
async fn stamp_signature_for_session(
    db: &Arc<MemexDb>,
    session_id: &str,
    signature: &str,
) -> Result<(), memex_core::Error> {
    if signature.is_empty() {
        return Ok(());
    }
    db.query(
        "UPDATE provisional_extraction SET context_signature = $sig \
         WHERE status = 'pending' AND session_id = $session AND (context_signature = NONE OR context_signature = '')",
    )
    .bind(("sig", signature.to_string()))
    .bind(("session", session_id.to_string()))
    .await
    .map_err(|e| memex_core::Error::Db(e.to_string()))?;
    Ok(())
}

async fn resolve_id_by_name(
    db: &MemexDb,
    resolved_map: &HashMap<String, (String, String)>,
    name: &str,
) -> Option<String> {
    if let Some((id, _)) = resolved_map.get(name) {
        return Some(id.clone());
    }
    // Fallback: maybe the relationship references an entity that
    // already existed in the graph and the model didn't bother
    // re-listing it under "entities". Try a direct lookup.
    match entities::find_entity_any_type(db, name).await {
        Ok(Some((_, e))) => e.id,
        _ => None,
    }
}

fn truncate(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    let mut end = max_chars;
    while !s.is_char_boundary(end) && end > 0 {
        end -= 1;
    }
    &s[..end]
}

fn preview(s: &str, max_chars: usize) -> String {
    let truncated = truncate(s, max_chars);
    if truncated.len() < s.len() {
        format!("{truncated}…")
    } else {
        truncated.to_string()
    }
}

fn truncate_for_log(s: &str) -> String {
    let snippet = truncate(s, 200);
    snippet.replace('\n', " ").trim().to_string()
}
