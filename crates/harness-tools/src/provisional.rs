//! Provisional-extraction buffer for the passive memory pipeline.
//!
//! When the resolver lands in the uncertain band (embedding similarity
//! 0.75–0.85) it can't confidently merge but shouldn't create a
//! duplicate either. Those candidate extractions get parked here for a
//! few turns; if the same name shows up again with overlapping
//! conversational context, we promote the merge — otherwise we drop
//! the row after `MAX_AGE_TURNS`.
//!
//! Lives in the memex-db (alongside entities) so candidate IDs are
//! locally referenceable. Schema is applied on top of memex-core's own
//! schema after `init_memex_db`.
//!
//! Status flow: `pending` (just parked) → `promoted` (merged into a
//! candidate; we keep the row briefly so the UI can show what was
//! merged) → eventually deleted by the maintenance pass.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use memex_core::MemexDb;
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

pub const SCHEMA_EXTENSION: &str = r#"
DEFINE TABLE IF NOT EXISTS provisional_extraction SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS entity_name        ON provisional_extraction TYPE string;
DEFINE FIELD IF NOT EXISTS entity_type        ON provisional_extraction TYPE string;
-- Existing-entity ids the resolver thought might match (table:uuid).
DEFINE FIELD IF NOT EXISTS candidate_ids      ON provisional_extraction TYPE array<string> DEFAULT [];
-- Sorted, comma-joined hash of every other resolved entity id seen in
-- the same source turn. Used to disambiguate via context overlap on
-- the next turn that mentions the same name.
DEFINE FIELD IF NOT EXISTS context_signature  ON provisional_extraction TYPE option<string>;
DEFINE FIELD IF NOT EXISTS session_id         ON provisional_extraction TYPE string;
DEFINE FIELD IF NOT EXISTS top_score          ON provisional_extraction TYPE option<float>;
DEFINE FIELD IF NOT EXISTS seen_count         ON provisional_extraction TYPE int DEFAULT 1;
DEFINE FIELD IF NOT EXISTS status             ON provisional_extraction TYPE string ASSERT $value INSIDE ['pending', 'promoted', 'discarded'] DEFAULT 'pending';
DEFINE FIELD IF NOT EXISTS resolved_id        ON provisional_extraction TYPE option<string>;
DEFINE FIELD IF NOT EXISTS first_seen_at      ON provisional_extraction TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS last_seen_at       ON provisional_extraction TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS idx_prov_name      ON provisional_extraction FIELDS entity_name, entity_type;
DEFINE INDEX IF NOT EXISTS idx_prov_session   ON provisional_extraction FIELDS session_id;
DEFINE INDEX IF NOT EXISTS idx_prov_status    ON provisional_extraction FIELDS status;
"#;

/// Apply the harness-specific extension schema on top of memex-core's.
/// Idempotent — safe to call on every boot.
pub async fn apply_schema(db: &MemexDb) -> Result<(), memex_core::Error> {
    db.query(SCHEMA_EXTENSION)
        .await
        .map_err(|e| memex_core::Error::Db(e.to_string()))?;
    Ok(())
}

/// One parked extraction. The DB carries an `id` Thing on read; we
/// flatten it to the string form for downstream consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionalExtraction {
    #[serde(default)]
    pub id: Option<String>,
    pub entity_name: String,
    pub entity_type: String,
    #[serde(default)]
    pub candidate_ids: Vec<String>,
    #[serde(default)]
    pub context_signature: Option<String>,
    pub session_id: String,
    #[serde(default)]
    pub top_score: Option<f64>,
    #[serde(default = "default_seen_count")]
    pub seen_count: i64,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub resolved_id: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

fn default_seen_count() -> i64 {
    1
}
fn default_status() -> String {
    "pending".to_string()
}

#[derive(Debug, Clone)]
pub struct ParkRequest<'a> {
    pub entity_name: &'a str,
    pub entity_type: &'a str,
    pub candidate_ids: Vec<String>,
    pub context_signature: Option<String>,
    pub session_id: &'a str,
    pub top_score: Option<f64>,
}

/// Insert (or bump) a provisional extraction. If a `pending` row
/// already exists for the same `(entity_name, entity_type)`, increment
/// `seen_count`, refresh `last_seen_at`, and union the candidate ids /
/// merge the latest context signature in.
pub async fn park(
    db: &Arc<MemexDb>,
    req: ParkRequest<'_>,
) -> Result<String, memex_core::Error> {
    #[derive(Deserialize)]
    struct Row {
        id: Thing,
        candidate_ids: Vec<String>,
    }

    let mut existing: Vec<Row> = db
        .query(
            "SELECT id, candidate_ids FROM provisional_extraction \
             WHERE status = 'pending' AND entity_name = $name AND entity_type = $etype LIMIT 1",
        )
        .bind(("name", req.entity_name.to_string()))
        .bind(("etype", req.entity_type.to_string()))
        .await
        .map_err(db_err)?
        .take(0)
        .map_err(db_err)?;

    if let Some(row) = existing.pop() {
        let id_str = row.id.to_string();
        let mut union: Vec<String> = row.candidate_ids;
        for c in &req.candidate_ids {
            if !union.contains(c) {
                union.push(c.clone());
            }
        }
        db.query(
            "UPDATE type::thing($id) SET \
             seen_count = seen_count + 1, \
             last_seen_at = time::now(), \
             context_signature = $sig, \
             candidate_ids = $cands, \
             top_score = $score",
        )
        .bind(("id", id_str.clone()))
        .bind(("sig", req.context_signature.clone()))
        .bind(("cands", union))
        .bind(("score", req.top_score))
        .await
        .map_err(db_err)?;
        return Ok(id_str);
    }

    #[derive(Deserialize)]
    struct CreatedRow {
        id: Thing,
    }
    let created: Vec<CreatedRow> = db
        .query(
            "CREATE provisional_extraction SET \
             entity_name = $name, \
             entity_type = $etype, \
             candidate_ids = $cands, \
             context_signature = $sig, \
             session_id = $session, \
             top_score = $score",
        )
        .bind(("name", req.entity_name.to_string()))
        .bind(("etype", req.entity_type.to_string()))
        .bind(("cands", req.candidate_ids.clone()))
        .bind(("sig", req.context_signature.clone()))
        .bind(("session", req.session_id.to_string()))
        .bind(("score", req.top_score))
        .await
        .map_err(db_err)?
        .take(0)
        .map_err(db_err)?;
    created
        .into_iter()
        .next()
        .map(|r| r.id.to_string())
        .ok_or_else(|| memex_core::Error::Db("CREATE provisional_extraction returned no row".into()))
}

