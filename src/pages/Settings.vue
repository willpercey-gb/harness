<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { getSettings, setSettings, type Settings } from '@/services/settings'

const router = useRouter()
const loading = ref(true)
const saving = ref(false)
const error = ref('')
const success = ref('')
const allowlistText = ref('')
const settings = ref<Settings>({
  openrouter_api_key: null,
  openrouter_referrer: null,
  openrouter_app_title: null,
  ollama_host: 'http://localhost:11434',
  default_agent_id: null,
  http_fetch_allowlist: [],
  read_file_sandbox_root: null,
  memex_db_path: null,
})

onMounted(async () => {
  try {
    const loaded = await getSettings()
    settings.value = loaded
    allowlistText.value = loaded.http_fetch_allowlist.join('\n')
  } catch (e: any) {
    error.value = e?.message ?? String(e)
  } finally {
    loading.value = false
  }
})

async function save() {
  saving.value = true
  error.value = ''
  success.value = ''
  try {
    settings.value.http_fetch_allowlist = allowlistText.value
      .split('\n')
      .map((s) => s.trim())
      .filter(Boolean)
    await setSettings(settings.value)
    success.value = 'Saved'
    setTimeout(() => (success.value = ''), 2000)
  } catch (e: any) {
    error.value = e?.message ?? String(e)
  } finally {
    saving.value = false
  }
}

function back() {
  router.push('/chat')
}
</script>

<template>
  <div class="settings">
    <header class="head">
      <button class="back" @click="back">
        <span class="material-symbols-outlined">arrow_back</span>
        Back
      </button>
      <h1 class="title">Settings</h1>
    </header>

    <div v-if="loading" class="loading">Loading…</div>

    <form v-else class="form" @submit.prevent="save">
      <section class="group">
        <h2 class="group-title">OpenRouter</h2>
        <p class="group-desc">
          Cloud agents become selectable when an API key is set. Get one at
          <a href="https://openrouter.ai/keys" target="_blank">openrouter.ai/keys</a>.
        </p>

        <label class="field">
          <span class="label">API key</span>
          <input
            v-model="settings.openrouter_api_key"
            type="password"
            placeholder="sk-or-..."
            autocomplete="off"
            spellcheck="false"
          />
        </label>

        <label class="field">
          <span class="label">HTTP-Referer (optional)</span>
          <input
            v-model="settings.openrouter_referrer"
            type="text"
            placeholder="https://example.com"
          />
        </label>

        <label class="field">
          <span class="label">App title — X-Title (optional)</span>
          <input
            v-model="settings.openrouter_app_title"
            type="text"
            placeholder="Harness"
          />
        </label>
      </section>

      <section class="group">
        <h2 class="group-title">Ollama</h2>
        <label class="field">
          <span class="label">Host URL</span>
          <input
            v-model="settings.ollama_host"
            type="text"
            placeholder="http://localhost:11434"
          />
          <span class="hint">Local Ollama daemon endpoint.</span>
        </label>
      </section>

      <section class="group">
        <h2 class="group-title">Tools</h2>
        <p class="group-desc">
          <code>http_fetch</code> and <code>read_file</code> only run when their sandbox is configured.
        </p>

        <label class="field">
          <span class="label">http_fetch allowlist</span>
          <textarea
            v-model="allowlistText"
            placeholder="example.com&#10;api.github.com"
            rows="4"
          ></textarea>
          <span class="hint">One host per line.</span>
        </label>

        <label class="field">
          <span class="label">read_file sandbox root</span>
          <input
            v-model="settings.read_file_sandbox_root"
            type="text"
            placeholder="/Users/you/Documents/notes"
          />
          <span class="hint">Files outside this directory are refused.</span>
        </label>
      </section>

      <section class="group">
        <h2 class="group-title">Knowledge store</h2>
        <p class="group-desc">
          A SurrealDB graph + vector store that the passive memory extractor
          writes to after each turn (and that the agent reads from via
          <code>recall</code> / <code>lookup_entity</code>). Default location:
          <code>~/.harness/memex-db</code>.
        </p>

        <label class="field">
          <span class="label">Memex DB path</span>
          <input
            v-model="settings.memex_db_path"
            type="text"
            placeholder="/Users/you/.harness/memex-db"
          />
          <span class="hint">Override the default. Restart the app after changing.</span>
        </label>
      </section>

      <div class="actions">
        <button class="primary" type="submit" :disabled="saving">
          {{ saving ? 'Saving…' : 'Save changes' }}
        </button>
        <span v-if="error" class="msg err">{{ error }}</span>
        <span v-if="success" class="msg ok">{{ success }}</span>
      </div>
    </form>
  </div>
</template>

<style scoped lang="scss">
.settings {
  height: 100%;
  overflow-y: auto;
  padding: calc(var(--titlebar-h, 32px) + 24px) 32px 64px;
  max-width: 720px;
  margin: 0 auto;
  width: 100%;
}

.head {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 28px;

  .back {
    background: transparent;
    border: 1px solid var(--rule);
    border-radius: var(--radius-md);
    padding: 5px 10px;
    cursor: pointer;
    color: var(--ink-muted);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 13px;
    transition: all 0.12s;
    &:hover { color: var(--ink); background: var(--bg-soft); }
    .material-symbols-outlined { font-size: 16px; }
  }
  .title {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.015em;
    color: var(--ink);
  }
}

.loading {
  color: var(--ink-faint);
}

.form {
  display: flex;
  flex-direction: column;
  gap: 28px;
}

.group {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 20px;
  background: var(--bg-soft);
  border: 1px solid var(--rule);
  border-radius: var(--radius-lg);

  .group-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--ink);
  }
  .group-desc {
    margin: 0;
    font-size: 13px;
    color: var(--ink-muted);
    line-height: 1.5;
    a { color: #2563eb; text-decoration: underline; }
    code {
      font-family: ui-monospace, SFMono-Regular, monospace;
      font-size: 12px;
      background: var(--bg-deep);
      padding: 1px 5px;
      border-radius: 4px;
    }
  }
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;

  .label {
    font-size: 12.5px;
    color: var(--ink-muted);
    font-weight: 500;
  }
  .hint {
    font-size: 12px;
    color: var(--ink-faint);
  }
  input, textarea {
    font: inherit;
    background-color: var(--bg);
    border: 1px solid var(--rule);
    border-radius: var(--radius-md);
    padding: 8px 12px;
    color: var(--ink);
    transition: border-color 0.12s, box-shadow 0.12s;
    &:focus {
      outline: 0;
      border-color: var(--rule-strong);
      box-shadow: 0 0 0 3px var(--accent-soft);
    }
    &::placeholder { color: var(--ink-faint); }
  }
  textarea {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 13px;
    line-height: 1.5;
    resize: vertical;
    min-height: 90px;
  }
}

.actions {
  display: flex;
  align-items: center;
  gap: 14px;
  padding-top: 8px;
}
.primary {
  background: var(--ink);
  color: var(--bg);
  border: 0;
  border-radius: var(--radius-md);
  padding: 9px 18px;
  font-size: 13.5px;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.12s;
  &:hover:not(:disabled) { opacity: 0.9; }
  &:disabled { opacity: 0.5; cursor: not-allowed; }
}
.msg {
  font-size: 13px;
  &.err { color: #dc2626; }
  &.ok { color: #16a34a; }
}
</style>
