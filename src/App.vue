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
  <div class="app-shell" :class="{ 'right-open': rightOpen }">
    <LeftSidebar :dark-mode="darkMode" @toggle-dark="setDark(!darkMode)" />
    <main class="main-stage">
      <RouterView />
    </main>
    <RightSidebar :open="rightOpen" @toggle="toggleRight" />
  </div>
</template>

<style scoped lang="scss">
.app-shell {
  display: grid;
  grid-template-columns: 268px 1fr 44px;
  height: 100vh;
  transition: grid-template-columns 0.32s cubic-bezier(0.4, 0, 0.15, 1);

  &.right-open {
    grid-template-columns: 268px 1fr 280px;
  }
}
.main-stage {
  position: relative;
  overflow: hidden;
  border-left: 1px solid var(--rule);
  border-right: 1px solid var(--rule);
  background-color: var(--bg);
}
</style>
