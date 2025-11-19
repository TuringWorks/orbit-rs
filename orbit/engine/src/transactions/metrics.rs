use crate::cluster::NodeId;
use metrics::{counter, gauge, histogram};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Transaction metrics collector
#[derive(Clone)]
pub struct TransactionMetrics {
    /// Node identifier for this metrics instance
    _node_id: NodeId,
    /// Prefix for metric names
    metrics_prefix: String,
    /// Map of transaction IDs to their start times
    start_times: Arc<RwLock<HashMap<String, Instant>>>,
}

impl TransactionMetrics {
    /// Creates a new transaction metrics collector for the given node.
    pub fn new(node_id: NodeId) -> Self {
        let metrics_prefix = format!("orbit.transaction.{node_id}");

        info!(
            "Transaction metrics initialized with prefix: {}",
            metrics_prefix
        );

        Self {
            _node_id: node_id,
            metrics_prefix,
            start_times: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record transaction start
    pub async fn record_transaction_started(&self, transaction_id: &str) {
        counter!(format!("{}.started.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).increment(1.0);

        let mut start_times = self.start_times.write().await;
        start_times.insert(transaction_id.to_string(), Instant::now());

        debug!("Transaction started: {}", transaction_id);
    }

    /// Record transaction commit
    pub async fn record_transaction_committed(&self, transaction_id: &str) {
        counter!(format!("{}.committed.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).decrement(1.0);

        if let Some(duration) = self.get_duration(transaction_id).await {
            histogram!(format!("{}.duration.seconds", self.metrics_prefix))
                .record(duration.as_secs_f64());
        }

        debug!("Transaction committed: {}", transaction_id);
    }

    /// Record transaction abort
    pub async fn record_transaction_aborted(&self, transaction_id: &str) {
        counter!(format!("{}.aborted.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).decrement(1.0);

        if let Some(duration) = self.get_duration(transaction_id).await {
            histogram!(format!("{}.duration.seconds", self.metrics_prefix))
                .record(duration.as_secs_f64());
        }

        debug!("Transaction aborted: {}", transaction_id);
    }

    /// Record transaction failure
    pub async fn record_transaction_failed(&self, transaction_id: &str, _reason: &str) {
        counter!(format!("{}.failed.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).decrement(1.0);

        if let Some(duration) = self.get_duration(transaction_id).await {
            histogram!(format!("{}.duration.seconds", self.metrics_prefix))
                .record(duration.as_secs_f64());
        }

        debug!("Transaction failed: {}", transaction_id);
    }

    /// Record transaction timeout
    pub async fn record_transaction_timeout(&self, transaction_id: &str) {
        counter!(format!("{}.timeout.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).decrement(1.0);

        if let Some(duration) = self.get_duration(transaction_id).await {
            histogram!(format!("{}.duration.seconds", self.metrics_prefix))
                .record(duration.as_secs_f64());
        }

        debug!("Transaction timed out: {}", transaction_id);
    }

    /// Record prepare phase duration
    pub async fn record_prepare_duration(&self, transaction_id: &str, duration: Duration) {
        histogram!(format!("{}.duration.seconds", self.metrics_prefix))
            .record(duration.as_secs_f64());
        debug!(
            "Transaction {} prepare phase took {:?}",
            transaction_id, duration
        );
    }

    /// Record commit phase duration
    pub async fn record_commit_duration(&self, transaction_id: &str, duration: Duration) {
        histogram!(format!("{}.duration.seconds", self.metrics_prefix))
            .record(duration.as_secs_f64());
        debug!(
            "Transaction {} commit phase took {:?}",
            transaction_id, duration
        );
    }

    /// Record number of participants
    pub fn record_participant_count(&self, count: usize) {
        histogram!(format!("{}.participants.count", self.metrics_prefix)).record(count as f64);
    }

    /// Record queued transactions
    pub fn record_queued_transactions(&self, count: usize) {
        gauge!(format!("{}.queued", self.metrics_prefix)).set(count as f64);
    }

    /// Get duration since transaction started
    async fn get_duration(&self, transaction_id: &str) -> Option<Duration> {
        let mut start_times = self.start_times.write().await;
        start_times
            .remove(transaction_id)
            .map(|start| start.elapsed())
    }
}

/// Saga metrics collector
#[derive(Clone)]
pub struct SagaMetrics {
    /// Node identifier for this metrics instance
    _node_id: NodeId,
    /// Prefix for metric names
    metrics_prefix: String,
    /// Map of saga IDs to their start times
    start_times: Arc<RwLock<HashMap<String, Instant>>>,
}

impl SagaMetrics {
    /// Creates a new saga metrics collector for the given node.
    pub fn new(node_id: NodeId) -> Self {
        let metrics_prefix = format!("orbit.saga.{node_id}");

        info!("Saga metrics initialized with prefix: {}", metrics_prefix);

        Self {
            _node_id: node_id,
            metrics_prefix,
            start_times: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record saga started
    pub async fn record_saga_started(&self, saga_id: &str) {
        counter!(format!("{}.started.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).increment(1.0);

        let mut start_times = self.start_times.write().await;
        start_times.insert(saga_id.to_string(), Instant::now());

        debug!("Saga started: {}", saga_id);
    }

    /// Record saga completed
    pub async fn record_saga_completed(&self, saga_id: &str, step_count: usize) {
        counter!(format!("{}.completed.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).decrement(1.0);
        histogram!(format!("{}.steps.count", self.metrics_prefix)).record(step_count as f64);

        if let Some(duration) = self.get_duration(saga_id).await {
            histogram!(format!("{}.duration.seconds", self.metrics_prefix))
                .record(duration.as_secs_f64());
        }

        debug!("Saga completed: {} with {} steps", saga_id, step_count);
    }

    /// Record saga failed
    pub async fn record_saga_failed(&self, saga_id: &str) {
        counter!(format!("{}.failed.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).decrement(1.0);

        if let Some(duration) = self.get_duration(saga_id).await {
            histogram!(format!("{}.duration.seconds", self.metrics_prefix))
                .record(duration.as_secs_f64());
        }

        debug!("Saga failed: {}", saga_id);
    }

    /// Record saga compensated
    pub async fn record_saga_compensated(&self, saga_id: &str) {
        counter!(format!("{}.compensated.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.active", self.metrics_prefix)).decrement(1.0);

        if let Some(duration) = self.get_duration(saga_id).await {
            histogram!(format!("{}.duration.seconds", self.metrics_prefix))
                .record(duration.as_secs_f64());
        }

        debug!("Saga compensated: {}", saga_id);
    }

    /// Record step executed
    pub fn record_step_executed(&self, step_id: &str, duration: Duration) {
        counter!(format!("{}.step.executed.total", self.metrics_prefix)).increment(1);
        histogram!(format!("{}.duration.seconds", self.metrics_prefix))
            .record(duration.as_secs_f64());
        debug!("Saga step executed: {} in {:?}", step_id, duration);
    }

    /// Record step compensated
    pub fn record_step_compensated(&self, step_id: &str, duration: Duration) {
        counter!(format!("{}.step.compensated.total", self.metrics_prefix)).increment(1);
        histogram!(format!("{}.duration.seconds", self.metrics_prefix))
            .record(duration.as_secs_f64());
        debug!("Saga step compensated: {} in {:?}", step_id, duration);
    }

    /// Record step failed
    pub fn record_step_failed(&self, step_id: &str) {
        counter!(format!("{}.step.failed.total", self.metrics_prefix)).increment(1);
        debug!("Saga step failed: {}", step_id);
    }

    /// Record queued sagas
    pub fn record_queued_sagas(&self, count: usize) {
        gauge!(format!("{}.queued", self.metrics_prefix)).set(count as f64);
    }

    /// Get duration since saga started
    async fn get_duration(&self, saga_id: &str) -> Option<Duration> {
        let mut start_times = self.start_times.write().await;
        start_times.remove(saga_id).map(|start| start.elapsed())
    }
}

/// Lock metrics collector
#[derive(Clone)]
pub struct LockMetrics {
    /// Node identifier for this metrics instance
    _node_id: NodeId,
    /// Prefix for metric names
    metrics_prefix: String,
}

impl LockMetrics {
    /// Creates a new lock metrics collector for the given node.
    pub fn new(node_id: NodeId) -> Self {
        let metrics_prefix = format!("orbit.locks.{node_id}");

        info!("Lock metrics initialized with prefix: {}", metrics_prefix);

        Self {
            _node_id: node_id,
            metrics_prefix,
        }
    }

    /// Record lock acquired
    pub fn record_lock_acquired(&self, resource_id: &str) {
        counter!(format!("{}.acquired.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.held.count", self.metrics_prefix)).increment(1.0);
        debug!("Lock acquired: {}", resource_id);
    }

    /// Record lock released
    pub fn record_lock_released(&self, resource_id: &str, hold_duration: Duration) {
        counter!(format!("{}.released.total", self.metrics_prefix)).increment(1);
        gauge!(format!("{}.held.count", self.metrics_prefix)).decrement(1.0);
        histogram!(format!("{}.hold.duration.seconds", self.metrics_prefix))
            .record(hold_duration.as_secs_f64());
        debug!("Lock released: {} after {:?}", resource_id, hold_duration);
    }

    /// Record lock timeout
    pub fn record_lock_timeout(&self, resource_id: &str, wait_duration: Duration) {
        counter!(format!("{}.timeout.total", self.metrics_prefix)).increment(1);
        histogram!(format!("{}.wait.duration.seconds", self.metrics_prefix))
            .record(wait_duration.as_secs_f64());
        debug!(
            "Lock timeout: {} after waiting {:?}",
            resource_id, wait_duration
        );
    }

    /// Record deadlock detected
    pub fn record_deadlock_detected(&self, transaction_count: usize) {
        counter!(format!("{}.deadlock.detected.total", self.metrics_prefix)).increment(1);
        debug!(
            "Deadlock detected involving {} transactions",
            transaction_count
        );
    }

    /// Record deadlock resolved
    pub fn record_deadlock_resolved(&self, victim_transaction: &str) {
        counter!(format!("{}.deadlock.resolved.total", self.metrics_prefix)).increment(1);
        debug!(
            "Deadlock resolved by aborting transaction: {}",
            victim_transaction
        );
    }

    /// Record waiting locks
    pub fn record_waiting_locks(&self, count: usize) {
        gauge!(format!("{}.waiting.count", self.metrics_prefix)).set(count as f64);
    }
}

/// Metrics aggregator for all transaction-related metrics
#[derive(Clone)]
pub struct TransactionMetricsAggregator {
    /// Transaction metrics collector
    pub transaction_metrics: TransactionMetrics,
    /// Saga metrics collector
    pub saga_metrics: SagaMetrics,
    /// Lock metrics collector
    pub lock_metrics: LockMetrics,
}

impl TransactionMetricsAggregator {
    /// Creates a new metrics aggregator combining all metric collectors for the given node.
    pub fn new(node_id: NodeId) -> Self {
        Self {
            transaction_metrics: TransactionMetrics::new(node_id.clone()),
            saga_metrics: SagaMetrics::new(node_id.clone()),
            lock_metrics: LockMetrics::new(node_id),
        }
    }
}

/// Statistics snapshot for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStats {
    /// Total number of transactions started
    pub total_started: u64,
    /// Total number of transactions committed
    pub total_committed: u64,
    /// Total number of transactions aborted
    pub total_aborted: u64,
    /// Total number of transactions failed
    pub total_failed: u64,
    /// Total number of transactions timed out
    pub total_timeout: u64,
    /// Current number of active transactions
    pub active_count: u64,
    /// Current number of queued transactions
    pub queued_count: u64,
    /// Average transaction duration in milliseconds
    pub average_duration_ms: f64,
    /// 50th percentile transaction duration in milliseconds
    pub p50_duration_ms: f64,
    /// 95th percentile transaction duration in milliseconds
    pub p95_duration_ms: f64,
    /// 99th percentile transaction duration in milliseconds
    pub p99_duration_ms: f64,
}

/// Statistics snapshot for saga operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStats {
    /// Total number of sagas started
    pub total_started: u64,
    /// Total number of sagas completed
    pub total_completed: u64,
    /// Total number of sagas failed
    pub total_failed: u64,
    /// Total number of sagas compensated
    pub total_compensated: u64,
    /// Current number of active sagas
    pub active_count: u64,
    /// Current number of queued sagas
    pub queued_count: u64,
    /// Total number of steps executed across all sagas
    pub total_steps_executed: u64,
    /// Total number of steps compensated
    pub total_steps_compensated: u64,
    /// Total number of steps that failed
    pub total_steps_failed: u64,
    /// Average saga duration in milliseconds
    pub average_duration_ms: f64,
    /// Average number of steps per saga
    pub average_steps_per_saga: f64,
}

/// Statistics snapshot for lock operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStats {
    /// Total number of locks acquired
    pub total_acquired: u64,
    /// Total number of locks released
    pub total_released: u64,
    /// Total number of lock timeout events
    pub total_timeout: u64,
    /// Total number of deadlocks detected
    pub total_deadlocks_detected: u64,
    /// Total number of deadlocks resolved
    pub total_deadlocks_resolved: u64,
    /// Current number of held locks
    pub currently_held: u64,
    /// Current number of transactions waiting for locks
    pub currently_waiting: u64,
    /// Average time waiting for locks in milliseconds
    pub average_wait_duration_ms: f64,
    /// Average time holding locks in milliseconds
    pub average_hold_duration_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction_metrics() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let metrics = TransactionMetrics::new(node_id);

        let tx_id = "test-tx-1";

        metrics.record_transaction_started(tx_id).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        metrics.record_transaction_committed(tx_id).await;

        // Metrics should be recorded (verified through metrics backend)
    }

    #[tokio::test]
    async fn test_saga_metrics() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let metrics = SagaMetrics::new(node_id);

        let saga_id = "test-saga-1";

        metrics.record_saga_started(saga_id).await;
        metrics.record_step_executed("step1", Duration::from_millis(5));
        metrics.record_step_executed("step2", Duration::from_millis(5));
        tokio::time::sleep(Duration::from_millis(10)).await;
        metrics.record_saga_completed(saga_id, 2).await;

        // Metrics should be recorded (verified through metrics backend)
    }

    #[test]
    fn test_lock_metrics() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let metrics = LockMetrics::new(node_id);

        let resource_id = "test-resource";

        metrics.record_lock_acquired(resource_id);
        metrics.record_lock_released(resource_id, Duration::from_millis(100));
        metrics.record_deadlock_detected(2);
        metrics.record_deadlock_resolved("victim-tx");

        // Metrics should be recorded (verified through metrics backend)
    }
}
