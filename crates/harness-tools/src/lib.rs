//! Built-in tools for the harness chat agent.
//!
//! Each tool is a `strands_core::Tool` implementation. Tools that need
//! configuration (allowlists, sandbox roots) hold their state in the
//! struct itself — built once per chat turn from the current Settings.

pub mod builtins;

pub use builtins::{
    calculator::Calculator,
    get_time::GetTime,
    http_fetch::HttpFetch,
    memex::{LinkEntities, LookupEntity, NoteEntity, Recall, Remember},
    read_file::ReadFile,
};
pub use memex_core::{EmbeddingService, MemexDb};

use std::path::Path;

/// Open (or create) the harness-owned Memex database at `path`.
/// Thin wrapper around `memex_core::db::init_db` so the binary crate
/// doesn't need to depend on memex-core directly.
pub async fn init_memex_db(path: &Path) -> Result<MemexDb, memex_core::Error> {
    memex_core::db::init_db(&path.to_path_buf()).await
}
