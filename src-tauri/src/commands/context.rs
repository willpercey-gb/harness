use harness_storage::{context_store, ConversationContext};
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn get_context(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationContext, String> {
    context_store::load(&state.db, &session_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_context(
    session_id: String,
    context: ConversationContext,
    state: State<'_, AppState>,
) -> Result<(), String> {
    context_store::save(&state.db, &session_id, &context)
        .await
        .map_err(|e| e.to_string())
}
