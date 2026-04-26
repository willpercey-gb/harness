use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// In-process registry mapping a Tauri channel id (`u32`) to a
/// [`CancellationToken`]. The chat command inserts a token before kicking
/// off the streaming task, the streaming task watches the token in a
/// `tokio::select!`, and the cancel command flips the token from anywhere.
#[derive(Default, Clone)]
pub struct CancellationRegistry {
    inner: Arc<Mutex<HashMap<u32, CancellationToken>>>,
}

impl CancellationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a fresh token for `channel_id`. Returns the token so the
    /// caller can `select!` on it.
    pub async fn register(&self, channel_id: u32) -> CancellationToken {
        let token = CancellationToken::new();
        self.inner.lock().await.insert(channel_id, token.clone());
        token
    }

    /// Cancel the token (if any) keyed by `channel_id`. Idempotent — a
    /// missing entry is a no-op.
    pub async fn cancel(&self, channel_id: u32) {
        if let Some(token) = self.inner.lock().await.remove(&channel_id) {
            token.cancel();
        }
    }

    /// Drop the registry entry for `channel_id` without flipping its
    /// token. The streaming task should call this when it completes
    /// successfully.
    pub async fn release(&self, channel_id: u32) {
        self.inner.lock().await.remove(&channel_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn cancel_flips_registered_token() {
        let reg = CancellationRegistry::new();
        let token = reg.register(42).await;
        assert!(!token.is_cancelled());
        reg.cancel(42).await;
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_unregistered_is_noop() {
        let reg = CancellationRegistry::new();
        // Must not panic.
        reg.cancel(999).await;
    }

    #[tokio::test]
    async fn release_does_not_cancel() {
        let reg = CancellationRegistry::new();
        let token = reg.register(7).await;
        reg.release(7).await;
        assert!(!token.is_cancelled());
        // After release, cancelling the same id is a no-op.
        reg.cancel(7).await;
        assert!(!token.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_unblocks_a_select_loop_quickly() {
        let reg = CancellationRegistry::new();
        let token = reg.register(1).await;
        let reg2 = reg.clone();
        let task = tokio::spawn(async move {
            // Simulated long-running stream: tick forever until cancelled.
            tokio::select! {
                _ = token.cancelled() => "cancelled",
                _ = tokio::time::sleep(Duration::from_secs(60)) => "timed out",
            }
        });
        // Give the task a moment to enter the select.
        tokio::time::sleep(Duration::from_millis(10)).await;
        reg2.cancel(1).await;
        let outcome = tokio::time::timeout(Duration::from_millis(200), task)
            .await
            .expect("did not return within deadline")
            .expect("task panicked");
        assert_eq!(outcome, "cancelled");
    }
}
