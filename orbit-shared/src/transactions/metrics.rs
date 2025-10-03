use crate::mesh::NodeId;
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
    node_id: NodeId,
    metrics_prefix: String,
    start_times: Arc<RwLock<HashMap<String, Instant>>>,
}

impl TransactionMetrics {
    pub fn new(node_id: NodeId) -> Self {
        let metrics_prefix = format!("orbit.transaction.{}", node_id);

        info!(
            "Transaction metrics initialized with prefix: {}",
            metrics_prefix
        );

        Self {
            node_id,
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
    node_id: NodeId,
    metrics_prefix: String,
    start_times: Arc<RwLock<HashMap<String, Instant>>>,
}

impl SagaMetrics {
    pub fn new(node_id: NodeId) -> Self {
        let metrics_prefix = format!("orbit.saga.{}", node_id);

        info!("Saga metrics initialized with prefix: {}", metrics_prefix);

        Self {
            node_id,
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
    node_id: NodeId,
    metrics_prefix: String,
}

impl LockMetrics {
    pub fn new(node_id: NodeId) -> Self {
        let metrics_prefix = format!("orbit.locks.{}", node_id);

        info!("Lock metrics initialized with prefix: {}", metrics_prefix);

        Self {
            node_id,
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
    pub transaction_metrics: TransactionMetrics,
    pub saga_metrics: SagaMetrics,
    pub lock_metrics: LockMetrics,
}

impl TransactionMetricsAggregator {
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
    pub total_started: u64,
    pub total_committed: u64,
    pub total_aborted: u64,
    pub total_failed: u64,
    pub total_timeout: u64,
    pub active_count: u64,
    pub queued_count: u64,
    pub average_duration_ms: f64,
    pub p50_duration_ms: f64,
    pub p95_duration_ms: f64,
    pub p99_duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SagaStats {
    pub total_started: u64,
    pub total_completed: u64,
    pub total_failed: u64,
    pub total_compensated: u64,
    pub active_count: u64,
    pub queued_count: u64,
    pub total_steps_executed: u64,
    pub total_steps_compensated: u64,
    pub total_steps_failed: u64,
    pub average_duration_ms: f64,
    pub average_steps_per_saga: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStats {
    pub total_acquired: u64,
    pub total_released: u64,
    pub total_timeout: u64,
    pub total_deadlocks_detected: u64,
    pub total_deadlocks_resolved: u64,
    pub currently_held: u64,
    pub currently_waiting: u64,
    pub average_wait_duration_ms: f64,
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
