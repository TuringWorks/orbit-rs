//! Actor Memory Behavior and Profiling
//!
//! This module defines traits and types for actors to declare their memory
//! characteristics, enabling the memory management system to optimize placement,
//! pinning, and prefetching based on actor requirements.
//!
//! # Memory Profiles
//!
//! Actors can be classified into different memory profiles based on their
//! latency requirements:
//!
//! - **Hot**: Sub-millisecond tail latency requirements (real-time analytics)
//! - **Warm**: Sub-10ms tail latency requirements (interactive queries)
//! - **Cold**: Higher latency acceptable (batch processing)
//!
//! # Usage
//!
//! ```rust,ignore
//! use orbit_shared::actor_memory::{ActorMemoryBehavior, ActorMemoryProfile, MemoryFootprint};
//!
//! impl ActorMemoryBehavior for MyAnalyticsActor {
//!     fn memory_profile(&self) -> ActorMemoryProfile {
//!         ActorMemoryProfile::Hot
//!     }
//!
//!     fn estimate_footprint(&self) -> MemoryFootprint {
//!         MemoryFootprint::new(128, 64) // 128MB persistent, 64MB working set
//!     }
//! }
//! ```

use crate::addressable::AddressableReference;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Memory profile classification for actors based on latency requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum ActorMemoryProfile {
    /// Hot profile: <1ms tail latency requirement
    /// Used for: real-time analytics, high-frequency trading, gaming
    /// Strategy: Pin all data in memory, use huge pages, NUMA-local placement
    Hot,

    /// Warm profile: <10ms tail latency requirement
    /// Used for: interactive queries, dashboards, API responses
    /// Strategy: Keep frequently accessed data pinned, allow some page faults
    #[default]
    Warm,

    /// Cold profile: >100ms acceptable
    /// Used for: batch processing, background jobs, archival queries
    /// Strategy: Allow data to be paged out, use background prefetching
    Cold,

    /// Custom profile with specific SLA
    Custom {
        /// Target latency in microseconds
        target_latency_us: u64,
        /// Maximum acceptable latency spike
        max_latency_us: u64,
    },
}

impl ActorMemoryProfile {
    /// Get the target latency for this profile
    pub fn target_latency(&self) -> Duration {
        match self {
            ActorMemoryProfile::Hot => Duration::from_micros(500),
            ActorMemoryProfile::Warm => Duration::from_millis(5),
            ActorMemoryProfile::Cold => Duration::from_millis(100),
            ActorMemoryProfile::Custom { target_latency_us, .. } => {
                Duration::from_micros(*target_latency_us)
            }
        }
    }

    /// Get the maximum acceptable latency for this profile
    pub fn max_latency(&self) -> Duration {
        match self {
            ActorMemoryProfile::Hot => Duration::from_millis(1),
            ActorMemoryProfile::Warm => Duration::from_millis(10),
            ActorMemoryProfile::Cold => Duration::from_millis(500),
            ActorMemoryProfile::Custom { max_latency_us, .. } => {
                Duration::from_micros(*max_latency_us)
            }
        }
    }

    /// Check if this profile requires memory pinning
    pub fn requires_pinning(&self) -> bool {
        matches!(self, ActorMemoryProfile::Hot | ActorMemoryProfile::Warm)
    }

    /// Check if this profile benefits from huge pages
    pub fn prefer_huge_pages(&self) -> bool {
        matches!(self, ActorMemoryProfile::Hot)
    }

    /// Get memory priority level (higher = more important)
    pub fn priority_level(&self) -> u8 {
        match self {
            ActorMemoryProfile::Hot => 100,
            ActorMemoryProfile::Warm => 50,
            ActorMemoryProfile::Cold => 10,
            ActorMemoryProfile::Custom { target_latency_us, .. } => {
                if *target_latency_us < 1000 {
                    100
                } else if *target_latency_us < 10000 {
                    50
                } else {
                    10
                }
            }
        }
    }
}


