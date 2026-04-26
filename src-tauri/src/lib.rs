mod commands;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "info,harness=debug,harness_chat=debug,harness_storage=debug".into()
            }),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = tauri::async_runtime::block_on(AppState::build())
                .map_err(|e| anyhow::anyhow!("build app state: {e}"))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agents::list_agents,
            commands::sessions::list_sessions,
            commands::sessions::get_history,
            commands::sessions::delete_session,
            commands::chat::chat_send,
            commands::chat::chat_cancel,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::context::get_context,
            commands::context::update_context,
            commands::knowledge::get_full_graph,
            commands::knowledge::get_entity_graph,
            commands::knowledge::get_recent_memories,
            commands::knowledge::query_knowledge,
            commands::knowledge::get_knowledge_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
