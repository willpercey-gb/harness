use harness_chat::AgentDto;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<AgentDto>, String> {
    Ok(state
        .current_agents()
        .await
        .into_iter()
        .map(AgentDto::from)
        .collect())
}
