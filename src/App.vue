<script setup lang="ts">
import { onMounted, ref } from 'vue'
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
</script>

<template>
  <div class="drag-strip" data-tauri-drag-region></div>
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
  height: var(--titlebar-h, 28px);
  z-index: 50;
  -webkit-app-region: drag;
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
}
</style>
