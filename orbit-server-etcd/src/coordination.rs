/// Distributed coordination primitives using etcd
use crate::error::{EtcdError, EtcdResult};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// Distributed lock using etcd
#[derive(Debug)]
pub struct DistributedLock {
    key: String,
    lease_id: Option<i64>,
    acquired_at: Option<Instant>,
    ttl: Duration,
}

impl DistributedLock {
    /// Create a new distributed lock
    pub fn new(key: String, ttl: Duration) -> Self {
        Self {
            key,
            lease_id: None,
            acquired_at: None,
            ttl,
        }
    }

    /// Try to acquire the lock
    pub async fn try_lock(&mut self) -> EtcdResult<bool> {
        if self.is_held() {
            return Ok(true);
        }

        // In a real implementation with etcd:
        // 1. Create a lease
        // 2. Try to create the lock key with lease
        // 3. If key exists, lock is held by someone else
        
        // Simulate lock acquisition (70% success)
        let acquired = fastrand::f32() < 0.7;
        
        if acquired {
            self.lease_id = Some(fastrand::i64(1..1000000));
            self.acquired_at = Some(Instant::now());
            debug!("Acquired lock: {}", self.key);
        } else {
            debug!("Failed to acquire lock: {}", self.key);
        }
        
        Ok(acquired)
    }

    /// Release the lock
    pub async fn unlock(&mut self) -> EtcdResult<()> {
        if let Some(lease_id) = self.lease_id {
            // In real implementation: revoke the lease in etcd
            debug!("Released lock: {} (lease: {})", self.key, lease_id);
            self.lease_id = None;
            self.acquired_at = None;
        }
        Ok(())
    }

    /// Check if lock is currently held
    pub fn is_held(&self) -> bool {
        self.lease_id.is_some()
    }

    /// Get how long the lock has been held
    pub fn held_duration(&self) -> Option<Duration> {
        self.acquired_at.map(|t| t.elapsed())
    }
}

/// Distributed barrier for synchronization
#[derive(Debug)]
pub struct Barrier {
    name: String,
    participant_count: usize,
    arrived: Arc<RwLock<usize>>,
}

impl Barrier {
    /// Create a new barrier
    pub fn new(name: String, participant_count: usize) -> Self {
        Self {
            name,
            participant_count,
            arrived: Arc::new(RwLock::new(0)),
        }
    }

    /// Wait at the barrier until all participants arrive
    pub async fn wait(&self) -> EtcdResult<()> {
        info!("Participant waiting at barrier: {}", self.name);
        
        // Increment arrival count
        {
            let mut arrived = self.arrived.write().await;
            *arrived += 1;
            
            if *arrived >= self.participant_count {
                info!("All participants arrived at barrier: {}", self.name);
                return Ok(());
            }
        }
        
        // Wait for others (in real implementation, watch etcd key)
        let check_interval = Duration::from_millis(100);
        loop {
            tokio::time::sleep(check_interval).await;
            
            let arrived = *self.arrived.read().await;
            if arrived >= self.participant_count {
                break;
            }
        }
        
        Ok(())
    }

    /// Reset the barrier
    pub async fn reset(&self) {
        let mut arrived = self.arrived.write().await;
        *arrived = 0;
    }
}

/// Distributed semaphore
#[derive(Debug)]
pub struct Semaphore {
    name: String,
    max_permits: usize,
    available: Arc<Mutex<usize>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(name: String, max_permits: usize) -> Self {
        Self {
            name,
            max_permits,
            available: Arc::new(Mutex::new(max_permits)),
        }
    }

    /// Acquire a permit
    pub async fn acquire(&self) -> EtcdResult<bool> {
        let mut available = self.available.lock().await;
        
        if *available > 0 {
            *available -= 1;
            debug!("Acquired semaphore permit: {} ({} remaining)", self.name, *available);
            Ok(true)
        } else {
            debug!("No permits available for semaphore: {}", self.name);
            Ok(false)
        }
    }

    /// Release a permit
    pub async fn release(&self) -> EtcdResult<()> {
        let mut available = self.available.lock().await;
        
        if *available < self.max_permits {
            *available += 1;
            debug!("Released semaphore permit: {} ({} available)", self.name, *available);
        }
        
        Ok(())
    }

    /// Get available permits
    pub async fn available_permits(&self) -> usize {
        *self.available.lock().await
    }
}

