//! Basic markdown ingestion. Walks a directory, reads `*.md` / `*.markdown`
//! files, splits each into chunks (~2KB at paragraph boundaries with a
//! sliding overlap), and inserts the chunks into the Memex memory store
//! with `source_type = "markdown"` and `source_path = absolute path`.
//!
//! Deliberately small: no front-matter parsing, no entity extraction,
//! no incremental indexing. Re-running the ingest re-inserts; the
//! `insert_memories_batch` deduplicates by content hash, so you can
//! safely re-ingest the same folder without bloating the store.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use memex_core::{memories, types::MemoryChunk, EmbeddingService, MemexDb};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct IngestProgress {
    pub phase: String,
    pub files_seen: usize,
    pub files_ingested: usize,
    pub chunks_inserted: usize,
    pub errors: usize,
    pub current_file: Option<String>,
}

/// Recursively walk `root` for markdown files and insert chunks into
/// the Memex DB. Skips hidden directories (starting with `.`),
/// `node_modules`, `target`, and anything Git-internal.
///
/// `progress` is invoked at meaningful state changes (per-file done,
/// final summary). Pass `|_| {}` to ignore.
pub async fn ingest_folder(
    db: Arc<MemexDb>,
    embedder: Arc<EmbeddingService>,
    root: PathBuf,
    progress: impl Fn(IngestProgress) + Send + Sync + 'static,
) -> Result<IngestProgress, String> {
    let files = collect_markdown_files(&root);
    let total = files.len();
    progress(IngestProgress {
        phase: "discovered".into(),
        files_seen: total,
        files_ingested: 0,
        chunks_inserted: 0,
        errors: 0,
        current_file: None,
    });

    let mut files_done = 0usize;
    let mut total_chunks = 0usize;
    let mut errors = 0usize;

    for path in files {
        progress(IngestProgress {
            phase: "ingesting".into(),
            files_seen: total,
            files_ingested: files_done,
            chunks_inserted: total_chunks,
            errors,
            current_file: Some(path.to_string_lossy().to_string()),
        });

        let body = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("ingest: read {} failed: {e}", path.display());
                errors += 1;
                continue;
            }
        };
        let chunks = chunk_markdown(&body, &path);
        if chunks.is_empty() {
            files_done += 1;
            continue;
        }
        match memories::insert_memories_batch(&db, &embedder, &chunks).await {
            Ok(ids) => {
                total_chunks += ids
                    .into_iter()
                    .filter(|id| !id.starts_with("duplicate:"))
                    .count();
            }
            Err(e) => {
                tracing::warn!("ingest: insert {} failed: {e}", path.display());
                errors += 1;
            }
        }
        files_done += 1;
    }

    let summary = IngestProgress {
        phase: "done".into(),
        files_seen: total,
        files_ingested: files_done,
        chunks_inserted: total_chunks,
        errors,
        current_file: None,
    };
    progress(summary.clone());
    Ok(summary)
}

fn collect_markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "dist"
            || name == "build"
        {
            continue;
        }
        if path.is_dir() {
            walk(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext, "md" | "markdown" | "mdx") {
                out.push(path);
            }
        }
    }
}

const TARGET_CHUNK_BYTES: usize = 2_000;
const CHUNK_OVERLAP_BYTES: usize = 200;

/// Split markdown text into roughly TARGET_CHUNK_BYTES-sized chunks at
/// paragraph boundaries (blank lines), with a small overlap so context
/// isn't lost across cuts. Strips leading whitespace per chunk.
fn chunk_markdown(body: &str, path: &Path) -> Vec<MemoryChunk> {
    let body = body.trim();
    if body.is_empty() {
        return Vec::new();
    }
    let path_str = path.to_string_lossy().to_string();
    let summary = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string());

    // Walk paragraphs; accumulate into chunks ~TARGET_CHUNK_BYTES.
    let paragraphs: Vec<&str> = body.split("\n\n").filter(|p| !p.trim().is_empty()).collect();
    let mut chunks: Vec<MemoryChunk> = Vec::new();
    let mut current = String::new();
    for para in paragraphs {
        if !current.is_empty()
            && current.len() + para.len() + 2 > TARGET_CHUNK_BYTES
        {
            chunks.push(make_chunk(&current, &path_str, &summary));
            // Carry an overlap of the last chars into the next chunk.
            let overlap_start = current.len().saturating_sub(CHUNK_OVERLAP_BYTES);
            let mut start = overlap_start;
            while start < current.len() && !current.is_char_boundary(start) {
                start += 1;
            }
            current = current[start..].to_string();
            if !current.is_empty() {
                current.push_str("\n\n");
            }
        }
        current.push_str(para.trim());
        current.push_str("\n\n");
    }
    if !current.trim().is_empty() {
        chunks.push(make_chunk(current.trim(), &path_str, &summary));
    }
    chunks
}

fn make_chunk(content: &str, source_path: &str, summary: &Option<String>) -> MemoryChunk {
    MemoryChunk {
        id: None,
        content: content.to_string(),
        summary: summary.clone(),
        source_type: "markdown".into(),
        source_id: None,
        source_path: Some(source_path.to_string()),
        timestamp: Utc::now(),
        metadata: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn collects_markdown_recursively_skipping_known_dirs() {
        let d = tempdir().unwrap();
        let root = d.path();
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::write(root.join("README.md"), "# top").unwrap();
        fs::write(root.join("docs/a.md"), "## a").unwrap();
        fs::write(root.join(".git/config.md"), "skip me").unwrap();
        fs::write(root.join("node_modules/x.md"), "skip").unwrap();
        fs::write(root.join("not.txt"), "ignored").unwrap();

        let files = collect_markdown_files(root);
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(files.len(), 2);
        assert!(names.contains(&"README.md".to_string()));
        assert!(names.contains(&"a.md".to_string()));
    }

    #[test]
    fn chunks_a_long_doc_at_paragraph_boundaries() {
        let body = (0..50)
            .map(|i| {
                let stem = format!("Paragraph {i} ");
                stem.repeat(10)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let chunks = chunk_markdown(&body, Path::new("/tmp/test.md"));
        assert!(chunks.len() > 1);
        // Each chunk under target by enough margin to still allow overlap.
        for c in &chunks {
            assert!(c.content.len() <= TARGET_CHUNK_BYTES + CHUNK_OVERLAP_BYTES + 200);
            assert_eq!(c.source_type, "markdown");
            assert_eq!(c.source_path.as_deref(), Some("/tmp/test.md"));
        }
    }

    #[test]
    fn empty_document_yields_no_chunks() {
        assert!(chunk_markdown("", Path::new("/tmp/x.md")).is_empty());
        assert!(chunk_markdown("   \n\n   ", Path::new("/tmp/x.md")).is_empty());
    }
}
