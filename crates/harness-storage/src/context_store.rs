use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::HarnessDb;
use crate::error::{Result, StorageError};

/// One supporting constraint or aside note attached to a session's
/// conversation context. Both `Priority` and `Aside` share this shape;
/// they live in distinct arrays on `chat_session`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextCard {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub edited_by_user: bool,
}

impl ContextCard {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text: text.into(),
            edited_by_user: false,
        }
    }

    pub fn edited(text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text: text.into(),
            edited_by_user: true,
        }
    }
}

/// Session-level context surfaced as the right-sidebar cards.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationContext {
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub priorities: Vec<ContextCard>,
    #[serde(default)]
    pub asides: Vec<ContextCard>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

impl ConversationContext {
    pub fn is_empty(&self) -> bool {
        self.anchor.is_none() && self.priorities.is_empty() && self.asides.is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct ContextRow {
    #[serde(default)]
    context_anchor: Option<String>,
    #[serde(default)]
    context_priorities: Vec<ContextCard>,
    #[serde(default)]
    context_asides: Vec<ContextCard>,
    #[serde(default)]
    context_updated_at: Option<DateTime<Utc>>,
}

pub async fn load(db: &HarnessDb, session_id: &str) -> Result<ConversationContext> {
    let mut res = db
        .query(
            "SELECT context_anchor, context_priorities, context_asides, context_updated_at \
             FROM type::thing('chat_session', $id)",
        )
        .bind(("id", session_id.to_string()))
        .await?;
    let rows: Vec<ContextRow> = res.take(0)?;
    let row = rows.into_iter().next().ok_or(StorageError::NotFound)?;
    Ok(ConversationContext {
        anchor: row.context_anchor,
        priorities: row.context_priorities,
        asides: row.context_asides,
        updated_at: row.context_updated_at,
    })
}

pub async fn save(
    db: &HarnessDb,
    session_id: &str,
    ctx: &ConversationContext,
) -> Result<()> {
    db.query(
        "UPDATE type::thing('chat_session', $id) SET \
         context_anchor      = $anchor, \
         context_priorities  = $priorities, \
         context_asides      = $asides, \
         context_updated_at  = time::now()",
    )
    .bind(("id", session_id.to_string()))
    .bind(("anchor", ctx.anchor.clone()))
    .bind(("priorities", ctx.priorities.clone()))
    .bind(("asides", ctx.asides.clone()))
    .await?;
    Ok(())
}