/// Find pending rows whose normalised `entity_name` matches `name`.
/// Caller normalises both sides — we don't reapply normalisation here
/// to keep the function logic-free.
pub async fn find_pending_for_name(
    db: &Arc<MemexDb>,
    name: &str,
    entity_type: &str,
) -> Result<Vec<ProvisionalExtraction>, memex_core::Error> {
    let rows: Vec<RawRow> = db
        .query(
            "SELECT * FROM provisional_extraction \
             WHERE status = 'pending' AND entity_name = $name AND entity_type = $etype",
        )
        .bind(("name", name.to_string()))
        .bind(("etype", entity_type.to_string()))
        .await
        .map_err(db_err)?
        .take(0)
        .map_err(db_err)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn list_pending(
    db: &Arc<MemexDb>,
    limit: usize,
) -> Result<Vec<ProvisionalExtraction>, memex_core::Error> {
    let rows: Vec<RawRow> = db
        .query(
            "SELECT * FROM provisional_extraction \
             WHERE status = 'pending' ORDER BY last_seen_at DESC LIMIT $limit",
        )
        .bind(("limit", limit as i64))
        .await
        .map_err(db_err)?
        .take(0)
        .map_err(db_err)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn mark_promoted(
    db: &Arc<MemexDb>,
    id: &str,
    resolved_id: &str,
) -> Result<(), memex_core::Error> {
    db.query(
        "UPDATE type::thing($id) SET status = 'promoted', resolved_id = $resolved, last_seen_at = time::now()",
    )
    .bind(("id", id.to_string()))
    .bind(("resolved", resolved_id.to_string()))
    .await
    .map_err(db_err)?;
    Ok(())
}

pub async fn mark_discarded(db: &Arc<MemexDb>, id: &str) -> Result<(), memex_core::Error> {
    db.query("UPDATE type::thing($id) SET status = 'discarded', last_seen_at = time::now()")
        .bind(("id", id.to_string()))
        .await
        .map_err(db_err)?;
    Ok(())
}

/// Drop pending rows older than `max_age_hours` that haven't been
/// touched. Phase-2 cleanup cadence is the chat session itself —
/// for v1 this is invoked nowhere automatic; future maintenance pass.
pub async fn prune_stale(
    db: &Arc<MemexDb>,
    max_age_hours: i64,
) -> Result<usize, memex_core::Error> {
    #[derive(Deserialize)]
    struct R {
        c: Option<i64>,
    }
    let cutoff = Utc::now() - chrono::Duration::hours(max_age_hours);
    let mut res = db
        .query(
            "DELETE FROM provisional_extraction WHERE status = 'pending' AND last_seen_at < $cutoff RETURN BEFORE; \
             SELECT count() AS c FROM $before GROUP ALL",
        )
        .bind(("cutoff", cutoff))
        .await
        .map_err(db_err)?;
    let _ : Vec<serde_json::Value> = res.take(0).unwrap_or_default();
    let counts: Vec<R> = res.take(1).unwrap_or_default();
    Ok(counts.into_iter().next().and_then(|r| r.c).unwrap_or(0) as usize)
}

#[derive(Debug, Deserialize)]
struct RawRow {
    id: Thing,
    entity_name: String,
    entity_type: String,
    #[serde(default)]
    candidate_ids: Vec<String>,
    #[serde(default)]
    context_signature: Option<String>,
    session_id: String,
    #[serde(default)]
    top_score: Option<f64>,
    #[serde(default = "default_seen_count")]
    seen_count: i64,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    resolved_id: Option<String>,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
}

impl From<RawRow> for ProvisionalExtraction {
    fn from(r: RawRow) -> Self {
        Self {
            id: Some(r.id.to_string()),
            entity_name: r.entity_name,
            entity_type: r.entity_type,
            candidate_ids: r.candidate_ids,
            context_signature: r.context_signature,
            session_id: r.session_id,
            top_score: r.top_score,
            seen_count: r.seen_count,
            status: r.status,
            resolved_id: r.resolved_id,
            first_seen_at: r.first_seen_at,
            last_seen_at: r.last_seen_at,
        }
    }
}

fn db_err(e: surrealdb::Error) -> memex_core::Error {
    memex_core::Error::Db(e.to_string())
}
