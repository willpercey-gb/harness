<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import LeftSidebar from '@/components/LeftSidebar.vue'
import RightSidebar from '@/components/RightSidebar.vue'

const darkMode = ref(true)
const rightOpen = ref(false)

onMounted(() => {
  const stored = window.localStorage.getItem('darkMode')
  const initial = stored ? stored === 'true' : true
  setDark(initial)
  rightOpen.value = window.localStorage.getItem('rightOpen') === 'true'
})

function setDark(on: boolean) {
  darkMode.value = on
  document.documentElement.classList.toggle('dark', on)
  window.localStorage.setItem('darkMode', String(on))
}

function toggleRight() {
  rightOpen.value = !rightOpen.value
  window.localStorage.setItem('rightOpen', String(rightOpen.value))
}

// JS fallback for the strip. `data-tauri-drag-region` is the declarative
// path and should work on its own — but if a Vue scoped-style or stray
// pointer-events ever swallows the event, an explicit startDragging on
// mousedown keeps the window movable. Double-click toggles maximise to
// match macOS title-bar behaviour.
async function onStripMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  // Don't hijack mousedown that started on a real interactive child
  // (the strip has none today, but futureproof against accidental
  // overlays inside the strip).
  const target = e.target as HTMLElement
  if (target.closest('button, a, input, textarea, [role="button"]')) return
  try {
    await getCurrentWindow().startDragging()
  } catch (err) {
    // Browser-only dev preview (e.g. `vite` outside Tauri) — silent.
    console.debug('startDragging unavailable:', err)
  }
}

async function onStripDoubleClick() {
  try {
    const w = getCurrentWindow()
    if (await w.isMaximized()) await w.unmaximize()
    else await w.maximize()
  } catch {
    /* non-Tauri dev preview */
  }
}
</script>

<template>
  <div
    class="drag-strip"
    data-tauri-drag-region
    @mousedown="onStripMouseDown"
    @dblclick="onStripDoubleClick"
  ></div>
  <div class="app-shell" :class="{ 'right-open': rightOpen }">
    <LeftSidebar :dark-mode="darkMode" @toggle-dark="setDark(!darkMode)" />
    <main class="main-stage">
      <RouterView />
    </main>
    <RightSidebar :open="rightOpen" @toggle="toggleRight" />
  </div>
</template>

<style scoped lang="scss">
.drag-strip {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  // Cover the full overlay-titlebar area. Driven by --titlebar-h so
  // every component's top padding stays in lockstep.
  height: var(--titlebar-h, 38px);
  z-index: 1000;
  -webkit-app-region: drag;
  // Defensive: ensure nothing higher up sets pointer-events:none on a
  // wrapping container. The strip needs hit-testing on for both the
  // declarative drag and the JS fallback.
  pointer-events: auto;
  // Don't draw anything, but keep an explicit (transparent) background
  // so the element actually receives hit-testing on every platform.
  background: transparent;
}
.app-shell {
  display: grid;
  grid-template-columns: 260px 1fr 0px;
  grid-template-rows: 100vh;
  height: 100vh;
  background-color: var(--bg);
  transition: grid-template-columns 0.25s ease;

  &.right-open {
    grid-template-columns: 260px 1fr 280px;
  }
}
.main-stage {
  position: relative;
  overflow: hidden;
  background-color: var(--bg);
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  padding-top: var(--titlebar-h, 32px);
}
</style>
