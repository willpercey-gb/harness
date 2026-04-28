mod bridge;
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

            // Spawn the local TCP bridge so the harness-mcp proxy
            // binary (used by the Claude Code plugin) can call our
            // Memex DB without contending for the RocksDB lock.
            let bridge_db = state.memex_db.clone();
            let bridge_emb = state.embedder.clone();
            tauri::async_runtime::spawn(async move {
                bridge::start_bridge(bridge_db, bridge_emb).await;
            });

            // Periodic graph-health pass — flags near-duplicate pairs
            // and zero-rel orphans. Runs once a few minutes after boot
            // (so the embedder has finished warming) and then hourly.
            let maint_db = state.memex_db.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(180)).await;
                loop {
                    if let Err(e) =
                        harness_tools::maintenance::run_once(&maint_db).await
                    {
                        tracing::warn!("maintenance pass failed: {e}");
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                }
            });

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::agents::list_agents,
            commands::sessions::list_sessions,
            commands::sessions::get_history,
            commands::sessions::delete_session,
            commands::sessions::get_session_extract_disabled,
            commands::sessions::set_session_extract_disabled,
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
            commands::knowledge::ingest_markdown_folder,
            commands::knowledge::list_provisional,
            commands::knowledge::promote_provisional,
            commands::knowledge::promote_provisional_as_new,
            commands::knowledge::discard_provisional,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
