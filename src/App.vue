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

// `data-tauri-drag-region` is the declarative path, but on macOS overlay
// titlebars it's flaky — we make startDragging() the load-bearing
// mechanism via an explicit mousedown listener. Double-click toggles
// maximise to match the native macOS title-bar gesture.
async function onDragMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  // Don't hijack drags that started on an interactive child.
  const target = e.target as HTMLElement
  if (target.closest('button, a, input, textarea, select, [role="button"]')) return
  try {
    await getCurrentWindow().startDragging()
  } catch (err) {
    // Vite browser preview (no Tauri host) — silent.
    console.debug('startDragging unavailable:', err)
  }
}

async function onDragDoubleClick(e: MouseEvent) {
  const target = e.target as HTMLElement
  if (target.closest('button, a, input, textarea, select, [role="button"]')) return
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
    class="app-shell"
    :class="{ 'right-open': rightOpen }"
    @mousedown="onDragMouseDown"
    @dblclick="onDragDoubleClick"
    data-tauri-drag-region
  >
    <LeftSidebar :dark-mode="darkMode" @toggle-dark="setDark(!darkMode)" />
    <main class="main-stage">
      <RouterView />
    </main>
    <RightSidebar :open="rightOpen" @toggle="toggleRight" />
  </div>
</template>

<style scoped lang="scss">
// No fixed-overlay drag strip anymore — event handlers live on the
// app-shell and bubble up from any non-interactive element. Buttons,
// inputs, links etc. short-circuit the drag in the JS handler, so the
// rest of the chrome (sidebar background, main-stage padding, empty
// title-bar areas) is grab-and-drag.
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
  // Without this, padding-top adds *on top of* `height: 100%` and the
  // bottom of the page falls off-screen by exactly --titlebar-h.
  box-sizing: border-box;
}
</style>