/// Estimated memory footprint for an actor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFootprint {
    /// Size of persistent state in MB (stored data)
    pub persistent_state_mb: u64,
    /// Size of working set in MB (temporary computation buffers)
    pub working_set_mb: u64,
    /// Optional list of data extent references for prefetching
    pub prefetch_candidates: Vec<PrefetchCandidate>,
    /// Whether the actor has GPU-accelerated workloads
    pub uses_gpu: bool,
    /// Estimated peak memory usage multiplier (e.g., 2.0 = can spike to 2x normal)
    pub peak_multiplier: f32,
}

impl MemoryFootprint {
    /// Create a new memory footprint estimate
    pub fn new(persistent_state_mb: u64, working_set_mb: u64) -> Self {
        Self {
            persistent_state_mb,
            working_set_mb,
            prefetch_candidates: Vec::new(),
            uses_gpu: false,
            peak_multiplier: 1.5,
        }
    }

    /// Create footprint with prefetch candidates
    pub fn with_prefetch(mut self, candidates: Vec<PrefetchCandidate>) -> Self {
        self.prefetch_candidates = candidates;
        self
    }

    /// Mark as GPU-accelerated
    pub fn with_gpu(mut self) -> Self {
        self.uses_gpu = true;
        self
    }

    /// Set peak memory multiplier
    pub fn with_peak_multiplier(mut self, multiplier: f32) -> Self {
        self.peak_multiplier = multiplier;
        self
    }

    /// Get total estimated memory in MB
    pub fn total_mb(&self) -> u64 {
        self.persistent_state_mb + self.working_set_mb
    }

    /// Get peak estimated memory in MB
    pub fn peak_mb(&self) -> u64 {
        ((self.persistent_state_mb + self.working_set_mb) as f32 * self.peak_multiplier) as u64
    }
}

impl Default for MemoryFootprint {
    fn default() -> Self {
        Self::new(10, 5) // Default: 10MB persistent, 5MB working set
    }
}

/// Candidate data for prefetching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefetchCandidate {
    /// Type of data to prefetch
    pub data_type: PrefetchDataType,
    /// Estimated size in MB
    pub size_mb: u64,
    /// Priority (higher = prefetch first)
    pub priority: u8,
    /// Optional identifier
    pub id: Option<String>,
}

/// Types of data that can be prefetched
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrefetchDataType {
    /// Graph partition data
    GraphPartition { partition_id: u64 },
    /// Time series data for a time range
    TimeSeries { metric: String, duration_hours: u32 },
    /// Vector embeddings
    VectorIndex { index_name: String },
    /// Document collection shard
    DocumentShard { collection: String, shard_id: u32 },
    /// Generic data extent
    Extent { file_id: u64, offset: u64, length: u64 },
}

/// Trait for actors to declare their memory behavior
///
/// Implement this trait to enable the memory management system to:
/// - Optimize memory placement based on latency requirements
/// - Pre-allocate appropriate memory pools
/// - Prefetch related data for connected actors
/// - Coordinate memory during actor migration
pub trait ActorMemoryBehavior: Send + Sync {
    /// Get the memory profile for this actor
    ///
    /// The profile determines latency guarantees and memory management strategy.
    fn memory_profile(&self) -> ActorMemoryProfile {
        ActorMemoryProfile::default()
    }

    /// Estimate the memory footprint for this actor
    ///
    /// Used for capacity planning and memory budget allocation.
    fn estimate_footprint(&self) -> MemoryFootprint {
        MemoryFootprint::default()
    }

    /// Get connected actors that should be co-located for locality
    ///
    /// Returns references to actors that are frequently accessed together.
    /// The memory system will try to place these actors on the same NUMA node
    /// or at least the same physical node.
    fn connected_actors(&self) -> Vec<AddressableReference> {
        Vec::new()
    }

    /// Get the preferred NUMA node for this actor (if known)
    ///
    /// Returns None to let the system decide placement.
    fn preferred_numa_node(&self) -> Option<u16> {
        None
    }

    /// Called before actor activation to prepare memory
    ///
    /// Override to perform custom pre-activation memory setup.
    fn on_memory_prepare(&self) -> MemoryPrepareResult {
        MemoryPrepareResult::default()
    }

