<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { getRecentMemories, ingestMarkdownFolder } from '@/services/knowledge'
import type { MemoryChunk } from '@/types/knowledge.types'

const memories = ref<MemoryChunk[]>([])
const loading = ref(false)
const error = ref('')
const search = ref('')
const selectedSource = ref<string>('all')
const ingesting = ref(false)
const ingestStatus = ref<string>('')

async function reload() {
  loading.value = true
  error.value = ''
  try {
    memories.value = await getRecentMemories(200)
  } catch (e: any) {
    error.value = e?.message ?? String(e)
  } finally {
    loading.value = false
  }
}

const sources = computed(() => {
  const s = new Set<string>()
  for (const m of memories.value) s.add(m.source_type)
  return ['all', ...Array.from(s).sort()]
})

const filtered = computed(() => {
  const q = search.value.trim().toLowerCase()
  return memories.value.filter((m) => {
    if (selectedSource.value !== 'all' && m.source_type !== selectedSource.value) return false
    if (!q) return true
    return (
      m.content.toLowerCase().includes(q) ||
      (m.summary?.toLowerCase().includes(q) ?? false)
    )
  })
})

function relativeTime(iso: string): string {
  const then = new Date(iso).getTime()
  const mins = Math.floor((Date.now() - then) / 60000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  const h = Math.floor(mins / 60)
  if (h < 24) return `${h}h ago`
  const d = Math.floor(h / 24)
  if (d < 7) return `${d}d ago`
  return new Date(iso).toLocaleDateString('en-GB', { month: 'short', day: 'numeric' })
}

const expandedId = ref<string | null>(null)
function toggleExpand(id: string | null) {
  expandedId.value = expandedId.value === id ? null : id
}

async function runIngest() {
  if (ingesting.value) return
  ingesting.value = true
  ingestStatus.value = 'Ingesting ~/…'
  try {
    const result = await ingestMarkdownFolder()
    ingestStatus.value = `Done: ${result.files_ingested} files, ${result.chunks_inserted} new chunks${
      result.errors ? `, ${result.errors} errors` : ''
    }`
    await reload()
  } catch (e: any) {
    ingestStatus.value = `Failed: ${e?.message ?? String(e)}`
  } finally {
    ingesting.value = false
  }
}

onMounted(reload)
</script>

<template>
  <div class="timeline">
    <header class="head">
      <input
        v-model="search"
        type="search"
        placeholder="Filter memories…"
        class="search"
      />
      <select v-model="selectedSource" class="source">
        <option v-for="s in sources" :key="s" :value="s">
          {{ s === 'all' ? 'All sources' : s }}
        </option>
      </select>
      <button
        class="refresh"
        :class="{ on: ingesting }"
        :disabled="ingesting"
        @click="runIngest"
        :title="ingesting ? 'Ingesting…' : 'Ingest markdown from ~/'"
      >
        <span class="material-symbols-outlined">{{ ingesting ? 'progress_activity' : 'folder_open' }}</span>
      </button>
      <button class="refresh" @click="reload" title="Reload">
        <span class="material-symbols-outlined">refresh</span>
      </button>
      <span class="count">{{ filtered.length }} / {{ memories.length }}</span>
    </header>

    <div v-if="ingestStatus" class="ingest-status-bar">{{ ingestStatus }}</div>

    <div v-if="loading" class="empty">Loading memories…</div>
    <div v-else-if="error" class="empty err">{{ error }}</div>
    <div v-else-if="memories.length === 0" class="empty">
      No memories yet. Have a conversation — the passive extractor saves salient
      facts after each turn. Ingesting a markdown folder via the
      <span class="material-symbols-outlined inline-icon">folder_open</span>
      button above also populates this list.
    </div>

    <ul v-else class="entries">
      <li
        v-for="m in filtered"
        :key="m.id ?? m.content"
        class="entry"
        :class="{ expanded: expandedId === m.id }"
        @click="toggleExpand(m.id)"
      >
        <div class="entry-meta">
          <span class="src">{{ m.source_type }}</span>
          <span class="time">{{ relativeTime(m.timestamp) }}</span>
          <span v-if="m.source_id" class="origin" :title="m.source_id">
            · {{ m.source_id.slice(0, 8) }}
          </span>
        </div>
        <p v-if="m.summary" class="summary">{{ m.summary }}</p>
        <p class="content" :class="{ truncated: expandedId !== m.id }">{{ m.content }}</p>
      </li>
    </ul>
  </div>
</template>

<style scoped lang="scss">
.timeline {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  background: var(--bg);
}
.ingest-status-bar {
  padding: 6px 14px;
  border-bottom: 1px solid var(--rule);
  background: var(--bg-soft);
  font-size: 11.5px;
  color: var(--ink-muted);
  font-family: var(--font-mono, ui-monospace, monospace);
}
.head {
  display: flex;
  align-items: center;
  gap: 10px;
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
  .source {
    font: inherit;
    font-size: 12px;
    background: var(--bg-soft);
    border: 1px solid var(--rule);
    border-radius: var(--radius-md);
    padding: 5px 8px;
    color: var(--ink);
  }
  .refresh {
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
  .count {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 11px;
    color: var(--ink-faint);
  }
}
.empty {
  padding: 32px;
  text-align: center;
  color: var(--ink-faint);
  font-size: 13px;
  &.err { color: #ef4444; }
  code {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
    background: var(--bg-soft);
    padding: 1px 5px;
    border-radius: 3px;
    margin: 0 3px;
  }
  .inline-icon {
    font-size: 14px;
    vertical-align: middle;
    margin: 0 2px;
  }
}
.entries {
  list-style: none;
  margin: 0;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
}
.entry {
  background: var(--bg-soft);
  border: 1px solid var(--rule);
  border-radius: var(--radius-md);
  padding: 10px 12px;
  cursor: pointer;
  transition: border-color 0.12s;
  &:hover { border-color: var(--rule-strong); }
  &.expanded { border-color: var(--accent); }
}
.entry-meta {
  display: inline-flex;
  gap: 6px;
  align-items: baseline;
  font-size: 10.5px;
  color: var(--ink-faint);
  margin-bottom: 4px;
  .src {
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-weight: 700;
    color: var(--ink-muted);
  }
  .origin { font-family: var(--font-mono, ui-monospace, monospace); }
}
.summary {
  margin: 0 0 4px 0;
  font-size: 12.5px;
  color: var(--ink-muted);
  font-style: italic;
}
.content {
  margin: 0;
  font-size: 13px;
  line-height: 1.55;
  color: var(--ink);
  white-space: pre-wrap;
  &.truncated {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
}
</style>
