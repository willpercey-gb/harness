<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ darkMode: boolean }>()
const emit = defineEmits<{ (e: 'update:dark-mode', v: boolean): void }>()

const icon = computed(() => (props.darkMode ? 'light_mode' : 'dark_mode'))

function toggle() {
  emit('update:dark-mode', !props.darkMode)
}
</script>

<template>
  <header class="app-header">
    <div class="app-header-left">
      <RouterLink class="app-icon" to="/">
        <span class="app-glyph">⌘</span>
      </RouterLink>
      <p class="app-name">
        Harness <RouterLink to="/chat" class="app-dash">-</RouterLink> Local Agent Playground<br />
        <span class="version">Version: 0.1.0</span>
      </p>
    </div>
    <div class="app-header-right">
      <button class="mode-switch" :title="darkMode ? 'Switch to light' : 'Switch to dark'" @click="toggle">
        <span class="material-symbols-outlined">{{ icon }}</span>
      </button>
    </div>
  </header>
</template>

<style scoped lang="scss">
.app-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 18px;
  border-bottom: 1px solid var(--message-box-border, #dee2e6);
  background-color: var(--projects-section, #ffffff);
}
.app-header-left {
  display: flex;
  align-items: center;
  gap: 14px;
}
.app-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 8px;
  background-color: var(--link-color, #007bff);
  color: white;
  text-decoration: none;
  font-size: 18px;
  font-weight: 700;
}
.app-glyph {
  font-family: ui-monospace, monospace;
}
.app-name {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--main-color, #212529);
  line-height: 1.3;
  .version {
    font-size: 11px;
    color: var(--inactive-color, #6c757d);
    font-weight: 400;
  }
}
.app-dash {
  cursor: pointer;
  color: var(--inactive-color, #6c757d);
  text-decoration: none;
  margin: 0 4px;
  &:hover {
    color: var(--link-color, #007bff);
  }
}
.mode-switch {
  background: transparent;
  border: 1px solid var(--message-box-border, #dee2e6);
  color: var(--main-color, #212529);
  border-radius: 8px;
  width: 36px;
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  &:hover {
    background-color: var(--hover-menu-bg, #f1f3f5);
  }
}
</style>
