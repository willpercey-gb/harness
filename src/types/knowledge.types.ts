// Mirrors src-tauri/src/commands/knowledge.rs

export type EntityType =
  | 'person'
  | 'organization'
  | 'project'
  | 'technology'
  | 'topic'
  | 'location'
  | 'component'

export interface FullGraphEntity {
  id: string
  entity_type: EntityType | string
  name: string
  description: string | null
  content: string | null
  aliases: string[]
}

export interface GraphEdge {
  from_id: string
  to_id: string
  relation_type: string
}

export interface FullGraphResponse {
  entities: FullGraphEntity[]
  edges: GraphEdge[]
}

export interface GraphNode {
  id: string
  entity_type: string
  name: string
  description: string | null
}

export interface EntityGraph {
  center: GraphNode
  nodes: GraphNode[]
  edges: GraphEdge[]
}

export interface MemoryChunk {
  id: string | null
  content: string
  summary: string | null
  source_type: string
  source_id: string | null
  timestamp: string
}

export interface EntityRef {
  entity_type: string
  name: string
  id: string
}

export interface QueryResult {
  content: string
  summary: string | null
  source_type: string
  source_id: string | null
  timestamp: string
  score: number
  linked_entities: EntityRef[]
}

export interface KnowledgeStats {
  memory_chunks: number
  entities_total: number
  entities_by_type: Record<string, number>
  relationships: number
}