/// Main coordinator for distributed primitives
pub struct Coordinator {
    locks: Arc<RwLock<HashMap<String, Arc<Mutex<DistributedLock>>>>>,
    barriers: Arc<RwLock<HashMap<String, Arc<Barrier>>>>,
    semaphores: Arc<RwLock<HashMap<String, Arc<Semaphore>>>>,
}

impl Coordinator {
    /// Create a new coordinator
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
            barriers: Arc::new(RwLock::new(HashMap::new())),
            semaphores: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a distributed lock
    pub async fn get_lock(&self, key: String, ttl: Duration) -> Arc<Mutex<DistributedLock>> {
        let mut locks = self.locks.write().await;
        
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(DistributedLock::new(key, ttl))))
            .clone()
    }

    /// Get or create a barrier
    pub async fn get_barrier(&self, name: String, participant_count: usize) -> Arc<Barrier> {
        let mut barriers = self.barriers.write().await;
        
        barriers
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Barrier::new(name, participant_count)))
            .clone()
    }

    /// Get or create a semaphore
    pub async fn get_semaphore(&self, name: String, max_permits: usize) -> Arc<Semaphore> {
        let mut semaphores = self.semaphores.write().await;
        
        semaphores
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(name, max_permits)))
            .clone()
    }

    /// Perform distributed coordination for a set of operations
    pub async fn coordinate(
        &self,
        operation_name: &str,
        node_count: usize,
    ) -> EtcdResult<()> {
        info!("Coordinating operation '{}' across {} nodes", operation_name, node_count);
        
        // Create a barrier for this operation
        let barrier = self.get_barrier(operation_name.to_string(), node_count).await;
        
        // Wait for all nodes to be ready
        barrier.wait().await?;
        
        info!("Operation '{}' synchronized across all nodes", operation_name);
        Ok(())
    }

    /// Clean up stale locks and resources
    pub async fn cleanup_stale_resources(&self, max_age: Duration) {
        let mut locks = self.locks.write().await;
        
        locks.retain(|key, lock| {
            let lock_guard = lock.try_lock();
            if let Ok(mut lock) = lock_guard {
                if let Some(duration) = lock.held_duration() {
                    if duration > max_age {
                        warn!("Cleaning up stale lock: {}", key);
                        let _ = lock.lease_id.take();
                        return false;
                    }
                }
            }
            true
        });
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_distributed_lock() {
        let mut lock = DistributedLock::new("test-lock".to_string(), Duration::from_secs(10));
        
        assert!(!lock.is_held());
        
        // Try to acquire (may succeed or fail based on simulation)
        let _ = lock.try_lock().await;
        
        // If acquired, can release
        if lock.is_held() {
            lock.unlock().await.unwrap();
            assert!(!lock.is_held());
        }
    }

    #[tokio::test]
    async fn test_barrier() {
        let barrier = Barrier::new("test-barrier".to_string(), 2);
        
        let barrier_clone = Arc::new(barrier);
        let barrier1 = Arc::clone(&barrier_clone);
        let barrier2 = Arc::clone(&barrier_clone);
        
        // Spawn two tasks that wait at barrier
        let task1 = tokio::spawn(async move {
            barrier1.wait().await
        });
        
        let task2 = tokio::spawn(async move {
            barrier2.wait().await
        });
        
        // Both should complete
        let (r1, r2) = tokio::join!(task1, task2);
        assert!(r1.is_ok());
        assert!(r2.is_ok());
    }

    #[tokio::test]
    async fn test_semaphore() {
        let sem = Semaphore::new("test-sem".to_string(), 3);
        
        assert_eq!(sem.available_permits().await, 3);
        
        // Acquire permits
        sem.acquire().await.unwrap();
        assert_eq!(sem.available_permits().await, 2);
        
        sem.acquire().await.unwrap();
        assert_eq!(sem.available_permits().await, 1);
        
        // Release a permit
        sem.release().await.unwrap();
        assert_eq!(sem.available_permits().await, 2);
    }

    #[tokio::test]
    async fn test_coordinator() {
        let coordinator = Coordinator::new();
        
        // Get a lock
        let lock = coordinator.get_lock("test".to_string(), Duration::from_secs(10)).await;
        let mut lock_guard = lock.lock().await;
        let _ = lock_guard.try_lock().await;
        
        // Get a semaphore
        let sem = coordinator.get_semaphore("test-sem".to_string(), 5).await;
        assert_eq!(sem.available_permits().await, 5);
    }
}
