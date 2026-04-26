use harness_chat::{pipeline::events::StreamEvent, run_chat};
use tauri::{ipc::Channel, State};

use crate::state::AppState;

#[tauri::command]
pub async fn chat_send(
    agent: String,
    prompt: String,
    session_id: Option<String>,
    on_event: Channel<StreamEvent>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let cfg = state
        .agent_by_id(&agent)
        .await
        .ok_or_else(|| format!("unknown agent {agent}"))?;
    if cfg.disabled {
        return Err(cfg
            .disabled_message
            .unwrap_or_else(|| "agent is disabled".into()));
    }

    let channel_id = on_event.id();
    let token = state.cancellations.register(channel_id).await;
    let cancellations = state.cancellations.clone();
    let db = state.db.clone();
    let settings = state.settings.read().await.clone();

    let channel_for_emit = on_event;
    let outcome = run_chat(db, settings, cfg, prompt, session_id, token, move |event| {
        let _ = channel_for_emit.send(event);
    })
    .await;

    cancellations.release(channel_id).await;
    Ok(outcome.session_id)
}

#[tauri::command]
pub async fn chat_cancel(channel_id: u32, state: State<'_, AppState>) -> Result<(), String> {
    state.cancellations.cancel(channel_id).await;
    Ok(())
}
