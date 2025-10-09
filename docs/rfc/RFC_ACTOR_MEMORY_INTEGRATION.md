# RFC: Virtual Actor Memory Management Integration

**Document**: RFC_ACTOR_MEMORY_INTEGRATION  
**Status**: Draft  
**Author**: Orbit-RS Team  
**Created**: 2025-01-08  
**Updated**: 2025-01-08  

## Abstract

This RFC defines how Orbit-RS's advanced memory management system integrates with the virtual actor model to provide zero-copy, ultra-low latency data access. The integration makes actors the fundamental unit of memory locality, automatic lifetime management, and distributed coordination, enabling sub-10μs tail latencies at petabyte scale.

## 1. Overview

### 1.1 Problem Statement

Traditional database systems struggle to provide consistent low latencies because:
- Memory management is disconnected from application logic boundaries
- No automatic coordination between data placement and computation placement
- Manual memory tuning required for optimal performance
- Difficult to maintain locality in distributed systems

### 1.2 Solution Approach

Orbit-RS solves this by making **virtual actors the fundamental unit of memory management**:

```rust
// Actors automatically manage their memory footprint

#[derive(Addressable)]
pub struct GraphTraversalActor {
    partition_id: u64,
    // Memory management is automatic and transparent
}

impl GraphTraversalActor {
    // Actor methods get sub-10μs latency automatically
    pub async fn shortest_path(&self, from: NodeId, to: NodeId) -> Result<Path> {
        // Data is automatically pinned based on actor's memory profile
        self.traverse_pinned_partition(from, to).await
    }
}
```

### 1.3 Key Benefits

- **Zero-Configuration Performance**: Actors automatically get optimal memory placement
- **Natural Locality**: Actor boundaries define memory locality boundaries  
- **Distributed Coordination**: Actor migration triggers coordinated memory management
- **SLA Guarantees**: Different actor types get different latency guarantees

## 2. Actor Memory Profiles

### 2.1 Memory Profile Classification

```rust

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActorMemoryProfile {
    Hot,    // <1ms tail latency requirement
    Warm,   // <10ms tail latency requirement  
    Cold,   // >100ms acceptable
}

pub trait ActorMemoryBehavior: Addressable {
    fn get_memory_profile(&self) -> ActorMemoryProfile;
    fn get_connected_actors(&self) -> Option<Vec<AddressableReference>>;
    fn estimate_memory_footprint(&self) -> MemoryFootprint;
}

#[derive(Debug, Clone)]
pub struct MemoryFootprint {
    pub persistent_state_mb: u64,
    pub working_set_mb: u64,
    pub prefetch_candidates: Vec<ExtentRef>,
}
```

### 2.2 Profile-Based Memory Strategies

**Hot Profile Actors:**
```rust
impl ActorMemoryBehavior for RealTimeAnalyticsActor {
    fn get_memory_profile(&self) -> ActorMemoryProfile {
        ActorMemoryProfile::Hot
    }
    
    fn estimate_memory_footprint(&self) -> MemoryFootprint {
        MemoryFootprint {
            persistent_state_mb: 128,    // Recent time buckets
            working_set_mb: 64,          // Calculation buffers
            prefetch_candidates: self.get_adjacent_time_buckets(),
        }
    }
}

// Hot actors get guaranteed memory pinning
let pin_opts = PinOpts {
    priority: PinPriority::TailLatencyCritical,
    use_hugepages: true,
    numa_prefer: Some(self.get_numa_node()),
    lifetime_class: LifetimeClass::LongLived,
    ttl_ms: None, // Never expire
    prefetch_adjacent: 3,
};
```

**Warm Profile Actors:**
```rust
impl ActorMemoryBehavior for SessionCacheActor {
    fn get_memory_profile(&self) -> ActorMemoryProfile {
        ActorMemoryProfile::Warm
    }
    
    fn estimate_memory_footprint(&self) -> MemoryFootprint {
        MemoryFootprint {
            persistent_state_mb: 32,     // User session data
            working_set_mb: 16,          // Query buffers
            prefetch_candidates: self.get_related_sessions(),
        }
    }
}

// Warm actors get session-scoped pinning
let pin_opts = PinOpts {
    priority: PinPriority::QueryCritical,
    use_hugepages: false,
    lifetime_class: LifetimeClass::Session,
    ttl_ms: Some(30 * 60 * 1000), // 30 minute TTL
    prefetch_adjacent: 1,
};
```

