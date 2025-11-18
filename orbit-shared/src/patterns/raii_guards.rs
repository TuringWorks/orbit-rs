//! RAII (Resource Acquisition Is Initialization) guard patterns
//!
//! Guarantees cleanup of resources through Rust's ownership system and Drop trait.

use crate::error::{OrbitError, OrbitResult};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use tracing::{debug, warn};

/// Metric guard that automatically records operation duration on drop
pub struct MetricsGuard<'a> {
    operation: &'a str,
    start: std::time::Instant,
    metrics: Arc<RwLock<OperationMetrics>>,
}

#[derive(Debug, Default, Clone)]
pub struct OperationMetrics {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub average_duration_ms: f64,
}

impl<'a> MetricsGuard<'a> {
    pub fn new(operation: &'a str, metrics: Arc<RwLock<OperationMetrics>>) -> Self {
        debug!(operation = operation, "Starting operation");
        Self {
            operation,
            start: std::time::Instant::now(),
            metrics,
        }
    }

    /// Mark operation as successful (called explicitly or automatically on drop)
    pub async fn success(self) {
        // Explicit success - will not call Drop
        self.finish(true).await;
        std::mem::forget(self);
    }

    /// Mark operation as failed
    pub async fn failure(self) {
        self.finish(false).await;
        std::mem::forget(self);
    }

    async fn finish(&self, success: bool) {
        let duration = self.start.elapsed();
        let mut metrics = self.metrics.write().await;

        metrics.total_operations += 1;
        if success {
            metrics.successful_operations += 1;
        } else {
            metrics.failed_operations += 1;
        }

        let total = metrics.total_operations as f64;
        metrics.average_duration_ms =
            (metrics.average_duration_ms * (total - 1.0) + duration.as_secs_f64() * 1000.0) / total;

        debug!(
            operation = self.operation,
            duration_ms = duration.as_millis(),
            success = success,
            "Operation completed"
        );
    }
}

impl Drop for MetricsGuard<'_> {
    fn drop(&mut self) {
        // Default to failure if not explicitly marked
        warn!(
            operation = self.operation,
            "Operation ended without explicit success/failure - marking as failed"
        );

        // Note: Can't use async in Drop, so we use blocking
        let metrics = self.metrics.clone();
        let duration = self.start.elapsed();
        let operation = self.operation.to_string();

        tokio::spawn(async move {
            let mut m = metrics.write().await;
            m.total_operations += 1;
            m.failed_operations += 1;
            let total = m.total_operations as f64;
            m.average_duration_ms =
                (m.average_duration_ms * (total - 1.0) + duration.as_secs_f64() * 1000.0) / total;
            warn!(operation = %operation, "Implicit failure recorded");
        });
    }
}

/// Transaction guard that automatically rolls back on drop unless committed
pub struct TransactionGuard<T> {
    transaction_id: String,
    inner: Option<T>,
    rollback_fn: Box<dyn FnOnce(String) + Send>,
    committed: bool,
}

impl<T> TransactionGuard<T> {
    pub fn new<F>(transaction_id: String, inner: T, rollback_fn: F) -> Self
    where
        F: FnOnce(String) + Send + 'static,
    {
        Self {
            transaction_id,
            inner: Some(inner),
            rollback_fn: Box::new(rollback_fn),
            committed: false,
        }
    }

    /// Commit the transaction, preventing rollback
    pub fn commit(mut self) -> T {
        self.committed = true;
        self.inner.take().expect("Transaction already consumed")
    }

    /// Get transaction ID
    pub fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    /// Check if committed
    pub fn is_committed(&self) -> bool {
        self.committed
    }
}

impl<T> Drop for TransactionGuard<T> {
    fn drop(&mut self) {
        if !self.committed {
            warn!(tx_id = %self.transaction_id, "Transaction guard dropped without commit - rolling back");
            // Rollback happens here automatically
            let rollback = std::mem::replace(&mut self.rollback_fn, Box::new(|_| {}));
            rollback(self.transaction_id.clone());
        }
    }
}

impl<T> Deref for TransactionGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("Transaction already consumed")
    }
}

impl<T> DerefMut for TransactionGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("Transaction already consumed")
    }
}

/// Scope guard for cleanup actions
pub struct ScopeGuard<F>
where
    F: FnOnce(),
{
    cleanup: Option<F>,
}

impl<F> ScopeGuard<F>
where
    F: FnOnce(),
{
    pub fn new(cleanup: F) -> Self {
        Self {
            cleanup: Some(cleanup),
        }
    }

    /// Cancel the cleanup action
    pub fn cancel(mut self) {
        self.cleanup = None;
    }

    /// Execute cleanup early
    pub fn execute(mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

impl<F> Drop for ScopeGuard<F>
where
    F: FnOnce(),
{
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup();
        }
    }
}

/// Macro to create a scope guard easily
#[macro_export]
macro_rules! defer {
    ($($cleanup:tt)*) => {
        let _guard = $crate::patterns::ScopeGuard::new(|| {
            $($cleanup)*
        });
    };
}

