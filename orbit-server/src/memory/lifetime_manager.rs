use anyhow::{anyhow, Result};
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::ThreadId;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use super::pin_manager::{PinKey, LifetimeClass};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GcTrigger {
    TimeBasedTtl,
    MemoryPressure(f32),
    OperatorCompletion,
    ExplicitDrop,
}

#[derive(Debug, Clone)]
pub struct LifetimePolicy {
    pub class: LifetimeClass,
    pub ttl_ms: Option<u64>,
    pub max_memory_mb: Option<u64>,
    pub gc_trigger: GcTrigger,
}

#[derive(Debug)]
pub struct RetiredSlice {
    pub slice_key: PinKey,
    pub retire_epoch: u64,
    pub unmap_fn: Box<dyn FnOnce() + Send>,
}

pub trait LifetimeManager: Send + Sync {
    fn register_slice(&self, key: PinKey, policy: LifetimePolicy) -> Result<()>;
    fn retire_slice(&self, key: PinKey) -> Result<()>;
    fn advance_epoch(&self) -> u64;
    fn gc_expired(&self) -> Result<usize>;
    fn get_current_epoch(&self) -> u64;
}

pub struct EpochManager {
    current_epoch: AtomicU64,
    retire_lists: Vec<Mutex<Vec<RetiredSlice>>>,
    reader_epochs: DashMap<ThreadId, u64>,
    epoch_history_size: usize,
}

impl EpochManager {
    pub fn new(history_size: usize) -> Self {
        let mut retire_lists = Vec::new();
        for _ in 0..history_size {
            retire_lists.push(Mutex::new(Vec::new()));
        }

        Self {
            current_epoch: AtomicU64::new(0),
            retire_lists,
            reader_epochs: DashMap::new(),
            epoch_history_size: history_size,
        }
    }

    pub async fn enter_read(&self) -> ReadGuard {
        let epoch = self.current_epoch.load(Ordering::Acquire);
        let thread_id = std::thread::current().id();
        self.reader_epochs.insert(thread_id, epoch);
        ReadGuard {
            epoch_mgr: self,
            thread_id,
        }
    }

    pub async fn retire_slice(&self, slice: RetiredSlice) {
        let epoch = self.current_epoch.load(Ordering::Acquire);
        let list_index = (epoch as usize) % self.epoch_history_size;
        
        let mut retire_list = self.retire_lists[list_index].lock().await;
        retire_list.push(slice);
    }

    pub fn advance_epoch(&self) -> u64 {
        let new_epoch = self.current_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        
        // Start background GC for old epochs
        let epoch_mgr = self;
        tokio::spawn(async move {
            if let Err(e) = epoch_mgr.gc_old_slices(new_epoch).await {
                eprintln!("Error in epoch GC: {}", e);
            }
        });
        
        new_epoch
    }

    async fn gc_old_slices(&self, current_epoch: u64) -> Result<()> {
        // Find minimum reader epoch
        let min_reader_epoch = self.reader_epochs
            .iter()
            .map(|entry| *entry.value())
            .min()
            .unwrap_or(current_epoch);

        // GC slices from epochs that no readers can see
        let safe_gc_epoch = min_reader_epoch.saturating_sub(1);

        for epoch in 0..safe_gc_epoch {
            let list_index = (epoch as usize) % self.epoch_history_size;
            let mut retire_list = self.retire_lists[list_index].lock().await;
            
            // Process all retired slices from this epoch
            let mut processed_count = 0;
            while let Some(retired_slice) = retire_list.pop() {
                // Execute the unmap function
                (retired_slice.unmap_fn)();
                processed_count += 1;
            }
            
            if processed_count > 0 {
                println!("Garbage collected {} slices from epoch {}", processed_count, epoch);
            }
        }

        Ok(())
    }

    fn get_current_epoch(&self) -> u64 {
        self.current_epoch.load(Ordering::Acquire)
    }
}

pub struct ReadGuard<'a> {
    epoch_mgr: &'a EpochManager,
    thread_id: ThreadId,
}

impl<'a> Drop for ReadGuard<'a> {
    fn drop(&mut self) {
        self.epoch_mgr.reader_epochs.remove(&self.thread_id);
    }
}

pub struct DefaultLifetimeManager {
    epoch_manager: Arc<EpochManager>,
    slice_policies: DashMap<PinKey, LifetimePolicy>,
    slice_registered_at: DashMap<PinKey, Instant>,
    memory_usage_by_class: DashMap<LifetimeClass, AtomicU64>,
}

impl DefaultLifetimeManager {
    pub fn new() -> Self {
        Self {
            epoch_manager: Arc::new(EpochManager::new(64)), // 64 epoch history
            slice_policies: DashMap::new(),
            slice_registered_at: DashMap::new(),
            memory_usage_by_class: DashMap::new(),
        }
    }

    fn should_gc_slice(&self, key: PinKey, policy: &LifetimePolicy) -> bool {
        match policy.gc_trigger {
            GcTrigger::TimeBasedTtl => {
                if let Some(ttl_ms) = policy.ttl_ms {
                    if let Some(registered_at) = self.slice_registered_at.get(&key) {
                        let age = registered_at.elapsed();
                        return age.as_millis() as u64 > ttl_ms;
                    }
                }
                false
            }
            GcTrigger::MemoryPressure(threshold) => {
                if let Some(max_memory) = policy.max_memory_mb {
                    let current_usage = self.memory_usage_by_class
                        .get(&policy.class)
                        .map(|usage| usage.load(Ordering::Relaxed))
                        .unwrap_or(0);
                    
                    let max_bytes = max_memory * 1024 * 1024;
                    return (current_usage as f64 / max_bytes as f64) > threshold as f64;
                }
                false
            }
            GcTrigger::OperatorCompletion => {
                // This would be triggered externally by query operators
                false
            }
            GcTrigger::ExplicitDrop => {
                // Only GC when explicitly requested
                false
            }
        }
    }