## 3. Actor Lifecycle Integration

### 3.1 Enhanced Actor Lifecycle Trait

```rust

#[async_trait]
pub trait ActorLifecycleWithMemory: Addressable {
    /// Called when actor is first activated
    async fn on_activate(&mut self, context: &ActorActivationContext) -> Result<()>;
    
    /// Called when actor is deactivated due to inactivity
    async fn on_deactivate(&mut self, context: &ActorDeactivationContext) -> Result<()>;
    
    /// Called when actor is about to be migrated to another node
    async fn on_prepare_migration(&mut self, target_node: NodeId) -> Result<MigrationPlan>;
    
    /// Called after successful migration to new node
    async fn on_migration_complete(&mut self, source_node: NodeId) -> Result<()>;
}

pub struct ActorActivationContext {
    pub actor_ref: AddressableReference,
    pub memory_manager: Arc<dyn ActorMemoryManager>,
    pub extent_index: Arc<dyn ExtentIndex>,
    pub local_numa_node: u16,
    pub activation_reason: ActivationReason,
}

#[derive(Debug, Clone, Copy)]
pub enum ActivationReason {
    FirstInvocation,
    Migration,
    Reactivation,
    Prefetch,
}
```

### 3.2 Automatic Memory Management on Activation

```rust
// Example: Graph Database Actor

#[derive(Addressable)]
pub struct GraphPartitionActor {
    partition_id: u64,
    node_count: usize,
    edge_count: usize,
}

#[async_trait]
impl ActorLifecycleWithMemory for GraphPartitionActor {
    async fn on_activate(&mut self, context: &ActorActivationContext) -> Result<()> {
        println!("Activating graph partition {} with {} nodes", 
                 self.partition_id, self.node_count);
        
        // 1. Get memory footprint for this partition
        let footprint = self.estimate_memory_footprint();
        
        // 2. Pin persistent state (graph structure)
        let partition_extents = context.extent_index
            .lookup_graph_partition(self.partition_id);
            
        for extent in partition_extents {
            context.memory_manager.pin_actor_extent(
                &context.actor_ref,
                extent,
                &PinOpts {
                    priority: PinPriority::TailLatencyCritical,
                    use_hugepages: true,
                    numa_prefer: Some(context.local_numa_node),
                    lifetime_class: LifetimeClass::LongLived,
                    prefetch_adjacent: 2, // Prefetch 2 neighbor partitions
                    ..Default::default()
                },
            ).await?;
        }
        
        // 3. Prefetch connected partitions based on graph topology
        let connected_partitions = self.get_connected_partitions();
        for connected_id in connected_partitions {
            let connected_ref = AddressableReference {
                addressable_type: "GraphPartitionActor".to_string(),
                key: Key::Int64Key { key: connected_id as i64 },
            };
            
            // Send prefetch hint
            context.memory_manager.send_memory_hint(MemoryHint::WillAccessSoon {
                actor_refs: vec![connected_ref],
                estimated_delay_ms: 100, // Expect access within 100ms
            }).await?;
        }
        
        // 4. Set up working memory for traversal algorithms
        context.memory_manager.allocate_working_set(
            &context.actor_ref,
            footprint.working_set_mb,
            LifetimeClass::Session,
        ).await?;
        
        Ok(())
    }
    
    async fn on_deactivate(&mut self, context: &ActorDeactivationContext) -> Result<()> {
        println!("Deactivating graph partition {}", self.partition_id);
        
        // Automatically unpin all memory associated with this actor
        context.memory_manager.unpin_actor_memory(&context.actor_ref).await;
        
        Ok(())
    }
    
    async fn on_prepare_migration(&mut self, target_node: NodeId) -> Result<MigrationPlan> {
        // Create migration plan with memory pre-warming
        let current_extents = self.get_pinned_extents();
        
        Ok(MigrationPlan {
            target_node,
            prewarm_extents: current_extents,
            estimated_migration_time_ms: 5000,
            requires_memory_sync: true,
        })
    }
}

impl ActorMemoryBehavior for GraphPartitionActor {
    fn get_memory_profile(&self) -> ActorMemoryProfile {
        match self.node_count {
            0..=1000 => ActorMemoryProfile::Cold,
            1001..=100000 => ActorMemoryProfile::Warm, 
            _ => ActorMemoryProfile::Hot,
        }
    }
    
    fn estimate_memory_footprint(&self) -> MemoryFootprint {
        let nodes_mb = (self.node_count * 64) / (1024 * 1024); // 64 bytes per node
        let edges_mb = (self.edge_count * 32) / (1024 * 1024);  // 32 bytes per edge
        
        MemoryFootprint {
            persistent_state_mb: nodes_mb + edges_mb,
            working_set_mb: (nodes_mb / 4).max(16), // 25% of data size, min 16MB
            prefetch_candidates: self.get_neighbor_partition_extents(),
        }
    }
}
```

