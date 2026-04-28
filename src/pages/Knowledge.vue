<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import NetworkGraph from '@/components/knowledge/NetworkGraph.vue'
import EntityPanel from '@/components/knowledge/EntityPanel.vue'
import MemoryTimeline from '@/components/knowledge/MemoryTimeline.vue'
import ProvisionalPanel from '@/components/knowledge/ProvisionalPanel.vue'
import { getKnowledgeStats } from '@/services/knowledge'
import type { KnowledgeStats } from '@/types/knowledge.types'

const router = useRouter()
const tab = ref<'graph' | 'memories' | 'provisional'>('graph')
const stats = ref<KnowledgeStats | null>(null)

async function loadStats() {
  try {
    stats.value = await getKnowledgeStats()
  } catch {
    stats.value = null
  }
}

onMounted(loadStats)
</script>

<template>
  <div class="knowledge">
    <header class="head">
      <button class="back" @click="router.push('/chat')" title="Back to chat">
        <span class="material-symbols-outlined">arrow_back</span>
        Chat
      </button>
      <h1>Knowledge</h1>
      <nav class="tabs">
        <button
          class="tab"
          :class="{ on: tab === 'graph' }"
          @click="tab = 'graph'"
        >
          <span class="material-symbols-outlined">hub</span>
          Graph
          <span v-if="stats" class="badge">{{ stats.entities_total }}</span>
        </button>
        <button
          class="tab"
          :class="{ on: tab === 'memories' }"
          @click="tab = 'memories'"
        >
          <span class="material-symbols-outlined">history</span>
          Memories
          <span v-if="stats" class="badge">{{ stats.memory_chunks }}</span>
        </button>
        <button
          class="tab"
          :class="{ on: tab === 'provisional' }"
          @click="tab = 'provisional'"
          title="Pending entity decisions from the passive extractor"
        >
          <span class="material-symbols-outlined">help</span>
          Pending
        </button>
      </nav>
      <div class="spacer"></div>
      <button class="refresh" @click="loadStats" title="Refresh stats">
        <span class="material-symbols-outlined">refresh</span>
      </button>
    </header>

    <main class="body">
      <template v-if="tab === 'graph'">
        <NetworkGraph />
        <EntityPanel />
      </template>
      <template v-else-if="tab === 'memories'">
        <MemoryTimeline />
      </template>
      <template v-else>
        <ProvisionalPanel />
      </template>
    </main>
  </div>
</template>

<style scoped lang="scss">
.knowledge {
  display: grid;
  grid-template-rows: auto 1fr;
  height: 100%;
  width: 100%;
  background: var(--bg);
  overflow: hidden;
}
.head {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 18px;
  border-bottom: 1px solid var(--rule);

  .back {
    background: transparent;
    border: 1px solid var(--rule);
    border-radius: var(--radius-md);
    padding: 4px 10px;
    color: var(--ink-muted);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    transition: all 0.12s;
    &:hover { color: var(--ink); background: var(--bg-soft); }
    .material-symbols-outlined { font-size: 14px; }
  }
  h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: var(--ink);
    letter-spacing: -0.01em;
  }
  .tabs { display: inline-flex; gap: 2px; margin-left: 12px; }
  .tab {
    background: transparent;
    border: 1px solid var(--rule);
    border-radius: var(--radius-md);
    padding: 5px 11px;
    color: var(--ink-muted);
    cursor: pointer;
    font-size: 12.5px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    .material-symbols-outlined { font-size: 14px; }
    .badge {
      font-family: var(--font-mono, ui-monospace, monospace);
      font-size: 10.5px;
      color: var(--ink-faint);
      background: var(--bg-soft);
      border-radius: 999px;
      padding: 1px 6px;
    }
    &:hover { color: var(--ink); }
    &.on {
      color: var(--ink);
      background: var(--bg-soft);
      border-color: var(--rule-strong);
      .badge { color: var(--ink); }
    }
  }
  .spacer { flex: 1; }
  .refresh {
    background: transparent;
    border: 1px solid var(--rule);
    border-radius: var(--radius-sm, 3px);
    padding: 4px;
    cursor: pointer;
    color: var(--ink-muted);
    display: inline-flex;
    align-items: center;
    &:hover { color: var(--ink); background: var(--bg-soft); }
    .material-symbols-outlined { font-size: 16px; }
  }
}
.body {
  display: grid;
  grid-template-columns: 1fr auto;
  overflow: hidden;
  min-height: 0;
}
</style>
