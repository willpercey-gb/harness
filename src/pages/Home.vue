<script setup lang="ts">
import { useRouter } from 'vue-router'

const router = useRouter()
</script>

<template>
  <div class="home">
    <h1>Harness</h1>
    <p>A local agent chat playground.</p>
    <button class="primary" @click="router.push('/chat')">Open Chat</button>
    <p class="hint">Tip: the dash in the header also takes you to /chat.</p>
  </div>
</template>

<style scoped lang="scss">
.home {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  text-align: center;
  gap: 12px;

  h1 {
    margin: 0;
    font-size: 48px;
    font-weight: 800;
    letter-spacing: -1.5px;
  }
  .primary {
    margin-top: 8px;
    background-color: var(--link-color);
    color: white;
    border: none;
    padding: 10px 22px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    &:hover {
      opacity: 0.9;
    }
  }
  .hint {
    color: var(--inactive-color);
    font-size: 12px;
    margin-top: 18px;
  }
}
</style>