## 4. Actor Type-Specific Strategies

### 4.1 Time Series Actors

```rust

#[derive(Addressable)]
pub struct TimeSeriesActor {
    metric_name: String,
    time_range: TimeRange,
    bucket_size: Duration,
}

#[async_trait]
impl ActorLifecycleWithMemory for TimeSeriesActor {
    async fn on_activate(&mut self, context: &ActorActivationContext) -> Result<()> {
        // Pin recent time buckets for hot queries
        let recent_window = Duration::from_secs(3600); // Last hour
        let now = SystemTime::now();
        
        let recent_buckets = self.get_time_buckets_in_window(now - recent_window, now);
        
        for bucket in recent_buckets {
            let bucket_extents = context.extent_index
                .lookup_time_bucket(bucket.bucket_id);
                
            for extent in bucket_extents {
                let priority = if bucket.age < Duration::from_secs(300) {
                    PinPriority::TailLatencyCritical // Last 5 minutes = hot
                } else {
                    PinPriority::QueryCritical       // Last hour = warm
                };
                
                context.memory_manager.pin_actor_extent(
                    &context.actor_ref,
                    extent,
                    &PinOpts {
                        priority,
                        use_hugepages: extent.len >= 64 * 1024 * 1024,
                        lifetime_class: LifetimeClass::Session,
                        ttl_ms: Some(3600 * 1000), // 1 hour TTL
                        ..Default::default()
                    },
                ).await?;
            }
        }
        
        Ok(())
    }
}

impl TimeSeriesActor {
    pub async fn query_recent(&self, window: Duration) -> Result<Vec<DataPoint>> {
        // Data is already pinned and available at memory speed
        let buckets = self.get_recent_buckets(window);
        let mut results = Vec::new();
        
        for bucket in buckets {
            // Zero-copy access to pinned memory-mapped data
            let data = bucket.read_pinned_data().await?;
            results.extend(data);
        }
        
        Ok(results)
    }
    
    pub async fn append_data_point(&mut self, point: DataPoint) -> Result<()> {
        // Ensure current bucket is pinned for writes
        let current_bucket = self.get_current_bucket();
        self.ensure_bucket_pinned(current_bucket, PinPriority::TailLatencyCritical).await?;
        
        // Append to memory-mapped file
        current_bucket.append_point(point).await
    }
}
```

### 4.2 Document Store Actors

