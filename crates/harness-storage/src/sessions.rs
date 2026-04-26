use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

use crate::db::HarnessDb;
use crate::error::{Result, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: Thing,
    pub title: String,
    pub agent_id: String,
    pub created_at: DateTime<Utc>,
    pub last_msg_at: DateTime<Utc>,
    pub message_count: i64,
    #[serde(default)]
    pub memory: serde_json::Value,
    #[serde(default)]
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
struct NewSession {
    title: String,
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub title: String,
    pub agent_id: Option<String>,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub last_msg_at: DateTime<Utc>,
}

impl From<ChatSession> for SessionSummary {
    fn from(s: ChatSession) -> Self {
        Self {
            session_id: s.id.id.to_string(),
            title: s.title,
            agent_id: Some(s.agent_id),
            message_count: s.message_count,
            created_at: s.created_at,
            last_msg_at: s.last_msg_at,
        }
    }
}

pub async fn create(db: &HarnessDb, title: &str, agent_id: &str) -> Result<ChatSession> {
    let created: Option<ChatSession> = db
        .create("chat_session")
        .content(NewSession {
            title: title.to_string(),
            agent_id: agent_id.to_string(),
        })
        .await?;
    created.ok_or_else(|| StorageError::Db("create returned empty".into()))
}

pub async fn get(db: &HarnessDb, session_id: &str) -> Result<ChatSession> {
    let s: Option<ChatSession> = db.select(("chat_session", session_id)).await?;
    s.ok_or(StorageError::NotFound)
}

/// All non-deleted sessions, newest-first.
///
/// Sessions are no longer scoped to a single agent — switching agents
/// mid-conversation is supported, so the picker shows everything.
pub async fn list_all(
    db: &HarnessDb,
    limit: u32,
    offset: u32,
) -> Result<Vec<SessionSummary>> {
    let mut res = db
        .query(
            "SELECT * FROM chat_session WHERE deleted_at IS NONE \
             ORDER BY last_msg_at DESC LIMIT $limit START $offset",
        )
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?;
    let rows: Vec<ChatSession> = res.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn count_all(db: &HarnessDb) -> Result<i64> {
    let mut res = db
        .query(
            "SELECT count() AS c FROM chat_session \
             WHERE deleted_at IS NONE GROUP ALL",
        )
        .await?;
    #[derive(Deserialize)]
    struct Row {
        c: i64,
    }
    let rows: Vec<Row> = res.take(0)?;
    Ok(rows.into_iter().next().map(|r| r.c).unwrap_or(0))
}

pub async fn list_for_agent(
    db: &HarnessDb,
    agent_id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<SessionSummary>> {
    let mut res = db
        .query(
            "SELECT * FROM chat_session \
             WHERE agent_id = $agent AND deleted_at IS NONE \
             ORDER BY last_msg_at DESC LIMIT $limit START $offset",
        )
        .bind(("agent", agent_id.to_string()))
        .bind(("limit", limit as i64))
        .bind(("offset", offset as i64))
        .await?;
    let rows: Vec<ChatSession> = res.take(0)?;
    Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn count_for_agent(db: &HarnessDb, agent_id: &str) -> Result<i64> {
    let mut res = db
        .query(
            "SELECT count() AS c FROM chat_session \
             WHERE agent_id = $agent AND deleted_at IS NONE GROUP ALL",
        )
        .bind(("agent", agent_id.to_string()))
        .await?;
    #[derive(Deserialize)]
    struct Row {
        c: i64,
    }
    let rows: Vec<Row> = res.take(0)?;
    Ok(rows.into_iter().next().map(|r| r.c).unwrap_or(0))
}

pub async fn touch(
    db: &HarnessDb,
    session_id: &str,
    new_message_count: i64,
) -> Result<()> {
    db.query(
        "UPDATE type::thing('chat_session', $id) SET \
         last_msg_at = time::now(), message_count = $count",
    )
    .bind(("id", session_id.to_string()))
    .bind(("count", new_message_count))
    .await?;
    Ok(())
}

pub async fn rename(db: &HarnessDb, session_id: &str, title: &str) -> Result<()> {
    db.query("UPDATE type::thing('chat_session', $id) SET title = $title")
        .bind(("id", session_id.to_string()))
        .bind(("title", title.to_string()))
        .await?;
    Ok(())
}

pub async fn soft_delete(db: &HarnessDb, session_id: &str) -> Result<()> {
    db.query("UPDATE type::thing('chat_session', $id) SET deleted_at = time::now()")
        .bind(("id", session_id.to_string()))
        .await?;
    Ok(())
}
