//! Cancelling a statement that is already running.
//!
//! PostgreSQL's `CancelRequest` arrives on a second connection while the first
//! is busy, so the flag it sets has to be readable from wherever the busy
//! statement happens to be. That is the whole reason this lives here rather
//! than in the PostgreSQL query engine: most of a large scan's time is spent
//! in the storage layer, and a check the storage layer cannot reach is a check
//! that fires only after the expensive part is over.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::protocols::error::{ProtocolError, ProtocolResult};

/// Sessions that can be cancelled, by backend id, with the key that proves a
/// caller is allowed to cancel them.
type Sessions = Mutex<std::collections::HashMap<i32, (Vec<u8>, Arc<AtomicBool>)>>;

static CANCELLABLE: OnceLock<Sessions> = OnceLock::new();

fn cancellable() -> &'static Sessions {
    CANCELLABLE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Register a session so its queries can be cancelled.
///
/// The returned flag is the one [`check_cancelled`] reads; hold it for the
/// life of the session and scope it around each statement with
/// [`with_cancel`].
#[must_use]
pub fn register_cancellable(process_id: i32, secret_key: Vec<u8>) -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    if let Ok(mut sessions) = cancellable().lock() {
        sessions.insert(process_id, (secret_key, Arc::clone(&flag)));
    }
    flag
}

/// Forget a session that has gone away.
pub fn forget_cancellable(process_id: i32) {
    if let Ok(mut sessions) = cancellable().lock() {
        sessions.remove(&process_id);
    }
}

tokio::task_local! {
    /// The cancel flag of the session running this statement.
    static CANCEL_FLAG: Arc<AtomicBool>;
}

/// Run a statement with a cancel flag the rest of the stack can consult.
pub async fn with_cancel<F, T>(flag: Arc<AtomicBool>, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    CANCEL_FLAG.scope(flag, future).await
}

/// Whether a cancel has been asked for and not yet acted on.
///
/// Reading clears it, so one cancel stops one statement rather than every
/// statement that follows.
#[must_use]
pub fn cancel_requested() -> bool {
    CANCEL_FLAG
        .try_with(|flag| flag.swap(false, Ordering::Relaxed))
        .unwrap_or(false)
}

/// Stop if a cancel is waiting, without consuming it for a later check.
///
/// Called inside the loops a long statement spends its time in, so a cancel
/// interrupts the statement running rather than only the one after it.
///
/// # Errors
/// Returns the error PostgreSQL reports for a cancelled statement.
pub fn check_cancelled() -> ProtocolResult<()> {
    let pending = CANCEL_FLAG
        .try_with(|flag| flag.load(Ordering::Relaxed))
        .unwrap_or(false);
    if pending {
        return Err(ProtocolError::PostgresError(
            "canceling statement due to user request".to_string(),
        ));
    }
    Ok(())
}

/// How many rows pass between cancel checks.
///
/// Checking every row would put an atomic load in the innermost loop; once per
/// batch is often enough for a person waiting on a query and costs nothing
/// measurable.
pub const CANCEL_CHECK_INTERVAL: usize = 512;

/// Ask a session to stop what it is doing.
///
/// The key must match: without that check any client could cancel any other's
/// work by guessing a backend id.
pub fn request_cancel(process_id: i32, secret_key: &[u8]) {
    let Ok(sessions) = cancellable().lock() else {
        return;
    };
    if let Some((expected, flag)) = sessions.get(&process_id) {
        if expected == secret_key {
            flag.store(true, Ordering::Relaxed);
            tracing::debug!(process_id, "cancel requested");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_registered_session_sees_its_own_cancel() {
        let flag = register_cancellable(9001, b"key".to_vec());
        request_cancel(9001, b"key");

        with_cancel(flag, async {
            assert!(check_cancelled().is_err());
        })
        .await;
        forget_cancellable(9001);
    }

    #[tokio::test]
    async fn the_wrong_key_cancels_nothing() {
        let flag = register_cancellable(9002, b"key".to_vec());
        request_cancel(9002, b"guess");

        with_cancel(flag, async {
            assert!(check_cancelled().is_ok());
        })
        .await;
        forget_cancellable(9002);
    }

    #[tokio::test]
    async fn checking_does_not_consume_but_asking_does() {
        let flag = register_cancellable(9003, b"key".to_vec());
        request_cancel(9003, b"key");

        with_cancel(flag, async {
            // Two checks in a row both see it: a statement checks many times.
            assert!(check_cancelled().is_err());
            assert!(check_cancelled().is_err());
            // Asking consumes it, so the next statement is not also killed.
            assert!(cancel_requested());
            assert!(!cancel_requested());
            assert!(check_cancelled().is_ok());
        })
        .await;
        forget_cancellable(9003);
    }

    #[tokio::test]
    async fn outside_a_session_nothing_is_cancelled() {
        // Background work runs with no flag in scope and must not be stopped.
        assert!(check_cancelled().is_ok());
        assert!(!cancel_requested());
    }

    #[tokio::test]
    async fn each_session_is_cancelled_separately() {
        let one = register_cancellable(9004, b"a".to_vec());
        let two = register_cancellable(9005, b"b".to_vec());
        request_cancel(9004, b"a");

        with_cancel(Arc::clone(&two), async {
            assert!(check_cancelled().is_ok(), "cancelling one hit the other");
        })
        .await;
        with_cancel(one, async {
            assert!(check_cancelled().is_err());
        })
        .await;
        forget_cancellable(9004);
        forget_cancellable(9005);
    }
}