```rust

#[derive(Addressable)]
pub struct DocumentCollectionActor {
    collection_name: String,
    key_range: KeyRange,
    document_count: u64,
}

#[async_trait]
impl ActorLifecycleWithMemory for DocumentCollectionActor {
    async fn on_activate(&mut self, context: &ActorActivationContext) -> Result<()> {
        // Pin frequently accessed document ranges
        let hot_ranges = self.get_hot_key_ranges();
        
        for range in hot_ranges {
            let range_extents = context.extent_index
                .lookup_document_range(&self.collection_name, &range);
                
            for extent in range_extents {
                context.memory_manager.pin_actor_extent(
                    &context.actor_ref,
                    extent,
                    &PinOpts {
                        priority: PinPriority::QueryCritical,
                        use_hugepages: false, // Documents are typically small
                        lifetime_class: LifetimeClass::Session,
                        ttl_ms: Some(15 * 60 * 1000), // 15 minute TTL
                        ..Default::default()
                    },
                ).await?;
            }
        }
        
        // Pin index structures for this key range
        let index_extents = context.extent_index
            .lookup_collection_indices(&self.collection_name);
            
        for extent in index_extents {
            context.memory_manager.pin_actor_extent(
                &context.actor_ref,
                extent,
                &PinOpts {
                    priority: PinPriority::TailLatencyCritical,
                    use_hugepages: true,
                    lifetime_class: LifetimeClass::LongLived,
                    ..Default::default()
                },
            ).await?;
        }
        
        Ok(())
    }
}

impl DocumentCollectionActor {
    pub async fn get_document(&self, key: &str) -> Result<Option<Document>> {
        // Check if key is in our range
        if !self.key_range.contains(key) {
            return Err(anyhow!("Key {} not in range {:?}", key, self.key_range));
        }
        
        // Access is zero-copy from pinned memory
        let extent = self.find_extent_for_key(key).await?;
        extent.read_document(key).await
    }
    
    pub async fn put_document(&mut self, key: &str, doc: Document) -> Result<()> {
        // Ensure target extent is pinned for writes
        let extent = self.find_or_create_extent_for_key(key).await?;
        self.ensure_extent_pinned(extent, PinPriority::QueryCritical).await?;
        
        // Write to memory-mapped file
        extent.write_document(key, doc).await
    }
}
```

## 5. Actor Communication and Memory Hints

### 5.1 Memory Hint Messages

```rust

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryHint {
    /// Hint that these actors will be accessed soon
    WillAccessSoon {
        actor_refs: Vec<AddressableReference>,
        estimated_delay_ms: u64,
        access_pattern: AccessPattern,
    },
    
    /// Hint that access to these actors is complete
    AccessComplete {
        actor_refs: Vec<AddressableReference>,
    },
    
    /// Request to migrate actor for better locality
    MigrateForLocality {
        actor_ref: AddressableReference,
        target_numa_node: u16,
        locality_score: f32,
    },
    
    /// Hint about expected query pattern
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
    JoinWith(u64), // Join with another actor
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum QueryPattern {
    PointLookup,
    RangeScan,
    Aggregation,
    GraphTraversal,
    TimeSeriesAnalytics,
}
```

### 5.2 Smart Prefetching Based on Actor Relationships

```rust
// Graph traversal actor automatically prefetches neighbor partitions
impl GraphTraversalActor {
    pub async fn breadth_first_search(&self, start: NodeId, max_depth: u32) -> Result<Vec<Path>> {
        let mut current_level = vec![start];
        let mut visited = HashSet::new();
        let mut paths = Vec::new();
        
        for depth in 0..max_depth {
            let mut next_level = Vec::new();
            
            // Send prefetch hints for next level partitions
            let next_partitions = self.predict_next_partitions(&current_level);
            for partition_id in next_partitions {
                let partition_ref = AddressableReference {
                    addressable_type: "GraphPartitionActor".to_string(),
                    key: Key::Int64Key { key: partition_id as i64 },
                };
                
                self.send_memory_hint(MemoryHint::WillAccessSoon {
                    actor_refs: vec![partition_ref],
                    estimated_delay_ms: 50, // Will access in ~50ms
                    access_pattern: AccessPattern::GraphTraversal,
                }).await?;
            }
            
            // Process current level (data is already pinned)
            for node_id in current_level {
                if visited.contains(&node_id) {
                    continue;
                }
                
                visited.insert(node_id);
                let neighbors = self.get_neighbors(node_id).await?; // Zero-copy from pinned memory
                next_level.extend(neighbors);
            }
            
            current_level = next_level;
        }
        
        Ok(paths)
    }
}
```

## 6. NUMA-Aware Actor Placement

### 6.1 Actor Placement Strategy

