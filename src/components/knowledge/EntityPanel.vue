<script setup lang="ts">
import { computed } from 'vue'
import { useGraphData } from '@/composables/useGraphData'

const { selectedEntity, relatedMemories, isLoadingMemories, unfocus } = useGraphData()

const ENTITY_COLORS: Record<string, string> = {
  person: '#4fc3f7',
  organization: '#81c784',
  project: '#ba68c8',
  technology: '#ffb74d',
  topic: '#90a4ae',
  location: '#f48fb1',
  component: '#7986cb',
}

const color = computed(() =>
  selectedEntity.value ? ENTITY_COLORS[selectedEntity.value.entity_type] || '#90a4ae' : '#90a4ae',
)

function relativeTime(iso: string): string {
  const then = new Date(iso).getTime()
  const mins = Math.floor((Date.now() - then) / 60000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  const h = Math.floor(mins / 60)
  if (h < 24) return `${h}h ago`
  const d = Math.floor(h / 24)
  return `${d}d ago`
}
</script>

<template>
  <aside v-if="selectedEntity" class="panel">
    <header class="head">
      <div class="title-row">
        <span class="dot" :style="{ background: color }"></span>
        <h2 class="name">{{ selectedEntity.name }}</h2>
      </div>
      <button class="close" @click="unfocus" title="Close">
        <span class="material-symbols-outlined">close</span>
      </button>
    </header>

    <div class="type-row">
      <span class="type-pill" :style="{ '--c': color }">{{ selectedEntity.entity_type }}</span>
      <span v-if="selectedEntity.aliases.length" class="aliases">
        also known as {{ selectedEntity.aliases.join(', ') }}
      </span>
    </div>

    <div v-if="selectedEntity.description" class="description">
      {{ selectedEntity.description }}
    </div>

    <div v-if="selectedEntity.content" class="content">
      <h3>Notes</h3>
      <p>{{ selectedEntity.content }}</p>
    </div>

    <section class="memories">
      <h3>Related memories</h3>
      <div v-if="isLoadingMemories" class="loading">searching…</div>
      <div v-else-if="relatedMemories.length === 0" class="empty">
        No memories mention this entity yet.
      </div>
      <ul v-else class="mem-list">
        <li v-for="(m, i) in relatedMemories" :key="i" class="mem">
          <div class="mem-meta">
            <span class="src">{{ m.source_type }}</span>
            <span class="time">· {{ relativeTime(m.timestamp) }}</span>
            <span class="score">· score {{ m.score.toFixed(2) }}</span>
          </div>
          <p class="mem-content">{{ m.content }}</p>
        </li>
      </ul>
    </section>
  </aside>
</template>

<style scoped lang="scss">
.panel {
  width: 360px;
  height: 100%;
  background: var(--bg-soft);
  border-left: 1px solid var(--rule);
  padding: 16px 18px;
  overflow-y: auto;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  .title-row {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .name {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .close {
    background: transparent;
    border: 0;
    cursor: pointer;
    color: var(--ink-faint);
    padding: 4px;
    border-radius: var(--radius-sm, 3px);
    display: inline-flex;
    align-items: center;
    &:hover { color: var(--ink); background: var(--bg); }
    .material-symbols-outlined { font-size: 16px; }
  }
}
.type-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  .type-pill {
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 2px 8px;
    border-radius: 999px;
    color: var(--c);
    border: 1px solid var(--c);
    background: transparent;
  }
  .aliases {
    font-size: 12px;
    color: var(--ink-faint);
    font-style: italic;
  }
}
.description {
  font-size: 13.5px;
  color: var(--ink);
  line-height: 1.5;
  padding: 8px 0;
  border-top: 1px solid var(--rule);
  border-bottom: 1px solid var(--rule);
}
.content {
  h3 {
    margin: 0 0 6px 0;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-faint);
    font-weight: 700;
  }
  p {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--ink);
    white-space: pre-wrap;
  }
}
.memories h3 {
  margin: 0 0 8px 0;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--ink-faint);
  font-weight: 700;
}
.loading,
.empty {
  font-size: 12px;
  color: var(--ink-faint);
  padding: 6px 0;
}
.mem-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.mem {
  background: var(--bg);
  border: 1px solid var(--rule);
  border-radius: var(--radius-md);
  padding: 8px 10px;
}
.mem-meta {
  font-size: 10.5px;
  color: var(--ink-faint);
  display: inline-flex;
  gap: 4px;
  margin-bottom: 4px;
  .src { text-transform: uppercase; letter-spacing: 0.08em; }
}
.mem-content {
  margin: 0;
  font-size: 12.5px;
  line-height: 1.5;
  color: var(--ink);
  white-space: pre-wrap;
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
