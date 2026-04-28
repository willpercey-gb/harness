import { invoke } from '@tauri-apps/api/core'
import type {
  EntityGraph,
  FullGraphResponse,
  KnowledgeStats,
  MemoryChunk,
  QueryResult,
} from '@/types/knowledge.types'

export async function getFullGraph(): Promise<FullGraphResponse> {
  return await invoke<FullGraphResponse>('get_full_graph')
}

export async function getEntityGraph(entityId: string, depth = 2): Promise<EntityGraph> {
  return await invoke<EntityGraph>('get_entity_graph', { entityId, depth })
}

export async function getRecentMemories(limit = 50): Promise<MemoryChunk[]> {
  return await invoke<MemoryChunk[]>('get_recent_memories', { limit })
}

export async function queryKnowledge(
  queryText: string,
  entityTypes?: string[],
  limit = 10,
): Promise<QueryResult[]> {
  return await invoke<QueryResult[]>('query_knowledge', {
    queryText,
    entityTypes,
    limit,
  })
}

export async function getKnowledgeStats(): Promise<KnowledgeStats> {
  return await invoke<KnowledgeStats>('get_knowledge_stats')
}

export interface IngestProgress {
  phase: string
  files_seen: number
  files_ingested: number
  chunks_inserted: number
  errors: number
  current_file: string | null
}

export async function ingestMarkdownFolder(path?: string): Promise<IngestProgress> {
  return await invoke<IngestProgress>('ingest_markdown_folder', { path: path ?? null })
}

// ---------------------------------------------------------------------------
// Provisional extractions (Phase 2 — uncertain-band drawer)
// ---------------------------------------------------------------------------

export interface ProvisionalCandidate {
  id: string
  canonical_name: string
  entity_type: string
  description: string | null
}

export interface ProvisionalEntry {
  id: string
  entity_name: string
  entity_type: string
  seen_count: number
  top_score: number | null
  session_id: string
  first_seen_at: string
  last_seen_at: string
  candidates: ProvisionalCandidate[]
}

export async function listProvisional(limit = 100): Promise<ProvisionalEntry[]> {
  return await invoke<ProvisionalEntry[]>('list_provisional', { limit })
}

export async function promoteProvisional(
  provisionalId: string,
  resolvedId: string,
): Promise<void> {
  await invoke<void>('promote_provisional', { provisionalId, resolvedId })
}

export async function promoteProvisionalAsNew(provisionalId: string): Promise<string> {
  return await invoke<string>('promote_provisional_as_new', { provisionalId })
}

export async function discardProvisional(provisionalId: string): Promise<void> {
  await invoke<void>('discard_provisional', { provisionalId })
}
