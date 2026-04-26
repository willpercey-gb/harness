use crate::db::HarnessDb;
use crate::error::Result;
use crate::messages::{self, ChatMessage};

/// Retrieve up to the last `n` messages from a session, oldest-first, for
/// feeding back into the model as conversation context.
pub async fn sliding_window(
    db: &HarnessDb,
    session_id: &str,
    n: usize,
) -> Result<Vec<ChatMessage>> {
    let all = messages::ordered_for_session(db, session_id).await?;
    if all.len() <= n {
        return Ok(all);
    }
    Ok(all[all.len() - n..].to_vec())
}
