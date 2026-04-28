use harness_chat::{pipeline::events::StreamEvent, run_chat, Intent};
use tauri::{ipc::Channel, State};

use crate::state::AppState;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn chat_send(
    agent: String,
    prompt: String,
    session_id: Option<String>,
    intent_override: Option<String>,
    // `extract`: per-message kill switch for Stage 4. None/true =
    // extract; false = skip. Session-level `extract_disabled`
    // (incognito mode) wins regardless.
    extract: Option<bool>,
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

    let intent = intent_override.as_deref().and_then(|s| match s {
        "expand" => Some(Intent::Expand),
        "revise" => Some(Intent::Revise),
        "redirect" => Some(Intent::Redirect),
        "aside" => Some(Intent::Aside),
        _ => None,
    });

    // Resolve the effective extract flag. Per-message toggle is the
    // primary control; we'll AND it with the session-level
    // `extract_disabled` flag inside `run_chat` once the session id is
    // known (because for fresh sessions the row doesn't exist yet).
    let extract = extract.unwrap_or(true);

    let channel_id = on_event.id();
    let token = state.cancellations.register(channel_id).await;
    let cancellations = state.cancellations.clone();
    let db = state.db.clone();
    let settings = state.settings.read().await.clone();
    let memex_db = state.memex_db.clone();
    let embedder = state.embedder.clone();

    let channel_for_emit = on_event;
    let outcome = run_chat(
        db,
        settings,
        cfg,
        prompt,
        session_id,
        intent,
        extract,
        memex_db,
        embedder,
        token,
        move |event| {
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
