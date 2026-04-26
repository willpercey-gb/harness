import { computed, ref } from 'vue'
import { defineConfigs } from 'v-network-graph'
import { ForceLayout } from 'v-network-graph/lib/force-layout'
import { getFullGraph, queryKnowledge } from '@/services/knowledge'
import type { FullGraphEntity, GraphEdge, QueryResult } from '@/types/knowledge.types'

const ENTITY_COLORS: Record<string, string> = {
  person: '#4fc3f7',
  organization: '#81c784',
  project: '#ba68c8',
  technology: '#ffb74d',
  topic: '#90a4ae',
  location: '#f48fb1',
  component: '#7986cb',
}

const ENTITY_SIZES: Record<string, number> = {
  person: 14,
  organization: 16,
  project: 18,
  technology: 14,
  topic: 12,
  location: 12,
  component: 10,
}

export interface VNode {
  name: string
  color: string
  size: number
  entityType: string
  description?: string
  focused?: boolean
}

export interface VEdge {
  source: string
  target: string
  label?: string
  priority: number
}

function edgePriority(srcType: string, tgtType: string, relType: string): number {
  if (relType === 'works_at' || relType === 'works_on') return 1
  if (relType === 'uses_tech' || relType === 'part_of' || relType === 'knows_about') return 2
  if (
    (srcType === 'organization' && tgtType === 'person') ||
    (srcType === 'person' && tgtType === 'organization')
  )
    return 1
  if (srcType === 'project' && tgtType === 'technology') return 1
  if (srcType === 'organization' && tgtType === 'project') return 1
  if (srcType === 'person' && tgtType === 'project') return 1
  if (relType === 'related_to' && srcType === tgtType) return 3
  return 2
}

// Singleton state — shared across components.
const allNodes = ref<Record<string, VNode>>({})
const allEdges = ref<Record<string, VEdge>>({})
const nodes = ref<Record<string, VNode>>({})
const edges = ref<Record<string, VEdge>>({})
const isLoading = ref(false)
const selectedNodeId = ref<string | null>(null)
const focusedNodeId = ref<string | null>(null)
const rawEntities = ref<FullGraphEntity[]>([])
const rawEdges = ref<GraphEdge[]>([])
const searchQuery = ref('')
const hiddenTypes = ref<Set<string>>(new Set(['component']))
const relatedMemories = ref<QueryResult[]>([])
const isLoadingMemories = ref(false)

export function useGraphData() {
  const selectedEntity = computed(() => {
    if (!selectedNodeId.value) return null
    return rawEntities.value.find((e) => e.id === selectedNodeId.value) ?? null
  })
  const nodeCount = computed(() => Object.keys(allNodes.value).length)
  const edgeCount = computed(() => Object.keys(allEdges.value).length)
  const entityTypes = computed(() => {
    const types = new Set<string>()
    for (const n of Object.values(allNodes.value)) types.add(n.entityType)
    return Array.from(types).sort()
  })

  async function loadGraph() {
    isLoading.value = true
    try {
      const res = await getFullGraph()
      rawEntities.value = res.entities
      rawEdges.value = res.edges
      buildGraphData(res.entities, res.edges)
    } catch (e) {
      console.error('[graph] load failed', e)
    } finally {
      isLoading.value = false
    }
  }

  function buildGraphData(entities: FullGraphEntity[], gEdges: GraphEdge[]) {
    const newNodes: Record<string, VNode> = {}
    const newEdges: Record<string, VEdge> = {}

    for (const e of entities) {
      newNodes[e.id] = {
        name: e.name,
        color: ENTITY_COLORS[e.entity_type] || '#90a4ae',
        size: ENTITY_SIZES[e.entity_type] || 14,
        entityType: e.entity_type,
        description: e.description ?? undefined,
      }
    }
    const ids = new Set(Object.keys(newNodes))
    for (const ed of gEdges) {
      if (ids.has(ed.from_id) && ids.has(ed.to_id)) {
        const key = `${ed.from_id}--${ed.relation_type}--${ed.to_id}`
        const srcType = newNodes[ed.from_id]?.entityType ?? ''
        const tgtType = newNodes[ed.to_id]?.entityType ?? ''
        newEdges[key] = {
          source: ed.from_id,
          target: ed.to_id,
          label: ed.relation_type.replace(/_/g, ' '),
          priority: edgePriority(srcType, tgtType, ed.relation_type),
        }
      }
    }
    allNodes.value = newNodes
    allEdges.value = newEdges
    if (!focusedNodeId.value) {
      const v = filterTopLevel(newNodes, newEdges)
      nodes.value = v.nodes
      edges.value = v.edges
    }
  }

  function filterTopLevel(
    src: Record<string, VNode>,
    srcEdges: Record<string, VEdge>,
  ): { nodes: Record<string, VNode>; edges: Record<string, VEdge> } {
    const q = searchQuery.value.toLowerCase().trim()
    const visible: Record<string, VNode> = {}
    for (const [id, n] of Object.entries(src)) {
      if (hiddenTypes.value.has(n.entityType)) continue
      if (q && !n.name.toLowerCase().includes(q)) continue
      visible[id] = n
    }
    const ids = new Set(Object.keys(visible))
    const ve: Record<string, VEdge> = {}
    for (const [k, e] of Object.entries(srcEdges)) {
      if (ids.has(e.source) && ids.has(e.target)) ve[k] = e
    }
    return { nodes: visible, edges: ve }
  }

  function applyFilters() {
    if (focusedNodeId.value) return
    const v = filterTopLevel(allNodes.value, allEdges.value)
    nodes.value = v.nodes
    edges.value = v.edges
  }

  function setSearchQuery(q: string) {
    searchQuery.value = q
    applyFilters()
  }

  function toggleTypeFilter(t: string) {
    const s = new Set(hiddenTypes.value)
    if (s.has(t)) s.delete(t)
    else s.add(t)
    hiddenTypes.value = s
    applyFilters()
  }

  function focusOnNode(nodeId: string) {
    focusedNodeId.value = nodeId
    selectedNodeId.value = nodeId
    const connected: Record<string, VEdge> = {}
    const neighbours = new Set<string>([nodeId])
    for (const [k, e] of Object.entries(allEdges.value)) {
      if (e.source === nodeId || e.target === nodeId) {
        connected[k] = e
        neighbours.add(e.source)
        neighbours.add(e.target)
      }
    }
    const filtered: Record<string, VNode> = {}
    for (const id of neighbours) {
      const n = allNodes.value[id]
      if (n) filtered[id] = { ...n, focused: id === nodeId }
    }
    nodes.value = filtered
    edges.value = connected
    void loadRelatedMemories(nodeId)
  }

  function unfocus() {
    focusedNodeId.value = null
    selectedNodeId.value = null
    relatedMemories.value = []
    const v = filterTopLevel(allNodes.value, allEdges.value)
    nodes.value = v.nodes
    edges.value = v.edges
  }

  function selectNode(nodeId: string | null) {
    if (nodeId === null) {
      if (focusedNodeId.value) unfocus()
      else {
        selectedNodeId.value = null
        relatedMemories.value = []
      }
      return
    }
    if (focusedNodeId.value === nodeId) {
      unfocus()
      return
    }
    focusOnNode(nodeId)
  }

  async function loadRelatedMemories(nodeId: string) {
    const entity = rawEntities.value.find((e) => e.id === nodeId)
    if (!entity) return
    isLoadingMemories.value = true
    try {
      relatedMemories.value = await queryKnowledge(entity.name, undefined, 8)
    } catch {
      relatedMemories.value = []
    } finally {
      isLoadingMemories.value = false
    }
  }

  return {
    nodes,
    edges,
    isLoading,
    selectedNodeId,
    focusedNodeId,
    selectedEntity,
    relatedMemories,
    isLoadingMemories,
    nodeCount,
    edgeCount,
    searchQuery,
    hiddenTypes,
    entityTypes,
    loadGraph,
    selectNode,
    unfocus,
    setSearchQuery,
    toggleTypeFilter,
  }
}