    /// Called when memory pressure is detected
    ///
    /// Override to shed non-essential memory or adjust working set size.
    fn on_memory_pressure(&self, pressure_level: MemoryPressureLevel) -> MemoryPressureResponse {
        MemoryPressureResponse::default_for_level(pressure_level)
    }
}

/// Result of memory preparation before activation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryPrepareResult {
    /// Additional memory needed beyond estimate (MB)
    pub additional_memory_mb: u64,
    /// Data extents to prefetch
    pub prefetch_requests: Vec<PrefetchCandidate>,
    /// Whether to delay activation until memory is ready
    pub wait_for_memory: bool,
}

/// Memory pressure levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryPressureLevel {
    /// Normal operation, no pressure
    None,
    /// Low pressure - start reducing non-essential memory
    Low,
    /// Medium pressure - reduce working set sizes
    Medium,
    /// High pressure - aggressive memory reduction needed
    High,
    /// Critical - release all non-essential memory immediately
    Critical,
}

/// Actor's response to memory pressure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPressureResponse {
    /// Memory that can be released (MB)
    pub releasable_memory_mb: u64,
    /// Whether the actor can be deactivated
    pub can_deactivate: bool,
    /// Minimum memory required to continue operation (MB)
    pub minimum_required_mb: u64,
    /// Actions taken in response to pressure
    pub actions_taken: Vec<MemoryPressureAction>,
}

impl MemoryPressureResponse {
    /// Create default response based on pressure level
    pub fn default_for_level(level: MemoryPressureLevel) -> Self {
        match level {
            MemoryPressureLevel::None | MemoryPressureLevel::Low => Self {
                releasable_memory_mb: 0,
                can_deactivate: false,
                minimum_required_mb: 0,
                actions_taken: Vec::new(),
            },
            MemoryPressureLevel::Medium => Self {
                releasable_memory_mb: 0,
                can_deactivate: false,
                minimum_required_mb: 0,
                actions_taken: vec![MemoryPressureAction::ReducedWorkingSet],
            },
            MemoryPressureLevel::High | MemoryPressureLevel::Critical => Self {
                releasable_memory_mb: 0,
                can_deactivate: true,
                minimum_required_mb: 0,
                actions_taken: vec![
                    MemoryPressureAction::ReducedWorkingSet,
                    MemoryPressureAction::FlushedCaches,
                ],
            },
        }
    }
}

impl Default for MemoryPressureResponse {
    fn default() -> Self {
        Self::default_for_level(MemoryPressureLevel::None)
    }
}

/// Actions actors can take in response to memory pressure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryPressureAction {
    /// Reduced working set size
    ReducedWorkingSet,
    /// Flushed internal caches
    FlushedCaches,
    /// Spilled data to disk
    SpilledToDisk,
    /// Released prefetched data
    ReleasedPrefetch,
    /// Compressed in-memory data
    CompressedData,
    /// Custom action with description
    Custom(String),
}

/// Configuration for actor memory management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorMemoryConfig {
    /// Enable automatic memory profiling
    pub auto_profiling: bool,
    /// Default memory profile for actors without explicit profile
    pub default_profile: ActorMemoryProfile,
    /// Maximum memory per actor (MB)
    pub max_memory_per_actor_mb: u64,
    /// Enable NUMA-aware placement
    pub numa_aware: bool,
    /// Enable transparent huge pages
    pub use_huge_pages: bool,
    /// Memory pressure thresholds
    pub pressure_thresholds: MemoryPressureThresholds,
}

impl Default for ActorMemoryConfig {
    fn default() -> Self {
        Self {
            auto_profiling: true,
            default_profile: ActorMemoryProfile::Warm,
            max_memory_per_actor_mb: 1024, // 1GB default max
            numa_aware: true,
            use_huge_pages: true,
            pressure_thresholds: MemoryPressureThresholds::default(),
        }
    }
}

/// Memory pressure detection thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPressureThresholds {
    /// Low pressure threshold (% of memory budget used)
    pub low: f32,
    /// Medium pressure threshold
    pub medium: f32,
    /// High pressure threshold
    pub high: f32,
    /// Critical pressure threshold
    pub critical: f32,
}

