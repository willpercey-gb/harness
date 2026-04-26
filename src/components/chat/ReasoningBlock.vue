<script setup lang="ts">
defineProps<{ content: string }>()
</script>

<template>
  <details class="reasoning-block" open>
    <summary>
      <span class="material-symbols-outlined">psychology</span>
      Reasoning
    </summary>
    <div class="reasoning-content">{{ content }}</div>
  </details>
</template>

<style scoped lang="scss">
.reasoning-block {
  margin: 12px 0;
  border: 1px solid #e3f2fd;
  border-radius: 8px;
  overflow: hidden;
  background-color: #f5f9ff;

  summary {
    padding: 10px 14px;
    background: linear-gradient(135deg, #e3f2fd 0%, #bbdefb 100%);
    color: #1565c0;
    cursor: pointer;
    font-weight: 700;
    font-size: 13px;
    display: flex;
    align-items: center;
    gap: 8px;
    user-select: none;

    .material-symbols-outlined {
      font-size: 18px;
    }
  }
  .reasoning-content {
    padding: 12px 14px;
    background-color: var(--projects-section, white);
    border-top: 1px solid #e3f2fd;
    font-size: 13px;
    line-height: 1.6;
    color: var(--secondary-color, #424242);
    white-space: pre-wrap;
    font-family: ui-monospace, SFMono-Regular, monospace;
  }
}

:global(html.dark) .reasoning-block {
  border-color: rgba(30, 136, 229, 0.3);
  background-color: rgba(30, 136, 229, 0.05);
  summary {
    background: linear-gradient(135deg, rgba(30, 136, 229, 0.15) 0%, rgba(30, 136, 229, 0.25) 100%);
    color: #90caf9;
  }
  .reasoning-content {
    border-color: rgba(30, 136, 229, 0.2);
  }
}
</style>
