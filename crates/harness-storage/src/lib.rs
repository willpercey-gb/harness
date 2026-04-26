//! SurrealDB-backed storage for harness chat sessions and messages.

pub mod context_store;
pub mod db;
pub mod error;
pub mod memory;
pub mod messages;
pub mod schema;
pub mod sessions;
pub mod settings;

pub use context_store::{ContextCard, ConversationContext};
pub use db::{default_db_path, init_db, init_in_memory, HarnessDb};
pub use error::{Result, StorageError};
pub use messages::ChatMessage;
pub use sessions::{ChatSession, SessionSummary};
pub use settings::Settings;

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh() -> HarnessDb {
        init_in_memory().await.expect("init in-memory db")
    }

    #[tokio::test]
    async fn create_and_get_session() {
        let db = fresh().await;
        let s = sessions::create(&db, "first chat", "ollama:llama3.2")
            .await
            .unwrap();
        let id = s.id.id.to_string();
        let again = sessions::get(&db, &id).await.unwrap();
        assert_eq!(again.title, "first chat");
        assert_eq!(again.agent_id, "ollama:llama3.2");
        assert_eq!(again.message_count, 0);
    }

    #[tokio::test]
    async fn append_and_page_messages() {
        let db = fresh().await;
        let s = sessions::create(&db, "t", "ollama:llama3.2").await.unwrap();
        let id = s.id.id.to_string();

        for i in 0..5 {
            messages::append(&db, &id, "user", &format!("msg {i}"), vec![])
                .await
                .unwrap();
        }
        let count = messages::count_for_session(&db, &id).await.unwrap();
        assert_eq!(count, 5);

        let page1 = messages::page(&db, &id, 3, 0).await.unwrap();
        assert_eq!(page1.len(), 3);
        // newest-first
        assert_eq!(page1[0].content, "msg 4");
        assert_eq!(page1[2].content, "msg 2");

        let page2 = messages::page(&db, &id, 3, 3).await.unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].content, "msg 1");
    }

    #[tokio::test]
    async fn list_for_agent_filters_deleted() {
        let db = fresh().await;
        let _a = sessions::create(&db, "a", "ollama:x").await.unwrap();
        let b = sessions::create(&db, "b", "ollama:x").await.unwrap();
        let _c = sessions::create(&db, "c", "ollama:y").await.unwrap();
        sessions::soft_delete(&db, &b.id.id.to_string()).await.unwrap();

        let listed = sessions::list_for_agent(&db, "ollama:x", 50, 0).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "a");
    }

    #[tokio::test]
    async fn touch_updates_count_and_timestamp() {
        let db = fresh().await;
        let s = sessions::create(&db, "t", "ollama:x").await.unwrap();
        let id = s.id.id.to_string();
        sessions::touch(&db, &id, 7).await.unwrap();
        let after = sessions::get(&db, &id).await.unwrap();
        assert_eq!(after.message_count, 7);
        assert!(after.last_msg_at >= s.last_msg_at);
    }

    #[tokio::test]
    async fn settings_default_when_missing() {
        let db = fresh().await;
        let s = settings::load(&db).await.unwrap();
        assert!(!s.openrouter_enabled());
        assert_eq!(s.ollama_host, "http://localhost:11434");
        assert!(s.http_fetch_allowlist.is_empty());
    }

    #[tokio::test]
    async fn settings_round_trip() {
        let db = fresh().await;
        let mut s = Settings::default();
        s.openrouter_api_key = Some("sk-test".into());
        s.http_fetch_allowlist = vec!["example.com".into(), "api.github.com".into()];
        settings::save(&db, &s).await.unwrap();
        let loaded = settings::load(&db).await.unwrap();
        assert_eq!(loaded.openrouter_api_key.as_deref(), Some("sk-test"));
        assert!(loaded.openrouter_enabled());
        assert!(loaded.http_fetch_allows("EXAMPLE.com"));
        assert!(!loaded.http_fetch_allows("evil.com"));
    }

    #[tokio::test]
    async fn context_default_when_session_has_none() {
        let db = fresh().await;
        let s = sessions::create(&db, "t", "ollama:x").await.unwrap();
        let ctx = context_store::load(&db, &s.id.id.to_string()).await.unwrap();
        assert!(ctx.anchor.is_none());
        assert!(ctx.priorities.is_empty());
        assert!(ctx.asides.is_empty());
        assert!(ctx.is_empty());
    }

    #[tokio::test]
    async fn context_round_trip() {
        let db = fresh().await;
        let s = sessions::create(&db, "t", "ollama:x").await.unwrap();
        let id = s.id.id.to_string();
        let ctx = ConversationContext {
            anchor: Some("plan a 4-day Lisbon trip".into()),
            priorities: vec![
                ContextCard::new("4-day duration"),
                ContextCard::edited("budget under £500"),
            ],
            asides: vec![ContextCard::new("note: timezone is GMT+0")],
            ..Default::default()
        };
        context_store::save(&db, &id, &ctx).await.unwrap();
        let loaded = context_store::load(&db, &id).await.unwrap();
        assert_eq!(loaded.anchor.as_deref(), Some("plan a 4-day Lisbon trip"));
        assert_eq!(loaded.priorities.len(), 2);
        assert!(loaded.priorities[1].edited_by_user);
        assert_eq!(loaded.asides.len(), 1);
        assert!(loaded.updated_at.is_some());
    }

    #[tokio::test]
    async fn context_save_overwrites() {
        let db = fresh().await;
        let s = sessions::create(&db, "t", "ollama:x").await.unwrap();
        let id = s.id.id.to_string();
        let ctx_a = ConversationContext {
            anchor: Some("first".into()),
            priorities: vec![ContextCard::new("p1")],
            ..Default::default()
        };
        context_store::save(&db, &id, &ctx_a).await.unwrap();
        let ctx_b = ConversationContext {
            anchor: Some("second".into()),
            priorities: vec![],
            ..Default::default()
        };
        context_store::save(&db, &id, &ctx_b).await.unwrap();
        let loaded = context_store::load(&db, &id).await.unwrap();
        assert_eq!(loaded.anchor.as_deref(), Some("second"));
        assert!(loaded.priorities.is_empty());
    }

    #[tokio::test]
    async fn context_turns_counter_round_trips() {
        let db = fresh().await;
        let s = sessions::create(&db, "t", "ollama:x").await.unwrap();
        let id = s.id.id.to_string();
        let ctx = ConversationContext {
            anchor: Some("a".into()),
            turns_since_refresh: 3,
            ..Default::default()
        };
        context_store::save(&db, &id, &ctx).await.unwrap();
        let loaded = context_store::load(&db, &id).await.unwrap();
        assert_eq!(loaded.turns_since_refresh, 3);
    }

    #[tokio::test]
    async fn settings_save_overwrites() {
        let db = fresh().await;
        let mut s = Settings::default();
        s.openrouter_api_key = Some("first".into());
        settings::save(&db, &s).await.unwrap();
        s.openrouter_api_key = Some("second".into());
        settings::save(&db, &s).await.unwrap();
        let loaded = settings::load(&db).await.unwrap();
        assert_eq!(loaded.openrouter_api_key.as_deref(), Some("second"));
    }

    #[tokio::test]
    async fn sliding_window_returns_last_n() {
        let db = fresh().await;
        let s = sessions::create(&db, "t", "ollama:x").await.unwrap();
        let id = s.id.id.to_string();
        for i in 0..10 {
            messages::append(&db, &id, "user", &format!("m{i}"), vec![])
                .await
                .unwrap();
        }
        let win = memory::sliding_window(&db, &id, 4).await.unwrap();
        assert_eq!(win.len(), 4);
        // oldest-first ordering
        assert_eq!(win[0].content, "m6");
        assert_eq!(win[3].content, "m9");
    }
}
