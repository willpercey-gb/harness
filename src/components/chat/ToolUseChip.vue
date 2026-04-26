<script setup lang="ts">
defineProps<{ name: string }>()
</script>

<template>
  <div class="tool-use">
    <span class="tool-icon material-symbols-outlined">build_circle</span>
    <span class="tool-name">{{ name }}</span>
  </div>
</template>

<style scoped lang="scss">
.tool-use {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  margin: 8px 0;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  color: white;
  border-radius: 16px;
  font-size: 12px;
  font-weight: 600;
  width: fit-content;
  box-shadow: 0 2px 6px rgba(102, 126, 234, 0.2);

  .tool-icon { font-size: 14px; }
  .tool-name { text-transform: capitalize; letter-spacing: 0.3px; }
}
</style>