export function createGraphConfigs(isDark: boolean) {
  return defineConfigs({
    view: {
      scalingObjects: true,
      minZoomLevel: 0.2,
      maxZoomLevel: 4,
      layoutHandler: new ForceLayout({
        positionFixedByDrag: false,
        positionFixedByClickWithAltKey: true,
        createSimulation: (d3, nodes, edges) => {
          const forceLink = d3.forceLink<any, any>(edges).id((d: any) => d.id)
          return d3
            .forceSimulation(nodes)
            .force('edge', forceLink.distance(80).strength(0.6))
            .force('charge', d3.forceManyBody().strength(-600))
            .force('collide', d3.forceCollide(40).strength(0.7))
            .force('center', d3.forceCenter().strength(0.05))
            .alphaMin(0.001)
        },
      }),
    },
    node: {
      selectable: false,
      normal: {
        type: 'circle',
        radius: (node: any) => node.size || 14,
        color: (node: any) => node.color || '#90a4ae',
        strokeWidth: (node: any) => (node.focused ? 3 : 0),
        strokeColor: '#3b82f6',
      },
      hover: {
        radius: (node: any) => (node.size || 14) + 2,
        color: (node: any) => node.color || '#90a4ae',
        strokeWidth: 2,
        strokeColor: isDark ? '#fff' : '#333',
      },
      label: {
        visible: true,
        fontSize: 11,
        color: isDark ? '#e0e0e0' : '#1a1a1a',
        directionAutoAdjustment: true,
        margin: 4,
        background: {
          visible: true,
          color: isDark ? 'rgba(0,0,0,0.6)' : 'rgba(255,255,255,0.85)',
          padding: { vertical: 1, horizontal: 4 },
          borderRadius: 3,
        },
      },
      focusring: { width: 0 },
    },
    edge: {
      selectable: false,
      normal: {
        width: (edge: any) => (edge.priority === 1 ? 2.5 : edge.priority === 3 ? 0.8 : 1.5),
        color: (edge: any) =>
          edge.priority === 1
            ? isDark
              ? '#6366f1'
              : '#4f46e5'
            : edge.priority === 3
              ? isDark
                ? '#333'
                : '#ddd'
              : isDark
                ? '#444'
                : '#ccc',
        dasharray: (edge: any) => (edge.priority === 3 ? '4 3' : '0'),
      },
      hover: { width: 2.5, color: isDark ? '#777' : '#999' },
      marker: { target: { type: 'none' }, source: { type: 'none' } },
    },
  })
}
