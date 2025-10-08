//! Advanced memory management for orbit-rs
//!
//! This module provides advanced memory management capabilities including:
//! - Memory pinning for hot data with mlock/mlock2 support
//! - Extent-based object slicing and indexing
//! - NUMA-aware memory placement
//! - Huge page optimization
//! - Adaptive hotness tracking and garbage collection
//!
//! # Core Components
//! 
//! - [`PinManager`] - Manages memory pinning with priority-based budgets
//! - [`ExtentIndex`] - Tracks object extents and connection patterns
//! - [`LifetimeManager`] - Handles object lifecycle and garbage collection
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use orbit_server::memory::{PinManager, DefaultPinManager, PinOpts, PinPriority, LifetimeClass};
//!
//! # fn main() -> anyhow::Result<()> {
//! // Create a pin manager with 1GB budget
//! let pin_manager = DefaultPinManager::new(1024 * 1024 * 1024);
//!
//! // Pin a critical data structure
//! let opts = PinOpts {
//!     priority: PinPriority::TailLatencyCritical,
//!     use_hugepages: true,
//!     lifetime_class: LifetimeClass::Session,
//!     ..Default::default()
//! };
//!
//! // pin_manager.pin_slice(PinKey(123), &opts)?;
//! # Ok(())
//! # }
//! ```

pub mod pin_manager;
pub mod extent_index;
pub mod lifetime_manager;
pub mod actor_memory_manager;

// Re-export main interfaces
pub use pin_manager::{
    PinManager, DefaultPinManager, PinKey, PinOpts, PinPriority, 
    PinStats, PinPriorityStats, LifetimeClass
};

pub use extent_index::{
    ExtentIndex, DefaultExtentIndex, ExtentRef, ExtentFlags, 
    ExtentConnections, AccessType
};

pub use lifetime_manager::{
    LifetimeManager, DefaultLifetimeManager, EpochManager, 
    RetiredSlice, LifetimePolicy, GcTrigger
};

pub use actor_memory_manager::{
    ActorMemoryManager, DefaultActorMemoryManager, ActorMemoryStats,
    MemoryHint, MemoryHintProcessor, AccessPattern, QueryPattern, QueryFrequency
};

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_integrated_memory_management() {
        // Create components
        let pin_manager = Arc::new(DefaultPinManager::new(64 * 1024 * 1024)); // 64MB budget
        let extent_index = Arc::new(DefaultExtentIndex::default());
        let lifetime_manager = Arc::new(DefaultLifetimeManager::new());

        // Add some test extents
        let extent = ExtentRef {
            file_id: 1,
            offset: 0,
            len: 4096,
            flags: ExtentFlags::HOT | ExtentFlags::HUGEPAGE,
        };

        extent_index.add_table_extent(100, 1, extent);

        // Record access patterns
        extent_index.record_access(&extent, AccessType::RandomRead);
        extent_index.record_access(&extent, AccessType::RandomRead);

        // Get hot extents
        let hot_extents = extent_index.get_hot_extents(10);
        assert!(!hot_extents.is_empty());
        assert!(hot_extents[0].1 > 0.0); // Should have hotness score

        // Check pin manager stats
        let stats = pin_manager.stats();
        assert_eq!(stats.budget_total_bytes, 64 * 1024 * 1024);
        assert_eq!(stats.total_pinned_count, 0); // Nothing pinned yet
    }

    #[test]
    fn test_extent_to_pin_key_conversion() {
        let extent = ExtentRef {
            file_id: 42,
            offset: 8192,
            len: 2048,
            flags: ExtentFlags::HOT,
        };

        let pin_key = PinKey(extent.as_pin_key());
        
        // Pin key should be deterministic based on file_id and offset
        let expected = ((42u64) << 32) | (8192 >> 12);
        assert_eq!(pin_key.0, expected);
    }
}