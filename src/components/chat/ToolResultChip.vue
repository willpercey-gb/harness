<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ name: string; status: 'success' | 'error' }>()
const icon = computed(() => (props.status === 'success' ? 'check_circle' : 'error'))
</script>

<template>
  <div class="tool-result" :class="status">
    <span class="tool-icon material-symbols-outlined">{{ icon }}</span>
    <span class="tool-name">{{ name }}</span>
    <span class="tool-status">{{ status }}</span>
  </div>
</template>

<style scoped lang="scss">
.tool-result {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  margin: 8px 0;
  border-radius: 16px;
  font-size: 12px;
  font-weight: 600;
  width: fit-content;

  &.success {
    background-color: #d4edda;
    color: #155724;
    border: 1px solid #c3e6cb;
  }
  &.error {
    background-color: #f8d7da;
    color: #721c24;
    border: 1px solid #f5c6cb;
  }
  .tool-icon { font-size: 14px; }
  .tool-name { text-transform: capitalize; }
  .tool-status { text-transform: uppercase; font-size: 10px; opacity: 0.8; }
}
</style>
