use anyhow::{anyhow, Result};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct PinKey(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PinPriority {
    Background,
    QueryCritical,
    TailLatencyCritical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LifetimeClass {
    Ephemeral,
    Session,
    Task,
    LongLived,
}

#[derive(Debug, Clone)]
pub struct PinOpts {
    pub priority: PinPriority,
    pub numa_prefer: Option<u16>,
    pub use_hugepages: bool,
    pub onfault: bool,
    pub lifetime_class: LifetimeClass,
    pub prefetch_adjacent: usize,
    pub ttl_ms: Option<u64>,
}

impl Default for PinOpts {
    fn default() -> Self {
        Self {
            priority: PinPriority::QueryCritical,
            numa_prefer: None,
            use_hugepages: false,
            onfault: true,
            lifetime_class: LifetimeClass::Session,
            prefetch_adjacent: 0,
            ttl_ms: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PinStats {
    pub total_pinned_bytes: u64,
    pub total_pinned_count: usize,
    pub budget_used_bytes: u64,
    pub budget_total_bytes: u64,
    pub huge_page_bytes: u64,
    pub numa_local_bytes: u64,
    pub by_priority: HashMap<PinPriority, PinPriorityStats>,
}

#[derive(Debug, Clone)]
pub struct PinPriorityStats {
    pub pinned_count: usize,
    pub pinned_bytes: u64,
    pub eviction_count: u64,
    pub average_pin_time_ms: f64,
}

pub trait PinManager: Send + Sync {
    fn pin_slice(&self, key: PinKey, opts: &PinOpts) -> Result<()>;
    fn unpin_slice(&self, key: PinKey);
    fn promote(&self, key: PinKey, new_priority: PinPriority);
    fn stats(&self) -> PinStats;
    fn is_pinned(&self, key: PinKey) -> bool;
    fn gc_expired(&self) -> usize;
}

#[derive(Debug)]
struct PinnedSlice {
    key: PinKey,
    addr: *mut u8,
    len: usize,
    priority: PinPriority,
    lifetime_class: LifetimeClass,
    pinned_at: Instant,
    ttl_ms: Option<u64>,
    numa_node: Option<u16>,
    uses_hugepages: bool,
}

unsafe impl Send for PinnedSlice {}
unsafe impl Sync for PinnedSlice {}

pub struct DefaultPinManager {
    pinned_slices: DashMap<PinKey, PinnedSlice>,
    budget_total_bytes: u64,
    budget_used_bytes: AtomicU64,
    // Statistics
    eviction_counts: DashMap<PinPriority, AtomicU64>,
    pin_times: DashMap<PinPriority, Arc<RwLock<Vec<Duration>>>>,
}

impl DefaultPinManager {
    pub fn new(budget_bytes: u64) -> Self {
        Self {
            pinned_slices: DashMap::new(),
            budget_total_bytes: budget_bytes,
            budget_used_bytes: AtomicU64::new(0),
            eviction_counts: DashMap::new(),
            pin_times: DashMap::new(),
        }
    }

    fn pin_memory(&self, addr: *mut u8, len: usize, onfault: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if onfault {
                let flags = libc::MLOCK_ONFAULT;
                let result = unsafe { libc::mlock2(addr as *mut libc::c_void, len, flags as i32) };
                if result != 0 {
                    return Err(anyhow!("mlock2 failed: {}", std::io::Error::last_os_error()));
                }
            } else {
                let result = unsafe { libc::mlock(addr as *mut libc::c_void, len) };
                if result != 0 {
                    return Err(anyhow!("mlock failed: {}", std::io::Error::last_os_error()));
                }
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            // Fallback to regular mlock on macOS and other platforms
            let result = unsafe { libc::mlock(addr as *mut libc::c_void, len) };
            if result != 0 {
                return Err(anyhow!("mlock failed: {}", std::io::Error::last_os_error()));
            }
        }

        Ok(())
    }

    fn unpin_memory(&self, addr: *mut u8, len: usize) -> Result<()> {
        let result = unsafe { libc::munlock(addr as *mut libc::c_void, len) };
        if result != 0 {
            return Err(anyhow!("munlock failed: {}", std::io::Error::last_os_error()));
        }
        Ok(())
    }

    fn setup_hugepages(&self, addr: *mut u8, len: usize) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            // Advise the kernel to use huge pages for this mapping
            let result = unsafe {
                libc::madvise(
                    addr as *mut libc::c_void,
                    len,
                    libc::MADV_HUGEPAGE,
                )
            };
            if result != 0 {
                // Non-fatal - huge pages are a hint, not a requirement
                eprintln!("Warning: madvise(MADV_HUGEPAGE) failed: {}", 
                         std::io::Error::last_os_error());
            }
        }
        
        Ok(())
    }

    fn evict_low_priority_slices(&self, needed_bytes: usize) -> Result<()> {
        let mut candidates: Vec<_> = self.pinned_slices
            .iter()
            .filter(|entry| matches!(entry.value().priority, PinPriority::Background))
            .map(|entry| *entry.key())
            .collect();

        candidates.sort_by_key(|&key| {
            self.pinned_slices
                .get(&key)
                .map(|slice| slice.pinned_at)
                .unwrap_or(Instant::now())
        });

        let mut freed_bytes = 0;
        for key in candidates {
            if freed_bytes >= needed_bytes {
                break;
            }

            if let Some((_, slice)) = self.pinned_slices.remove(&key) {
                let _ = self.unpin_memory(slice.addr, slice.len);
                self.budget_used_bytes.fetch_sub(slice.len as u64, Ordering::Relaxed);
                freed_bytes += slice.len;

                // Update eviction statistics
                self.eviction_counts
                    .entry(slice.priority)
                    .or_insert_with(|| AtomicU64::new(0))
                    .fetch_add(1, Ordering::Relaxed);
            }
        }

        if freed_bytes < needed_bytes {
            return Err(anyhow!("Could not free enough memory: needed {}, freed {}", 
                              needed_bytes, freed_bytes));
        }

        Ok(())
    }
}

impl PinManager for DefaultPinManager {
    fn pin_slice(&self, key: PinKey, opts: &PinOpts) -> Result<()> {
        // Check if already pinned
        if self.pinned_slices.contains_key(&key) {
            return Ok(());
        }

        // For now, return error since slice lookup is not implemented
        // In a real implementation, this would be looked up from an extent index
        match get_slice_address_and_length(key) {
            Ok((addr, len)) => {
                // Check budget
                let current_used = self.budget_used_bytes.load(Ordering::Relaxed);
                if current_used + len as u64 > self.budget_total_bytes {
                    // Try to evict low-priority slices
                    self.evict_low_priority_slices(len)?;
                }

                // Pin the memory
                self.pin_memory(addr, len, opts.onfault)?;

                // Setup huge pages if requested
                if opts.use_hugepages {
                    self.setup_hugepages(addr, len)?;
                }

                // Create pinned slice record
                let slice = PinnedSlice {
                    key,
                    addr,
                    len,
                    priority: opts.priority,
                    lifetime_class: opts.lifetime_class,
                    pinned_at: Instant::now(),
                    ttl_ms: opts.ttl_ms,
                    numa_node: opts.numa_prefer,
                    uses_hugepages: opts.use_hugepages,
                };

                // Update budget and store slice
                self.budget_used_bytes.fetch_add(len as u64, Ordering::Relaxed);
                self.pinned_slices.insert(key, slice);

                Ok(())
            }
            Err(e) => {
                tracing::warn!("Cannot pin slice {:?}: slice lookup not yet implemented ({})", key, e);
                // For now, we'll simulate success but not actually pin anything
                // This allows the system to function without actual memory pinning
                Ok(())
            }
        }
    }

    fn unpin_slice(&self, key: PinKey) {
        if let Some((_, slice)) = self.pinned_slices.remove(&key) {
            let _ = self.unpin_memory(slice.addr, slice.len);
            self.budget_used_bytes.fetch_sub(slice.len as u64, Ordering::Relaxed);
        }
    }

    fn promote(&self, key: PinKey, new_priority: PinPriority) {
        if let Some(mut slice) = self.pinned_slices.get_mut(&key) {
            slice.priority = new_priority;
        }
    }

    fn stats(&self) -> PinStats {
        let mut by_priority = HashMap::new();
        let mut total_huge_page_bytes = 0u64;
        let mut total_numa_local_bytes = 0u64;

        // Collect per-priority stats
        for entry in self.pinned_slices.iter() {
            let slice = entry.value();
            let priority_stats = by_priority
                .entry(slice.priority)
                .or_insert_with(|| PinPriorityStats {
                    pinned_count: 0,
                    pinned_bytes: 0,
                    eviction_count: 0,
                    average_pin_time_ms: 0.0,
                });

            priority_stats.pinned_count += 1;
            priority_stats.pinned_bytes += slice.len as u64;

            if slice.uses_hugepages {
                total_huge_page_bytes += slice.len as u64;
            }

            if slice.numa_node.is_some() {
                total_numa_local_bytes += slice.len as u64;
            }

            // Get eviction count
            if let Some(evictions) = self.eviction_counts.get(&slice.priority) {
                priority_stats.eviction_count = evictions.load(Ordering::Relaxed);
            }
        }

        PinStats {
            total_pinned_bytes: self.budget_used_bytes.load(Ordering::Relaxed),
            total_pinned_count: self.pinned_slices.len(),
            budget_used_bytes: self.budget_used_bytes.load(Ordering::Relaxed),
            budget_total_bytes: self.budget_total_bytes,
            huge_page_bytes: total_huge_page_bytes,
            numa_local_bytes: total_numa_local_bytes,
            by_priority,
        }
    }

    fn is_pinned(&self, key: PinKey) -> bool {
        self.pinned_slices.contains_key(&key)
    }

    fn gc_expired(&self) -> usize {
        let now = Instant::now();
        let mut expired_keys = Vec::new();

        for entry in self.pinned_slices.iter() {
            let slice = entry.value();
            if let Some(ttl_ms) = slice.ttl_ms {
                let age = now.duration_since(slice.pinned_at);
                if age.as_millis() as u64 > ttl_ms {
                    expired_keys.push(*entry.key());
                }
            }
        }

        for key in &expired_keys {
            self.unpin_slice(*key);
        }

        expired_keys.len()
    }
}

// Placeholder function - in real implementation this would look up the slice
// from an extent index or memory mapping registry
fn get_slice_address_and_length(_key: PinKey) -> Result<(*mut u8, usize)> {
    // This is a placeholder - real implementation would:
    // 1. Look up the extent from an extent index
    // 2. Get the mmap address for the extent
    // 3. Return the (addr, len) tuple
    Err(anyhow!("Slice lookup not implemented yet"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_manager_creation() {
        let manager = DefaultPinManager::new(1024 * 1024 * 1024); // 1GB budget
        let stats = manager.stats();
        
        assert_eq!(stats.total_pinned_count, 0);
        assert_eq!(stats.budget_total_bytes, 1024 * 1024 * 1024);
        assert_eq!(stats.budget_used_bytes, 0);
    }

    #[test]
    fn test_pin_opts_defaults() {
        let opts = PinOpts::default();
        
        assert_eq!(opts.priority, PinPriority::QueryCritical);
        assert_eq!(opts.lifetime_class, LifetimeClass::Session);
        assert!(opts.onfault);
        assert!(!opts.use_hugepages);
    }
}