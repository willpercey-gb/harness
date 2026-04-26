<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { getAgents } from '@/services/chat'
import { useChatStore } from '@/stores/chat'
import type { Agent } from '@/types/chat.types'

defineProps<{ compact?: boolean }>()

const chat = useChatStore()
const agents = ref<Agent[]>([])
const loading = ref(false)
const error = ref('')
const open = ref(false)
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

const providerCounts = computed(() => {
  const c: Record<string, number> = { all: agents.value.length }
  for (const a of agents.value) {
    const k = a.attributes.provider.toLowerCase()
    c[k] = (c[k] ?? 0) + 1
  }
  return c
})

const typeCounts = computed(() => {
  const c: Record<string, number> = {
    all: agents.value.length,
    agent: 0,
    swarm: 0,
    graph: 0,
    a2a: 0,
    distributed: 0,
  }
  for (const a of agents.value) c[a.type] = (c[a.type] ?? 0) + 1
  return c
})

function pick(a: Agent) {
  if (a.attributes.disabled) return
  chat.selectAgent(a)
  open.value = false
}

watch(open, (v) => { if (!v) search.value = '' })

function costLabel(c: string) {
  return ({ free: 'Free', 'very-low': 'Very low', low: 'Low', medium: 'Medium', high: 'High', 'very-high': 'Very high', uncalculated: '—' } as Record<string, string>)[c] ?? c
}
</script>

<template>
  <div class="selector" :class="{ compact }">
    <button class="trigger" @click="open = !open">
      <span class="model" v-if="chat.selectedAgent">{{ chat.selectedAgent.attributes.name }}</span>
      <span class="model muted" v-else>Select agent</span>
      <span class="caret material-symbols-outlined">expand_more</span>
    </button>

    <Transition name="fade">
      <div v-if="open" class="panel">
        <div class="search-row">
          <span class="material-symbols-outlined search-icon">search</span>
          <input
            v-model="search"
            class="search"
            placeholder="Search agents…"
            spellcheck="false"
          />
        </div>

        <div class="chips">
          <button
            v-for="p in ['all', 'ollama', 'openrouter', 'claudecli']"
            :key="`p-${p}`"
            class="chip"
            :class="{ on: filterProvider === p }"
            @click="filterProvider = p"
          >{{ p === 'all' ? 'All' : p === 'claudecli' ? 'Claude CLI' : p.charAt(0).toUpperCase() + p.slice(1) }} <span class="n">{{ providerCounts[p] ?? 0 }}</span></button>
          <span class="chip-sep"></span>
          <button
            v-for="t in ['all', 'agent']"
            :key="`t-${t}`"
            class="chip"
            :class="{ on: filterType === t }"
            @click="filterType = t"
          >{{ t === 'all' ? 'All types' : t }} <span class="n">{{ typeCounts[t] ?? 0 }}</span></button>
        </div>

        <div class="results">
          <div v-if="loading" class="empty">Loading agents…</div>
          <div v-else-if="error" class="empty err">{{ error }}</div>
          <div v-else-if="agents.length === 0" class="empty">No agents — is Ollama running?</div>

          <template v-else v-for="(group, provider) in grouped" :key="provider">
            <div class="group-label">{{ provider }}</div>
            <button
              v-for="a in group"
              :key="a.id"
              class="agent-row"
              :class="{ selected: chat.selectedAgent?.id === a.id, disabled: a.attributes.disabled }"
              @click="pick(a)"
            >
              <div class="row-main">
                <span class="agent-name">{{ a.attributes.name }}</span>
                <span class="agent-meta">
                  <span v-if="a.attributes.parameters" class="tag">{{ a.attributes.parameters }}B</span>
                  <span class="tag">{{ costLabel(a.attributes.cost) }}</span>
                  <span v-if="a.attributes.supportsTools" class="tag tools">tools</span>
                  <span v-if="a.attributes.disabled" class="tag warn">disabled</span>
                </span>
              </div>
              <p class="agent-desc">{{ a.attributes.description }}</p>
            </button>
          </template>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped lang="scss">
.selector { position: relative; display: inline-block; min-width: 0; }
.selector.compact { min-width: 0; max-width: 100%; }

.trigger {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: transparent;
  border: 1px solid var(--rule);
  padding: 6px 8px 6px 12px;
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--ink);
  font-size: 13.5px;
  font-weight: 500;
  transition: all 0.12s;
  max-width: 100%;
  .model {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .model.muted { color: var(--ink-faint); font-weight: 400; }
  .caret { font-size: 18px; color: var(--ink-muted); flex-shrink: 0; }
  &:hover {
    background: var(--bg-soft);
    border-color: var(--rule-strong);
  }
}

.selector.compact .trigger {
  border: 0;
  padding: 4px 4px 4px 8px;
  font-size: 12.5px;
  font-weight: 500;
  color: var(--ink-muted);
  border-radius: var(--radius-md);
  .caret { font-size: 16px; }
  &:hover {
    background: var(--bg);
    color: var(--ink);
  }
}

.panel {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  width: min(520px, 80vw);
  max-height: 460px;
  background: var(--bg);
  border: 1px solid var(--rule-strong);
  border-radius: var(--radius-lg);
  box-shadow: 0 -8px 32px rgba(0, 0, 0, 0.12);
  z-index: 40;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.search-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--rule);
  .search-icon { color: var(--ink-faint); font-size: 18px; }
}
.search {
  flex: 1;
  border: 0;
  outline: 0;
  font: inherit;
  font-size: 14px;
  background: transparent;
  color: var(--ink);
  &::placeholder { color: var(--ink-faint); }
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--rule);
}
.chip-sep {
  width: 1px;
  background: var(--rule);
  margin: 2px 4px;
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 4px 10px;
  background: transparent;
  color: var(--ink-muted);
  border: 1px solid var(--rule);
  border-radius: 999px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  transition: all 0.12s;
  text-transform: capitalize;
  .n { color: var(--ink-faint); font-weight: 400; }
  &:hover { background: var(--bg-soft); color: var(--ink); }
  &.on {
    background: var(--ink);
    color: var(--bg);
    border-color: var(--ink);
    .n { color: var(--bg); opacity: 0.7; }
  }
}

.results {
  overflow-y: auto;
  padding: 6px 6px 8px;
}
.empty {
  padding: 18px;
  text-align: center;
  font-size: 13px;
  color: var(--ink-faint);
  &.err { color: #dc3545; }
}
.group-label {
  padding: 10px 12px 4px;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--ink-faint);
  font-weight: 600;
}
.agent-row {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 9px 12px;
  background: transparent;
  border: 0;
  border-radius: var(--radius-md);
  cursor: pointer;
  text-align: left;
  transition: background 0.12s;

  .row-main {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .agent-name { font-size: 14px; font-weight: 500; color: var(--ink); }
  .agent-meta { display: inline-flex; gap: 4px; align-items: center; }
  .tag {
    font-size: 10.5px;
    color: var(--ink-faint);
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--bg-deep);
    font-weight: 500;
    &.tools { color: #16a34a; background: #16a34a14; }
    &.warn { color: #dc6803; }
  }
  .agent-desc {
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--ink-muted);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 1;
    -webkit-box-orient: vertical;
  }

  &:hover:not(.disabled) { background: var(--bg-soft); }
  &.selected { background: var(--bg-soft); }
  &.disabled { opacity: 0.5; cursor: not-allowed; }
}

.fade-enter-active, .fade-leave-active {
  transition: opacity 0.14s ease, transform 0.14s ease;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>
