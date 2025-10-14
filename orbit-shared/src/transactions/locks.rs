use crate::exception::OrbitResult;
use crate::mesh::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// Unique identifier for a distributed lock
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LockId {
    pub resource_id: String,
    pub lock_key: String,
}

impl LockId {
    pub fn new(resource_id: String) -> Self {
        Self {
            resource_id: resource_id.clone(),
            lock_key: format!("lock:{resource_id}"),
        }
    }

    pub fn from_key(lock_key: String) -> Self {
        let resource_id = lock_key
            .strip_prefix("lock:")
            .unwrap_or(&lock_key)
            .to_string();
        Self {
            resource_id,
            lock_key,
        }
    }
}

impl std::fmt::Display for LockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.lock_key)
    }
}

/// Owner information for a lock
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LockOwner {
    pub node_id: NodeId,
    pub transaction_id: String,
    pub acquired_at: SystemTime,
}

/// Types of locks available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockMode {
    /// Exclusive lock - prevents any other locks
    Exclusive,
    /// Shared lock - allows other shared locks but not exclusive
    Shared,
}

/// Lock request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRequest {
    pub lock_id: LockId,
    pub owner: LockOwner,
    pub mode: LockMode,
    pub timeout: Duration,
    pub requested_at: SystemTime,
}

/// Status of a lock
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockStatus {
    /// Lock is available
    Available,
    /// Lock is held exclusively
    HeldExclusive { owner: LockOwner },
    /// Lock is held by one or more shared holders
    HeldShared { owners: Vec<LockOwner> },
    /// Lock request is waiting in queue
    Waiting { position: usize },
}

/// Distributed lock with metadata
#[derive(Debug, Clone)]
pub struct DistributedLock {
    pub lock_id: LockId,
    pub mode: LockMode,
    pub owners: Vec<LockOwner>,
    pub acquired_at: SystemTime,
    pub expires_at: SystemTime,
    pub wait_queue: VecDeque<LockRequest>,
}

impl DistributedLock {
    pub fn new(lock_id: LockId) -> Self {
        Self {
            lock_id,
            mode: LockMode::Exclusive,
            owners: Vec::new(),
            acquired_at: SystemTime::now(),
            expires_at: SystemTime::now(),
            wait_queue: VecDeque::new(),
        }
    }

    /// Check if the lock is currently held
    pub fn is_held(&self) -> bool {
        !self.owners.is_empty() && SystemTime::now() < self.expires_at
    }

    /// Check if the lock is available
    pub fn is_available(&self) -> bool {
        !self.is_held()
    }

    /// Check if a specific owner holds this lock
    pub fn is_held_by(&self, owner: &LockOwner) -> bool {
        self.owners.iter().any(|o| o == owner)
    }

    /// Try to acquire the lock
    pub fn try_acquire(&mut self, owner: LockOwner, mode: LockMode, duration: Duration) -> bool {
        if self.is_available() {
            // Lock is available, acquire it
            self.owners.push(owner);
            self.mode = mode;
            self.acquired_at = SystemTime::now();
            self.expires_at = self.acquired_at + duration;
            true
        } else if matches!(mode, LockMode::Shared) && matches!(self.mode, LockMode::Shared) {
            // Can add another shared lock holder
            self.owners.push(owner);
            true
        } else {
            // Lock is not available
            false
        }
    }

    /// Release the lock for a specific owner
    pub fn release(&mut self, owner: &LockOwner) -> bool {
        if let Some(pos) = self.owners.iter().position(|o| o == owner) {
            self.owners.remove(pos);
            if self.owners.is_empty() {
                // Last owner released, process wait queue
                return true;
            }
        }
        false
    }

    /// Add a request to the wait queue
    pub fn enqueue_request(&mut self, request: LockRequest) {
        self.wait_queue.push_back(request);
    }

    /// Process the next request in the wait queue
    pub fn process_wait_queue(&mut self) -> Option<LockRequest> {
        if self.is_available() {
            self.wait_queue.pop_front()
        } else {
            None
        }
    }
}

/// Wait-for graph edge representing a dependency
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WaitForEdge {
    pub waiting_transaction: String,
    pub holding_transaction: String,
    pub resource_id: String,
}

/// Deadlock detection result
#[derive(Debug, Clone)]
pub struct DeadlockCycle {
    pub transactions: Vec<String>,
    pub resources: Vec<String>,
    pub detected_at: SystemTime,
}

impl DeadlockCycle {
    /// Get the victim transaction to abort (youngest transaction)
    pub fn select_victim(&self) -> Option<String> {
        // Simple heuristic: select the last transaction in the cycle
        self.transactions.last().cloned()
    }
}

