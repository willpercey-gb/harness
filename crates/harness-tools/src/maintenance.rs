//! Periodic graph-health pass for the Memex DB. Runs once an hour from
//! a tokio task spawned at app boot. Surfaces near-duplicate suspect
//! pairs and flags zero-relationship orphans, but never auto-merges —
//! everything ends up in the provisional drawer for human-in-the-loop
//! review.

use std::sync::Arc;

use memex_core::{EntityType, MemexDb};
use serde::Deserialize;
use tracing::{debug, info, warn};

/// Minimum cosine similarity between two entities of the same type
/// before they're flagged as a potential duplicate.
const DUPE_SIMILARITY_THRESHOLD: f32 = 0.92;
/// Minimum number of mentions before a zero-rel entity is treated as
/// suspicious. One-off mentions don't get flagged.
const ORPHAN_MENTION_FLOOR: i64 = 3;

#[derive(Debug, Default)]
pub struct MaintenanceReport {
    pub duplicate_pairs: usize,
    pub orphan_entities: usize,
}

/// Run a single maintenance pass. Idempotent — safe to call on app
/// boot, on a timer, or via a manual trigger.
pub async fn run_once(db: &Arc<MemexDb>) -> Result<MaintenanceReport, memex_core::Error> {
    info!("memex maintenance: starting pass");
    let dupes = scan_duplicate_pairs(db).await?;
    let orphans = scan_orphan_entities(db).await?;
    info!(
        "memex maintenance: done — {} duplicate pairs flagged, {} orphan entities",
        dupes, orphans
    );
    Ok(MaintenanceReport {
        duplicate_pairs: dupes,
        orphan_entities: orphans,
    })
}

async fn scan_duplicate_pairs(db: &Arc<MemexDb>) -> Result<usize, memex_core::Error> {
    let mut total = 0usize;
    for et in EntityType::all() {
        match scan_duplicates_for_type(db, et).await {
            Ok(n) => total += n,
            Err(e) => warn!("maintenance: dup scan for {} failed: {e}", et.table_name()),
        }
    }
    Ok(total)
}

async fn scan_duplicates_for_type(
    db: &Arc<MemexDb>,
    et: &EntityType,
) -> Result<usize, memex_core::Error> {
    #[derive(Deserialize)]
    struct Row {
        id: surrealdb::sql::Thing,
        name: String,
        #[serde(default)]
        embedding: Option<Vec<f32>>,
    }
    let table = et.table_name();
    let mut res = db
        .query(format!(
            "SELECT id, name, embedding FROM {table} \
             WHERE archived != true AND embedding != NONE LIMIT 500"
        ))
        .await
        .map_err(|e| memex_core::Error::Db(e.to_string()))?;
    let rows: Vec<Row> = res
        .take(0)
        .map_err(|e| memex_core::Error::Db(e.to_string()))?;
    let mut flagged = 0usize;
    for i in 0..rows.len() {
        for j in (i + 1)..rows.len() {
            let (Some(a), Some(b)) = (&rows[i].embedding, &rows[j].embedding) else {
                continue;
            };
            let score = cosine(a, b);
            if score >= DUPE_SIMILARITY_THRESHOLD {
                debug!(
                    "maintenance: flagged duplicate {} <-> {} ({:.3})",
                    rows[i].name, rows[j].name, score
                );
                if let Err(e) = upsert_duplicate_suspect(
                    db,
                    et.table_name(),
                    &rows[i].id.to_string(),
                    &rows[i].name,
                    &rows[j].id.to_string(),
                    &rows[j].name,
                    score as f64,
                )
                .await
                {
                    warn!("maintenance: persist dup pair failed: {e}");
                }
                flagged += 1;
            }
        }
    }
    Ok(flagged)
}

async fn scan_orphan_entities(db: &Arc<MemexDb>) -> Result<usize, memex_core::Error> {
    // For each type, count entities with access_count >= floor and no
    // incoming/outgoing edges. We keep this lightweight — a single
    // SurrealQL pass per type rather than one query per entity.
    let mut total = 0usize;
    for et in EntityType::all() {
        let table = et.table_name();
        let q = format!(
            "SELECT id, name, access_count FROM {table} \
             WHERE archived != true AND access_count >= $floor \
             AND count((SELECT * FROM works_at, part_of, works_on, uses_tech, knows_about, related_to, mentions WHERE in = $parent.id OR out = $parent.id)) = 0"
        );
        match db
            .query(&q)
            .bind(("floor", ORPHAN_MENTION_FLOOR))
            .await
            .and_then(|mut r| r.take::<Vec<serde_json::Value>>(0))
        {
            Ok(rows) => total += rows.len(),
            Err(e) => debug!("maintenance: orphan scan {table} skipped: {e}"),
        }
    }
    Ok(total)
}

async fn upsert_duplicate_suspect(
    db: &Arc<MemexDb>,
    entity_type: &str,
    a_id: &str,
    a_name: &str,
    b_id: &str,
    b_name: &str,
    score: f64,
) -> Result<(), memex_core::Error> {
    // Order ids so (a,b) and (b,a) collapse to the same row.
    let (left, left_name, right, right_name) = if a_id < b_id {
        (a_id, a_name, b_id, b_name)
    } else {
        (b_id, b_name, a_id, a_name)
    };
    db.query(
        "UPSERT duplicate_suspect SET \
         entity_type = $etype, \
         left_id = $left, left_name = $lname, \
         right_id = $right, right_name = $rname, \
         score = $score, \
         flagged_at = time::now() \
         WHERE left_id = $left AND right_id = $right",
    )
    .bind(("etype", entity_type.to_string()))
    .bind(("left", left.to_string()))
    .bind(("lname", left_name.to_string()))
    .bind(("right", right.to_string()))
    .bind(("rname", right_name.to_string()))
    .bind(("score", score))
    .await
    .map_err(|e| memex_core::Error::Db(e.to_string()))?;
    Ok(())
}

pub const SCHEMA_EXTENSION: &str = r#"
DEFINE TABLE IF NOT EXISTS duplicate_suspect SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS entity_type ON duplicate_suspect TYPE string;
DEFINE FIELD IF NOT EXISTS left_id     ON duplicate_suspect TYPE string;
DEFINE FIELD IF NOT EXISTS left_name   ON duplicate_suspect TYPE string;
DEFINE FIELD IF NOT EXISTS right_id    ON duplicate_suspect TYPE string;
DEFINE FIELD IF NOT EXISTS right_name  ON duplicate_suspect TYPE string;
DEFINE FIELD IF NOT EXISTS score       ON duplicate_suspect TYPE float;
DEFINE FIELD IF NOT EXISTS flagged_at  ON duplicate_suspect TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS resolved_at ON duplicate_suspect TYPE option<datetime>;
DEFINE INDEX IF NOT EXISTS idx_dup_pair ON duplicate_suspect FIELDS left_id, right_id;
"#;

pub async fn apply_schema(db: &MemexDb) -> Result<(), memex_core::Error> {
    db.query(SCHEMA_EXTENSION)
        .await
        .map_err(|e| memex_core::Error::Db(e.to_string()))?;
    Ok(())
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut na = 0.0_f32;
    let mut nb = 0.0_f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}
