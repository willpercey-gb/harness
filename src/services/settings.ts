import { invoke } from '@tauri-apps/api/core'

export interface Settings {
  openrouter_api_key: string | null
  openrouter_referrer: string | null
  openrouter_app_title: string | null
  ollama_host: string
  default_agent_id: string | null
  http_fetch_allowlist: string[]
  read_file_sandbox_root: string | null
  memex_db_path: string | null
}

export async function getSettings(): Promise<Settings> {
  return await invoke<Settings>('settings_get')
}

export async function setSettings(next: Settings): Promise<void> {
  await invoke('settings_set', { new: next })
}
