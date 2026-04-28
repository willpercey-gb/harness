<script setup lang="ts">
import { computed } from 'vue'
import { useContextStore } from '@/stores/context'
import { useChatStore } from '@/stores/chat'
import { useMemoryStore } from '@/stores/memory'
import AnchorCard from '@/components/context/AnchorCard.vue'
import CardList from '@/components/context/CardList.vue'
import IntentBadge from '@/components/context/IntentBadge.vue'

defineProps<{ open: boolean }>()
defineEmits<{ (e: 'toggle'): void }>()

const ctx = useContextStore()
const chat = useChatStore()
const memory = useMemoryStore()
const hasSession = computed(() => !!chat.currentSessionId)
</script>

<template>
  <aside class="right" :class="{ open }">
    <button class="toggle" :title="open ? 'Close panel' : 'Open panel'" @click="$emit('toggle')">
      <span class="material-symbols-outlined">
        {{ open ? 'right_panel_close' : 'right_panel_open' }}
      </span>
    </button>

    <div class="content" v-show="open">
      <header class="meta">
        <span class="title">Context</span>
        <IntentBadge
          :intent="(ctx.lastIntent?.intent ?? null) as any"
          :source="ctx.lastIntent?.source ?? null"
        />
      </header>

      <p v-if="!hasSession" class="placeholder">
        Start a conversation to see anchor / priorities / asides.
      </p>

      <template v-else>
        <AnchorCard
          :text="ctx.context.anchor"
          :refreshing="ctx.refreshing"
          @update="ctx.setAnchor"
        />

        <CardList
          label="Priorities"
          kind="priority"
          :cards="ctx.context.priorities"
          @add="ctx.addPriority"
          @edit="ctx.editPriority"
          @delete="ctx.deletePriority"
          @move="ctx.moveCard"
        />

        <CardList
          label="Asides"
          kind="aside"
          :cards="ctx.context.asides"
          @add="ctx.addAside"
          @edit="ctx.editAside"
          @delete="ctx.deleteAside"
          @move="ctx.moveCard"
        />

        <p v-if="ctx.saveError" class="error">{{ ctx.saveError }}</p>

        <section class="memory" v-if="memory.active || memory.finishedAt">
          <header>
            <span class="title">Memory</span>
            <span v-if="memory.active" class="status active">extracting…</span>
            <span v-else class="status idle">idle</span>
          </header>
          <p class="counts">
            <strong>{{ memory.entities }}</strong> entities ·
            <strong>{{ memory.relationships }}</strong> links ·
            <strong>{{ memory.memories }}</strong> memories
          </p>
          <ul v-if="memory.recent.length" class="recent">
            <li v-for="(r, i) in memory.recent" :key="i" :class="r.kind">
              <span class="kind">{{ r.kind }}</span>
              <span class="label">{{ r.label }}</span>
              <span v-if="r.detail" class="detail">{{ r.detail }}</span>
            </li>
          </ul>
        </section>
      </template>
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
  position: fixed;
  top: calc(var(--titlebar-h, 32px) + 8px);
  right: 12px;
  width: 32px;
  height: 32px;
  background: var(--bg-soft);
  border: 1px solid var(--rule);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--ink-muted);
  border-radius: var(--radius-md, 4px);
  z-index: 30;
  transition: all 0.15s;
  &:hover {
    color: var(--ink);
    background: var(--bg-hover, var(--bg-soft));
  }
  .material-symbols-outlined { font-size: 18px; }
}
.content {
  padding: calc(var(--titlebar-h, 32px) + 52px) 18px 18px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  overflow-y: auto;
  height: 100%;
  box-sizing: border-box;
}
.meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--rule);
  .title {
    font-family: var(--font-display, inherit);
    font-size: 16px;
    font-weight: 500;
    letter-spacing: -0.01em;
    color: var(--ink);
  }
}
.placeholder {
  font-size: 13px;
  color: var(--ink-faint);
  margin: 0;
}
.error {
  font-size: 12px;
  color: #ef4444;
  margin: 0;
}
.memory {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 12px;
  border-top: 1px solid var(--rule);
  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    .title {
      font-size: 13px;
      font-weight: 600;
      color: var(--ink);
      letter-spacing: 0.01em;
    }
    .status {
      font-size: 10.5px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      &.active {
        color: var(--accent, #3b82f6);
      }
      &.idle {
        color: var(--ink-faint);
      }
    }
  }
  .counts {
    margin: 0;
    font-size: 12px;
    color: var(--ink-muted);
    font-family: var(--font-mono, ui-monospace, monospace);
    strong {
      color: var(--ink);
      font-weight: 600;
    }
  }
  .recent {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    li {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: 6px;
      align-items: baseline;
      font-size: 11.5px;
      .kind {
        text-transform: uppercase;
        letter-spacing: 0.08em;
        font-size: 9.5px;
        color: var(--ink-faint);
        font-weight: 600;
      }
      .label {
        color: var(--ink);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }
      .detail {
        color: var(--ink-muted);
        font-family: var(--font-mono, ui-monospace, monospace);
        font-size: 10.5px;
      }
      &.entity .kind { color: #3b82f6; }
      &.relationship .kind { color: #8b5cf6; }
      &.memory .kind { color: #10b981; }
    }
  }
}
</style>
