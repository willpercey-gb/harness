<script setup lang="ts">
defineProps<{ open: boolean }>()
defineEmits<{ (e: 'toggle'): void }>()
</script>

<template>
  <aside class="right" :class="{ open }">
    <button class="toggle" :title="open ? 'Collapse panel' : 'Expand panel'" @click="$emit('toggle')">
      <span class="material-symbols-outlined">
        {{ open ? 'chevron_right' : 'chevron_left' }}
      </span>
    </button>
    <div class="content" v-show="open">
      <span class="eyebrow vertical">Notes</span>
      <p class="placeholder">
        This panel is reserved.<br />
        Tooling, references, scratchpad — coming soon.
      </p>
    </div>
    <div class="rail" v-show="!open">
      <span class="rail-text">— soon —</span>
    </div>
  </aside>
</template>

<style scoped lang="scss">
.right {
  background-color: var(--bg-soft);
  border-left: 1px solid var(--rule);
  position: relative;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.toggle {
  position: absolute;
  top: 18px;
  left: 8px;
  width: 28px;
  height: 28px;
  background: transparent;
  border: 1px solid var(--rule);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--ink-faint);
  transition: all 0.18s;
  z-index: 2;
  &:hover {
    color: var(--ink);
    border-color: var(--rule-strong);
    background: var(--bg);
  }
}
.content {
  padding: 80px 24px 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  height: 100%;

  .eyebrow.vertical {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.25em;
    text-transform: uppercase;
    color: var(--ink-faint);
    opacity: 0.6;
    margin-bottom: 8px;
  }
  .placeholder {
    font-family: var(--font-body);
    font-size: 15px;
    line-height: 1.7;
    color: var(--ink-faint);
    font-style: italic;
    margin: 0;
    opacity: 0.8;
  }
}
.rail {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  position: absolute;
  inset: 0;
  pointer-events: none;
  .rail-text {
    transform: rotate(-90deg);
    transform-origin: center;
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.4em;
    text-transform: uppercase;
    color: var(--ink-faint);
    white-space: nowrap;
    opacity: 0.4;
  }
}
</style>