/// Deadlock detector using wait-for graph
pub struct DeadlockDetector {
    /// Wait-for graph edges
    edges: Arc<RwLock<HashSet<WaitForEdge>>>,
    /// Transaction to resources mapping
    transaction_resources: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Resource to transactions mapping
    resource_transactions: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Detection interval
    detection_interval: Duration,
}

impl DeadlockDetector {
    pub fn new(detection_interval: Duration) -> Self {
        Self {
            edges: Arc::new(RwLock::new(HashSet::new())),
            transaction_resources: Arc::new(RwLock::new(HashMap::new())),
            resource_transactions: Arc::new(RwLock::new(HashMap::new())),
            detection_interval,
        }
    }

    /// Add a wait-for edge
    pub async fn add_wait_for(&self, waiting_tx: String, holding_tx: String, resource_id: String) {
        let edge = WaitForEdge {
            waiting_transaction: waiting_tx.clone(),
            holding_transaction: holding_tx,
            resource_id: resource_id.clone(),
        };

        let mut edges = self.edges.write().await;
        edges.insert(edge);

        // Update transaction-resource mappings
        let mut tx_resources = self.transaction_resources.write().await;
        tx_resources
            .entry(waiting_tx.clone())
            .or_insert_with(HashSet::new)
            .insert(resource_id.clone());

        let mut resource_txs = self.resource_transactions.write().await;
        resource_txs
            .entry(resource_id)
            .or_insert_with(HashSet::new)
            .insert(waiting_tx);
    }

    /// Remove wait-for edges for a transaction
    pub async fn remove_transaction(&self, transaction_id: &str) {
        let mut edges = self.edges.write().await;
        edges.retain(|edge| {
            edge.waiting_transaction != transaction_id && edge.holding_transaction != transaction_id
        });

        let mut tx_resources = self.transaction_resources.write().await;
        if let Some(resources) = tx_resources.remove(transaction_id) {
            let mut resource_txs = self.resource_transactions.write().await;
            for resource in resources {
                if let Some(txs) = resource_txs.get_mut(&resource) {
                    txs.remove(transaction_id);
                    if txs.is_empty() {
                        resource_txs.remove(&resource);
                    }
                }
            }
        }
    }

    /// Detect deadlock cycles in the wait-for graph
    pub async fn detect_deadlock(&self) -> Option<DeadlockCycle> {
        let edges = self.edges.read().await;

        // Build adjacency list
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_transactions: HashSet<String> = HashSet::new();

        for edge in edges.iter() {
            all_transactions.insert(edge.waiting_transaction.clone());
            all_transactions.insert(edge.holding_transaction.clone());

            graph
                .entry(edge.waiting_transaction.clone())
                .or_default()
                .push(edge.holding_transaction.clone());
        }

        // Detect cycle using DFS
        for start_tx in all_transactions.iter() {
            if let Some(cycle) = self.detect_cycle_dfs(&graph, start_tx) {
                // Get resources involved in the cycle
                let cycle_set: HashSet<_> = cycle.iter().cloned().collect();
                let resources: Vec<String> = edges
                    .iter()
                    .filter(|edge| {
                        cycle_set.contains(&edge.waiting_transaction)
                            && cycle_set.contains(&edge.holding_transaction)
                    })
                    .map(|edge| edge.resource_id.clone())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();

                return Some(DeadlockCycle {
                    transactions: cycle,
                    resources,
                    detected_at: SystemTime::now(),
                });
            }
        }

        None
    }

    /// DFS-based cycle detection
    fn detect_cycle_dfs(
        &self,
        graph: &HashMap<String, Vec<String>>,
        start: &str,
    ) -> Option<Vec<String>> {
        let mut visited = HashSet::new();
        let mut path = Vec::new();

        Self::dfs_helper(graph, start, start, &mut visited, &mut path)
    }

    fn dfs_helper(
        graph: &HashMap<String, Vec<String>>,
        current: &str,
        target: &str,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        if !path.is_empty() && current == target {
            // Found a cycle
            return Some(path.clone());
        }

        if visited.contains(current) {
            return None;
        }

        visited.insert(current.to_string());
        path.push(current.to_string());

        if let Some(neighbors) = graph.get(current) {
            for neighbor in neighbors {
                if let Some(cycle) = Self::dfs_helper(graph, neighbor, target, visited, path) {
                    return Some(cycle);
                }
            }
        }

        path.pop();
        None
    }

    /// Start background deadlock detection
    pub async fn start_detection(&self) {
        let detector = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(detector.detection_interval);

            loop {
                interval.tick().await;

                if let Some(deadlock) = detector.detect_deadlock().await {
                    warn!(
                        "Deadlock detected! Transactions: {:?}, Resources: {:?}",
                        deadlock.transactions, deadlock.resources
                    );

                    if let Some(victim) = deadlock.select_victim() {
                        info!("Selected victim transaction for abort: {}", victim);
                        // In a real system, this would trigger transaction abort
                        detector.remove_transaction(&victim).await;
                    }
                }
            }
        });
    }
}

