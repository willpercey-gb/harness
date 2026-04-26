<script setup lang="ts">
defineProps<{ thinking: boolean }>()
</script>

<template>
  <div class="streaming-indicator">
    <span class="dot"></span>
    <span class="dot"></span>
    <span class="dot"></span>
    <span v-if="thinking" class="thinking-text">Thinking...</span>
  </div>
</template>

<style scoped lang="scss">
.streaming-indicator {
  display: flex;
  gap: 6px;
  padding: 10px 14px;
  align-self: flex-start;
  background-color: var(--hover-menu-bg, #f1f3f5);
  border-radius: 16px;
  align-items: center;

  .dot {
    width: 7px;
    height: 7px;
    background-color: var(--inactive-color, #adb5bd);
    border-radius: 50%;
    animation: pulse 1.4s infinite ease-in-out;
    &:nth-child(1) { animation-delay: 0s; }
    &:nth-child(2) { animation-delay: 0.2s; }
    &:nth-child(3) { animation-delay: 0.4s; }
  }
  .thinking-text {
    margin-left: 8px;
    font-size: 13px;
    color: var(--inactive-color, #6c757d);
    font-style: italic;
  }
}
@keyframes pulse {
  0%, 80%, 100% { opacity: 0.3; transform: scale(0.8); }
  40% { opacity: 1; transform: scale(1); }
}
</style>
