<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'

const router = useRouter()

// /  →  /chat (single-purpose app, no landing page)
onMounted(() => {
  router.replace('/chat')
})
</script>

<template>
  <div class="redirect"></div>
</template>

<style scoped>
.redirect { height: 100%; }
</style>
