use std::path::{Path, PathBuf};
use surrealdb::engine::local::{Db, Mem, RocksDb};
use surrealdb::Surreal;

use crate::error::Result;
use crate::schema::SCHEMA;

pub type HarnessDb = Surreal<Db>;

/// Default database path: `~/.harness/db`.
pub fn default_db_path() -> PathBuf {
    dirs::home_dir()
        .expect("could not determine home directory")
        .join(".harness")
        .join("db")
}

/// Open the harness database at the given filesystem path (RocksDB engine)
/// and apply the schema. Creates parent directories if missing.
pub async fn init_db(path: &Path) -> Result<HarnessDb> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::error::StorageError::Db(format!("create db dir: {e}")))?;
    }
    let db: Surreal<Db> = Surreal::new::<RocksDb>(path.to_string_lossy().as_ref()).await?;
    db.use_ns("harness").use_db("chat").await?;
    db.query(SCHEMA).await?;
    tracing::info!("harness db initialised at {}", path.display());
    Ok(db)
}

/// Open an in-memory database (used for tests).
pub async fn init_in_memory() -> Result<HarnessDb> {
    let db: Surreal<Db> = Surreal::new::<Mem>(()).await?;
    db.use_ns("harness").use_db("chat").await?;
    db.query(SCHEMA).await?;
    Ok(db)
}