```rust
pub struct NumaAwareActorPlacement {
    topology: NumaTopology,
    memory_managers: HashMap<u16, Arc<dyn ActorMemoryManager>>,
    placement_history: LruCache<AddressableReference, PlacementStats>,
}

impl NumaAwareActorPlacement {
    pub async fn optimal_placement(&self, actor_ref: &AddressableReference) 
        -> Result<PlacementDecision> {
        
        // Analyze actor's expected memory access patterns
        let memory_analysis = self.analyze_actor_memory_patterns(actor_ref).await?;
        
        // Score each NUMA node based on data locality
        let mut numa_scores = HashMap::new();
        for numa_node in self.topology.get_nodes() {
            let mut score = 0.0f64;
            
            // Score based on already-pinned data
            for extent in &memory_analysis.expected_extents {
                if let Some(pinned_node) = self.get_extent_numa_node(extent) {
                    if pinned_node == numa_node {
                        score += extent.len as f64 * 1.0; // Full score for local data
                    } else {
                        let distance = self.topology.get_distance(numa_node, pinned_node);
                        score += extent.len as f64 * (1.0 / distance as f64);
                    }
                }
            }
            
            // Score based on co-located actors
            for connected_actor in &memory_analysis.connected_actors {
                if let Some(connected_node) = self.get_actor_numa_node(connected_actor) {
                    if connected_node == numa_node {
                        score += 1000.0; // High score for co-location
                    }
                }
            }
            
            // Score based on available memory
            let available_memory = self.get_available_memory(numa_node);
            let memory_pressure = 1.0 - (available_memory as f64 / self.get_total_memory(numa_node) as f64);
            score *= 1.0 - memory_pressure; // Reduce score for high memory pressure
            
            numa_scores.insert(numa_node, score);
        }
        
        let optimal_numa = numa_scores
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(node, _)| node);
        
        Ok(PlacementDecision {
            preferred_numa_node: optimal_numa,
            expected_memory_mb: memory_analysis.total_memory_mb,
            co_location_score: memory_analysis.co_location_score,
            data_locality_score: memory_analysis.locality_score,
        })
    }
}

#[derive(Debug)]
pub struct MemoryAnalysis {
    pub expected_extents: Vec<ExtentRef>,
    pub connected_actors: Vec<AddressableReference>,
    pub total_memory_mb: u64,
    pub locality_score: f64,
    pub co_location_score: f64,
}
```

## 7. Distributed Memory Coordination

### 7.1 Cross-Node Memory Management

```rust
pub struct DistributedActorMemoryCoordinator {
    local_memory_manager: Arc<dyn ActorMemoryManager>,
    cluster_view: Arc<ClusterMemoryView>,
    replication_manager: Arc<ReplicationManager>,
    migration_coordinator: Arc<ActorMigrationCoordinator>,
}

impl DistributedActorMemoryCoordinator {
    /// Coordinate actor migration with memory pre-warming
    pub async fn migrate_actor_with_prewarm(&self, 
        actor_ref: &AddressableReference,
        target_node: NodeId
    ) -> Result<()> {
        println!("Migrating actor {} to node {}", actor_ref, target_node);
        
        // 1. Analyze current memory footprint
        let memory_stats = self.local_memory_manager
            .get_actor_memory_stats(actor_ref).await?;
        
        println!("Actor memory footprint: {} MB across {} extents", 
                 memory_stats.total_memory_mb, memory_stats.pinned_extents.len());
        
        // 2. Pre-warm target node by pinning actor's data there
        let prewarm_start = Instant::now();
        for extent in &memory_stats.pinned_extents {
            self.send_remote_pin_request(
                target_node,
                extent,
                PinPriority::QueryCritical,
            ).await?;
        }
        
        // 3. Wait for pre-warming to complete
        self.wait_for_prewarm_completion(target_node, &memory_stats.pinned_extents).await?;
        let prewarm_time = prewarm_start.elapsed();
        println!("Pre-warming completed in {:?}", prewarm_time);
        
        // 4. Migrate the actor (fast because data is already warm)
        let migration_start = Instant::now();
        self.migration_coordinator.migrate_actor(actor_ref, target_node).await?;
        let migration_time = migration_start.elapsed();
        println!("Actor migration completed in {:?}", migration_time);
        
        // 5. Gradually unpin memory on source node (delayed for safety)
        tokio::spawn({
            let local_manager = self.local_memory_manager.clone();
            let extents = memory_stats.pinned_extents.clone();
            async move {
                tokio::time::sleep(Duration::from_secs(30)).await; // Grace period
                for extent in extents {
                    local_manager.unpin_extent(extent).await.ok();
                }
                println!("Source node memory cleanup completed");
            }
        });
        
        Ok(())
    }
    
    /// Handle replication-aware memory management
    pub async fn coordinate_replicated_actor_memory(&self,
        actor_ref: &AddressableReference
    ) -> Result<()> {
        let actor_extents = self.get_actor_extents(actor_ref).await?;
        
        for extent in actor_extents {
            let replica_nodes = self.replication_manager.get_replica_nodes(&extent)?;
            
            // Pin on primary replica with high priority
            if let Some(primary_node) = replica_nodes.primary {
                if primary_node == self.get_local_node_id() {
                    self.local_memory_manager.pin_extent(
                        extent,
                        PinOpts {
                            priority: PinPriority::TailLatencyCritical,
                            use_hugepages: true,
                            lifetime_class: LifetimeClass::LongLived,
                            ..Default::default()
                        },
                    ).await?;
                } else {
                    self.send_remote_pin_request(
                        primary_node, 
                        &extent, 
                        PinPriority::TailLatencyCritical
                    ).await?;
                }
            }
            
            // Pin on secondary replicas with lower priority
            for secondary_node in replica_nodes.secondaries {
                let secondary_priority = PinPriority::QueryCritical;
                
                if secondary_node == self.get_local_node_id() {
                    self.local_memory_manager.pin_extent(
                        extent,
                        PinOpts {
                            priority: secondary_priority,
                            lifetime_class: LifetimeClass::Task,
                            ..Default::default()
                        },
                    ).await?;
                } else {
                    self.send_remote_pin_request(
                        secondary_node, 
                        &extent, 
                        secondary_priority
                    ).await?;
                }
            }
        }
        
        Ok(())
    }
}
```

