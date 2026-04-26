//! SurrealDB-backed storage for harness chat sessions and messages.

pub mod db;
pub mod error;
pub mod memory;
pub mod messages;
pub mod schema;
pub mod sessions;

pub use db::{default_db_path, init_db, init_in_memory, HarnessDb};
pub use error::{Result, StorageError};
pub use messages::ChatMessage;
pub use sessions::{ChatSession, SessionSummary};

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
