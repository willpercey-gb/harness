//! Deterministic entity resolver for the passive memory extractor.
//!
//! Sits between the extractor agent's structured-JSON output and the
//! actual graph writes. Given a candidate name + claimed type, decides
//! whether it refers to an existing entity or warrants a new one — and
//! accumulates spelling/casing variants as `aliases[]` on the existing
//! row so the graph stays consistent over time.
//!
//! Tiers, cheapest first, stop on first confident hit:
//!   1. Exact match on canonical name (case-insensitive).
//!   2. Exact match on any alias (case-insensitive).
//!   3. Normalised match (lowercase, strip punctuation/TLDs/whitespace)
//!      against canonical + aliases of every entity of the same type.
//!   4. Embedding-similarity ≥ 0.85 cosine against entity-name vectors.
//!   5. Else → New.
//!
//! The 0.75–0.85 "uncertain band" lives in Phase 2 — for now we either
//! match or create. Type-scoped: we never merge across entity types.

use std::sync::Arc;

use memex_core::{entities, EmbeddingService, EntityType, MemexDb};
use serde::Serialize;
use surrealdb::sql::Thing;
use tracing::warn;

/// Cosine-similarity floor for confident embedding matches. Above this,
/// we merge into the existing entity and append the input form as an
/// alias.
const EMBEDDING_MATCH_THRESHOLD: f32 = 0.85;
/// Lower edge of the "uncertain" embedding band. Hits in
/// [UNCERTAIN_THRESHOLD, EMBEDDING_MATCH_THRESHOLD) are parked in the
/// provisional buffer rather than merged or created — the next turn
/// that mentions the same name with overlapping context is what
/// disambiguates.
pub const UNCERTAIN_THRESHOLD: f32 = 0.75;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Resolution {
    /// Resolved to an existing entity. `id` is `table:uuid`. If the
    /// matched entity didn't already have the input form as an alias,
    /// `appended_alias` carries the form we just added.
    Existing {
        id: String,
        entity_type: EntityType,
        canonical_name: String,
        appended_alias: Option<String>,
    },
    /// Embedding similarity fell in [0.75, 0.85). Caller should park
    /// the extraction in the provisional buffer with these candidates;
    /// the next turn that mentions the same name with overlapping
    /// context will promote it.
    Uncertain {
        candidates: Vec<UncertainCandidate>,
        top_score: f32,
    },
    /// No confident or candidate match — caller should create a fresh
    /// entity in the requested table.
    New,
}

#[derive(Debug, Clone, Serialize)]
pub struct UncertainCandidate {
    pub id: String,
    pub canonical_name: String,
    pub score: f32,
}

/// Lowercase, strip common punctuation (`.`, `'`, `-`, `_`, `,`, `/`),
/// collapse whitespace, drop trailing common TLDs (`.it`, `.com`,
/// `.io`, `.co.uk`, `.uk`, `.net`, `.org`). Pure function.
pub fn normalise(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let stripped = strip_trailing_tld(&lower);
    let mut out = String::with_capacity(stripped.len());
    let mut last_was_space = false;
    for c in stripped.chars() {
        match c {
            '.' | '\'' | '-' | '_' | ',' | '/' | '\\' | '(' | ')' | '[' | ']' | '"' => {}
            ws if ws.is_whitespace() => {
                if !last_was_space && !out.is_empty() {
                    out.push(' ');
                    last_was_space = true;
                }
            }
            other => {
                out.push(other);
                last_was_space = false;
            }
        }
    }
    out.trim().to_string()
}

fn strip_trailing_tld(s: &str) -> &str {
    for tld in [".com", ".io", ".co.uk", ".co", ".uk", ".it", ".net", ".org"] {
        if let Some(stripped) = s.strip_suffix(tld) {
            return stripped;
        }
    }
    s
}

/// Run the tiered match. Type-scoped: matches are only considered against
/// entities of the same `entity_type`. Embedding tier requires the
/// embedder so we can hash the candidate name; if it's unavailable, we
/// stop after tier 3.
pub async fn resolve(
    db: &Arc<MemexDb>,
    embedder: Option<&Arc<EmbeddingService>>,
    name: &str,
    entity_type: &EntityType,
) -> Result<Resolution, memex_core::Error> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(Resolution::New);
    }
    let trimmed_lower = trimmed.to_lowercase();
    let target_norm = normalise(trimmed);

    // ---- Tiers 1+2+3: scan entities of this type ----
    let rows = entities::list_entities(db, entity_type, None, 500).await?;
    for row in &rows {
        let row_id = match row.id.as_ref() {
            Some(s) => s.clone(),
            None => continue,
        };
        let canonical = row.name.clone();
        let canonical_lower = canonical.to_lowercase();

        // Tier 1: exact (case-insensitive) on canonical.
        if canonical_lower == trimmed_lower {
            return Ok(Resolution::Existing {
                id: row_id,
                entity_type: entity_type.clone(),
                canonical_name: canonical,
                appended_alias: None,
            });
        }

        // Tier 2: exact on any alias.
        if row
            .aliases
            .iter()
            .any(|a| a.to_lowercase() == trimmed_lower)
        {
            return Ok(Resolution::Existing {
                id: row_id,
                entity_type: entity_type.clone(),
                canonical_name: canonical,
                appended_alias: None,
            });
        }

        // Tier 3: normalised match against canonical + aliases.
        if normalise(&canonical) == target_norm
            || row.aliases.iter().any(|a| normalise(a) == target_norm)
        {
            let mut new_aliases = row.aliases.clone();
            new_aliases.push(trimmed.to_string());
            if let Err(e) = append_alias(db, &row_id, &new_aliases).await {
                warn!("failed to append alias '{trimmed}' to {row_id}: {e}");
            }
            return Ok(Resolution::Existing {
                id: row_id,
                entity_type: entity_type.clone(),
                canonical_name: canonical,
                appended_alias: Some(trimmed.to_string()),
            });
        }
    }

    // ---- Tier 4: embedding similarity ----
    if let Some(emb) = embedder {
        if let Some(hit) = embedding_match(db, emb, entity_type, trimmed, &trimmed_lower).await? {
            return Ok(hit);
        }
    }

    Ok(Resolution::New)
}

