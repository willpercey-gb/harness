<script setup lang="ts">
import type { Intent } from '@/types/context.types'

defineProps<{ value: Intent | 'auto' }>()
defineEmits<{ (e: 'update:value', v: Intent | 'auto'): void }>()
</script>

<template>
  <label class="dropdown" :class="{ manual: value !== 'auto' }">
    <span class="label">Intent</span>
    <select
      :value="value"
      @change="$emit('update:value', ($event.target as HTMLSelectElement).value as Intent | 'auto')"
    >
      <option value="auto">Auto</option>
      <option value="expand">Expand</option>
      <option value="revise">Revise</option>
      <option value="redirect">Redirect</option>
      <option value="aside">Aside</option>
    </select>
  </label>
</template>

<style scoped lang="scss">
.dropdown {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--ink-faint);
  letter-spacing: 0.05em;
  text-transform: uppercase;
  font-weight: 600;

  select {
    font: inherit;
    font-size: 12px;
    text-transform: none;
    letter-spacing: 0;
    background: var(--bg);
    border: 1px solid var(--rule);
    border-radius: var(--radius-md, 4px);
    padding: 4px 8px;
    color: var(--ink);
    cursor: pointer;
    &:focus { outline: 0; border-color: var(--accent); }
  }
  &.manual select {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
}
</style>
