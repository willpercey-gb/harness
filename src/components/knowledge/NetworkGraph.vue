<script setup lang="ts">
import { computed, onMounted, ref, watch, nextTick } from 'vue'
import { VNetworkGraph } from 'v-network-graph'
import 'v-network-graph/lib/style.css'
import { useGraphData, createGraphConfigs } from '@/composables/useGraphData'

const {
  nodes,
  edges,
  isLoading,
  nodeCount,
  edgeCount,
  focusedNodeId,
  loadGraph,
  selectNode,
  unfocus,
  searchQuery,
  hiddenTypes,
  entityTypes,
  setSearchQuery,
  toggleTypeFilter,
} = useGraphData()

const ENTITY_COLORS: Record<string, string> = {
  person: '#4fc3f7',
  organization: '#81c784',
  project: '#ba68c8',
  technology: '#ffb74d',
  topic: '#90a4ae',
  location: '#f48fb1',
  component: '#7986cb',
}

const graphRef = ref<any>(null)
const isDark = ref(false)
const configs = computed(() => createGraphConfigs(isDark.value))

const emit = defineEmits<{ 'node-selected': [id: string | null] }>()

const eventHandlers = {
  'node:click': ({ node }: { node: string }) => {
    selectNode(node)
    emit('node-selected', focusedNodeId.value)
  },
  'view:click': () => {
    if (focusedNodeId.value) {
      unfocus()
      emit('node-selected', null)
    }
  },
}

watch(focusedNodeId, () => {
  nextTick(() => setTimeout(() => graphRef.value?.fitToContents(), 100))
})

function handleFitToView() {
  graphRef.value?.fitToContents()
}
function handleZoomIn() {
  graphRef.value?.zoomIn()
}
function handleZoomOut() {
  graphRef.value?.zoomOut()
}

onMounted(async () => {
  isDark.value = document.documentElement.classList.contains('dark')
  await loadGraph()
})
</script>

<template>
  <div class="net-graph">
    <header class="toolbar">
      <input
        :value="searchQuery"
        @input="(e) => setSearchQuery((e.target as HTMLInputElement).value)"
        type="search"
        placeholder="Search entities…"
        class="search"
      />
      <span class="count">
        {{ nodeCount }} entit{{ nodeCount === 1 ? 'y' : 'ies' }} · {{ edgeCount }} edge{{ edgeCount === 1 ? '' : 's' }}
      </span>
      <div class="zoom">
        <button class="zoom-btn" @click="handleZoomOut" title="Zoom out">
          <span class="material-symbols-outlined">remove</span>
        </button>
        <button class="zoom-btn" @click="handleFitToView" title="Fit to view">
          <span class="material-symbols-outlined">fit_screen</span>
        </button>
        <button class="zoom-btn" @click="handleZoomIn" title="Zoom in">
          <span class="material-symbols-outlined">add</span>
        </button>
      </div>
    </header>

    <div class="filters">
      <span class="filter-label">Types:</span>
      <button
        v-for="t in entityTypes"
        :key="t"
        class="filter-pill"
        :class="{ off: hiddenTypes.has(t) }"
        :style="{ '--c': ENTITY_COLORS[t] || '#90a4ae' }"
        @click="toggleTypeFilter(t)"
      >
        <span class="dot"></span>{{ t }}
      </button>
    </div>

    <div v-if="isLoading" class="empty">Loading graph…</div>
    <div v-else-if="nodeCount === 0" class="empty">
      No entities yet. Seed a few by hand using the panel on the right — the
      extractor's resolver matches new mentions against existing rows, so a
      handful of pre-seeded anchors sharply improves quality from turn one.
      Anything you don't seed gets mapped automatically as you chat.
    </div>
    <div v-else class="canvas">
      <VNetworkGraph
        ref="graphRef"
        :nodes="nodes"
        :edges="edges"
        :configs="configs"
        :event-handlers="eventHandlers"
      />
    </div>
  </div>
</template>

<style scoped lang="scss">
.net-graph {
  display: grid;
  grid-template-rows: auto auto 1fr;
  height: 100%;
  width: 100%;
  background: var(--bg);
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--rule);
  .search {
    flex: 1;
    background: var(--bg-soft);
    border: 1px solid var(--rule);
    border-radius: var(--radius-md);
    padding: 6px 10px;
    font: inherit;
    font-size: 13px;
    color: var(--ink);
    &:focus { outline: 0; border-color: var(--accent); }
  }
  .count {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 11px;
    color: var(--ink-faint);
    white-space: nowrap;
  }
  .zoom { display: inline-flex; gap: 2px; }
  .zoom-btn {
    background: transparent;
    border: 1px solid var(--rule);
    border-radius: var(--radius-sm, 3px);
    padding: 4px;
    color: var(--ink-muted);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    &:hover { color: var(--ink); background: var(--bg-soft); }
    .material-symbols-outlined { font-size: 16px; }
  }
}
.filters {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border-bottom: 1px solid var(--rule);
  flex-wrap: wrap;
  .filter-label {
    font-size: 11px;
    color: var(--ink-faint);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin-right: 4px;
  }
  .filter-pill {
    background: transparent;
    border: 1px solid var(--rule);
    border-radius: 999px;
    padding: 3px 10px 3px 8px;
    color: var(--ink-muted);
    font-size: 11.5px;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    transition: opacity 0.12s, color 0.12s;
    .dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: var(--c, #90a4ae);
    }
    &:hover { color: var(--ink); }
    &.off {
      opacity: 0.4;
      .dot { background: transparent; border: 1px solid var(--c, #90a4ae); }
    }
  }
}
.canvas {
  position: relative;
  overflow: hidden;
}
.empty {
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--ink-faint);
  font-size: 13px;
  padding: 32px;
  text-align: center;
  code {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
    background: var(--bg-soft);
    padding: 1px 5px;
    border-radius: 3px;
    margin: 0 3px;
  }
}
</style>
