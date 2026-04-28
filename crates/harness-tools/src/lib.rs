//! Built-in tools for the harness chat agent.
//!
//! Each tool is a `strands_core::Tool` implementation. Tools that need
//! configuration (allowlists, sandbox roots) hold their state in the
//! struct itself — built once per chat turn from the current Settings.

pub mod builtins;
pub mod ingest;
pub mod maintenance;
pub mod provisional;

pub use builtins::{
    calculator::Calculator,
    get_time::GetTime,
    http_fetch::HttpFetch,
    memex::{LookupEntity, Recall},
    read_file::ReadFile,
};
pub use memex_core::{EmbeddingService, MemexDb};

/// Re-exports of memex-core's functional modules so the binary crate
/// can issue knowledge queries without a direct memex-core dep.
pub mod memex_api {
    pub use memex_core::{entities, memories, query, relationships, types};
}

use std::path::Path;

/// Open (or create) the harness-owned Memex database at `path`.
/// Thin wrapper around `memex_core::db::init_db` plus the harness-only
/// extension schema (e.g. `provisional_extraction`).
pub async fn init_memex_db(path: &Path) -> Result<MemexDb, memex_core::Error> {
    let db = memex_core::db::init_db(&path.to_path_buf()).await?;
    provisional::apply_schema(&db).await?;
    maintenance::apply_schema(&db).await?;
    Ok(db)
}