async fn embedding_match(
    db: &Arc<MemexDb>,
    embedder: &EmbeddingService,
    entity_type: &EntityType,
    trimmed: &str,
    trimmed_lower: &str,
) -> Result<Option<Resolution>, memex_core::Error> {
    #[derive(serde::Deserialize)]
    struct Row {
        id: Thing,
        name: String,
        #[serde(default)]
        aliases: Vec<String>,
        #[serde(default)]
        embedding: Option<Vec<f32>>,
    }

    let target_vec = embedder.embed_text(trimmed).await?;
    let table = entity_type.table_name().to_string();
    let mut res = db
        .query(
            "SELECT id, name, aliases, embedding FROM type::table($table) \
             WHERE archived != true AND embedding != NONE LIMIT 500",
        )
        .bind(("table", table))
        .await
        .map_err(memex_core_db_err)?;
    let rows: Vec<Row> = res.take(0).map_err(memex_core_db_err)?;

    // Score every row, then partition into the confident-match tier
    // and the uncertain tier. We need both because the uncertain
    // resolution carries up to 3 candidates for the UI / promotion
    // logic, not just the top one.
    let mut scored: Vec<(f32, &Row)> = rows
        .iter()
        .filter_map(|r| r.embedding.as_ref().map(|v| (cosine(&target_vec, v), r)))
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    let Some((score, chosen)) = scored.first().copied() else {
        return Ok(None);
    };

    if score < UNCERTAIN_THRESHOLD {
        return Ok(None);
    }
    if score < EMBEDDING_MATCH_THRESHOLD {
        // Uncertain band — don't merge or create. Return the top up to
        // 3 candidates to the caller for parking.
        let candidates = scored
            .iter()
            .take(3)
            .map(|(s, r)| UncertainCandidate {
                id: r.id.to_string(),
                canonical_name: r.name.clone(),
                score: *s,
            })
            .collect();
        return Ok(Some(Resolution::Uncertain {
            candidates,
            top_score: score,
        }));
    }

    let id_str = chosen.id.to_string();
    let already_aliased = chosen.name.to_lowercase() == trimmed_lower
        || chosen
            .aliases
            .iter()
            .any(|a| a.to_lowercase() == trimmed_lower);
    let appended = if already_aliased {
        None
    } else {
        let mut new_aliases = chosen.aliases.clone();
        new_aliases.push(trimmed.to_string());
        if let Err(e) = append_alias(db, &id_str, &new_aliases).await {
            warn!("failed to append alias '{trimmed}' to {id_str}: {e}");
        }
        Some(trimmed.to_string())
    };

    Ok(Some(Resolution::Existing {
        id: id_str,
        entity_type: entity_type.clone(),
        canonical_name: chosen.name.clone(),
        appended_alias: appended,
    }))
}

async fn append_alias(
    db: &MemexDb,
    id_str: &str,
    new_aliases: &[String],
) -> Result<(), memex_core::Error> {
    db.query("UPDATE type::thing($id) SET aliases = $aliases, updated_at = time::now()")
        .bind(("id", id_str.to_string()))
        .bind(("aliases", new_aliases.to_vec()))
        .await
        .map_err(memex_core_db_err)?;
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

fn memex_core_db_err(e: surrealdb::Error) -> memex_core::Error {
    memex_core::Error::Db(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_handles_apostrophes_and_case() {
        assert_eq!(normalise("Org O"), "orgo");
        assert_eq!(normalise("Acme.IT"), "acme");
        assert_eq!(normalise("AcMe"), "acme");
        // "acmeit" lacks the TLD separator so it stays distinct from
        // "acme"; alias accumulation on first reconciliation is what
        // closes that gap in practice.
        assert_eq!(normalise("acmeit"), "acmeit");
    }

    #[test]
    fn normalise_collapses_whitespace_and_strips_brackets() {
        assert_eq!(normalise("Foo  Bar"), "foo bar");
        assert_eq!(normalise(" (Org O) "), "orgo");
    }
}