impl Default for MemoryPressureThresholds {
    fn default() -> Self {
        Self {
            low: 0.70,      // 70% usage
            medium: 0.80,   // 80% usage
            high: 0.90,     // 90% usage
            critical: 0.95, // 95% usage
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHotActor {
        data_size_mb: u64,
    }

    impl ActorMemoryBehavior for TestHotActor {
        fn memory_profile(&self) -> ActorMemoryProfile {
            ActorMemoryProfile::Hot
        }

        fn estimate_footprint(&self) -> MemoryFootprint {
            MemoryFootprint::new(self.data_size_mb, 32)
                .with_peak_multiplier(2.0)
        }

        fn connected_actors(&self) -> Vec<AddressableReference> {
            vec![AddressableReference {
                addressable_type: "GraphPartitionActor".to_string(),
                key: crate::addressable::Key::Int64Key { key: 1 },
            }]
        }
    }

    struct TestColdActor;

    impl ActorMemoryBehavior for TestColdActor {
        fn memory_profile(&self) -> ActorMemoryProfile {
            ActorMemoryProfile::Cold
        }
    }

    #[test]
    fn test_memory_profile_latency() {
        assert!(ActorMemoryProfile::Hot.target_latency() < ActorMemoryProfile::Warm.target_latency());
        assert!(ActorMemoryProfile::Warm.target_latency() < ActorMemoryProfile::Cold.target_latency());
    }

    #[test]
    fn test_memory_profile_priority() {
        assert!(ActorMemoryProfile::Hot.priority_level() > ActorMemoryProfile::Warm.priority_level());
        assert!(ActorMemoryProfile::Warm.priority_level() > ActorMemoryProfile::Cold.priority_level());
    }

    #[test]
    fn test_hot_actor_behavior() {
        let actor = TestHotActor { data_size_mb: 128 };

        assert_eq!(actor.memory_profile(), ActorMemoryProfile::Hot);
        assert!(actor.memory_profile().requires_pinning());
        assert!(actor.memory_profile().prefer_huge_pages());

        let footprint = actor.estimate_footprint();
        assert_eq!(footprint.persistent_state_mb, 128);
        assert_eq!(footprint.working_set_mb, 32);
        assert_eq!(footprint.total_mb(), 160);
        assert_eq!(footprint.peak_mb(), 320); // 2.0x multiplier

        let connected = actor.connected_actors();
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].addressable_type, "GraphPartitionActor");
    }

    #[test]
    fn test_cold_actor_behavior() {
        let actor = TestColdActor;

        assert_eq!(actor.memory_profile(), ActorMemoryProfile::Cold);
        assert!(!actor.memory_profile().requires_pinning());
        assert!(!actor.memory_profile().prefer_huge_pages());

        // Uses defaults
        let footprint = actor.estimate_footprint();
        assert_eq!(footprint.persistent_state_mb, 10);
        assert_eq!(footprint.working_set_mb, 5);
    }

    #[test]
    fn test_memory_pressure_response() {
        let response = MemoryPressureResponse::default_for_level(MemoryPressureLevel::High);
        assert!(response.can_deactivate);
        assert!(!response.actions_taken.is_empty());

        let response = MemoryPressureResponse::default_for_level(MemoryPressureLevel::None);
        assert!(!response.can_deactivate);
        assert!(response.actions_taken.is_empty());
    }

    #[test]
    fn test_prefetch_candidate() {
        let footprint = MemoryFootprint::new(100, 50)
            .with_prefetch(vec![
                PrefetchCandidate {
                    data_type: PrefetchDataType::GraphPartition { partition_id: 42 },
                    size_mb: 64,
                    priority: 100,
                    id: Some("main_partition".to_string()),
                },
            ])
            .with_gpu();

        assert!(footprint.uses_gpu);
        assert_eq!(footprint.prefetch_candidates.len(), 1);
    }

    #[test]
    fn test_custom_memory_profile() {
        let profile = ActorMemoryProfile::Custom {
            target_latency_us: 500,
            max_latency_us: 2000,
        };

        assert_eq!(profile.target_latency(), Duration::from_micros(500));
        assert_eq!(profile.max_latency(), Duration::from_micros(2000));
        assert_eq!(profile.priority_level(), 100); // <1000us = high priority
    }
}
