<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useGraphData } from '@/composables/useGraphData'
import EntitySeedForm from '@/components/knowledge/EntitySeedForm.vue'
import {
  ENTITY_TYPES,
  RELATION_TYPES,
  archiveEntity,
  createRelationshipManual,
  editEntity,
  type EntityTypeName,
  type RelationTypeName,
} from '@/services/knowledge'

const {
  selectedEntity,
  relatedMemories,
  isLoadingMemories,
  unfocus,
  selectNode,
  allEntities,
  refresh,
} = useGraphData()

const linkRel = ref<RelationTypeName | ''>('')
const linkTargetId = ref<string>('')
const linkBusy = ref(false)
const linkError = ref('')

// Edit-mode state. While `editing` is true, all fields swap to inputs
// pre-populated from the current entity. Save commits via editEntity;
// Cancel restores from the live `selectedEntity`.
const editing = ref(false)
const editName = ref('')
const editType = ref<EntityTypeName>('project')
const editAliasesText = ref('')
const editDescription = ref('')
const editContent = ref('')
const editBusy = ref(false)
const editError = ref('')

function enterEdit() {
  if (!selectedEntity.value) return
  editName.value = selectedEntity.value.name
  editType.value = selectedEntity.value.entity_type as EntityTypeName
  editAliasesText.value = (selectedEntity.value.aliases ?? []).join(', ')
  editDescription.value = selectedEntity.value.description ?? ''
  editContent.value = selectedEntity.value.content ?? ''
  editError.value = ''
  editing.value = true
}

function cancelEdit() {
  editing.value = false
  editError.value = ''
}

async function saveEdit() {
  if (!selectedEntity.value || editBusy.value) return
  if (!editName.value.trim()) {
    editError.value = 'Name is required.'
    return
  }
  editBusy.value = true
  editError.value = ''
  try {
    const aliases = editAliasesText.value
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    const newId = await editEntity(selectedEntity.value.id, {
      name: editName.value.trim(),
      entityType: editType.value,
      aliases,
      description: editDescription.value.trim() || null,
      content: editContent.value.trim() || null,
    })
    await refresh()
    // If retype produced a new id, re-select that one. Otherwise the
    // existing selection still resolves correctly.
    if (newId !== selectedEntity.value?.id) {
      selectNode(newId)
    }
    editing.value = false
  } catch (e: any) {
    editError.value = e?.message ?? String(e)
  } finally {
    editBusy.value = false
  }
}

async function archive() {
  if (!selectedEntity.value || editBusy.value) return
  if (!confirm(`Archive "${selectedEntity.value.name}"? Edges remain but the node is hidden.`)) {
    return
  }
  editBusy.value = true
  try {
    await archiveEntity(selectedEntity.value.id)
    unfocus()
    await refresh()
  } catch (e: any) {
    editError.value = e?.message ?? String(e)
  } finally {
    editBusy.value = false
  }
}

// Drop edit-mode state if the user picks a different entity.
watch(
  () => selectedEntity.value?.id,
  () => {
    editing.value = false
    editError.value = ''
  },
)

const linkOptions = computed(() =>
  [...allEntities.value]
    .filter((e) => e.id !== selectedEntity.value?.id)
    .sort((a, b) => a.name.localeCompare(b.name)),
)

const canLink = computed(
  () => !!linkRel.value && !!linkTargetId.value && !linkBusy.value && !!selectedEntity.value,
)

async function addLink() {
  if (!canLink.value || !selectedEntity.value || !linkRel.value || !linkTargetId.value) return
  linkBusy.value = true
  linkError.value = ''
  try {
    await createRelationshipManual(
      selectedEntity.value.id,
      linkTargetId.value,
      linkRel.value as RelationTypeName,
    )
    linkRel.value = ''
    linkTargetId.value = ''
    await refresh()
  } catch (e: any) {
    linkError.value = e?.message ?? String(e)
  } finally {
    linkBusy.value = false
  }
}

