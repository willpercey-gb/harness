<script setup lang="ts">
import { onMounted, ref } from 'vue'
import Header from './components/Layout/Header.vue'

const darkMode = ref(false)

onMounted(() => {
  const stored = window.localStorage.getItem('darkMode')
  const initial = stored ? stored === 'true' : window.matchMedia?.('(prefers-color-scheme: dark)').matches
  setDark(initial)
})

function setDark(on: boolean) {
  darkMode.value = on
  document.documentElement.classList.toggle('dark', on)
  window.localStorage.setItem('darkMode', String(on))
}

function onToggleDark(value: boolean) {
  setDark(value)
}
</script>

<template>
  <div class="app-shell">
    <Header :dark-mode="darkMode" @update:dark-mode="onToggleDark" />
    <main class="main-content">
      <RouterView />
    </main>
  </div>
</template>
