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
    success.value = 'Saved.'
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
      <h1 class="display">Settings</h1>
      <span class="eyebrow">App configuration</span>
    </header>

    <div v-if="loading" class="loading">Loading…</div>

    <form v-else class="form" @submit.prevent="save">
      <section class="card">
        <h2 class="display section">OpenRouter</h2>
        <p class="muted">
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
          <span class="label">HTTP-Referer</span>
          <input
            v-model="settings.openrouter_referrer"
            type="text"
            placeholder="https://example.com (optional)"
          />
        </label>

        <label class="field">
          <span class="label">App title (X-Title)</span>
          <input
            v-model="settings.openrouter_app_title"
            type="text"
            placeholder="Harness (optional)"
          />
        </label>
      </section>

      <section class="card">
        <h2 class="display section">Ollama</h2>
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

      <section class="card">
        <h2 class="display section">Tool sandboxing</h2>
        <p class="muted">
          Phase-2 tools (<code>http_fetch</code>, <code>read_file</code>) only run when
          their sandbox is configured.
        </p>

        <label class="field">
          <span class="label">http_fetch host allowlist</span>
          <textarea
            v-model="allowlistText"
            placeholder="example.com&#10;api.github.com"
            rows="4"
          ></textarea>
          <span class="hint">One host per line. Scheme stripped, case-insensitive.</span>
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

      <div class="actions">
        <button class="primary" type="submit" :disabled="saving">
          {{ saving ? 'Saving…' : 'Save settings' }}
        </button>
        <span v-if="error" class="error">{{ error }}</span>
        <span v-if="success" class="success">{{ success }}</span>
      </div>
    </form>
  </div>
</template>

<style scoped lang="scss">
.settings {
  height: 100%;
  overflow-y: auto;
  padding: 32px 48px 64px;
  max-width: 760px;
  margin: 0 auto;
}

.head {
  display: grid;
  grid-template-columns: auto 1fr;
  align-items: baseline;
  column-gap: 18px;
  row-gap: 4px;
  margin-bottom: 36px;

  .back {
    grid-row: 1 / span 2;
    justify-self: start;
    background: transparent;
    border: 1px solid var(--rule-strong);
    border-radius: 999px;
    padding: 6px 12px;
    color: var(--ink-muted);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    &:hover { color: var(--ink); border-color: var(--ink); }
    .material-symbols-outlined { font-size: 14px; }
  }
  .display {
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 38px;
    margin: 0;
    letter-spacing: -0.5px;
  }
  .eyebrow {
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-weight: 500;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
}

.loading {
  color: var(--ink-faint);
  font-style: italic;
}

.form {
  display: flex;
  flex-direction: column;
  gap: 22px;
}

.card {
  background-color: var(--bg-soft);
  border: 1px solid var(--rule);
  border-radius: 4px;
  padding: 22px 26px 24px;

  .section {
    font-family: var(--font-display);
    font-weight: 500;
    font-size: 22px;
    margin: 0 0 6px 0;
    letter-spacing: -0.2px;
  }
  .muted {
    color: var(--ink-muted);
    font-size: 14px;
    margin: 0 0 18px 0;
    a { text-decoration: underline; text-decoration-color: var(--rule-strong); }
  }
  code {
    font-family: var(--font-mono);
    font-size: 12px;
    background: var(--bg-deep);
    padding: 1px 5px;
    border-radius: 3px;
  }
}

.field {
  display: flex;
  flex-direction: column;
  gap: 5px;
  margin-bottom: 16px;

  &:last-child { margin-bottom: 0; }

  .label {
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-weight: 500;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--ink-faint);
  }
  .hint {
    font-size: 12px;
    color: var(--ink-faint);
    font-style: italic;
  }
  input, textarea {
    font: inherit;
    background-color: var(--bg);
    border: 1px solid var(--rule-strong);
    border-radius: 3px;
    padding: 9px 11px;
    color: var(--ink);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.45;
    transition: border-color 0.15s;
    &:focus {
      outline: none;
      border-color: var(--accent);
    }
  }
  textarea {
    resize: vertical;
    min-height: 80px;
  }
}

.actions {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-top: 8px;
  .primary {
    background-color: var(--ink);
    color: var(--bg);
    border: none;
    padding: 10px 22px;
    border-radius: 3px;
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 500;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    &:disabled { opacity: 0.5; cursor: not-allowed; }
    &:hover:not(:disabled) { background-color: var(--accent); }
  }
  .error {
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .success {
    color: var(--ink-muted);
    font-family: var(--font-mono);
    font-size: 12px;
    font-style: italic;
  }
}
</style>