// Reset the link form whenever the selected entity changes.
watch(
  () => selectedEntity.value?.id,
  () => {
    linkRel.value = ''
    linkTargetId.value = ''
    linkError.value = ''
  },
)

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
  <EntitySeedForm v-if="!selectedEntity" />
  <aside v-else class="panel">
    <header class="head">
      <div class="title-row">
        <span class="dot" :style="{ background: color }"></span>
        <h2 v-if="!editing" class="name">{{ selectedEntity.name }}</h2>
        <input
          v-else
          v-model="editName"
          class="name-input"
          :disabled="editBusy"
          autocomplete="off"
        />
      </div>
      <div class="head-actions">
        <button v-if="!editing" class="icon-btn" @click="enterEdit" title="Edit">
          <span class="material-symbols-outlined">edit</span>
        </button>
        <button v-if="!editing" class="icon-btn danger" @click="archive" title="Archive">
          <span class="material-symbols-outlined">archive</span>
        </button>
        <button class="icon-btn" @click="unfocus" title="Close">
          <span class="material-symbols-outlined">close</span>
        </button>
      </div>
    </header>

    <div class="type-row" v-if="!editing">
      <span class="type-pill" :style="{ '--c': color }">{{ selectedEntity.entity_type }}</span>
      <span v-if="selectedEntity.aliases.length" class="aliases">
        also known as {{ selectedEntity.aliases.join(', ') }}
      </span>
    </div>
    <div class="type-row" v-else>
      <select v-model="editType" :disabled="editBusy" class="type-select">
        <option v-for="t in ENTITY_TYPES" :key="t" :value="t">{{ t }}</option>
      </select>
      <span class="hint">Changing type moves the row to a different table; relationships follow.</span>
    </div>

    <template v-if="editing">
      <label class="edit-row">
        <span class="lbl">Aliases</span>
        <input
          v-model="editAliasesText"
          type="text"
          placeholder="comma-separated"
          :disabled="editBusy"
          autocomplete="off"
        />
      </label>
      <label class="edit-row">
        <span class="lbl">Description</span>
        <input
          v-model="editDescription"
          type="text"
          placeholder="One-line summary"
          :disabled="editBusy"
          autocomplete="off"
        />
      </label>
      <label class="edit-row">
        <span class="lbl">Notes</span>
        <textarea v-model="editContent" rows="4" :disabled="editBusy"></textarea>
      </label>
      <p v-if="editError" class="err">{{ editError }}</p>
      <div class="edit-actions">
        <button class="ghost" :disabled="editBusy" @click="cancelEdit">Cancel</button>
        <button class="primary" :disabled="editBusy" @click="saveEdit">
          {{ editBusy ? 'Saving…' : 'Save' }}
        </button>
      </div>
    </template>

    <template v-else>
      <div v-if="selectedEntity.description" class="description">
        {{ selectedEntity.description }}
      </div>

      <div v-if="selectedEntity.content" class="content">
        <h3>Notes</h3>
        <p>{{ selectedEntity.content }}</p>
      </div>
    </template>

    <section v-if="!editing" class="add-link">
      <h3>Link to another entity</h3>
      <div class="link-row">
        <select v-model="linkRel" :disabled="linkBusy">
          <option value="">— relation —</option>
          <option v-for="r in RELATION_TYPES" :key="r" :value="r">{{ r }}</option>
        </select>
        <select v-model="linkTargetId" :disabled="linkBusy || !linkRel">
          <option value="">— target —</option>
          <option v-for="ent in linkOptions" :key="ent.id" :value="ent.id">
            {{ ent.name }} ({{ ent.entity_type }})
          </option>
        </select>
        <button class="add" :disabled="!canLink" @click="addLink">
          {{ linkBusy ? '…' : 'Link' }}
        </button>
      </div>
      <p v-if="linkError" class="err">{{ linkError }}</p>
    </section>

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
  .name-input {
    flex: 1;
    min-width: 0;
    background: var(--bg);
    border: 1px solid var(--rule);
    border-radius: var(--radius-sm, 4px);
    padding: 4px 8px;
    font: inherit;
    font-size: 16px;
    font-weight: 600;
    color: var(--ink);
    &:focus { outline: 0; border-color: var(--accent, #3b82f6); }
  }
  .head-actions {
    display: inline-flex;
    gap: 2px;
  }
  .icon-btn {
    background: transparent;
    border: 0;
    cursor: pointer;
    color: var(--ink-faint);
    padding: 4px;
    border-radius: var(--radius-sm, 3px);
    display: inline-flex;
    align-items: center;
    &:hover { color: var(--ink); background: var(--bg); }
    &.danger:hover { color: #ef4444; }
    &:disabled { opacity: 0.4; cursor: not-allowed; }
    .material-symbols-outlined { font-size: 16px; }
  }
}
.type-row .type-select {
  background: var(--bg);
  border: 1px solid var(--rule);
  border-radius: var(--radius-sm, 4px);
  padding: 4px 8px;
  font: inherit;
  font-size: 12px;
  color: var(--ink);
  text-transform: lowercase;
}
.type-row .hint {
  font-size: 11px;
  color: var(--ink-faint);
  font-style: italic;
}
.edit-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
  .lbl {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-faint);
    font-weight: 600;
  }
  input,
  textarea {
    background: var(--bg);
    border: 1px solid var(--rule);
    border-radius: var(--radius-sm, 4px);
    padding: 6px 8px;
    font: inherit;
    font-size: 12.5px;
    color: var(--ink);
    &:focus { outline: 0; border-color: var(--accent, #3b82f6); }
    &:disabled { opacity: 0.5; }
  }
  textarea {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12px;
    resize: vertical;
  }
}
.edit-actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
  .ghost,
  .primary {
    border-radius: var(--radius-sm, 4px);
    padding: 5px 12px;
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid var(--rule);
    &:disabled { opacity: 0.4; cursor: not-allowed; }
  }
  .ghost {
    background: transparent;
    color: var(--ink-muted);
    &:hover:not(:disabled) { color: var(--ink); background: var(--bg); }
  }
  .primary {
    background: var(--ink);
    color: var(--bg);
    border-color: var(--ink);
    &:hover:not(:disabled) {
      background: var(--accent, #3b82f6);
      border-color: var(--accent, #3b82f6);
    }
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
.add-link {
  h3 {
    margin: 0 0 6px 0;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-faint);
    font-weight: 700;
  }
  .link-row {
    display: grid;
    grid-template-columns: 1fr 1fr auto;
    gap: 6px;
    select {
      background: var(--bg);
      border: 1px solid var(--rule);
      border-radius: var(--radius-sm, 4px);
      padding: 5px 6px;
      font: inherit;
      font-size: 11.5px;
      color: var(--ink);
      &:disabled { opacity: 0.5; }
    }
    .add {
      background: var(--ink);
      color: var(--bg);
      border: 0;
      border-radius: var(--radius-sm, 4px);
      padding: 4px 12px;
      font: inherit;
      font-size: 11.5px;
      font-weight: 600;
      cursor: pointer;
      &:disabled { opacity: 0.4; cursor: not-allowed; }
      &:hover:not(:disabled) { background: var(--accent, #3b82f6); }
    }
  }
  .err {
    margin: 6px 0 0 0;
    font-size: 11.5px;
    color: #ef4444;
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