## 8. Performance Guarantees and SLA Enforcement

### 8.1 Actor-Level SLA Definition

```rust

#[derive(Debug, Clone)]
pub struct ActorSLA {
    pub max_activation_time_ms: u64,
    pub max_response_time_p99_ms: u64,
    pub max_response_time_p999_ms: u64,
    pub memory_guarantee: MemoryGuarantee,
    pub availability_guarantee: f64, // 0.0 to 1.0
}

#[derive(Debug, Clone)]
pub enum MemoryGuarantee {
    AlwaysPinned,                    // Memory always resident
    PinnedDuringAccess,             // Memory pinned only when active
    BestEffort,                      // No guarantee, subject to eviction
}

// Example SLA definitions
impl GraphPartitionActor {
    fn get_sla(&self) -> ActorSLA {
        ActorSLA {
            max_activation_time_ms: 100,           // 100ms to activate
            max_response_time_p99_ms: 1,           // 1ms P99 response time
            max_response_time_p999_ms: 10,         // 10ms P99.9 response time
            memory_guarantee: MemoryGuarantee::AlwaysPinned,
            availability_guarantee: 0.9999,        // 99.99% availability
        }
    }
}

impl SessionCacheActor {
    fn get_sla(&self) -> ActorSLA {
        ActorSLA {
            max_activation_time_ms: 500,           // 500ms to activate
            max_response_time_p99_ms: 10,          // 10ms P99 response time
            max_response_time_p999_ms: 100,        // 100ms P99.9 response time
            memory_guarantee: MemoryGuarantee::PinnedDuringAccess,
            availability_guarantee: 0.999,         // 99.9% availability
        }
    }
}
```

### 8.2 SLA Monitoring and Enforcement

