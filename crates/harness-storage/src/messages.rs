use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::db::HarnessDb;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: Thing,
    pub session: Thing,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub content_blocks: Vec<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
struct NewMessage {
    session: Thing,
    role: String,
    content: String,
    content_blocks: Vec<serde_json::Value>,
}

pub async fn append(
    db: &HarnessDb,
    session_id: &str,
    role: &str,
    content: &str,
    content_blocks: Vec<serde_json::Value>,
) -> Result<ChatMessage> {
    let session_thing = Thing::from(("chat_session", session_id));
    let created: Option<ChatMessage> = db
        .create("chat_message")
        .content(NewMessage {
            session: session_thing,
            role: role.to_string(),
            content: content.to_string(),
            content_blocks,
        })
        .await?;
    created.ok_or_else(|| crate::error::StorageError::Db("append returned empty".into()))
}

/// Page of messages for a session, newest-first by `created_at`. Frontend reverses
/// for display order.
pub async fn page(
    db: &HarnessDb,
    session_id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<ChatMessage>> {
    let mut res = db
        .query(
            "SELECT * FROM chat_message \
             WHERE session = type::thing('chat_session', $id) \
             ORDER BY created_at DESC LIMIT $limit START $offset",
        )
        .bind(("id", session_id.to_string()))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?;
    let rows: Vec<ChatMessage> = res.take(0)?;
    Ok(rows)
}

pub async fn count_for_session(db: &HarnessDb, session_id: &str) -> Result<i64> {
    let mut res = db
        .query(
            "SELECT count() AS c FROM chat_message \
             WHERE session = type::thing('chat_session', $id) GROUP ALL",
        )
        .bind(("id", session_id.to_string()))
        .await?;
    #[derive(Deserialize)]
    struct Row {
        c: i64,
    }
    let rows: Vec<Row> = res.take(0)?;
    Ok(rows.into_iter().next().map(|r| r.c).unwrap_or(0))
}

/// Oldest-first ordering — used to feed conversation memory back into the model.
pub async fn ordered_for_session(
    db: &HarnessDb,
    session_id: &str,
) -> Result<Vec<ChatMessage>> {
    let mut res = db
        .query(
            "SELECT * FROM chat_message \
             WHERE session = type::thing('chat_session', $id) \
             ORDER BY created_at ASC",
        )
        .bind(("id", session_id.to_string()))
        .await?;
    let rows: Vec<ChatMessage> = res.take(0)?;
    Ok(rows)
}
