<script setup lang="ts">
import { ref, watch } from 'vue'
import type { ContextCard } from '@/types/context.types'

const props = defineProps<{ label: string; cards: ContextCard[] }>()
const emit = defineEmits<{
  (e: 'edit', id: string, text: string): void
  (e: 'add', text: string): void
  (e: 'delete', id: string): void
}>()

const editingId = ref<string | null>(null)
const draft = ref('')
const adding = ref(false)
const newText = ref('')

watch(
  () => props.cards,
  () => {
    if (editingId.value && !props.cards.find((c) => c.id === editingId.value)) {
      editingId.value = null
    }
  },
)

function startEdit(card: ContextCard) {
  editingId.value = card.id
  draft.value = card.text
}
function commitEdit() {
  if (!editingId.value) return
  const txt = draft.value.trim()
  if (txt) emit('edit', editingId.value, txt)
  editingId.value = null
}
function cancelEdit() {
  editingId.value = null
}

function startAdd() {
  adding.value = true
  newText.value = ''
}
function commitAdd() {
  const txt = newText.value.trim()
  if (txt) emit('add', txt)
  adding.value = false
  newText.value = ''
}
function cancelAdd() {
  adding.value = false
  newText.value = ''
}
</script>

<template>
  <section class="cardlist">
    <header class="head">
      <span class="label">{{ label }}</span>
      <button class="add" :title="`Add ${label}`" @click="startAdd">
        <span class="material-symbols-outlined">add</span>
      </button>
    </header>

    <div class="rows">
      <div
        v-for="card in cards"
        :key="card.id"
        class="card"
        :class="{ edited: card.edited_by_user, editing: editingId === card.id }"
      >
        <template v-if="editingId === card.id">
          <textarea
            v-model="draft"
            rows="2"
            class="ed"
            @keydown.esc="cancelEdit"
            @keydown.meta.enter="commitEdit"
            @keydown.ctrl.enter="commitEdit"
          ></textarea>
          <div class="row-actions">
            <button class="btn ghost" @click="cancelEdit">Cancel</button>
            <button class="btn primary" @click="commitEdit">Save</button>
          </div>
        </template>
        <template v-else>
          <p class="text" @click="startEdit(card)">
            {{ card.text }}
            <span v-if="card.edited_by_user" class="badge">edited</span>
          </p>
          <button class="del" :title="`Delete`" @click.stop="$emit('delete', card.id)">
            <span class="material-symbols-outlined">close</span>
          </button>
        </template>
      </div>

      <div v-if="adding" class="card editing">
        <textarea
          v-model="newText"
          rows="2"
          class="ed"
          :placeholder="`New ${label.toLowerCase()}…`"
          @keydown.esc="cancelAdd"
          @keydown.meta.enter="commitAdd"
          @keydown.ctrl.enter="commitAdd"
        ></textarea>
        <div class="row-actions">
          <button class="btn ghost" @click="cancelAdd">Cancel</button>
          <button class="btn primary" @click="commitAdd">Add</button>
        </div>
      </div>

      <p v-else-if="cards.length === 0" class="empty">No {{ label.toLowerCase() }} yet.</p>
    </div>
  </section>
</template>

<style scoped lang="scss">
.cardlist {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  .label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .add {
    background: transparent;
    border: 0;
    cursor: pointer;
    color: var(--ink-faint);
    padding: 2px;
    border-radius: var(--radius-sm, 3px);
    display: inline-flex;
    align-items: center;
    &:hover { color: var(--ink); background: var(--bg-hover); }
    .material-symbols-outlined { font-size: 14px; }
  }
}
.rows {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.card {
  background: var(--bg);
  border: 1px solid var(--rule);
  border-radius: var(--radius-md, 4px);
  padding: 8px 10px;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 6px;
  font-size: 12.5px;
  line-height: 1.45;
  &.edited { border-left: 2px solid var(--accent); }
  &.editing { display: block; }
  .text {
    margin: 0;
    flex: 1;
    cursor: text;
    color: var(--ink);
  }
  .badge {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
    margin-left: 6px;
  }
  .del {
    background: transparent;
    border: 0;
    cursor: pointer;
    color: var(--ink-faint);
    border-radius: var(--radius-sm, 3px);
    padding: 1px;
    display: inline-flex;
    align-items: center;
    visibility: hidden;
    &:hover { color: var(--ink); }
    .material-symbols-outlined { font-size: 13px; }
  }
  &:hover .del { visibility: visible; }
  .ed {
    width: 100%;
    box-sizing: border-box;
    font: inherit;
    font-size: 12.5px;
    line-height: 1.5;
    background: var(--bg);
    border: 1px solid var(--rule-strong);
    border-radius: var(--radius-sm, 3px);
    padding: 4px 6px;
    color: var(--ink);
    resize: vertical;
    &:focus { outline: 0; border-color: var(--accent); }
  }
  .row-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    margin-top: 6px;
  }
}
.empty {
  margin: 0;
  font-size: 11.5px;
  color: var(--ink-faint);
  font-style: italic;
}
.btn {
  font: inherit;
  font-size: 11px;
  font-weight: 600;
  border-radius: var(--radius-sm, 3px);
  padding: 3px 8px;
  cursor: pointer;
  &.ghost {
    background: transparent;
    border: 1px solid var(--rule);
    color: var(--ink-muted);
    &:hover { color: var(--ink); }
  }
  &.primary {
    background: var(--ink);
    color: var(--bg);
    border: 0;
    &:hover { background: var(--accent); }
  }
}
</style>