```rust
pub struct ActorSLAMonitor {
    response_time_histograms: HashMap<String, Histogram>,
    activation_time_histograms: HashMap<String, Histogram>,
    memory_usage_trackers: HashMap<AddressableReference, MemoryUsageTracker>,
    sla_violations: Arc<Mutex<Vec<SLAViolation>>>,
}

impl ActorSLAMonitor {
    pub async fn record_actor_invocation(&self, 
        actor_ref: &AddressableReference,
        response_time: Duration,
        memory_access_pattern: MemoryAccessPattern
    ) -> Result<()> {
        let actor_type = &actor_ref.addressable_type;
        
        // Record response time
        if let Some(histogram) = self.response_time_histograms.get_mut(actor_type) {
            histogram.record(response_time.as_micros() as u64);
        }
        
        // Check for SLA violations
        let sla = self.get_actor_sla(actor_ref);
        let p99_micros = histogram.value_at_percentile(99.0) as u64;
        let p999_micros = histogram.value_at_percentile(99.9) as u64;
        
        if p99_micros > sla.max_response_time_p99_ms * 1000 {
            self.report_sla_violation(SLAViolation {
                actor_ref: actor_ref.clone(),
                violation_type: ViolationType::P99ResponseTime,
                actual_value: p99_micros,
                expected_value: sla.max_response_time_p99_ms * 1000,
                timestamp: SystemTime::now(),
            }).await;
        }
        
        Ok(())
    }
    
    pub async fn report_sla_violation(&self, violation: SLAViolation) {
        println!("SLA VIOLATION: {:?}", violation);
        
        // Trigger remediation actions
        match violation.violation_type {
            ViolationType::P99ResponseTime => {
                // Increase memory pinning priority
                self.upgrade_memory_priority(&violation.actor_ref, PinPriority::TailLatencyCritical).await;
            },
            ViolationType::ActivationTime => {
                // Pre-warm actor memory
                self.prewarm_actor_memory(&violation.actor_ref).await;
            },
            ViolationType::MemoryPressure => {
                // Migrate to node with more available memory
                self.migrate_for_memory_pressure(&violation.actor_ref).await;
            },
        }
        
        let mut violations = self.sla_violations.lock().await;
        violations.push(violation);
    }
}
```

## 9. Example Usage Scenarios

### 9.1 Real-Time Graph Analytics

```rust
// Example: Social media influence analysis

#[derive(Addressable)]
pub struct InfluenceAnalysisActor {
    user_id: u64,
    influence_score: f64,
}

impl InfluenceAnalysisActor {
    /// Calculate influence score by traversing follower network
    pub async fn calculate_influence(&self, depth: u32) -> Result<f64> {
        // Actor's memory is automatically optimized for graph traversal
        let followers = self.get_followers().await?; // Zero-copy from pinned memory
        let mut total_influence = 0.0;
        
        // Traverse follower network up to specified depth
        for follower in followers {
            // Memory hints ensure follower data is prefetched
            let follower_actor = self.get_follower_actor(follower).await?;
            let follower_influence = follower_actor.get_influence_score().await?;
            total_influence += follower_influence;
            
            if depth > 1 {
                // Recursive calculation with automatic memory optimization
                total_influence += follower_actor.calculate_influence(depth - 1).await? * 0.5;
            }
        }
        
        Ok(total_influence)
    }
}

#[async_trait]
impl ActorLifecycleWithMemory for InfluenceAnalysisActor {
    async fn on_activate(&mut self, context: &ActorActivationContext) -> Result<()> {
        // Pin user's social network data
        let network_extents = context.extent_index
            .lookup_social_network(self.user_id);
            
        for extent in network_extents {
            context.memory_manager.pin_actor_extent(
                &context.actor_ref,
                extent,
                &PinOpts {
                    priority: PinPriority::TailLatencyCritical, // Real-time requirements
                    use_hugepages: true,
                    lifetime_class: LifetimeClass::Session,
                    prefetch_adjacent: 3, // Prefetch 3 degrees of separation
                    ..Default::default()
                },
            ).await?;
        }
        
        Ok(())
    }
}
```

### 9.2 High-Frequency Trading Analytics

