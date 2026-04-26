use harness_chat::{run_chat, pipeline::events::StreamEvent};
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

    let channel_for_emit = on_event;
    let outcome = run_chat(
        db,
        cfg,
        prompt,
        session_id,
        token,
        move |event| {
            // Channel::send is non-blocking; ignore failures (frontend
            // disconnected mid-stream — nothing useful to do).
            let _ = channel_for_emit.send(event);
        },
    )
    .await;

    cancellations.release(channel_id).await;
    Ok(outcome.session_id)
}

#[tauri::command]
pub async fn chat_cancel(channel_id: u32, state: State<'_, AppState>) -> Result<(), String> {
    state.cancellations.cancel(channel_id).await;
    Ok(())
}
