<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  ENTITY_TYPES,
  RELATION_TYPES,
  upsertEntityManual,
  createRelationshipManual,
  type EntityTypeName,
  type RelationTypeName,
} from '@/services/knowledge'
import { useGraphData } from '@/composables/useGraphData'

const emit = defineEmits<{
  (e: 'created', id: string): void
}>()

const { allEntities, refresh } = useGraphData()

const entityType = ref<EntityTypeName>('project')
const name = ref('')
const aliasesText = ref('')
const description = ref('')
const content = ref('')

// Optional one-shot link to a parent / sibling entity. The most common
// seed shape is `<thing> part_of <existing parent>` (Project X part_of
// Org O) so it's worth wiring this directly in the form rather
// than making the user click through to the relationship UI.
const linkRelation = ref<RelationTypeName | ''>('')
const linkTargetId = ref<string>('')

const submitting = ref(false)
const error = ref('')
const lastCreated = ref<string | null>(null)

const canSubmit = computed(() => !submitting.value && name.value.trim().length > 0)

const sortedEntities = computed(() =>
  [...allEntities.value].sort((a, b) => a.name.localeCompare(b.name)),
)

function reset() {
  name.value = ''
  aliasesText.value = ''
  description.value = ''
  content.value = ''
  linkRelation.value = ''
  linkTargetId.value = ''
  error.value = ''
}

async function submit() {
  if (!canSubmit.value) return
  submitting.value = true
  error.value = ''
  try {
    const aliases = aliasesText.value
      .split(',')
      .map((s) => s.trim())
      .filter(Boolean)
    const id = await upsertEntityManual({
      entityType: entityType.value,
      name: name.value.trim(),
      aliases: aliases.length ? aliases : undefined,
      description: description.value.trim() || null,
      content: content.value.trim() || null,
    })
    lastCreated.value = id

    if (linkRelation.value && linkTargetId.value) {
      try {
        await createRelationshipManual(
          id,
          linkTargetId.value,
          linkRelation.value as RelationTypeName,
        )
      } catch (e: any) {
        error.value = `Entity created, but link failed: ${e?.message ?? String(e)}`
      }
    }

    emit('created', id)
    await refresh()
    reset()
  } catch (e: any) {
    error.value = e?.message ?? String(e)
  } finally {
    submitting.value = false
  }
}

// Keep linkTargetId valid: if the entity list refreshes and our
// previously-picked target is gone, clear it.
watch(sortedEntities, (rows) => {
  if (linkTargetId.value && !rows.some((r) => r.id === linkTargetId.value)) {
    linkTargetId.value = ''
  }
})
</script>

<template>
  <form class="seed-form" @submit.prevent="submit">
    <header>
      <h2>Add entity</h2>
      <p class="hint">
        Seed entities here so the passive extractor has anchors to match against.
        Best done in advance for the people, projects, and orgs you mention often.
      </p>
    </header>

    <label class="row">
      <span class="label">Type</span>
      <select v-model="entityType" :disabled="submitting">
        <option v-for="t in ENTITY_TYPES" :key="t" :value="t">{{ t }}</option>
      </select>
    </label>

    <label class="row">
      <span class="label">Name</span>
      <input
        v-model="name"
        type="text"
        placeholder="e.g. Canonical name"
        :disabled="submitting"
        autocomplete="off"
      />
    </label>

    <label class="row">
      <span class="label">Aliases</span>
      <input
        v-model="aliasesText"
        type="text"
        placeholder="comma-separated · e.g. AcMe, Acme.IT, acmeit"
        :disabled="submitting"
        autocomplete="off"
      />
      <span class="hint inline">All variants the extractor might encounter.</span>
    </label>

    <label class="row">
      <span class="label">Description</span>
      <input
        v-model="description"
        type="text"
        placeholder="One-line summary"
        :disabled="submitting"
        autocomplete="off"
      />
    </label>

    <label class="row">
      <span class="label">Notes</span>
      <textarea
        v-model="content"
        rows="2"
        placeholder="Longer body — the extractor appends future detail here"
        :disabled="submitting"
      ></textarea>
    </label>

    <fieldset class="link">
      <legend>Optional link to existing</legend>
      <div class="link-row">
        <select v-model="linkRelation" :disabled="submitting" class="rel">
          <option value="">— no link —</option>
          <option v-for="r in RELATION_TYPES" :key="r" :value="r">{{ r }}</option>
        </select>
        <select
          v-model="linkTargetId"
          :disabled="submitting || !linkRelation"
          class="target"
        >
          <option value="">— pick target —</option>
          <option v-for="ent in sortedEntities" :key="ent.id" :value="ent.id">
            {{ ent.name }} ({{ ent.entity_type }})
          </option>
        </select>
      </div>
    </fieldset>

    <p v-if="error" class="err">{{ error }}</p>

    <div class="actions">
      <button type="submit" class="primary" :disabled="!canSubmit">
        {{ submitting ? 'Saving…' : 'Add entity' }}
      </button>
    </div>
  </form>
</template>

<style scoped lang="scss">
.seed-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
  background: var(--bg-soft);
  border-left: 1px solid var(--rule);
  height: 100%;
  width: 360px;
  box-sizing: border-box;
  overflow-y: auto;

  header {
    h2 {
      margin: 0 0 4px 0;
      font-size: 15px;
      font-weight: 600;
      color: var(--ink);
    }
    .hint {
      margin: 0;
      font-size: 12px;
      color: var(--ink-muted);
      line-height: 1.45;
    }
  }
}
.row {
  display: flex;
  flex-direction: column;
  gap: 4px;
  .label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-faint);
    font-weight: 600;
  }
  .hint.inline {
    font-size: 11px;
    color: var(--ink-faint);
  }
  input,
  select,
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
.link {
  border: 1px solid var(--rule);
  border-radius: var(--radius-sm, 4px);
  padding: 8px 10px 10px;
  margin: 0;
  legend {
    padding: 0 6px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--ink-faint);
    font-weight: 600;
  }
  .link-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    select {
      background: var(--bg);
      border: 1px solid var(--rule);
      border-radius: var(--radius-sm, 4px);
      padding: 5px 6px;
      font: inherit;
      font-size: 12px;
      color: var(--ink);
      &:disabled { opacity: 0.5; }
    }
  }
}
.err {
  margin: 0;
  font-size: 12px;
  color: #ef4444;
}
.actions {
  display: flex;
  justify-content: flex-end;
  margin-top: 4px;
}
.primary {
  background: var(--ink);
  color: var(--bg);
  border: 0;
  border-radius: var(--radius-md, 6px);
  padding: 6px 14px;
  font: inherit;
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  &:disabled { opacity: 0.4; cursor: not-allowed; }
  &:hover:not(:disabled) { background: var(--accent, #3b82f6); }
}
</style>