```rust

#[derive(Addressable)]
pub struct TradingSignalActor {
    symbol: String,
    signal_window: Duration,
}

impl TradingSignalActor {
    /// Generate trading signal from recent price data
    pub async fn generate_signal(&self) -> Result<TradingSignal> {
        // Ultra-low latency access to recent price data (always pinned)
        let recent_prices = self.get_recent_prices(self.signal_window).await?;
        
        // Technical indicators calculated from pinned memory
        let sma_short = self.calculate_sma(&recent_prices, 10)?;
        let sma_long = self.calculate_sma(&recent_prices, 50)?;
        let rsi = self.calculate_rsi(&recent_prices, 14)?;
        
        // Generate signal based on indicators
        let signal = if sma_short > sma_long && rsi < 70.0 {
            TradingSignal::Buy
        } else if sma_short < sma_long && rsi > 30.0 {
            TradingSignal::Sell
        } else {
            TradingSignal::Hold
        };
        
        Ok(signal)
    }
}

#[async_trait]
impl ActorLifecycleWithMemory for TradingSignalActor {
    async fn on_activate(&mut self, context: &ActorActivationContext) -> Result<()> {
        // Pin last hour of price data for this symbol
        let price_extents = context.extent_index
            .lookup_price_data(&self.symbol, Duration::from_secs(3600));
            
        for extent in price_extents {
            context.memory_manager.pin_actor_extent(
                &context.actor_ref,
                extent,
                &PinOpts {
                    priority: PinPriority::TailLatencyCritical,
                    use_hugepages: true,
                    numa_prefer: Some(context.local_numa_node),
                    lifetime_class: LifetimeClass::LongLived, // Always keep recent data hot
                    ttl_ms: None, // Never expire
                    ..Default::default()
                },
            ).await?;
        }
        
        // Pre-warm related symbols for correlation analysis
        let correlated_symbols = self.get_correlated_symbols();
        for symbol in correlated_symbols {
            let symbol_ref = AddressableReference {
                addressable_type: "TradingSignalActor".to_string(),
                key: Key::StringKey { key: symbol },
            };
            
            context.memory_manager.send_memory_hint(MemoryHint::WillAccessSoon {
                actor_refs: vec![symbol_ref],
                estimated_delay_ms: 10, // Need within 10ms for correlation calc
                access_pattern: AccessPattern::Sequential,
            }).await?;
        }
        
        Ok(())
    }
}

impl ActorMemoryBehavior for TradingSignalActor {
    fn get_memory_profile(&self) -> ActorMemoryProfile {
        ActorMemoryProfile::Hot // Ultra-low latency requirement
    }
    
    fn estimate_memory_footprint(&self) -> MemoryFootprint {
        MemoryFootprint {
            persistent_state_mb: 64,    // 1 hour of minute-level price data
            working_set_mb: 16,         // Calculation buffers
            prefetch_candidates: self.get_correlated_symbol_extents(),
        }
    }
}
```

## 10. Implementation Roadmap

### 10.1 Phase 1: Core Integration (Weeks 1-4)
- [ ] Implement `ActorMemoryManager` trait and basic integration
- [ ] Add memory profile classification to actor traits
- [ ] Create actor lifecycle hooks for memory management
- [ ] Basic pin/unpin on activate/deactivate

### 10.2 Phase 2: Smart Optimization (Weeks 5-8)
- [ ] Implement memory hint messaging system
- [ ] Add NUMA-aware actor placement
- [ ] Implement cross-actor prefetching
- [ ] Memory pressure-based priority adjustment

### 10.3 Phase 3: Distributed Coordination (Weeks 9-12)
- [ ] Cross-node memory management coordination
- [ ] Actor migration with memory pre-warming
- [ ] Replication-aware memory management
- [ ] Cluster-wide memory budgeting

### 10.4 Phase 4: SLA and Monitoring (Weeks 13-16)
- [ ] Actor-level SLA definition and monitoring
- [ ] Automatic remediation for SLA violations
- [ ] Performance telemetry and dashboards
- [ ] Memory usage optimization recommendations

## 11. Conclusion

The integration of advanced memory management with Orbit-RS's virtual actor model provides:

1. **Zero-Configuration Performance**: Actors automatically get optimal memory placement
2. **Natural Boundaries**: Actor boundaries define memory locality and lifecycle
3. **Distributed Intelligence**: Actor relationships drive distributed memory coordination
4. **SLA Guarantees**: Different actor types get appropriate performance guarantees
5. **Transparent Optimization**: Memory management is invisible to application code

This creates a system where petabyte-scale data can be accessed with microsecond latencies through intelligent, automatic memory management that leverages the actor model's natural structure.

The approach scales from single-node development to distributed production deployments while maintaining the elegant simplicity that makes Orbit-RS easy to develop with and operate.