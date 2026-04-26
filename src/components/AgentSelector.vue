<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { getAgents } from '@/services/chat'
import { useChatStore } from '@/stores/chat'
import type { Agent } from '@/types/chat.types'

const chat = useChatStore()
const agents = ref<Agent[]>([])
const loading = ref(false)
const error = ref('')
const open = ref(false)
const detailsOpen = ref(false)
const search = ref('')
const filterType = ref<'all' | string>('all')
const filterProvider = ref<'all' | string>('all')

async function load() {
  loading.value = true
  error.value = ''
  try {
    agents.value = await getAgents()
    if (!chat.selectedAgent && agents.value.length) {
      const first = agents.value.find((a) => !a.attributes.disabled) ?? agents.value[0]
      chat.selectAgent(first)
    }
  } catch (e: any) {
    error.value = e?.message ?? String(e)
  } finally {
    loading.value = false
  }
}
onMounted(load)

const grouped = computed(() => {
  const filterText = search.value.trim().toLowerCase()
  const out: Record<string, Agent[]> = {}
  for (const a of agents.value) {
    const provider = a.attributes.provider
    if (filterProvider.value !== 'all' && provider.toLowerCase() !== filterProvider.value) continue
    if (filterType.value !== 'all' && a.type !== filterType.value) continue
    if (filterText && !`${a.attributes.name} ${a.attributes.description}`.toLowerCase().includes(filterText)) continue
    if (!out[provider]) out[provider] = []
    out[provider].push(a)
  }
  return out
})

const typeCounts = computed(() => {
  const c: Record<string, number> = { all: agents.value.length, agent: 0, swarm: 0, graph: 0, a2a: 0, distributed: 0 }
  for (const a of agents.value) c[a.type] = (c[a.type] ?? 0) + 1
  return c
})

const providerCounts = computed(() => {
  const c: Record<string, number> = { all: agents.value.length }
  for (const a of agents.value) {
    const k = a.attributes.provider.toLowerCase()
    c[k] = (c[k] ?? 0) + 1
  }
  return c
})

function pick(a: Agent) {
  if (a.attributes.disabled) return
  chat.selectAgent(a)
  open.value = false
  detailsOpen.value = false
}

watch(open, (v) => { if (!v) search.value = '' })

function costLabel(c: string) {
  return ({ free: 'free', 'very-low': 'very low', low: 'low', medium: 'medium', high: 'high', 'very-high': 'very high', uncalculated: '—' } as Record<string, string>)[c] ?? c
}
</script>

<template>
  <div class="selector">
    <button class="trigger" @click="open = !open" :class="{ disabled: !chat.selectedAgent }">
      <span class="eyebrow">talking with</span>
      <span class="name">
        <span v-if="!chat.selectedAgent" class="muted">— select an agent —</span>
        <template v-else>{{ chat.selectedAgent.attributes.name }}</template>
      </span>
      <span class="caret material-symbols-outlined">expand_more</span>
    </button>

    <Transition name="fade">
      <div v-if="open" class="panel">
        <div class="panel-head">
          <input
            v-model="search"
            class="search"
            placeholder="filter by name or description…"
            spellcheck="false"
          />
          <button class="details-btn" @click="detailsOpen = !detailsOpen">
            <span class="material-symbols-outlined">tune</span>
            <span>filters</span>
          </button>
        </div>

        <div v-if="detailsOpen" class="filters">
          <div class="filter-row">
            <span class="eyebrow">type</span>
            <button
              v-for="t in ['all', 'agent', 'swarm', 'graph', 'a2a', 'distributed']"
              :key="t"
              class="chip"
              :class="{ on: filterType === t }"
              @click="filterType = t"
            >
              {{ t }} <span class="n">{{ typeCounts[t] ?? 0 }}</span>
            </button>
          </div>
          <div class="filter-row">
            <span class="eyebrow">provider</span>
            <button
              v-for="p in ['all', 'ollama', 'openrouter', 'bedrock', 'vertex', 'openai']"
              :key="p"
              class="chip"
              :class="{ on: filterProvider === p }"
              @click="filterProvider = p"
            >
              {{ p }} <span class="n">{{ providerCounts[p] ?? 0 }}</span>
            </button>
          </div>
        </div>

        <div class="results">
          <div v-if="loading" class="empty">loading agents…</div>
          <div v-else-if="error" class="empty err">{{ error }}</div>
          <div v-else-if="agents.length === 0" class="empty">No agents — is Ollama running?</div>

          <template v-else v-for="(group, provider) in grouped" :key="provider">
            <div class="group-label">
              <span class="eyebrow">{{ provider }}</span>
              <span class="rule"></span>
              <span class="group-count">{{ group.length }}</span>
            </div>
            <button
              v-for="a in group"
              :key="a.id"
              class="agent-row"
              :class="{ selected: chat.selectedAgent?.id === a.id, disabled: a.attributes.disabled }"
              @click="pick(a)"
            >
              <span class="agent-name">{{ a.attributes.name }}</span>
              <span class="agent-meta">
                <span class="dim">{{ a.type }}</span>
                <span v-if="a.attributes.parameters" class="dim">{{ a.attributes.parameters }}B</span>
                <span class="dim">{{ costLabel(a.attributes.cost) }}</span>
                <span v-if="a.attributes.supportsTools" class="tools" title="Supports tools">tools</span>
                <span v-if="a.attributes.disabled" class="warn">disabled</span>
              </span>
              <span class="agent-desc">{{ a.attributes.description }}</span>
            </button>
          </template>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped lang="scss">
