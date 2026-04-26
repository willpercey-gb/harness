<script setup lang="ts">
import { computed } from 'vue'
import type { Intent, IntentSource } from '@/types/context.types'

const props = defineProps<{ intent: Intent | null; source: IntentSource | null }>()

const label = computed(() => (props.intent ? props.intent : '—'))
const klass = computed(() => ({
  manual: props.source === 'manual',
  auto: props.source === 'auto',
  [`intent-${props.intent ?? 'none'}`]: true,
}))
</script>

<template>
  <span class="intent" :class="klass" :title="source ? `${source} classification` : 'no intent yet'">
    <span class="dot"></span>
    <span class="label">{{ label }}</span>
    <span v-if="source" class="src">{{ source }}</span>
  </span>
</template>

<style scoped lang="scss">
.intent {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  padding: 3px 8px;
  border-radius: 999px;
  border: 1px solid var(--rule);
  background: var(--bg);
  color: var(--ink-muted);

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .src {
    font-size: 9px;
    opacity: 0.6;
    text-transform: lowercase;
    letter-spacing: 0;
  }
  &.manual {
    color: var(--accent);
    border-color: var(--accent);
  }
  &.intent-expand    { color: #16a34a; }
  &.intent-revise    { color: #f59e0b; }
  &.intent-redirect  { color: #ef4444; }
  &.intent-aside     { color: #6366f1; }
}
</style>
