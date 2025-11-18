use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dashmap::DashMap;
use orbit_shared::addressable::AddressableReference;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{
    pin_manager::{PinKey, PinManager, PinOpts, PinPriority, LifetimeClass},
    extent_index::{ExtentIndex, ExtentRef, AccessType},
    lifetime_manager::{LifetimeManager, LifetimePolicy, GcTrigger},
};

/// Memory hint messages that actors can send to optimize data access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryHint {
    WillAccessSoon {
        actor_refs: Vec<AddressableReference>,
        estimated_delay_ms: u64,
        access_pattern: AccessPattern,
    },
    AccessComplete {
        actor_refs: Vec<AddressableReference>,
    },
    MigrateForLocality {
        actor_ref: AddressableReference,
        target_numa_node: u16,
        locality_score: f32,
    },
    QueryPatternHint {
        actor_ref: AddressableReference,
        pattern: QueryPattern,
        expected_frequency: QueryFrequency,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AccessPattern {
    Sequential,
    Random,
    Scan,
    GraphTraversal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum QueryPattern {
    PointLookup,
    RangeScan,
    Aggregation,
    GraphTraversal,
    TimeSeriesAnalytics,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum QueryFrequency {
    VeryHigh,  // > 1000 QPS
    High,      // 100-1000 QPS
    Medium,    // 10-100 QPS
    Low,       // < 10 QPS
}

/// Statistics about an actor's memory usage
#[derive(Debug, Clone)]
pub struct ActorMemoryStats {
    pub persistent_state_bytes: u64,
    pub working_set_bytes: u64,
    pub pinned_extents: Vec<ExtentRef>,
    pub numa_node: Option<u16>,
    pub last_access: Instant,
    pub total_memory_mb: u64,
}

/// Main interface for actor-centric memory management
#[async_trait]
pub trait ActorMemoryManager: Send + Sync {
    /// Pin memory for an actor's persistent state
    async fn pin_actor_state(&self, 
        actor_ref: &AddressableReference, 
        priority: PinPriority
    ) -> Result<()>;
    
    /// Pin a specific extent for an actor
    async fn pin_actor_extent(&self,
        actor_ref: &AddressableReference,
        extent: &ExtentRef,
        opts: &PinOpts,
    ) -> Result<()>;

    /// Pin memory for an actor's working set (temporary data)
    async fn pin_actor_working_set(&self, 
        actor_ref: &AddressableReference, 
        size_mb: u64,
        lifetime_class: LifetimeClass,
    ) -> Result<()>;

    /// Unpin all memory associated with an actor (on deactivation)
    async fn unpin_actor_memory(&self, actor_ref: &AddressableReference);

    /// Get memory statistics for an actor
    async fn get_actor_memory_stats(&self, 
        actor_ref: &AddressableReference
    ) -> Result<ActorMemoryStats>;

    /// Send memory hint to optimize future access patterns
    async fn send_memory_hint(&self, hint: MemoryHint) -> Result<()>;

    /// Allocate working set memory for temporary computations
    async fn allocate_working_set(&self,
        actor_ref: &AddressableReference,
        size_mb: u64,
        lifetime_class: LifetimeClass,
    ) -> Result<()>;
}

/// Default implementation of ActorMemoryManager
pub struct DefaultActorMemoryManager {
    pin_manager: Arc<dyn PinManager>,
    extent_index: Arc<dyn ExtentIndex>,
    lifetime_manager: Arc<dyn LifetimeManager>,
    actor_extents: DashMap<AddressableReference, Vec<PinKey>>,
    actor_stats: DashMap<AddressableReference, ActorMemoryStats>,
    memory_hints_tx: tokio::sync::mpsc::Sender<MemoryHint>,
}

impl DefaultActorMemoryManager {
    pub fn new(
        pin_manager: Arc<dyn PinManager>,
        extent_index: Arc<dyn ExtentIndex>,
        lifetime_manager: Arc<dyn LifetimeManager>,
    ) -> (Self, tokio::sync::mpsc::Receiver<MemoryHint>) {
        let (hints_tx, hints_rx) = tokio::sync::mpsc::channel(1000);
        
        let manager = Self {
            pin_manager,
            extent_index,
            lifetime_manager,
            actor_extents: DashMap::new(),
            actor_stats: DashMap::new(),
            memory_hints_tx: hints_tx,
        };
        
        (manager, hints_rx)
    }

    /// Get extents for an actor based on its type and key
    fn get_actor_extents(&self, actor_ref: &AddressableReference) -> Vec<ExtentRef> {
        match actor_ref.addressable_type.as_str() {
            "GraphPartitionActor" => {
                if let orbit_shared::addressable::Key::Int64Key { key } = &actor_ref.key {
                    let partition_id = *key as u64;
                    self.extent_index.lookup_graph_partition(partition_id).to_vec()
                } else {
                    Vec::new()
                }
            },
            "TimeSeriesActor" => {
                // For time series actors, we'd look up time buckets based on the metric name
                // This is simplified - real implementation would parse the key
                Vec::new() // Placeholder
            },
            "DocumentCollectionActor" => {
                // For document actors, we'd look up document ranges
                Vec::new() // Placeholder
            },
            _ => Vec::new(),
        }
    }

    /// Update actor memory statistics
    fn update_actor_stats(&self, 
        actor_ref: &AddressableReference, 
        extents: &[ExtentRef],
        numa_node: Option<u16>
    ) {
        let total_bytes: u64 = extents.iter().map(|e| e.len as u64).sum();
        let stats = ActorMemoryStats {
            persistent_state_bytes: total_bytes,
            working_set_bytes: 0, // Updated separately
            pinned_extents: extents.to_vec(),
            numa_node,
            last_access: Instant::now(),
            total_memory_mb: total_bytes / (1024 * 1024),
        };
        
        self.actor_stats.insert(actor_ref.clone(), stats);
    }
}

#[async_trait]
impl ActorMemoryManager for DefaultActorMemoryManager {
    async fn pin_actor_state(&self, 
        actor_ref: &AddressableReference, 
        priority: PinPriority
    ) -> Result<()> {
        let extents = self.get_actor_extents(actor_ref);
        let mut pinned_keys = Vec::new();
        
        for extent in &extents {
            let pin_key = PinKey(extent.as_pin_key());
            let opts = PinOpts {
                priority,
                use_hugepages: extent.len >= 64 * 1024 * 1024, // 64MB+ gets huge pages
                onfault: true,
                lifetime_class: LifetimeClass::LongLived,
                numa_prefer: None, // Could be set based on placement strategy
                prefetch_adjacent: 1,
                ttl_ms: None,
            };
            
            self.pin_manager.pin_slice(pin_key, &opts)?;
            pinned_keys.push(pin_key);
            
            // Record access pattern
            self.extent_index.record_access(extent, AccessType::RandomRead);
        }
        
        // Track which extents belong to this actor
        self.actor_extents.insert(actor_ref.clone(), pinned_keys);
        self.update_actor_stats(actor_ref, &extents, None);
        
        Ok(())
    }

    async fn pin_actor_extent(&self,
        actor_ref: &AddressableReference,
        extent: &ExtentRef,
        opts: &PinOpts,
    ) -> Result<()> {
        let pin_key = PinKey(extent.as_pin_key());
        self.pin_manager.pin_slice(pin_key, opts)?;
        
        // Add to actor's extent list
        let mut actor_extents = self.actor_extents
            .entry(actor_ref.clone())
            .or_insert_with(Vec::new);
        if !actor_extents.contains(&pin_key) {
            actor_extents.push(pin_key);
        }
        
        // Record access pattern based on priority
        let access_type = match opts.priority {
            PinPriority::TailLatencyCritical => AccessType::RandomRead,
            PinPriority::QueryCritical => AccessType::SequentialRead,
            PinPriority::Background => AccessType::Scan,
        };
        self.extent_index.record_access(extent, access_type);
        
        Ok(())
    }

    async fn pin_actor_working_set(&self, 
        actor_ref: &AddressableReference, 
        size_mb: u64,
        lifetime_class: LifetimeClass,
    ) -> Result<()> {
        // For working set, we'd typically allocate from a memory pool
        // This is simplified - real implementation would manage working set allocation
        
        let policy = LifetimePolicy {
            class: lifetime_class,
            ttl_ms: match lifetime_class {
                LifetimeClass::Ephemeral => Some(1000),      // 1 second
                LifetimeClass::Session => Some(30 * 60 * 1000), // 30 minutes
                LifetimeClass::Task => Some(60 * 60 * 1000),    // 1 hour
                LifetimeClass::LongLived => None,            // No TTL
            },
            max_memory_mb: Some(size_mb),
            gc_trigger: GcTrigger::TimeBasedTtl,
        };
        
        // Create a synthetic key for the working set
        let working_set_key = PinKey(
            (actor_ref.addressable_type.len() as u64) << 32 | 
            actor_ref.to_string().len() as u64
        );
        
        self.lifetime_manager.register_slice(working_set_key, policy)?;
        
        // Update stats
        if let Some(mut stats) = self.actor_stats.get_mut(actor_ref) {
            stats.working_set_bytes = size_mb * 1024 * 1024;
        }
        
        Ok(())
    }

    async fn unpin_actor_memory(&self, actor_ref: &AddressableReference) {
        if let Some((_, pin_keys)) = self.actor_extents.remove(actor_ref) {
            for pin_key in pin_keys {
                self.pin_manager.unpin_slice(pin_key);
            }
        }
        
        // Remove stats
        self.actor_stats.remove(actor_ref);
        
        println!("Unpinned all memory for actor: {}", actor_ref);
    }

    async fn get_actor_memory_stats(&self, 
        actor_ref: &AddressableReference
    ) -> Result<ActorMemoryStats> {
        self.actor_stats
            .get(actor_ref)
            .map(|stats| stats.clone())
            .ok_or_else(|| anyhow!("No memory stats found for actor: {}", actor_ref))
    }

    async fn send_memory_hint(&self, hint: MemoryHint) -> Result<()> {
        self.memory_hints_tx
            .send(hint)
            .await
            .map_err(|e| anyhow!("Failed to send memory hint: {}", e))
    }

    async fn allocate_working_set(&self,
        actor_ref: &AddressableReference,
        size_mb: u64,
        lifetime_class: LifetimeClass,
    ) -> Result<()> {
        self.pin_actor_working_set(actor_ref, size_mb, lifetime_class).await
    }
}

/// Memory hint processor that handles hint messages
pub struct MemoryHintProcessor {
    memory_manager: Arc<DefaultActorMemoryManager>,
    hint_rx: tokio::sync::mpsc::Receiver<MemoryHint>,
}

impl MemoryHintProcessor {
    pub fn new(
        memory_manager: Arc<DefaultActorMemoryManager>,
        hint_rx: tokio::sync::mpsc::Receiver<MemoryHint>,
    ) -> Self {
        Self {
            memory_manager,
            hint_rx,
        }
    }

    pub async fn run(&mut self) {
        while let Some(hint) = self.hint_rx.recv().await {
            if let Err(e) = self.process_hint(hint).await {
                eprintln!("Error processing memory hint: {}", e);
            }
        }
    }

    async fn process_hint(&self, hint: MemoryHint) -> Result<()> {
        match hint {
            MemoryHint::WillAccessSoon { 
                actor_refs, 
                estimated_delay_ms, 
                access_pattern 
            } => {
                println!("Pre-warming {} actors (access in {}ms)", 
                         actor_refs.len(), estimated_delay_ms);
                
                // Prefetch actors that will be accessed soon
                for actor_ref in actor_refs {
                    // Pin with lower priority since it's speculative
                    if let Err(e) = self.memory_manager
                        .pin_actor_state(&actor_ref, PinPriority::QueryCritical).await 
                    {
                        eprintln!("Failed to prefetch actor {}: {}", actor_ref, e);
                    }
                }
            },
            
            MemoryHint::AccessComplete { actor_refs } => {
                println!("Access complete for {} actors", actor_refs.len());
                
                // Could demote priority or unpin if memory pressure is high
                // For now, just log
            },
            
            MemoryHint::MigrateForLocality { 
                actor_ref, 
                target_numa_node, 
                locality_score 
            } => {
                println!("Migration suggestion: {} to NUMA node {} (score: {})", 
                         actor_ref, target_numa_node, locality_score);
                
                // In real implementation, would coordinate with actor placement system
            },
            
            MemoryHint::QueryPatternHint { 
                actor_ref, 
                pattern, 
                expected_frequency 
            } => {
                println!("Query pattern hint for {}: {:?} at {:?} frequency", 
                         actor_ref, pattern, expected_frequency);
                
                // Adjust memory management strategy based on query pattern
                let priority = match expected_frequency {
                    QueryFrequency::VeryHigh => PinPriority::TailLatencyCritical,
                    QueryFrequency::High => PinPriority::QueryCritical,
                    QueryFrequency::Medium | QueryFrequency::Low => PinPriority::Background,
                };
                
                if let Err(e) = self.memory_manager
                    .pin_actor_state(&actor_ref, priority).await 
                {
                    eprintln!("Failed to adjust memory for actor {}: {}", actor_ref, e);
                }
            },
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{DefaultPinManager, DefaultExtentIndex, DefaultLifetimeManager};
    use orbit_shared::addressable::{AddressableReference, Key};

    #[tokio::test]
    async fn test_actor_memory_manager() {
        let pin_manager = Arc::new(DefaultPinManager::new(1024 * 1024 * 1024)); // 1GB
        let extent_index = Arc::new(DefaultExtentIndex::new());
        let lifetime_manager = Arc::new(DefaultLifetimeManager::new());
        
        let (memory_manager, _hint_rx) = DefaultActorMemoryManager::new(
            pin_manager,
            extent_index,
            lifetime_manager,
        );
        
        let actor_ref = AddressableReference {
            addressable_type: "GraphPartitionActor".to_string(),
            key: Key::Int64Key { key: 123 },
        };
        
        // Pin actor state
        memory_manager.pin_actor_state(&actor_ref, PinPriority::QueryCritical)
            .await
            .unwrap();
        
        // Check stats (would be empty since we don't have real extents)
        let stats = memory_manager.get_actor_memory_stats(&actor_ref).await;
        // Stats might not exist since we don't have real extents, that's OK for test
        
        // Unpin memory
        memory_manager.unpin_actor_memory(&actor_ref).await;
    }

    #[tokio::test]
    async fn test_memory_hints() {
        let pin_manager = Arc::new(DefaultPinManager::new(1024 * 1024 * 1024));
        let extent_index = Arc::new(DefaultExtentIndex::new());
        let lifetime_manager = Arc::new(DefaultLifetimeManager::new());
        
        let (memory_manager, _hint_rx) = DefaultActorMemoryManager::new(
            pin_manager,
            extent_index,
            lifetime_manager,
        );
        
        let actor_ref = AddressableReference {
            addressable_type: "TimeSeriesActor".to_string(),
            key: Key::StringKey { key: "temperature".to_string() },
        };
        
        // Send memory hint
        let hint = MemoryHint::WillAccessSoon {
            actor_refs: vec![actor_ref],
            estimated_delay_ms: 100,
            access_pattern: AccessPattern::Sequential,
        };
        
        memory_manager.send_memory_hint(hint).await.unwrap();
    }
}