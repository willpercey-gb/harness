<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps<{ text: string | null; refreshing: boolean }>()
const emit = defineEmits<{ (e: 'update', text: string): void }>()

const editing = ref(false)
const draft = ref(props.text ?? '')

watch(
  () => props.text,
  (v) => {
    if (!editing.value) draft.value = v ?? ''
  },
)

function start() {
  draft.value = props.text ?? ''
  editing.value = true
}

function commit() {
  editing.value = false
  emit('update', draft.value.trim())
}

function cancel() {
  editing.value = false
  draft.value = props.text ?? ''
}
</script>

<template>
  <section class="anchor" :class="{ refreshing, empty: !text && !editing }">
    <header class="head">
      <span class="label">Anchor</span>
      <span v-if="refreshing" class="status">refreshing…</span>
    </header>
    <div v-if="editing" class="editor">
      <textarea
        v-model="draft"
        class="ed"
        rows="3"
        placeholder="The user's overarching goal."
        @keydown.esc="cancel"
        @keydown.meta.enter="commit"
        @keydown.ctrl.enter="commit"
      ></textarea>
      <div class="actions">
        <button class="btn ghost" @click="cancel">Cancel</button>
        <button class="btn primary" @click="commit">Save</button>
      </div>
    </div>
    <div v-else-if="text" class="body" @click="start" title="Click to edit">
      {{ text }}
    </div>
    <button v-else class="empty-add" @click="start">
      <span class="material-symbols-outlined">add</span>
      Set anchor
    </button>
  </section>
</template>

<style scoped lang="scss">
.anchor {
  background: var(--bg);
  border: 1px solid var(--rule);
  border-radius: var(--radius-lg, 8px);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  &.refreshing { border-color: var(--accent); }
}
.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  .label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .status {
    font-size: 11px;
    font-style: italic;
    color: var(--accent);
  }
}
.body {
  font-size: 13px;
  line-height: 1.5;
  color: var(--ink);
  cursor: text;
  &:hover { background: var(--bg-soft); }
  border-radius: var(--radius-md, 4px);
  padding: 4px 0;
}
.empty-add {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: transparent;
  border: 1px dashed var(--rule-strong);
  border-radius: var(--radius-md, 4px);
  padding: 6px 10px;
  color: var(--ink-faint);
  font-size: 12px;
  cursor: pointer;
  &:hover { color: var(--ink); border-color: var(--ink-faint); }
  .material-symbols-outlined { font-size: 14px; }
}
.editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
  .ed {
    font: inherit;
    font-size: 13px;
    line-height: 1.5;
    background: var(--bg);
    border: 1px solid var(--rule-strong);
    border-radius: var(--radius-md, 4px);
    padding: 6px 8px;
    color: var(--ink);
    resize: vertical;
    min-height: 60px;
    &:focus { outline: 0; border-color: var(--accent); }
  }
  .actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }
}
.btn {
  font: inherit;
  font-size: 11px;
  font-weight: 600;
  border-radius: var(--radius-sm, 3px);
  padding: 4px 10px;
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
