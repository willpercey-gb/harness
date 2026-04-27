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
