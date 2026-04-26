use harness_storage::{settings, Settings};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<Settings, String> {
    settings::load(&state.db).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn settings_set(
    new: Settings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    settings::save(&state.db, &new)
        .await
        .map_err(|e| e.to_string())?;
    // Refresh AppState's cached settings so next chat_send sees the
    // new ollama_host / api key without an app restart.
    state.refresh_settings().await.map_err(|e| e.to_string())
}
