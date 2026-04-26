use chrono::{DateTime, Utc};
use harness_storage::{messages, sessions};
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub session_id: String,
    pub title: String,
    pub agent_id: Option<String>,
    pub message_count: i64,
    pub created_at: DateTime<Utc>,
    pub last_message_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationMeta {
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
    pub has_more: bool,
}

#[derive(Serialize)]
pub struct SessionsPage {
    pub data: Vec<SessionRow>,
    pub meta: SessionsPageMeta,
}

#[derive(Serialize)]
pub struct SessionsPageMeta {
    pub pagination: PaginationMeta,
}

#[tauri::command]
pub async fn list_sessions(
    // `agent` is accepted for backwards compatibility with the
    // frontend's existing call signature but is now ignored — sessions
    // are no longer scoped to a single agent. Kept rather than broken
    // so older builds keep working.
    agent: Option<String>,
    limit: u32,
    offset: u32,
    state: State<'_, AppState>,
) -> Result<SessionsPage, String> {
    let _ = agent;
    let summaries = sessions::list_all(&state.db, limit, offset)
        .await
        .map_err(|e| e.to_string())?;
    let total = sessions::count_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    let returned = summaries.len() as i64;
    let data = summaries
        .into_iter()
        .map(|s| SessionRow {
            session_id: s.session_id,
            title: s.title,
            agent_id: s.agent_id,
            message_count: s.message_count,
            created_at: s.created_at,
            last_message_at: s.last_msg_at,
        })
        .collect();
    Ok(SessionsPage {
        data,
        meta: SessionsPageMeta {
            pagination: PaginationMeta {
                total,
                limit,
                offset,
                has_more: (offset as i64 + returned) < total,
            },
        },
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRow {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub agent_id: Option<String>,
}

#[derive(Serialize)]
pub struct HistoryPage {
    pub data: Vec<MessageRow>,
    pub meta: SessionsPageMeta,
}

#[tauri::command]
pub async fn get_history(
    session_id: String,
    limit: u32,
    offset: u32,
    state: State<'_, AppState>,
) -> Result<HistoryPage, String> {
    let rows = messages::page(&state.db, &session_id, limit, offset)
        .await
        .map_err(|e| e.to_string())?;
    let total = messages::count_for_session(&state.db, &session_id)
        .await
        .map_err(|e| e.to_string())?;
    let returned = rows.len() as i64;
    let data = rows
        .into_iter()
        .map(|m| MessageRow {
            id: m.id.id.to_string(),
            role: m.role,
            content: m.content,
            created_at: m.created_at,
            agent_id: m.agent_id,
        })
        .collect();
    Ok(HistoryPage {
        data,
        meta: SessionsPageMeta {
            pagination: PaginationMeta {
                total,
                limit,
                offset,
                has_more: (offset as i64 + returned) < total,
            },
        },
    })
}

#[tauri::command]
pub async fn delete_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    sessions::soft_delete(&state.db, &session_id)
        .await
        .map_err(|e| e.to_string())
}