.selector {
  position: relative;
  display: inline-block;
}

// — Trigger ——————————————————————————————————————————————
.trigger {
  display: inline-flex;
  align-items: baseline;
  gap: 10px;
  background: transparent;
  border: 0;
  padding: 6px 0;
  cursor: pointer;
  color: var(--ink);
  border-bottom: 1px solid var(--rule);
  transition: border-color 0.18s, color 0.18s;

  .eyebrow {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink-faint);
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }
  .name {
    font-family: var(--font-display);
    font-size: 18px;
    font-weight: 400;
    font-style: italic;
    color: var(--ink);
    letter-spacing: -0.01em;
    .muted { color: var(--ink-faint); font-style: italic; }
  }
  .caret {
    font-size: 16px;
    color: var(--ink-faint);
    transform: translateY(2px);
    transition: transform 0.2s;
  }
  &:hover {
    border-color: var(--rule-strong);
    .caret { color: var(--ink); }
  }
}

// — Dropdown panel ——————————————————————————————————————————————
.panel {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  width: min(560px, 80vw);
  max-height: 480px;
  background: var(--bg);
  border: 1px solid var(--rule-strong);
  z-index: 50;
  display: flex;
  flex-direction: column;
  box-shadow: 0 24px 48px -16px rgba(0, 0, 0, 0.25);
}
.panel-head {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 0;
  border-bottom: 1px solid var(--rule);
}
.search {
  font: inherit;
  font-family: var(--font-body);
  font-size: 14px;
  padding: 12px 14px;
  background: transparent;
  border: 0;
  color: var(--ink);
  &::placeholder { color: var(--ink-faint); font-style: italic; }
  &:focus { outline: 0; background: var(--bg-soft); }
}
.details-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 14px;
  background: transparent;
  border: 0;
  border-left: 1px solid var(--rule);
  cursor: pointer;
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.06em;
  color: var(--ink-muted);
  text-transform: uppercase;
  &:hover { color: var(--ink); background: var(--bg-soft); }
  .material-symbols-outlined { font-size: 16px; }
}
.filters {
  border-bottom: 1px solid var(--rule);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.filter-row {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  .eyebrow {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--ink-faint);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    margin-right: 6px;
    min-width: 60px;
  }
}
.chip {
  font-family: var(--font-mono);
  font-size: 11px;
  letter-spacing: 0.04em;
  padding: 3px 8px;
  background: transparent;
  border: 1px solid var(--rule);
  color: var(--ink-muted);
  cursor: pointer;
  text-transform: lowercase;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  transition: all 0.16s;
  .n { color: var(--ink-faint); }
  &:hover { color: var(--ink); border-color: var(--rule-strong); }
  &.on {
    color: var(--bg);
    background: var(--ink);
    border-color: var(--ink);
    .n { color: var(--bg); opacity: 0.7; }
  }
}

// — Results list ——————————————————————————————————————————————
.results {
  overflow-y: auto;
  padding: 6px 0 12px;
}
.empty {
  padding: 24px 14px;
  font-family: var(--font-body);
  font-style: italic;
  color: var(--ink-faint);
  &.err { color: var(--accent); }
}
.group-label {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 10px;
  padding: 14px 14px 6px;
  .eyebrow {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink);
    letter-spacing: 0.18em;
    text-transform: uppercase;
  }
  .rule { height: 1px; background: var(--rule); }
  .group-count {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--ink-faint);
  }
}
.agent-row {
  display: grid;
  grid-template-columns: auto 1fr;
  grid-template-rows: auto auto;
  grid-template-areas:
    "name meta"
    "desc desc";
  gap: 4px 12px;
  padding: 10px 14px;
  background: transparent;
  border: 0;
  border-left: 2px solid transparent;
  cursor: pointer;
  text-align: left;
  transition: background 0.14s, border-color 0.14s;

  .agent-name {
    grid-area: name;
    font-family: var(--font-display);
    font-weight: 400;
    font-size: 16px;
    color: var(--ink);
    letter-spacing: -0.01em;
  }
  .agent-meta {
    grid-area: meta;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    justify-self: end;
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: lowercase;
    color: var(--ink-muted);
    .dim { color: var(--ink-faint); }
    .tools { color: var(--accent); }
    .warn { color: var(--accent); font-style: italic; }
  }
  .agent-desc {
    grid-area: desc;
    font-family: var(--font-body);
    font-size: 13px;
    color: var(--ink-muted);
    line-height: 1.45;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 1;
    -webkit-box-orient: vertical;
  }

  &:hover:not(.disabled) {
    background: var(--bg-soft);
    border-left-color: var(--accent);
  }
  &.selected {
    background: var(--bg-soft);
    border-left-color: var(--accent);
  }
  &.disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
}

// — Transitions ——————————————————————————————————————————————
.fade-enter-active, .fade-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
