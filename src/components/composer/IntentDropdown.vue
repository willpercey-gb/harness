<script setup lang="ts">
import type { Intent } from '@/types/context.types'

defineProps<{ value: Intent | 'auto'; compact?: boolean }>()
defineEmits<{ (e: 'update:value', v: Intent | 'auto'): void }>()
</script>

<template>
  <label class="dropdown" :class="{ manual: value !== 'auto', compact }">
    <select
      :value="value"
      :title="value === 'auto' ? 'Intent (auto-classified)' : `Intent: ${value}`"
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

  select {
    font: inherit;
    font-size: 12.5px;
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

  &.compact select {
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    border: 0;
    padding: 4px 18px 4px 8px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--ink-muted);
    border-radius: var(--radius-md, 4px);
    background-image: linear-gradient(45deg, transparent 50%, var(--ink-muted) 50%),
                      linear-gradient(135deg, var(--ink-muted) 50%, transparent 50%);
    background-position: calc(100% - 10px) 50%, calc(100% - 6px) 50%;
    background-size: 4px 4px, 4px 4px;
    background-repeat: no-repeat;
    transition: background-color 0.12s, color 0.12s;
    &:hover {
      background-color: var(--bg);
      color: var(--ink);
    }
  }
  &.compact.manual select {
    color: var(--accent);
    background-image: linear-gradient(45deg, transparent 50%, var(--accent) 50%),
                      linear-gradient(135deg, var(--accent) 50%, transparent 50%);
  }
}
</style>
