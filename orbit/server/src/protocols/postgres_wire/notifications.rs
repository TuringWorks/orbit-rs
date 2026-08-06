//! Asynchronous notifications: `LISTEN` / `NOTIFY`.
//!
//! A notification crosses sessions — one connection issues `NOTIFY`, others
//! receive it — so the registry of who is listening lives outside any single
//! connection and is shared by the listener that accepts them.
//!
//! Delivery is best-effort within a process, matching what the protocol
//! promises: a notification is delivered to sessions listening *at the time it
//! is sent*. Nothing is persisted and nothing is replayed to a session that
//! subscribes later.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::Mutex;

/// One notification in flight.
#[derive(Debug, Clone)]
pub struct Notification {
    /// Backend process id of the sender, as reported to the client.
    pub process_id: i32,
    pub channel: String,
    pub payload: String,
}

/// Registry of listening sessions, shared by every connection on a server.
#[derive(Default)]
pub struct NotificationHub {
    /// Channel name to the sessions listening on it.
    ///
    /// Keys are stored folded, because `LISTEN Foo` and `NOTIFY foo` name the
    /// same channel in PostgreSQL unless quoted.
    listeners: Mutex<HashMap<String, Vec<Subscriber>>>,
}

struct Subscriber {
    session: u64,
    sender: UnboundedSender<Notification>,
}

impl NotificationHub {
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Subscribe `session` to `channel`.
    ///
    /// Repeated `LISTEN` on the same channel from the same session is a no-op,
    /// as in PostgreSQL — it must not double-deliver.
    pub async fn listen(
        &self,
        channel: &str,
        session: u64,
        sender: UnboundedSender<Notification>,
    ) {
        let mut listeners = self.listeners.lock().await;
        let subscribers = listeners.entry(fold_channel(channel)).or_default();
        if subscribers.iter().any(|s| s.session == session) {
            return;
        }
        subscribers.push(Subscriber { session, sender });
    }

    /// Stop delivering `channel` to `session`. `None` unsubscribes everything,
    /// which is what bare `UNLISTEN *` means.
    pub async fn unlisten(&self, channel: Option<&str>, session: u64) {
        let mut listeners = self.listeners.lock().await;
        match channel {
            Some(channel) => {
                if let Some(subscribers) = listeners.get_mut(&fold_channel(channel)) {
                    subscribers.retain(|s| s.session != session);
                }
            }
            None => {
                for subscribers in listeners.values_mut() {
                    subscribers.retain(|s| s.session != session);
                }
            }
        }
    }

    /// Drop every subscription for a session that has gone away.
    pub async fn disconnect(&self, session: u64) {
        self.unlisten(None, session).await;
        self.listeners
            .lock()
            .await
            .retain(|_, subscribers| !subscribers.is_empty());
    }

    /// Deliver a notification to every listener, returning how many received it.
    ///
    /// Sessions whose receiver has been dropped are pruned here rather than
    /// accumulating: a hub that only ever adds entries grows for the life of
    /// the process.
    pub async fn notify(&self, channel: &str, payload: &str, process_id: i32) -> usize {
        let mut listeners = self.listeners.lock().await;
        let Some(subscribers) = listeners.get_mut(&fold_channel(channel)) else {
            return 0;
        };

        let notification = Notification {
            process_id,
            channel: channel.to_string(),
            payload: payload.to_string(),
        };

        subscribers.retain(|subscriber| subscriber.sender.send(notification.clone()).is_ok());
        subscribers.len()
    }
}

/// Fold a channel name the way an unquoted identifier folds.
fn fold_channel(channel: &str) -> String {
    let trimmed = channel.trim();
    match trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(quoted) => quoted.to_string(),
        None => trimmed.to_lowercase(),
    }
}

/// A session's end of the notification channel.
pub struct SessionNotifications {
    pub id: u64,
    pub sender: UnboundedSender<Notification>,
    pub receiver: UnboundedReceiver<Notification>,
}

impl SessionNotifications {
    /// Create a session's channel with a process-unique id.
    #[must_use]
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);

        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            sender,
            receiver,
        }
    }
}

impl Default for SessionNotifications {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SessionNotifications {
        SessionNotifications::new()
    }

    #[tokio::test]
    async fn a_listening_session_receives_a_notification() {
        let hub = NotificationHub::new();
        let mut listener = session();
        hub.listen("events", listener.id, listener.sender.clone())
            .await;

        assert_eq!(hub.notify("events", "hello", 42).await, 1);

        let received = listener.receiver.recv().await.expect("a notification");
        assert_eq!(received.channel, "events");
        assert_eq!(received.payload, "hello");
        assert_eq!(received.process_id, 42);
    }

    #[tokio::test]
    async fn a_session_that_is_not_listening_receives_nothing() {
        let hub = NotificationHub::new();
        let mut listener = session();
        hub.listen("events", listener.id, listener.sender.clone())
            .await;

        assert_eq!(hub.notify("other", "hello", 1).await, 0);
        assert!(listener.receiver.try_recv().is_err());
    }

    /// Unquoted channel names fold, so these name the same channel.
    #[tokio::test]
    async fn channel_names_are_case_insensitive() {
        let hub = NotificationHub::new();
        let mut listener = session();
        hub.listen("Events", listener.id, listener.sender.clone())
            .await;

        assert_eq!(hub.notify("EVENTS", "hi", 1).await, 1);
        assert!(listener.receiver.recv().await.is_some());
    }

    #[tokio::test]
    async fn listening_twice_delivers_once() {
        let hub = NotificationHub::new();
        let mut listener = session();
        hub.listen("events", listener.id, listener.sender.clone())
            .await;
        hub.listen("events", listener.id, listener.sender.clone())
            .await;

        assert_eq!(hub.notify("events", "once", 1).await, 1);
        assert!(listener.receiver.recv().await.is_some());
        assert!(listener.receiver.try_recv().is_err(), "delivered twice");
    }

    #[tokio::test]
    async fn unlisten_stops_delivery() {
        let hub = NotificationHub::new();
        let mut listener = session();
        hub.listen("events", listener.id, listener.sender.clone())
            .await;
        hub.unlisten(Some("events"), listener.id).await;

        assert_eq!(hub.notify("events", "hello", 1).await, 0);
        assert!(listener.receiver.try_recv().is_err());
    }

    /// A hub that only ever adds entries grows for the life of the process.
    #[tokio::test]
    async fn a_departed_session_is_pruned() {
        let hub = NotificationHub::new();
        let listener = session();
        let id = listener.id;
        hub.listen("events", id, listener.sender.clone()).await;

        drop(listener);
        assert_eq!(hub.notify("events", "hello", 1).await, 0);

        // The dead subscriber is gone, not merely skipped.
        let listeners = hub.listeners.lock().await;
        assert!(listeners.get("events").is_some_and(Vec::is_empty));
    }

    #[tokio::test]
    async fn two_sessions_both_receive() {
        let hub = NotificationHub::new();
        let mut first = session();
        let mut second = session();
        hub.listen("events", first.id, first.sender.clone()).await;
        hub.listen("events", second.id, second.sender.clone()).await;

        assert_eq!(hub.notify("events", "broadcast", 1).await, 2);
        assert!(first.receiver.recv().await.is_some());
        assert!(second.receiver.recv().await.is_some());
    }
}