impl Clone for DeadlockDetector {
    fn clone(&self) -> Self {
        Self {
            edges: Arc::clone(&self.edges),
            transaction_resources: Arc::clone(&self.transaction_resources),
            resource_transactions: Arc::clone(&self.resource_transactions),
            detection_interval: self.detection_interval,
        }
    }
}

/// Configuration for the distributed lock manager
#[derive(Debug, Clone)]
pub struct LockManagerConfig {
    pub default_lock_timeout: Duration,
    pub max_wait_time: Duration,
    pub deadlock_detection_interval: Duration,
    pub lock_cleanup_interval: Duration,
    pub enable_deadlock_detection: bool,
}

impl Default for LockManagerConfig {
    fn default() -> Self {
        Self {
            default_lock_timeout: Duration::from_secs(30),
            max_wait_time: Duration::from_secs(60),
            deadlock_detection_interval: Duration::from_secs(5),
            lock_cleanup_interval: Duration::from_secs(10),
            enable_deadlock_detection: true,
        }
    }
}

/// Distributed lock manager
pub struct DistributedLockManager {
    node_id: NodeId,
    locks: Arc<RwLock<HashMap<LockId, DistributedLock>>>,
    config: LockManagerConfig,
    deadlock_detector: DeadlockDetector,
    lock_waiters: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

impl DistributedLockManager {
    pub fn new(node_id: NodeId, config: LockManagerConfig) -> Self {
        let deadlock_detector = DeadlockDetector::new(config.deadlock_detection_interval);

        Self {
            node_id,
            locks: Arc::new(RwLock::new(HashMap::new())),
            config,
            deadlock_detector,
            lock_waiters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Acquire a lock
    pub async fn acquire_lock(
        &self,
        resource_id: String,
        transaction_id: String,
        mode: LockMode,
    ) -> OrbitResult<bool> {
        let lock_id = LockId::new(resource_id.clone());
        let owner = LockOwner {
            node_id: self.node_id.clone(),
            transaction_id: transaction_id.clone(),
            acquired_at: SystemTime::now(),
        };

        let mut locks = self.locks.write().await;
        let lock = locks
            .entry(lock_id.clone())
            .or_insert_with(|| DistributedLock::new(lock_id.clone()));

        if lock.try_acquire(owner.clone(), mode, self.config.default_lock_timeout) {
            debug!(
                "Lock acquired: {} by transaction {}",
                resource_id, transaction_id
            );
            Ok(true)
        } else {
            // Lock is not immediately available
            debug!(
                "Lock {} is held, adding to wait queue for transaction {}",
                resource_id, transaction_id
            );

            // Add to wait queue
            let request = LockRequest {
                lock_id: lock_id.clone(),
                owner,
                mode,
                timeout: self.config.max_wait_time,
                requested_at: SystemTime::now(),
            };

            lock.enqueue_request(request);

            // Add wait-for edge if deadlock detection is enabled
            if self.config.enable_deadlock_detection {
                for holding_owner in &lock.owners {
                    self.deadlock_detector
                        .add_wait_for(
                            transaction_id.clone(),
                            holding_owner.transaction_id.clone(),
                            resource_id.clone(),
                        )
                        .await;
                }
            }

            Ok(false)
        }
    }

    /// Release a lock
    pub async fn release_lock(
        &self,
        resource_id: String,
        transaction_id: String,
    ) -> OrbitResult<()> {
        let lock_id = LockId::new(resource_id.clone());
        let owner = LockOwner {
            node_id: self.node_id.clone(),
            transaction_id: transaction_id.clone(),
            acquired_at: SystemTime::now(), // Not used for comparison
        };

        let mut locks = self.locks.write().await;

        if let Some(lock) = locks.get_mut(&lock_id) {
            if lock.release(&owner) {
                debug!(
                    "Lock released: {} by transaction {}",
                    resource_id, transaction_id
                );

                // Remove from deadlock detector
                if self.config.enable_deadlock_detection {
                    self.deadlock_detector
                        .remove_transaction(&transaction_id)
                        .await;
                }

                // Process wait queue
                if let Some(next_request) = lock.process_wait_queue() {
                    if lock.try_acquire(
                        next_request.owner.clone(),
                        next_request.mode,
                        self.config.default_lock_timeout,
                    ) {
                        info!(
                            "Lock {} granted to waiting transaction {}",
                            resource_id, next_request.owner.transaction_id
                        );

                        // Notify waiter
                        let mut waiters = self.lock_waiters.lock().await;
                        if let Some(sender) = waiters.remove(&next_request.owner.transaction_id) {
                            let _ = sender.send(true);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Get lock status
    pub async fn get_lock_status(&self, resource_id: String) -> LockStatus {
        let lock_id = LockId::new(resource_id);
        let locks = self.locks.read().await;

        if let Some(lock) = locks.get(&lock_id) {
            if lock.is_available() {
                LockStatus::Available
            } else {
                match lock.mode {
                    LockMode::Exclusive => LockStatus::HeldExclusive {
                        owner: lock.owners[0].clone(),
                    },
                    LockMode::Shared => LockStatus::HeldShared {
                        owners: lock.owners.clone(),
                    },
                }
            }
        } else {
            LockStatus::Available
        }
    }

    /// Start background tasks
    pub async fn start(&self) {
        if self.config.enable_deadlock_detection {
            self.deadlock_detector.start_detection().await;
        }

        // Start lock cleanup task
        let manager = self.clone();
        tokio::spawn(async move {
            manager.cleanup_expired_locks().await;
        });

        info!("Distributed lock manager started on node {}", self.node_id);
    }

    /// Clean up expired locks periodically
    async fn cleanup_expired_locks(&self) {
        let mut interval = tokio::time::interval(self.config.lock_cleanup_interval);

        loop {
            interval.tick().await;

            let now = SystemTime::now();
            let mut locks = self.locks.write().await;

            let expired_locks: Vec<LockId> = locks
                .iter()
                .filter(|(_, lock)| lock.expires_at < now && lock.is_held())
                .map(|(id, _)| id.clone())
                .collect();

            for lock_id in expired_locks {
                if let Some(lock) = locks.get_mut(&lock_id) {
                    warn!("Cleaning up expired lock: {}", lock_id);
                    lock.owners.clear();

                    // Process wait queue
                    if let Some(next_request) = lock.process_wait_queue() {
                        if lock.try_acquire(
                            next_request.owner.clone(),
                            next_request.mode,
                            self.config.default_lock_timeout,
                        ) {
                            info!(
                                "Lock {} granted to waiting transaction {} after cleanup",
                                lock_id, next_request.owner.transaction_id
                            );
                        }
                    }
                }
            }
        }
    }
}

impl Clone for DistributedLockManager {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            locks: Arc::clone(&self.locks),
            config: self.config.clone(),
            deadlock_detector: self.deadlock_detector.clone(),
            lock_waiters: Arc::clone(&self.lock_waiters),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lock_acquisition() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let config = LockManagerConfig::default();
        let manager = DistributedLockManager::new(node_id, config);

        let resource = "test-resource".to_string();
        let tx1 = "tx1".to_string();

        let result = manager
            .acquire_lock(resource.clone(), tx1.clone(), LockMode::Exclusive)
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Second transaction should not be able to acquire the lock
        let tx2 = "tx2".to_string();
        let result = manager
            .acquire_lock(resource.clone(), tx2.clone(), LockMode::Exclusive)
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should be in wait queue

        // Release lock
        manager.release_lock(resource.clone(), tx1).await.unwrap();
    }

    #[tokio::test]
    async fn test_shared_locks() {
        let node_id = NodeId::new("test-node".to_string(), "default".to_string());
        let config = LockManagerConfig::default();
        let manager = DistributedLockManager::new(node_id, config);

        let resource = "test-resource".to_string();
        let tx1 = "tx1".to_string();
        let tx2 = "tx2".to_string();

        // Both transactions should be able to acquire shared locks
        let result1 = manager
            .acquire_lock(resource.clone(), tx1.clone(), LockMode::Shared)
            .await;
        assert!(result1.unwrap());

        let result2 = manager
            .acquire_lock(resource.clone(), tx2.clone(), LockMode::Shared)
            .await;
        assert!(result2.unwrap());

        // But exclusive lock should not be granted
        let tx3 = "tx3".to_string();
        let result3 = manager
            .acquire_lock(resource.clone(), tx3.clone(), LockMode::Exclusive)
            .await;
        assert!(!result3.unwrap());
    }

    #[tokio::test]
    async fn test_deadlock_detection() {
        let detector = DeadlockDetector::new(Duration::from_secs(1));

        // Create a deadlock cycle: tx1 -> tx2 -> tx1
        detector
            .add_wait_for(
                "tx1".to_string(),
                "tx2".to_string(),
                "resource1".to_string(),
            )
            .await;
        detector
            .add_wait_for(
                "tx2".to_string(),
                "tx1".to_string(),
                "resource2".to_string(),
            )
            .await;

        let deadlock = detector.detect_deadlock().await;
        assert!(deadlock.is_some());

        let cycle = deadlock.unwrap();
        assert_eq!(cycle.transactions.len(), 2);
        assert!(cycle.transactions.contains(&"tx1".to_string()));
        assert!(cycle.transactions.contains(&"tx2".to_string()));
    }
}