/// Lock guard with timeout for deadlock prevention
pub struct TimedLockGuard<'a, T> {
    guard: Option<RwLockWriteGuard<'a, T>>,
    acquired_at: std::time::Instant,
    timeout: std::time::Duration,
    lock_name: &'static str,
}

impl<'a, T> TimedLockGuard<'a, T> {
    pub async fn try_new(
        lock: &'a RwLock<T>,
        timeout: std::time::Duration,
        lock_name: &'static str,
    ) -> OrbitResult<Self> {
        let acquired_at = std::time::Instant::now();

        let guard = tokio::time::timeout(timeout, lock.write())
            .await
            .map_err(|_| {
                OrbitError::timeout(format!("Failed to acquire lock '{}' within {:?}", lock_name, timeout))
            })?;

        debug!(lock = lock_name, timeout_ms = timeout.as_millis(), "Lock acquired");

        Ok(Self {
            guard: Some(guard),
            acquired_at,
            timeout,
            lock_name,
        })
    }

    /// Get time held
    pub fn held_duration(&self) -> std::time::Duration {
        self.acquired_at.elapsed()
    }

    /// Check if approaching timeout
    pub fn is_approaching_timeout(&self) -> bool {
        self.held_duration() > self.timeout / 2
    }
}

impl<'a, T> Deref for TimedLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.as_ref().expect("Guard already dropped")
    }
}

impl<'a, T> DerefMut for TimedLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.as_mut().expect("Guard already dropped")
    }
}

impl<T> Drop for TimedLockGuard<'_, T> {
    fn drop(&mut self) {
        let held = self.held_duration();
        if held > self.timeout / 2 {
            warn!(
                lock = self.lock_name,
                held_ms = held.as_millis(),
                timeout_ms = self.timeout.as_millis(),
                "Lock held for significant portion of timeout"
            );
        }
        debug!(lock = self.lock_name, held_ms = held.as_millis(), "Lock released");
    }
}

/// Resource pool guard that returns resource to pool on drop
pub struct PooledResource<T> {
    resource: Option<T>,
    return_fn: Option<Box<dyn FnOnce(T) + Send>>,
}

impl<T> PooledResource<T> {
    pub fn new<F>(resource: T, return_fn: F) -> Self
    where
        F: FnOnce(T) + Send + 'static,
    {
        Self {
            resource: Some(resource),
            return_fn: Some(Box::new(return_fn)),
        }
    }

    /// Take ownership of the resource, preventing return to pool
    pub fn take(mut self) -> T {
        self.return_fn = None;
        self.resource.take().expect("Resource already taken")
    }
}

impl<T> Deref for PooledResource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.resource.as_ref().expect("Resource already taken")
    }
}

impl<T> DerefMut for PooledResource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.resource.as_mut().expect("Resource already taken")
    }
}

impl<T> Drop for PooledResource<T> {
    fn drop(&mut self) {
        if let (Some(resource), Some(return_fn)) = (self.resource.take(), self.return_fn.take()) {
            return_fn(resource);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_guard_success() {
        let metrics = Arc::new(RwLock::new(OperationMetrics::default()));
        let guard = MetricsGuard::new("test_op", metrics.clone());

        guard.success().await;

        let m = metrics.read().await;
        assert_eq!(m.successful_operations, 1);
        assert_eq!(m.failed_operations, 0);
    }

    #[tokio::test]
    async fn test_metrics_guard_implicit_failure() {
        let metrics = Arc::new(RwLock::new(OperationMetrics::default()));
        {
            let _guard = MetricsGuard::new("test_op", metrics.clone());
            // Dropped without calling success()
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let m = metrics.read().await;
        assert_eq!(m.failed_operations, 1);
    }

    #[test]
    fn test_transaction_guard_commit() {
        let mut rolled_back = false;

        {
            let guard = TransactionGuard::new(
                "tx-123".to_string(),
                42,
                |_| rolled_back = true,
            );
            let _value = guard.commit();
        }

        assert!(!rolled_back);
    }

    #[test]
    fn test_transaction_guard_rollback() {
        let mut rolled_back = false;

        {
            let _guard = TransactionGuard::new(
                "tx-123".to_string(),
                42,
                |_| rolled_back = true,
            );
            // Dropped without commit
        }

        assert!(rolled_back);
    }

    #[test]
    fn test_scope_guard() {
        let mut cleaned_up = false;

        {
            let _guard = ScopeGuard::new(|| cleaned_up = true);
        }

        assert!(cleaned_up);
    }

    #[test]
    fn test_scope_guard_cancel() {
        let mut cleaned_up = false;

        {
            let guard = ScopeGuard::new(|| cleaned_up = true);
            guard.cancel();
        }

        assert!(!cleaned_up);
    }

    #[tokio::test]
    async fn test_timed_lock_guard() {
        let data = Arc::new(RwLock::new(42));
        let timeout = std::time::Duration::from_secs(1);

        let mut guard = TimedLockGuard::try_new(&data, timeout, "test_lock")
            .await
            .unwrap();

        assert_eq!(**guard, 42);
        **guard = 100;
        assert!(!guard.is_approaching_timeout());
    }
}