    fn update_memory_usage(&self, class: LifetimeClass, delta: i64) {
        self.memory_usage_by_class
            .entry(class)
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(delta as u64, Ordering::Relaxed);
    }
}

impl Default for DefaultLifetimeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LifetimeManager for DefaultLifetimeManager {
    fn register_slice(&self, key: PinKey, policy: LifetimePolicy) -> Result<()> {
        self.slice_policies.insert(key, policy.clone());
        self.slice_registered_at.insert(key, Instant::now());

        // Update memory usage tracking
        if let Some(max_memory) = policy.max_memory_mb {
            self.update_memory_usage(policy.class, max_memory as i64);
        }

        Ok(())
    }

    fn retire_slice(&self, key: PinKey) -> Result<()> {
        let policy = self.slice_policies.remove(&key)
            .ok_or_else(|| anyhow!("Slice {} not registered", key.0))?
            .1;

        let current_epoch = self.epoch_manager.get_current_epoch();

        // Create retirement record
        let retired_slice = RetiredSlice {
            slice_key: key,
            retire_epoch: current_epoch,
            unmap_fn: Box::new(move || {
                // This would call the actual unmap/cleanup logic
                // For now, just log the retirement
                println!("Retired slice {} at epoch {}", key.0, current_epoch);
            }),
        };

        // Add to retirement queue
        let epoch_manager = self.epoch_manager.clone();
        tokio::spawn(async move {
            epoch_manager.retire_slice(retired_slice).await;
        });

        // Update memory usage
        if let Some(max_memory) = policy.max_memory_mb {
            self.update_memory_usage(policy.class, -(max_memory as i64));
        }

        self.slice_registered_at.remove(&key);

        Ok(())
    }

    fn advance_epoch(&self) -> u64 {
        self.epoch_manager.advance_epoch()
    }

    fn gc_expired(&self) -> Result<usize> {
        let mut expired_count = 0;
        let mut expired_keys = Vec::new();

        // Find expired slices
        for entry in self.slice_policies.iter() {
            let key = *entry.key();
            let policy = entry.value();

            if self.should_gc_slice(key, policy) {
                expired_keys.push(key);
            }
        }

        // Retire expired slices
        for key in expired_keys {
            if let Err(e) = self.retire_slice(key) {
                eprintln!("Error retiring expired slice {}: {}", key.0, e);
            } else {
                expired_count += 1;
            }
        }

        Ok(expired_count)
    }

    fn get_current_epoch(&self) -> u64 {
        self.epoch_manager.get_current_epoch()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_epoch_manager() {
        let epoch_mgr = EpochManager::new(8);
        
        let initial_epoch = epoch_mgr.get_current_epoch();
        assert_eq!(initial_epoch, 0);

        // Advance epoch
        let new_epoch = epoch_mgr.advance_epoch();
        assert_eq!(new_epoch, 1);
        assert_eq!(epoch_mgr.get_current_epoch(), 1);
    }

    #[tokio::test]
    async fn test_read_guard() {
        let epoch_mgr = EpochManager::new(8);
        
        {
            let _guard = epoch_mgr.enter_read().await;
            assert_eq!(epoch_mgr.reader_epochs.len(), 1);
        }
        
        // Guard should be dropped and reader removed
        assert_eq!(epoch_mgr.reader_epochs.len(), 0);
    }

    #[tokio::test]
    async fn test_lifetime_manager() {
        let lifetime_mgr = DefaultLifetimeManager::new();
        
        let key = PinKey(123);
        let policy = LifetimePolicy {
            class: LifetimeClass::Session,
            ttl_ms: Some(1000), // 1 second TTL
            max_memory_mb: Some(64),
            gc_trigger: GcTrigger::TimeBasedTtl,
        };

        // Register slice
        lifetime_mgr.register_slice(key, policy).unwrap();
        assert!(lifetime_mgr.slice_policies.contains_key(&key));

        // Should not be expired yet
        let expired = lifetime_mgr.gc_expired().unwrap();
        assert_eq!(expired, 0);

        // Wait for TTL to expire
        tokio::time::sleep(Duration::from_millis(1100)).await;

        // Should be expired now
        let expired = lifetime_mgr.gc_expired().unwrap();
        assert_eq!(expired, 1);
        assert!(!lifetime_mgr.slice_policies.contains_key(&key));
    }

    #[test]
    fn test_lifetime_policy() {
        let policy = LifetimePolicy {
            class: LifetimeClass::Ephemeral,
            ttl_ms: Some(5000),
            max_memory_mb: Some(128),
            gc_trigger: GcTrigger::MemoryPressure(0.8),
        };

        assert_eq!(policy.class, LifetimeClass::Ephemeral);
        assert_eq!(policy.ttl_ms, Some(5000));
        assert!(matches!(policy.gc_trigger, GcTrigger::MemoryPressure(p) if p == 0.8));
    }
}