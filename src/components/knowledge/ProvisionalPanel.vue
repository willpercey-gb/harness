<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  discardProvisional,
  listProvisional,
  promoteProvisional,
  promoteProvisionalAsNew,
  type ProvisionalEntry,
} from '@/services/knowledge'

const items = ref<ProvisionalEntry[]>([])
const loading = ref(false)
const error = ref('')
const busyIds = ref<Set<string>>(new Set())

async function reload() {
  loading.value = true
  error.value = ''
  try {
    items.value = await listProvisional(200)
  } catch (e: any) {
    error.value = e?.message ?? String(e)
  } finally {
    loading.value = false
  }
}

async function withBusy(id: string, fn: () => Promise<unknown>) {
  busyIds.value.add(id)
  try {
    await fn()
    items.value = items.value.filter((i) => i.id !== id)
  } catch (e: any) {
    error.value = e?.message ?? String(e)
  } finally {
    busyIds.value.delete(id)
  }
}

const empty = computed(() => !loading.value && items.value.length === 0)

onMounted(reload)
</script>

<template>
  <section class="provisional">
    <header>
      <h2>Pending decisions</h2>
      <button class="refresh" @click="reload" title="Reload">
        <span class="material-symbols-outlined">refresh</span>
      </button>
    </header>

    <p v-if="empty" class="empty">
      No pending entity decisions. Anything the extractor was unsure about will appear here for
      manual disambiguation.
    </p>

    <p v-if="error" class="err">{{ error }}</p>

    <ul v-if="!empty" class="rows">
      <li v-for="row in items" :key="row.id" :class="{ busy: busyIds.has(row.id) }">
        <div class="row-head">
          <span class="name">{{ row.entity_name }}</span>
          <span class="kind">{{ row.entity_type }}</span>
          <span v-if="row.top_score !== null" class="score">
            {{ (row.top_score * 100).toFixed(0) }}%
          </span>
          <span class="seen">×{{ row.seen_count }}</span>
        </div>

        <p v-if="row.candidates.length === 0" class="no-cands">
          No candidate matches found above the uncertain threshold.
        </p>

        <ul v-else class="candidates">
          <li v-for="c in row.candidates" :key="c.id">
            <button
              class="merge"
              :disabled="busyIds.has(row.id)"
              @click="withBusy(row.id, () => promoteProvisional(row.id, c.id))"
            >
              Merge with
              <strong>{{ c.canonical_name }}</strong>
              <span class="kind">{{ c.entity_type }}</span>
            </button>
            <p v-if="c.description" class="desc">{{ c.description }}</p>
          </li>
        </ul>

        <div class="actions">
          <button
            class="action create"
            :disabled="busyIds.has(row.id)"
            @click="withBusy(row.id, () => promoteProvisionalAsNew(row.id))"
          >
            Create as new
          </button>
          <button
            class="action discard"
            :disabled="busyIds.has(row.id)"
            @click="withBusy(row.id, () => discardProvisional(row.id))"
          >
            Discard
          </button>
        </div>
      </li>
    </ul>
  </section>
</template>

<style scoped lang="scss">
.provisional {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg);
  overflow-y: auto;

  > header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--rule);
    h2 {
      margin: 0;
      font-size: 14px;
      font-weight: 600;
      letter-spacing: -0.01em;
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
  }
}
.empty, .err {
  padding: 24px 16px;
  font-size: 13px;
  color: var(--ink-faint);
}
.err { color: #ef4444; }
.rows {
  list-style: none;
  margin: 0;
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  > li {
    border: 1px solid var(--rule);
    border-radius: var(--radius-md, 6px);
    padding: 10px 12px;
    background: var(--bg-soft);
    transition: opacity 0.12s;
    &.busy { opacity: 0.5; pointer-events: none; }
  }
}
.row-head {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 6px;
  .name {
    font-weight: 600;
    color: var(--ink);
    font-size: 14px;
  }
  .kind {
    text-transform: uppercase;
    letter-spacing: 0.08em;
    font-size: 9.5px;
    color: var(--ink-faint);
  }
  .score {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 11px;
    color: var(--ink-muted);
  }
  .seen {
    margin-left: auto;
    font-size: 11px;
    color: var(--ink-faint);
  }
}
.no-cands {
  margin: 4px 0 6px 0;
  font-size: 12px;
  color: var(--ink-faint);
  font-style: italic;
}
.candidates {
  list-style: none;
  margin: 0 0 8px 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  > li {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .merge {
    text-align: left;
    background: var(--bg);
    border: 1px solid var(--rule);
    border-radius: var(--radius-sm, 4px);
    padding: 6px 10px;
    cursor: pointer;
    color: var(--ink);
    font-size: 12.5px;
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    strong { font-weight: 600; }
    .kind {
      text-transform: uppercase;
      letter-spacing: 0.08em;
      font-size: 9.5px;
      color: var(--ink-faint);
      margin-left: auto;
    }
    &:hover:not(:disabled) {
      background: var(--bg-hover, var(--bg));
      border-color: var(--rule-strong);
    }
    &:disabled { cursor: not-allowed; opacity: 0.5; }
  }
  .desc {
    margin: 0;
    padding: 0 12px;
    font-size: 11.5px;
    color: var(--ink-muted);
  }
}
.actions {
  display: flex;
  gap: 6px;
  .action {
    flex: 1;
    background: transparent;
    border: 1px solid var(--rule);
    border-radius: var(--radius-sm, 3px);
    padding: 4px 8px;
    cursor: pointer;
    font-size: 11.5px;
    color: var(--ink-muted);
    &:hover:not(:disabled) { color: var(--ink); background: var(--bg); }
    &:disabled { cursor: not-allowed; opacity: 0.5; }
    &.discard:hover:not(:disabled) { color: #ef4444; }
  }
}
</style>
