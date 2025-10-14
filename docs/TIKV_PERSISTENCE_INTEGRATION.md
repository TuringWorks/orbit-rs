---
layout: default
title: TiKV Persistence Layer Integration
category: architecture
---

# TiKV Storage Provider Integration for Orbit-RS

**Date**: October 13, 2025  
**Author**: AI Assistant  
**Status**: Proposal  
**Version**: 2.0

## Executive Summary

This document outlines the integration of TiKV as a distributed storage provider for Orbit-RS, alongside RocksDB and other storage engines. TiKV provides an excellent storage option for Orbit-RS actors requiring distributed storage capabilities, leveraging its Rust implementation, ACID compliance, and proven Raft consensus algorithm.

**Key Benefits:**
- **Storage Provider Option**: Distributed storage choice alongside RocksDB
- **Native Rust Integration**: Optimal performance and memory safety
- **Distributed ACID**: Strong consistency for multi-actor scenarios
- **Raft Consensus**: Battle-tested distributed consensus
- **Production Ready**: Mature, operationally proven storage engine

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Technical Integration Design](#technical-integration-design)
3. [Actor-to-Region Mapping Strategy](#actor-to-region-mapping-strategy)
4. [Multi-Model Data Storage Patterns](#multi-model-data-storage-patterns)
5. [Transaction Management](#transaction-management)
6. [Performance Optimization](#performance-optimization)
7. [Implementation Roadmap](#implementation-roadmap)
8. [Operational Considerations](#operational-considerations)
9. [Migration Strategy](#migration-strategy)
10. [Testing and Validation](#testing-and-validation)

## Architecture Overview

### Current Orbit-RS Architecture
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Protocol      │    │   Query         │    │   Multi-Model   │
│   Adapters      │    │   Engines       │    │   Processors    │
│                 │    │                 │    │                 │
│ • PostgreSQL    │    │ • SQL           │    │ • Relational    │
│ • Redis         │────▶ • GraphQL       │────▶ • Vector        │
│ • gRPC          │    │ • Cypher        │    │ • Graph         │
│ • REST          │    │ • OrbitQL       │    │ • Time Series   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                        │
                              ┌─────────────────────────▼─────────────────────────┐
                              │                Actor System                        │
                              │                                                    │
                              │  ┌───────────┐  ┌───────────┐  ┌───────────┐     │
                              │  │  Actor A  │  │  Actor B  │  │  Actor C  │     │
                              │  │           │  │           │  │           │     │
                              │  └───────────┘  └───────────┘  └───────────┘     │
                              └─────────────────────────┬─────────────────────────┘
                                                        │
                              ┌─────────────────────────▼─────────────────────────┐
                              │             Storage Backend (Current)              │
                              │                                                    │
                              │              • Local Storage                      │
                              │              • Limited Distribution               │
                              └────────────────────────────────────────────────────┘
```

### Proposed Storage Provider Architecture
```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Protocol      │    │   Query         │    │   Multi-Model   │
│   Adapters      │    │   Engines       │    │   Processors    │
│                 │    │                 │    │                 │
│ • PostgreSQL    │    │ • SQL           │    │ • Relational    │
│ • Redis         │────▶ • GraphQL       │────▶ • Vector        │
│ • gRPC          │    │ • Cypher        │    │ • Graph         │
│ • REST          │    │ • OrbitQL       │    │ • Time Series   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                        │
                              ┌─────────────────────────▼─────────────────────────┐
                              │                Actor System                        │
                              │                                                    │
                              │  ┌───────────┐  ┌───────────┐  ┌───────────┐     │
                              │  │  Actor A  │  │  Actor B  │  │  Actor C  │     │
                              │  │ RocksDB   │  │   TiKV    │  │ RocksDB   │     │
                              │  └───────────┘  └───────────┘  └───────────┘     │
                              └─────────────────────────┬─────────────────────────┘
                                                        │
                              ┌─────────────────────────▼─────────────────────────┐
                              │              Storage Provider Layer               │
                              │                                                    │
                              │  • Unified Storage Interface  • Provider Selection│
                              │  • Multi-Model Abstraction   • Configuration Mgmt│
                              │  • Transaction Coordination  • Performance Tuning│
                              └─────────────────────────┬─────────────────────────┘
                                                        │
                    ┌───────────────────────────────────┼───────────────────────────────────┐
                    │                                   │                                   │
          ┌─────────▼─────────┐                ┌────────▼────────┐                ┌───────▼───────┐
          │   RocksDB         │                │     TiKV        │                │    Future     │
          │   Provider        │                │    Provider     │                │   Providers   │
          │                   │                │                 │                │               │
          │ • Local Storage   │                │ • Distributed   │                │ • FoundationDB│
          │ • High Performance│                │ • ACID Txns     │                │ • Apache      │
          │ • Embedded        │                │ • Raft Consensus│                │   Cassandra   │
          └───────────────────┘                │ • Multi-Region  │                │ • Others...   │
                                               └─────────────────┘                └───────────────┘
```

## Technical Integration Design

### Core Integration Components

#### 1. Storage Provider Interface

```rust
use orbit_core::storage::{StorageProvider, StorageError, Key, Value};
use tikv_client::{Client as TiKVClient, Transaction};
use rocksdb::{DB, Options};

// Unified storage provider trait
#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn get(&self, key: &Key) -> Result<Option<Value>, StorageError>;
    async fn put(&self, key: Key, value: Value) -> Result<(), StorageError>;
    async fn delete(&self, key: &Key) -> Result<(), StorageError>;
    async fn batch_operations(&self, ops: Vec<StorageOperation>) -> Result<(), StorageError>;
    
    // Provider-specific capabilities
    fn supports_distributed_transactions(&self) -> bool;
    fn supports_multi_region(&self) -> bool;
    fn provider_type(&self) -> StorageProviderType;
}

#[derive(Debug, Clone, PartialEq)]
pub enum StorageProviderType {
    RocksDB,
    TiKV,
    FoundationDB,
    // Future providers...
}

// TiKV Storage Provider Implementation
pub struct TiKVProvider {
    client: TiKVClient,
    config: TiKVConfig,
}

impl TiKVProvider {
    pub async fn new(endpoints: Vec<String>) -> Result<Self, StorageError> {
        let client = TiKVClient::new(endpoints).await?;
        let config = TiKVConfig::default();
        
        Ok(TiKVProvider { client, config })
    }
}

#[async_trait]
impl StorageProvider for TiKVProvider {
    async fn get(&self, key: &Key) -> Result<Option<Value>, StorageError> {
        let txn = self.client.begin_optimistic().await?;
        let result = txn.get(key.clone()).await?;
        Ok(result)
    }
    
    async fn put(&self, key: Key, value: Value) -> Result<(), StorageError> {
        let txn = self.client.begin_optimistic().await?;
        txn.put(key, value).await?;
        txn.commit().await?;
        Ok(())
    }
    
    async fn delete(&self, key: &Key) -> Result<(), StorageError> {
        let txn = self.client.begin_optimistic().await?;
        txn.delete(key.clone()).await?;
        txn.commit().await?;
        Ok(())
    }
    
    async fn batch_operations(&self, ops: Vec<StorageOperation>) -> Result<(), StorageError> {
        let txn = self.client.begin_optimistic().await?;
        
        for op in ops {
            match op {
                StorageOperation::Put(key, value) => txn.put(key, value).await?,
                StorageOperation::Delete(key) => txn.delete(key).await?,
                StorageOperation::Get(key) => { /* Handle get in batch */ },
            }
        }
        
        txn.commit().await?;
        Ok(())
    }
    
    fn supports_distributed_transactions(&self) -> bool { true }
    fn supports_multi_region(&self) -> bool { true }
    fn provider_type(&self) -> StorageProviderType { StorageProviderType::TiKV }
}

// RocksDB Storage Provider Implementation
pub struct RocksDBProvider {
    db: Arc<DB>,
    config: RocksDBConfig,
}

#[async_trait]
impl StorageProvider for RocksDBProvider {
    async fn get(&self, key: &Key) -> Result<Option<Value>, StorageError> {
        Ok(self.db.get(key)?.map(|v| v.into()))
    }
    
    async fn put(&self, key: Key, value: Value) -> Result<(), StorageError> {
        self.db.put(key, value)?;
        Ok(())
    }
    
    async fn delete(&self, key: &Key) -> Result<(), StorageError> {
        self.db.delete(key)?;
        Ok(())
    }
    
    async fn batch_operations(&self, ops: Vec<StorageOperation>) -> Result<(), StorageError> {
        let mut batch = rocksdb::WriteBatch::default();
        for op in ops {
            match op {
                StorageOperation::Put(key, value) => batch.put(&key, &value),
                StorageOperation::Delete(key) => batch.delete(&key),
                StorageOperation::Get(_) => { /* Handle get separately */ },
            }
        }
        self.db.write(batch)?;
        Ok(())
    }
    
    fn supports_distributed_transactions(&self) -> bool { false }
    fn supports_multi_region(&self) -> bool { false }
    fn provider_type(&self) -> StorageProviderType { StorageProviderType::RocksDB }
}
```

#### 2. Storage Provider Manager

```rust
// Storage provider selection and management
pub struct StorageManager {
    providers: HashMap<StorageProviderType, Box<dyn StorageProvider>>,
    actor_assignments: HashMap<ActorId, StorageProviderType>,
    selection_strategy: ProviderSelectionStrategy,
    config: StorageManagerConfig,
}

#[derive(Debug, Clone)]
pub enum ProviderSelectionStrategy {
    Default(StorageProviderType),
    ByActorType(HashMap<ActorType, StorageProviderType>),
    ByDataPattern(DataPatternMatcher),
    Custom(Box<dyn Fn(&ActorContext) -> StorageProviderType>),
}

#[derive(Debug, Clone)]
pub struct DataPatternMatcher {
    distributed_threshold: usize,  // Use TiKV for actors with >N cross-references
    performance_requirements: PerformanceProfile,
    consistency_requirements: ConsistencyLevel,
}

impl StorageManager {
    pub async fn new(config: StorageManagerConfig) -> Result<Self, StorageError> {
        let mut providers: HashMap<StorageProviderType, Box<dyn StorageProvider>> = HashMap::new();
        
        // Initialize configured providers
        if config.enable_rocksdb {
            let rocksdb = RocksDBProvider::new(&config.rocksdb_config)?;
            providers.insert(StorageProviderType::RocksDB, Box::new(rocksdb));
        }
        
        if config.enable_tikv {
            let tikv = TiKVProvider::new(config.tikv_endpoints.clone()).await?;
            providers.insert(StorageProviderType::TiKV, Box::new(tikv));
        }
        
        Ok(StorageManager {
            providers,
            actor_assignments: HashMap::new(),
            selection_strategy: config.selection_strategy,
            config,
        })
    }
    
    pub async fn get_provider_for_actor(&mut self, actor_id: ActorId, context: &ActorContext) -> Result<&dyn StorageProvider, StorageError> {
        // Check if actor already has assigned provider
        if let Some(provider_type) = self.actor_assignments.get(&actor_id) {
            return Ok(self.providers.get(provider_type).unwrap().as_ref());
        }
        
        // Select provider based on strategy
        let provider_type = self.select_provider(actor_id, context).await?;
        self.actor_assignments.insert(actor_id, provider_type.clone());
        
        Ok(self.providers.get(&provider_type).unwrap().as_ref())
    }
    
    async fn select_provider(&self, actor_id: ActorId, context: &ActorContext) -> Result<StorageProviderType, StorageError> {
        match &self.selection_strategy {
            ProviderSelectionStrategy::Default(provider_type) => Ok(provider_type.clone()),
            
            ProviderSelectionStrategy::ByActorType(mapping) => {
                Ok(mapping.get(&context.actor_type)
                    .cloned()
                    .unwrap_or(StorageProviderType::RocksDB))
            },
            
            ProviderSelectionStrategy::ByDataPattern(matcher) => {
                // Analyze data patterns to choose provider
                let cross_references = context.get_cross_actor_references().await?;
                let performance_needs = context.get_performance_requirements();
                let consistency_needs = context.get_consistency_requirements();
                
                if cross_references.len() > matcher.distributed_threshold {
                    Ok(StorageProviderType::TiKV)  // Use distributed storage
                } else if performance_needs.latency_sensitive {
                    Ok(StorageProviderType::RocksDB)  // Use local high-performance storage
                } else {
                    Ok(StorageProviderType::RocksDB)  // Default to RocksDB
                }
            },
            
            ProviderSelectionStrategy::Custom(selector) => {
                Ok(selector(context))
            },
        }
    }
    
    pub async fn migrate_actor_storage(&mut self, 
        actor_id: ActorId, 
        from_provider: StorageProviderType, 
        to_provider: StorageProviderType
    ) -> Result<(), StorageError> {
        let from = self.providers.get(&from_provider).unwrap();
        let to = self.providers.get(&to_provider).unwrap();
        
        // 1. Start migration transaction
        let migration_id = MigrationId::new();
        
        // 2. Copy data from source to target
        let actor_keys = self.get_actor_keys(actor_id).await?;
        for key in actor_keys {
            if let Some(value) = from.get(&key).await? {
                to.put(key.clone(), value).await?;
            }
        }
        
        // 3. Update assignment
        self.actor_assignments.insert(actor_id, to_provider);
        
        // 4. Clean up old data (optional, can be done async)
        for key in self.get_actor_keys(actor_id).await? {
            from.delete(&key).await?;
        }
        
        Ok(())
    }
}
```

#### 3. Configuration Management

```rust
#[derive(Debug, Clone)]
pub struct TiKVConfig {
    pub endpoints: Vec<String>,
    pub timeout: Duration,
    pub max_batch_size: usize,
    pub region_cache_size: usize,
    pub enable_async_commit: bool,
    pub enable_one_pc: bool,
    pub gc_interval: Duration,
    pub region_split_threshold: u64,
    pub coprocessor_pool_size: usize,
}

impl Default for TiKVConfig {
    fn default() -> Self {
        TiKVConfig {
            endpoints: vec!["127.0.0.1:2379".to_string()],
            timeout: Duration::from_secs(30),
            max_batch_size: 1000,
            region_cache_size: 1000,
            enable_async_commit: true,
            enable_one_pc: true,
            gc_interval: Duration::from_secs(600),
            region_split_threshold: 64 * 1024 * 1024, // 64MB
            coprocessor_pool_size: 8,
        }
    }
}
```

## Actor-to-Region Mapping Strategy

### Region Assignment Algorithm

```rust
pub struct ActorRegionMapper {
    region_cache: Arc<RwLock<HashMap<ActorId, RegionId>>>,
    region_manager: RegionManager,
    hash_ring: ConsistentHashRing,
}

impl ActorRegionMapper {
    pub async fn get_region(&self, actor_id: ActorId) -> Result<RegionId, MappingError> {
        // Check cache first
        if let Some(region_id) = self.region_cache.read().await.get(&actor_id) {
            return Ok(*region_id);
        }
        
        // Calculate region based on actor_id hash
        let hash = self.calculate_actor_hash(&actor_id);
        let region_id = self.hash_ring.get_region(hash).await?;
        
        // Cache the mapping
        self.region_cache.write().await.insert(actor_id, region_id);
        
        Ok(region_id)
    }
    
    pub async fn migrate_actor(&self, actor_id: ActorId, target_region: RegionId) -> Result<(), MappingError> {
        // 1. Begin distributed transaction
        let txn = self.region_manager.begin_migration_txn().await?;
        
        // 2. Copy data from source to target region
        let source_region = self.get_region(actor_id).await?;
        let actor_data = self.export_actor_data(source_region, actor_id).await?;
        self.import_actor_data(target_region, actor_id, actor_data).await?;
        
        // 3. Update mapping
        self.region_cache.write().await.insert(actor_id, target_region);
        
        // 4. Clean up source data
        self.cleanup_actor_data(source_region, actor_id).await?;
        
        // 5. Commit transaction
        txn.commit().await?;
        
        Ok(())
    }
    
    fn calculate_actor_hash(&self, actor_id: &ActorId) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        actor_id.hash(&mut hasher);
        hasher.finish()
    }
}
```

### Data Locality Optimization

```rust
pub struct DataLocalityOptimizer {
    region_manager: RegionManager,
    actor_mapper: ActorRegionMapper,
    metrics_collector: MetricsCollector,
}

impl DataLocalityOptimizer {
    pub async fn optimize_placement(&self) -> Result<(), OptimizationError> {
        let actors = self.get_all_actors().await?;
        let regions = self.region_manager.get_all_regions().await?;
        
        for actor in actors {
            let current_region = self.actor_mapper.get_region(actor.id).await?;
            let optimal_region = self.calculate_optimal_region(&actor, &regions).await?;
            
            if current_region != optimal_region {
                let migration_cost = self.calculate_migration_cost(&actor, current_region, optimal_region).await?;
                let locality_benefit = self.calculate_locality_benefit(&actor, optimal_region).await?;
                
                if locality_benefit > migration_cost {
                    self.actor_mapper.migrate_actor(actor.id, optimal_region).await?;
                }
            }
        }
        
        Ok(())
    }
    
    async fn calculate_optimal_region(&self, actor: &Actor, regions: &[Region]) -> Result<RegionId, OptimizationError> {
        let mut best_region = regions[0].id;
        let mut best_score = 0.0;
        
        for region in regions {
            let score = self.calculate_placement_score(actor, region).await?;
            if score > best_score {
                best_score = score;
                best_region = region.id;
            }
        }
        
        Ok(best_region)
    }
    
    async fn calculate_placement_score(&self, actor: &Actor, region: &Region) -> Result<f64, OptimizationError> {
        let mut score = 0.0;
        
        // Factor 1: Network latency to frequently accessed data
        let network_score = self.calculate_network_score(actor, region).await?;
        score += network_score * 0.4;
        
        // Factor 2: Resource utilization balance
        let resource_score = self.calculate_resource_score(region).await?;
        score += resource_score * 0.3;
        
        // Factor 3: Data affinity (related actors)
        let affinity_score = self.calculate_affinity_score(actor, region).await?;
        score += affinity_score * 0.3;
        
        Ok(score)
    }
}
```

## Multi-Model Data Storage Patterns

### Key Design Patterns

```rust
pub struct MultiModelKeyBuilder {
    namespace_separator: &'static str,
    model_prefixes: HashMap<DataModel, &'static str>,
}

impl MultiModelKeyBuilder {
    pub fn new() -> Self {
        let mut model_prefixes = HashMap::new();
        model_prefixes.insert(DataModel::Relational, "rel");
        model_prefixes.insert(DataModel::Document, "doc");
        model_prefixes.insert(DataModel::Graph, "grp");
        model_prefixes.insert(DataModel::Vector, "vec");
        model_prefixes.insert(DataModel::TimeSeries, "ts");
        
        MultiModelKeyBuilder {
            namespace_separator: ":",
            model_prefixes,
        }
    }
    
    pub fn build_key(&self, model: DataModel, entity: &str, id: &str) -> Key {
        let prefix = self.model_prefixes.get(&model).unwrap_or(&"unk");
        format!("{}{}{}{}{}", prefix, self.namespace_separator, entity, self.namespace_separator, id)
            .into_bytes()
            .into()
    }
    
    // Relational data keys
    pub fn table_row_key(&self, table: &str, row_id: &str) -> Key {
        format!("rel:table:{}:row:{}", table, row_id).into_bytes().into()
    }
    
    pub fn index_key(&self, table: &str, index: &str, value: &str) -> Key {
        format!("rel:index:{}:{}:{}", table, index, value).into_bytes().into()
    }
    
    // Vector data keys
    pub fn vector_key(&self, collection: &str, vector_id: &str) -> Key {
        format!("vec:collection:{}:id:{}", collection, vector_id).into_bytes().into()
    }
    
    pub fn vector_index_key(&self, collection: &str, index_type: &str, bucket: u32) -> Key {
        format!("vec:index:{}:{}:bucket:{}", collection, index_type, bucket).into_bytes().into()
    }
    
    // Graph data keys
    pub fn node_key(&self, graph: &str, node_id: &str) -> Key {
        format!("grp:node:{}:{}", graph, node_id).into_bytes().into()
    }
    
    pub fn edge_key(&self, graph: &str, from_id: &str, relation: &str, to_id: &str) -> Key {
        format!("grp:edge:{}:{}:{}:{}", graph, from_id, relation, to_id).into_bytes().into()
    }
    
    pub fn adjacency_key(&self, graph: &str, node_id: &str, direction: Direction) -> Key {
        let dir = match direction {
            Direction::Outbound => "out",
            Direction::Inbound => "in",
            Direction::Both => "both",
        };
        format!("grp:adj:{}:{}:{}", graph, node_id, dir).into_bytes().into()
    }
    
    // Time series data keys
    pub fn timeseries_key(&self, series_id: &str, timestamp: i64, sequence: u32) -> Key {
        format!("ts:series:{}:time:{}:seq:{}", series_id, timestamp, sequence).into_bytes().into()
    }
    
    pub fn timeseries_index_key(&self, series_id: &str, granularity: Granularity, bucket: i64) -> Key {
        format!("ts:index:{}:{}:bucket:{}", series_id, granularity.as_str(), bucket).into_bytes().into()
    }
}
```

### Storage Value Formats

```rust
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredRecord {
    pub model_type: DataModel,
    pub schema_version: u32,
    pub data: Vec<u8>,
    pub metadata: RecordMetadata,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecordMetadata {
    pub actor_id: ActorId,
    pub transaction_id: Option<TransactionId>,
    pub version: u64,
    pub checksum: u32,
    pub compression: CompressionType,
}

// Relational data storage
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RelationalRecord {
    pub table_name: String,
    pub row_id: String,
    pub columns: HashMap<String, ColumnValue>,
    pub schema_hash: u64,
}

// Vector data storage
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VectorRecord {
    pub collection: String,
    pub vector_id: String,
    pub dimensions: u32,
    pub data: Vec<f32>,
    pub metadata: serde_json::Value,
    pub index_hints: Vec<IndexHint>,
}

// Graph data storage
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GraphNodeRecord {
    pub graph: String,
    pub node_id: String,
    pub labels: Vec<String>,
    pub properties: serde_json::Value,
    pub adjacency_count: HashMap<Direction, u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GraphEdgeRecord {
    pub graph: String,
    pub from_id: String,
    pub to_id: String,
    pub relation: String,
    pub properties: serde_json::Value,
    pub weight: Option<f64>,
}

// Time series data storage
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TimeSeriesRecord {
    pub series_id: String,
    pub timestamp: i64,
    pub sequence: u32,
    pub value: TimeSeriesValue,
    pub tags: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TimeSeriesValue {
    Int64(i64),
    Float64(f64),
    String(String),
    Boolean(bool),
    JSON(serde_json::Value),
}
```

## Transaction Management

### Cross-Model Transaction Support

```rust
pub struct OrbitTransaction {
    tikv_txn: Transaction,
    transaction_id: TransactionId,
    affected_models: HashSet<DataModel>,
    affected_actors: HashSet<ActorId>,
    isolation_level: IsolationLevel,
    timeout: Duration,
    start_time: Instant,
}

impl OrbitTransaction {
    pub async fn begin(client: &TiKVClient, isolation_level: IsolationLevel) -> Result<Self, TransactionError> {
        let tikv_txn = match isolation_level {
            IsolationLevel::ReadCommitted => client.begin_optimistic().await?,
            IsolationLevel::RepeatableRead => client.begin_pessimistic().await?,
            IsolationLevel::Serializable => client.begin_pessimistic().await?,
        };
        
        Ok(OrbitTransaction {
            tikv_txn,
            transaction_id: TransactionId::new(),
            affected_models: HashSet::new(),
            affected_actors: HashSet::new(),
            isolation_level,
            timeout: Duration::from_secs(30),
            start_time: Instant::now(),
        })
    }
    
    pub async fn put_relational(&mut self, table: &str, row_id: &str, data: &RelationalRecord) -> Result<(), TransactionError> {
        let key = self.key_builder().table_row_key(table, row_id);
        let stored_record = StoredRecord {
            model_type: DataModel::Relational,
            schema_version: 1,
            data: bincode::serialize(data)?,
            metadata: RecordMetadata {
                actor_id: ActorId::from_table_row(table, row_id),
                transaction_id: Some(self.transaction_id),
                version: 1,
                checksum: calculate_checksum(&data),
                compression: CompressionType::None,
            },
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };
        
        let value = bincode::serialize(&stored_record)?;
        self.tikv_txn.put(key, value).await?;
        self.affected_models.insert(DataModel::Relational);
        
        Ok(())
    }
    
    pub async fn put_vector(&mut self, collection: &str, vector_id: &str, data: &VectorRecord) -> Result<(), TransactionError> {
        let key = self.key_builder().vector_key(collection, vector_id);
        let stored_record = StoredRecord {
            model_type: DataModel::Vector,
            schema_version: 1,
            data: bincode::serialize(data)?,
            metadata: RecordMetadata {
                actor_id: ActorId::from_vector(collection, vector_id),
                transaction_id: Some(self.transaction_id),
                version: 1,
                checksum: calculate_checksum(&data),
                compression: CompressionType::None,
            },
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };
        
        let value = bincode::serialize(&stored_record)?;
        self.tikv_txn.put(key, value).await?;
        self.affected_models.insert(DataModel::Vector);
        
        Ok(())
    }
    
    pub async fn create_graph_edge(&mut self, graph: &str, from_id: &str, relation: &str, to_id: &str, properties: serde_json::Value) -> Result<(), TransactionError> {
        // 1. Create edge record
        let edge_key = self.key_builder().edge_key(graph, from_id, relation, to_id);
        let edge_record = GraphEdgeRecord {
            graph: graph.to_string(),
            from_id: from_id.to_string(),
            to_id: to_id.to_string(),
            relation: relation.to_string(),
            properties,
            weight: None,
        };
        
        let stored_edge = StoredRecord {
            model_type: DataModel::Graph,
            schema_version: 1,
            data: bincode::serialize(&edge_record)?,
            metadata: RecordMetadata {
                actor_id: ActorId::from_graph_edge(graph, from_id, to_id),
                transaction_id: Some(self.transaction_id),
                version: 1,
                checksum: calculate_checksum(&edge_record),
                compression: CompressionType::None,
            },
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };
        
        self.tikv_txn.put(edge_key, bincode::serialize(&stored_edge)?).await?;
        
        // 2. Update adjacency lists
        self.update_adjacency_list(graph, from_id, Direction::Outbound, to_id, relation).await?;
        self.update_adjacency_list(graph, to_id, Direction::Inbound, from_id, relation).await?;
        
        self.affected_models.insert(DataModel::Graph);
        Ok(())
    }
    
    pub async fn commit(self) -> Result<(), TransactionError> {
        // Check timeout
        if self.start_time.elapsed() > self.timeout {
            return Err(TransactionError::Timeout);
        }
        
        // Validate cross-model constraints
        self.validate_cross_model_constraints().await?;
        
        // Commit TiKV transaction
        self.tikv_txn.commit().await?;
        
        // Notify affected actors
        self.notify_affected_actors().await?;
        
        Ok(())
    }
    
    pub async fn rollback(self) -> Result<(), TransactionError> {
        self.tikv_txn.rollback().await?;
        Ok(())
    }
    
    async fn validate_cross_model_constraints(&self) -> Result<(), TransactionError> {
        // Validate foreign key constraints across models
        // Validate vector dimension consistency
        // Validate graph relationship integrity
        // etc.
        Ok(())
    }
    
    async fn notify_affected_actors(&self) -> Result<(), TransactionError> {
        for actor_id in &self.affected_actors {
            // Send notification to actor about committed changes
            ActorSystem::notify_transaction_commit(*actor_id, self.transaction_id).await?;
        }
        Ok(())
    }
    
    fn key_builder(&self) -> &MultiModelKeyBuilder {
        &GLOBAL_KEY_BUILDER
    }
}
```

### Distributed Transaction Coordination

```rust
pub struct DistributedTransactionCoordinator {
    client: TiKVClient,
    region_manager: RegionManager,
    transaction_registry: Arc<RwLock<HashMap<TransactionId, TransactionState>>>,
}

#[derive(Debug, Clone)]
pub struct TransactionState {
    pub transaction_id: TransactionId,
    pub coordinator_region: RegionId,
    pub participant_regions: HashSet<RegionId>,
    pub state: TxnState,
    pub created_at: Instant,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TxnState {
    Active,
    Preparing,
    Prepared,
    Committing,
    Committed,
    Aborted,
}

impl DistributedTransactionCoordinator {
    pub async fn begin_distributed_transaction(&self, regions: Vec<RegionId>) -> Result<TransactionId, TransactionError> {
        let transaction_id = TransactionId::new();
        let coordinator_region = regions[0]; // First region is coordinator
        let participant_regions = regions.into_iter().collect();
        
        let state = TransactionState {
            transaction_id,
            coordinator_region,
            participant_regions: participant_regions.clone(),
            state: TxnState::Active,
            created_at: Instant::now(),
            timeout: Duration::from_secs(30),
        };
        
        // Register transaction
        self.transaction_registry.write().await.insert(transaction_id, state.clone());
        
        // Send begin transaction to all participants
        for region_id in participant_regions {
            self.send_begin_transaction(region_id, transaction_id).await?;
        }
        
        Ok(transaction_id)
    }
    
    pub async fn prepare_transaction(&self, transaction_id: TransactionId) -> Result<(), TransactionError> {
        let mut registry = self.transaction_registry.write().await;
        let state = registry.get_mut(&transaction_id).ok_or(TransactionError::NotFound)?;
        
        if state.state != TxnState::Active {
            return Err(TransactionError::InvalidState);
        }
        
        state.state = TxnState::Preparing;
        drop(registry);
        
        // Phase 1: Prepare
        let mut prepare_results = Vec::new();
        for region_id in &state.participant_regions {
            let result = self.send_prepare(region_id, transaction_id).await;
            prepare_results.push(result);
        }
        
        // Check if all participants voted to commit
        let all_prepared = prepare_results.iter().all(|r| r.is_ok());
        
        let mut registry = self.transaction_registry.write().await;
        let state = registry.get_mut(&transaction_id).unwrap();
        
        if all_prepared {
            state.state = TxnState::Prepared;
            Ok(())
        } else {
            state.state = TxnState::Aborted;
            Err(TransactionError::PrepareFailed)
        }
    }
    
    pub async fn commit_transaction(&self, transaction_id: TransactionId) -> Result<(), TransactionError> {
        let mut registry = self.transaction_registry.write().await;
        let state = registry.get_mut(&transaction_id).ok_or(TransactionError::NotFound)?;
        
        if state.state != TxnState::Prepared {
            return Err(TransactionError::InvalidState);
        }
        
        state.state = TxnState::Committing;
        drop(registry);
        
        // Phase 2: Commit
        for region_id in &state.participant_regions {
            self.send_commit(region_id, transaction_id).await?;
        }
        
        let mut registry = self.transaction_registry.write().await;
        let state = registry.get_mut(&transaction_id).unwrap();
        state.state = TxnState::Committed;
        
        Ok(())
    }
    
    async fn send_begin_transaction(&self, region_id: RegionId, transaction_id: TransactionId) -> Result<(), TransactionError> {
        // Send begin transaction message to region
        Ok(())
    }
    
    async fn send_prepare(&self, region_id: &RegionId, transaction_id: TransactionId) -> Result<(), TransactionError> {
        // Send prepare message to region
        Ok(())
    }
    
    async fn send_commit(&self, region_id: &RegionId, transaction_id: TransactionId) -> Result<(), TransactionError> {
        // Send commit message to region
        Ok(())
    }
}
```

## Performance Optimization

### Query Optimization Strategies

```rust
pub struct TiKVQueryOptimizer {
    statistics: StatisticsManager,
    cost_model: CostModel,
    region_manager: RegionManager,
}

impl TiKVQueryOptimizer {
    pub async fn optimize_multi_model_query(&self, query: &MultiModelQuery) -> Result<OptimizedQuery, OptimizerError> {
        let mut optimized = OptimizedQuery::new();
        
        // 1. Analyze query patterns
        let patterns = self.analyze_query_patterns(query)?;
        
        // 2. Determine optimal execution strategy
        let strategy = self.determine_execution_strategy(&patterns).await?;
        
        // 3. Generate region-aware execution plan
        let execution_plan = self.generate_execution_plan(query, &strategy).await?;
        
        // 4. Apply TiKV-specific optimizations
        let tikv_optimized = self.apply_tikv_optimizations(execution_plan).await?;
        
        optimized.execution_plan = tikv_optimized;
        optimized.estimated_cost = self.calculate_execution_cost(&optimized.execution_plan).await?;
        
        Ok(optimized)
    }
    
    async fn determine_execution_strategy(&self, patterns: &[QueryPattern]) -> Result<ExecutionStrategy, OptimizerError> {
        let mut strategy = ExecutionStrategy::default();
        
        for pattern in patterns {
            match pattern {
                QueryPattern::PointLookup { model, key } => {
                    strategy.add_point_lookup(model, key);
                },
                QueryPattern::RangeScan { model, start_key, end_key } => {
                    let region_splits = self.region_manager.get_region_splits(start_key, end_key).await?;
                    strategy.add_distributed_scan(model, region_splits);
                },
                QueryPattern::Join { left_model, right_model, join_type } => {
                    let join_strategy = self.choose_join_strategy(left_model, right_model, join_type).await?;
                    strategy.add_join(join_strategy);
                },
                QueryPattern::Aggregation { model, aggregation_type } => {
                    strategy.add_pushdown_aggregation(model, aggregation_type);
                },
            }
        }
        
        Ok(strategy)
    }
    
    async fn apply_tikv_optimizations(&self, plan: ExecutionPlan) -> Result<ExecutionPlan, OptimizerError> {
        let mut optimized_plan = plan;
        
        // 1. Coprocessor pushdown
        optimized_plan = self.apply_coprocessor_pushdown(optimized_plan).await?;
        
        // 2. Batch operations
        optimized_plan = self.batch_operations(optimized_plan).await?;
        
        // 3. Region-aware parallelization
        optimized_plan = self.parallelize_by_region(optimized_plan).await?;
        
        // 4. Index utilization
        optimized_plan = self.optimize_index_usage(optimized_plan).await?;
        
        Ok(optimized_plan)
    }
    
    async fn apply_coprocessor_pushdown(&self, plan: ExecutionPlan) -> Result<ExecutionPlan, OptimizerError> {
        // Identify operations that can be pushed to TiKV coprocessors
        // Transform plan to use coprocessor requests
        Ok(plan)
    }
    
    async fn parallelize_by_region(&self, plan: ExecutionPlan) -> Result<ExecutionPlan, OptimizerError> {
        // Split operations across regions for parallel execution
        // Generate region-specific sub-plans
        Ok(plan)
    }
}
```

### Caching Strategies

```rust
pub struct TiKVCacheManager {
    region_cache: Arc<LruCache<RegionId, RegionInfo>>,
    schema_cache: Arc<LruCache<String, SchemaInfo>>,
    query_result_cache: Arc<LruCache<QueryHash, QueryResult>>,
    statistics_cache: Arc<LruCache<String, TableStatistics>>,
}

impl TiKVCacheManager {
    pub async fn get_cached_query_result(&self, query_hash: QueryHash) -> Option<QueryResult> {
        self.query_result_cache.get(&query_hash).map(|r| r.clone())
    }
    
    pub async fn cache_query_result(&self, query_hash: QueryHash, result: QueryResult, ttl: Duration) {
        let cached_result = CachedQueryResult {
            result,
            created_at: Instant::now(),
            ttl,
        };
        self.query_result_cache.put(query_hash, cached_result.result);
    }
    
    pub async fn invalidate_region_cache(&self, region_id: RegionId) {
        self.region_cache.pop(&region_id);
        
        // Invalidate related query results
        self.invalidate_region_related_queries(region_id).await;
    }
    
    pub async fn warm_up_caches(&self) -> Result<(), CacheError> {
        // Pre-load frequently accessed schemas
        self.preload_schemas().await?;
        
        // Pre-load region information
        self.preload_regions().await?;
        
        // Pre-load table statistics
        self.preload_statistics().await?;
        
        Ok(())
    }
}
```

## Implementation Roadmap

### Phase 1: Foundation (Months 1-2)

#### Month 1: Storage Provider Interface
**Objectives:**
- Design unified storage provider trait
- Implement TiKV storage provider
- Create storage manager for provider selection

**Deliverables:**
- `StorageProvider` trait definition
- `TiKVProvider` implementation
- `RocksDBProvider` implementation
- `StorageManager` with provider selection

**Technical Tasks:**
```rust
// Week 1-2: Provider Interface Design
- Define StorageProvider trait
- Implement provider capability detection
- Create provider selection strategies
- Basic configuration management

// Week 3-4: TiKV Provider Implementation
- Implement TiKVProvider struct
- Add tikv-client integration
- Provider registration and lifecycle
- Actor-to-provider assignment logic
```

#### Month 2: Multi-Model Storage
**Objectives:**
- Implement multi-model key design patterns
- Create storage format specifications
- Build data serialization/deserialization

**Deliverables:**
- `MultiModelKeyBuilder` implementation
- Storage format definitions
- Serialization benchmarks
- Multi-model unit tests

**Technical Tasks:**
```rust
// Week 5-6: Key Design
- Implement key building patterns for all data models
- Create namespace separation strategies
- Optimize key distribution for performance
- Index key generation

// Week 7-8: Storage Formats
- Define storage record structures
- Implement compression strategies
- Create schema versioning system
- Metadata management
```

#### Month 3: Transaction Integration
**Objectives:**
- Integrate TiKV transactions with Orbit-RS
- Implement cross-model ACID guarantees
- Create transaction management layer

**Deliverables:**
- `OrbitTransaction` implementation
- Cross-model constraint validation
- Transaction timeout and retry logic
- Performance benchmarks

**Technical Tasks:**
```rust
// Week 9-10: Transaction Layer
- Implement OrbitTransaction wrapper
- Cross-model operation support
- Isolation level management
- Deadlock detection and resolution

// Week 11-12: ACID Guarantees
- Cross-model constraint validation
- Referential integrity across models
- Consistency level configuration
- Transaction recovery mechanisms
```

### Phase 2: Optimization (Months 4-6)

#### Month 4: Query Optimization
**Objectives:**
- Build TiKV-specific query optimizer
- Implement coprocessor pushdown
- Create region-aware execution plans

**Deliverables:**
- `TiKVQueryOptimizer` implementation
- Coprocessor integration
- Execution plan generation
- Performance benchmarks

#### Month 5: Caching and Performance
**Objectives:**
- Implement multi-level caching
- Optimize data locality
- Build performance monitoring

**Deliverables:**
- `TiKVCacheManager` implementation
- Data locality optimizer
- Metrics collection system
- Performance dashboard

#### Month 6: Advanced Features
**Objectives:**
- Implement advanced TiKV features
- Add backup and recovery
- Create monitoring and alerting

**Deliverables:**
- Point-in-time recovery
- Distributed backup system
- Comprehensive monitoring
- Operational runbooks

### Phase 3: Production Readiness (Months 7-9)

#### Month 7: Scalability Testing
**Objectives:**
- Large-scale performance testing
- Auto-scaling implementation
- Load balancing optimization

**Deliverables:**
- Performance benchmarks at scale
- Auto-scaling algorithms
- Load testing results
- Capacity planning tools

#### Month 8: Operations and Monitoring
**Objectives:**
- Production monitoring system
- Alerting and incident response
- Operational automation

**Deliverables:**
- Comprehensive monitoring dashboard
- Alerting rules and runbooks
- Automated deployment scripts
- Disaster recovery procedures

#### Month 9: Documentation and Training
**Objectives:**
- Complete documentation
- Training materials
- Migration tools

**Deliverables:**
- Technical documentation
- Operational guides
- Training workshops
- Migration automation tools

### Implementation Milestones

| Milestone | Timeline | Success Criteria |
|-----------|----------|------------------|
| **Storage Provider MVP** | Month 2 | TiKV and RocksDB providers working with basic operations |
| **Provider Selection** | Month 4 | Intelligent provider selection based on actor patterns |
| **Production Alpha** | Month 6 | Successful deployment with multiple storage providers |
| **Performance Optimization** | Month 9 | Provider-specific optimizations delivering expected benefits |
| **General Availability** | Month 12 | Full production readiness with monitoring and migration tools |

## Operational Considerations

### Deployment Architecture

```yaml
# TiKV Cluster Configuration for Orbit-RS
version: "3.8"

services:
  # Placement Driver (PD) - Cluster metadata management
  pd1:
    image: pingcap/pd:latest
    ports:
      - "2379:2379"
      - "2380:2380"
    volumes:
      - pd1-data:/data
    environment:
      - PD_NAME=pd1
      - PD_CLIENT_URLS=http://0.0.0.0:2379
      - PD_PEER_URLS=http://0.0.0.0:2380
      - PD_INITIAL_CLUSTER=pd1=http://pd1:2380,pd2=http://pd2:2380,pd3=http://pd3:2380
    
  # TiKV Storage Nodes
  tikv1:
    image: pingcap/tikv:latest
    ports:
      - "20160:20160"
    volumes:
      - tikv1-data:/data
    environment:
      - TIKV_ADDR=0.0.0.0:20160
      - PD_ENDPOINTS=http://pd1:2379,http://pd2:2379,http://pd3:2379
    depends_on:
      - pd1
      - pd2
      - pd3
      
  # Orbit-RS Application Nodes
  orbit-rs-1:
    image: orbit-rs:tikv-integration
    ports:
      - "5432:5432"  # PostgreSQL protocol
      - "6379:6379"  # Redis protocol
      - "8080:8080"  # HTTP/REST
    environment:
      - ORBIT_TIKV_ENDPOINTS=http://pd1:2379,http://pd2:2379,http://pd3:2379
      - ORBIT_ACTOR_REGIONS=region1,region2,region3
      - ORBIT_CACHE_SIZE=1GB
    depends_on:
      - tikv1
      - tikv2
      - tikv3

volumes:
  pd1-data:
  pd2-data:
  pd3-data:
  tikv1-data:
  tikv2-data:
  tikv3-data:
```

### Monitoring and Alerting

```rust
pub struct TiKVMonitoringSystem {
    metrics_collector: MetricsCollector,
    alert_manager: AlertManager,
    dashboard: MonitoringDashboard,
}

impl TiKVMonitoringSystem {
    pub async fn collect_metrics(&self) -> Result<SystemMetrics, MonitoringError> {
        let tikv_metrics = self.collect_tikv_metrics().await?;
        let orbit_metrics = self.collect_orbit_metrics().await?;
        let integration_metrics = self.collect_integration_metrics().await?;
        
        Ok(SystemMetrics {
            tikv: tikv_metrics,
            orbit: orbit_metrics,
            integration: integration_metrics,
            timestamp: chrono::Utc::now(),
        })
    }
    
    async fn collect_tikv_metrics(&self) -> Result<TiKVMetrics, MonitoringError> {
        // Collect TiKV-specific metrics
        Ok(TiKVMetrics {
            region_count: self.get_region_count().await?,
            qps: self.get_queries_per_second().await?,
            latency_p99: self.get_latency_percentile(99.0).await?,
            storage_usage: self.get_storage_usage().await?,
            raft_log_lag: self.get_raft_log_lag().await?,
            coprocessor_usage: self.get_coprocessor_usage().await?,
        })
    }
    
    async fn collect_integration_metrics(&self) -> Result<IntegrationMetrics, MonitoringError> {
        Ok(IntegrationMetrics {
            actor_region_mappings: self.get_actor_region_mappings().await?,
            cross_model_transactions: self.get_cross_model_transaction_count().await?,
            cache_hit_ratio: self.get_cache_hit_ratio().await?,
            data_locality_score: self.get_data_locality_score().await?,
            migration_operations: self.get_migration_operations().await?,
        })
    }
}
```

### Backup and Recovery

```rust
pub struct TiKVBackupManager {
    backup_storage: BackupStorageConfig,
    tikv_client: TiKVClient,
    scheduler: BackupScheduler,
}

impl TiKVBackupManager {
    pub async fn create_full_backup(&self, backup_id: &str) -> Result<BackupInfo, BackupError> {
        // 1. Create consistent snapshot across all regions
        let snapshot_ts = self.tikv_client.get_current_timestamp().await?;
        
        // 2. Backup each region to storage
        let regions = self.tikv_client.get_all_regions().await?;
        let mut backup_tasks = Vec::new();
        
        for region in regions {
            let task = self.backup_region(region.id, snapshot_ts, backup_id);
            backup_tasks.push(task);
        }
        
        // 3. Wait for all region backups to complete
        let backup_results = futures::future::join_all(backup_tasks).await;
        
        // 4. Create backup metadata
        let backup_info = BackupInfo {
            backup_id: backup_id.to_string(),
            snapshot_timestamp: snapshot_ts,
            regions: backup_results.into_iter().collect::<Result<Vec<_>, _>>()?,
            created_at: chrono::Utc::now(),
            backup_type: BackupType::Full,
            size_bytes: self.calculate_backup_size(backup_id).await?,
        };
        
        // 5. Store backup metadata
        self.store_backup_metadata(&backup_info).await?;
        
        Ok(backup_info)
    }
    
    pub async fn restore_from_backup(&self, backup_id: &str, target_timestamp: Option<u64>) -> Result<(), RestoreError> {
        // 1. Load backup metadata
        let backup_info = self.load_backup_metadata(backup_id).await?;
        
        // 2. Determine restore timestamp
        let restore_ts = target_timestamp.unwrap_or(backup_info.snapshot_timestamp);
        
        // 3. Restore each region
        for region_backup in backup_info.regions {
            self.restore_region(region_backup, restore_ts).await?;
        }
        
        // 4. Verify data consistency
        self.verify_restored_data(restore_ts).await?;
        
        Ok(())
    }
    
    pub async fn create_incremental_backup(&self, base_backup_id: &str, backup_id: &str) -> Result<BackupInfo, BackupError> {
        let base_backup = self.load_backup_metadata(base_backup_id).await?;
        let from_ts = base_backup.snapshot_timestamp;
        let to_ts = self.tikv_client.get_current_timestamp().await?;
        
        // Backup only changes since base backup
        let regions = self.tikv_client.get_all_regions().await?;
        let mut backup_tasks = Vec::new();
        
        for region in regions {
            let task = self.backup_region_incremental(region.id, from_ts, to_ts, backup_id);
            backup_tasks.push(task);
        }
        
        let backup_results = futures::future::join_all(backup_tasks).await;
        
        let backup_info = BackupInfo {
            backup_id: backup_id.to_string(),
            snapshot_timestamp: to_ts,
            regions: backup_results.into_iter().collect::<Result<Vec<_>, _>>()?,
            created_at: chrono::Utc::now(),
            backup_type: BackupType::Incremental { base_backup: base_backup_id.to_string() },
            size_bytes: self.calculate_backup_size(backup_id).await?,
        };
        
        self.store_backup_metadata(&backup_info).await?;
        
        Ok(backup_info)
    }
}
```

## Migration Strategy

### From Existing Storage Backends

```rust
pub struct StorageBackendMigrator {
    source_backend: Box<dyn StorageBackend>,
    tikv_backend: TiKVStorageBackend,
    migration_config: MigrationConfig,
    progress_tracker: MigrationProgressTracker,
}

impl StorageBackendMigrator {
    pub async fn migrate_to_tikv(&self) -> Result<MigrationReport, MigrationError> {
        let migration_id = MigrationId::new();
        self.progress_tracker.start_migration(migration_id).await?;
        
        // Phase 1: Schema Migration
        self.migrate_schemas().await?;
        self.progress_tracker.complete_phase(migration_id, MigrationPhase::Schema).await?;
        
        // Phase 2: Data Migration
        let data_report = self.migrate_data().await?;
        self.progress_tracker.complete_phase(migration_id, MigrationPhase::Data).await?;
        
        // Phase 3: Index Migration
        self.migrate_indexes().await?;
        self.progress_tracker.complete_phase(migration_id, MigrationPhase::Indexes).await?;
        
        // Phase 4: Validation
        let validation_report = self.validate_migration().await?;
        self.progress_tracker.complete_phase(migration_id, MigrationPhase::Validation).await?;
        
        let migration_report = MigrationReport {
            migration_id,
            data_migrated: data_report.rows_migrated,
            validation_results: validation_report,
            duration: self.progress_tracker.get_duration(migration_id).await?,
            success: validation_report.all_checks_passed,
        };
        
        self.progress_tracker.complete_migration(migration_id, &migration_report).await?;
        
        Ok(migration_report)
    }
    
    async fn migrate_data(&self) -> Result<DataMigrationReport, MigrationError> {
        let mut report = DataMigrationReport::new();
        
        // Migrate each data model separately
        report.relational = self.migrate_relational_data().await?;
        report.document = self.migrate_document_data().await?;
        report.graph = self.migrate_graph_data().await?;
        report.vector = self.migrate_vector_data().await?;
        report.timeseries = self.migrate_timeseries_data().await?;
        
        Ok(report)
    }
    
    async fn migrate_relational_data(&self) -> Result<ModelMigrationReport, MigrationError> {
        let tables = self.source_backend.list_tables().await?;
        let mut report = ModelMigrationReport::new();
        
        for table in tables {
            let start_time = Instant::now();
            
            // Stream data from source to target
            let mut stream = self.source_backend.stream_table_data(&table.name).await?;
            let mut batch = Vec::new();
            let mut row_count = 0;
            
            while let Some(row) = stream.next().await {
                let row = row?;
                batch.push(row);
                
                if batch.len() >= self.migration_config.batch_size {
                    self.tikv_backend.batch_insert_relational(&table.name, &batch).await?;
                    row_count += batch.len();
                    batch.clear();
                    
                    // Update progress
                    self.progress_tracker.update_progress(
                        MigrationPhase::Data, 
                        row_count as f64 / table.estimated_rows as f64
                    ).await?;
                }
            }
            
            // Insert remaining rows
            if !batch.is_empty() {
                self.tikv_backend.batch_insert_relational(&table.name, &batch).await?;
                row_count += batch.len();
            }
            
            report.tables_migrated += 1;
            report.rows_migrated += row_count;
            report.duration += start_time.elapsed();
        }
        
        Ok(report)
    }
    
    async fn validate_migration(&self) -> Result<ValidationReport, MigrationError> {
        let mut report = ValidationReport::new();
        
        // Row count validation
        report.row_count_check = self.validate_row_counts().await?;
        
        // Data integrity validation
        report.integrity_check = self.validate_data_integrity().await?;
        
        // Performance validation
        report.performance_check = self.validate_performance().await?;
        
        // Cross-model constraint validation
        report.constraint_check = self.validate_cross_model_constraints().await?;
        
        report.all_checks_passed = report.row_count_check.passed 
            && report.integrity_check.passed
            && report.performance_check.passed
            && report.constraint_check.passed;
        
        Ok(report)
    }
}
```

### Gradual Rollout Strategy

```rust
pub struct GradualRolloutManager {
    current_backend: Arc<dyn StorageBackend>,
    tikv_backend: Arc<TiKVStorageBackend>,
    rollout_config: RolloutConfig,
    traffic_router: TrafficRouter,
}

impl GradualRolloutManager {
    pub async fn start_rollout(&self) -> Result<RolloutId, RolloutError> {
        let rollout_id = RolloutId::new();
        
        // Phase 1: Read-only traffic (10%)
        self.traffic_router.route_read_traffic(0.1, &self.tikv_backend).await?;
        self.monitor_phase(rollout_id, RolloutPhase::ReadOnly10).await?;
        
        // Phase 2: Read-only traffic (50%)
        self.traffic_router.route_read_traffic(0.5, &self.tikv_backend).await?;
        self.monitor_phase(rollout_id, RolloutPhase::ReadOnly50).await?;
        
        // Phase 3: Read-only traffic (100%)
        self.traffic_router.route_read_traffic(1.0, &self.tikv_backend).await?;
        self.monitor_phase(rollout_id, RolloutPhase::ReadOnly100).await?;
        
        // Phase 4: Read-write traffic (10%)
        self.traffic_router.route_write_traffic(0.1, &self.tikv_backend).await?;
        self.monitor_phase(rollout_id, RolloutPhase::ReadWrite10).await?;
        
        // Phase 5: Read-write traffic (100%)
        self.traffic_router.route_write_traffic(1.0, &self.tikv_backend).await?;
        self.monitor_phase(rollout_id, RolloutPhase::ReadWrite100).await?;
        
        Ok(rollout_id)
    }
    
    async fn monitor_phase(&self, rollout_id: RolloutId, phase: RolloutPhase) -> Result<(), RolloutError> {
        let monitoring_duration = self.rollout_config.phase_duration;
        let start_time = Instant::now();
        
        while start_time.elapsed() < monitoring_duration {
            let health_check = self.perform_health_check().await?;
            
            if !health_check.is_healthy() {
                // Rollback on issues
                self.rollback_phase(rollout_id, phase).await?;
                return Err(RolloutError::HealthCheckFailed(health_check));
            }
            
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
        
        Ok(())
    }
    
    async fn rollback_phase(&self, rollout_id: RolloutId, phase: RolloutPhase) -> Result<(), RolloutError> {
        match phase {
            RolloutPhase::ReadOnly10 | RolloutPhase::ReadOnly50 | RolloutPhase::ReadOnly100 => {
                self.traffic_router.route_read_traffic(0.0, &self.tikv_backend).await?;
            },
            RolloutPhase::ReadWrite10 | RolloutPhase::ReadWrite100 => {
                self.traffic_router.route_write_traffic(0.0, &self.tikv_backend).await?;
                self.traffic_router.route_read_traffic(0.0, &self.tikv_backend).await?;
            },
        }
        
        Ok(())
    }
}
```

## Testing and Validation

### Integration Test Suite

```rust
#[cfg(test)]
mod tikv_integration_tests {
    use super::*;
    use testcontainers::*;
    
    struct TiKVTestCluster {
        pd_container: Container<clients::Cli, images::generic::GenericImage>,
        tikv_containers: Vec<Container<clients::Cli, images::generic::GenericImage>>,
        client: TiKVClient,
    }
    
    impl TiKVTestCluster {
        async fn new() -> Result<Self, TestError> {
            let docker = clients::Cli::default();
            
            // Start PD node
            let pd_container = docker.run(
                images::generic::GenericImage::new("pingcap/pd", "latest")
                    .with_exposed_port(2379)
                    .with_exposed_port(2380)
            );
            
            let pd_endpoint = format!("127.0.0.1:{}", pd_container.get_host_port(2379));
            
            // Start TiKV nodes
            let mut tikv_containers = Vec::new();
            for i in 0..3 {
                let tikv_container = docker.run(
                    images::generic::GenericImage::new("pingcap/tikv", "latest")
                        .with_exposed_port(20160 + i)
                        .with_env_var("PD_ENDPOINTS", &pd_endpoint)
                );
                tikv_containers.push(tikv_container);
            }
            
            // Wait for cluster to be ready
            tokio::time::sleep(Duration::from_secs(10)).await;
            
            // Create client
            let client = TiKVClient::new(vec![pd_endpoint]).await?;
            
            Ok(TiKVTestCluster {
                pd_container,
                tikv_containers,
                client,
            })
        }
    }
    
    #[tokio::test]
    async fn test_basic_operations() -> Result<(), TestError> {
        let cluster = TiKVTestCluster::new().await?;
        let backend = TiKVStorageBackend::new_with_client(cluster.client).await?;
        
        // Test basic put/get
        let key = b"test_key".to_vec().into();
        let value = b"test_value".to_vec();
        
        backend.put(key.clone(), value.clone()).await?;
        let retrieved = backend.get(&key).await?;
        
        assert_eq!(retrieved, Some(value));
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_multi_model_storage() -> Result<(), TestError> {
        let cluster = TiKVTestCluster::new().await?;
        let backend = TiKVStorageBackend::new_with_client(cluster.client).await?;
        
        // Test relational data
        let table_data = RelationalRecord {
            table_name: "users".to_string(),
            row_id: "123".to_string(),
            columns: {
                let mut cols = HashMap::new();
                cols.insert("name".to_string(), ColumnValue::String("John".to_string()));
                cols.insert("age".to_string(), ColumnValue::Int64(30));
                cols
            },
            schema_hash: 12345,
        };
        
        let key = MultiModelKeyBuilder::new().table_row_key("users", "123");
        let stored_record = StoredRecord {
            model_type: DataModel::Relational,
            schema_version: 1,
            data: bincode::serialize(&table_data)?,
            metadata: RecordMetadata {
                actor_id: ActorId::from_table_row("users", "123"),
                transaction_id: None,
                version: 1,
                checksum: calculate_checksum(&table_data),
                compression: CompressionType::None,
            },
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
        };
        
        backend.put(key.clone(), bincode::serialize(&stored_record)?).await?;
        
        let retrieved = backend.get(&key).await?;
        assert!(retrieved.is_some());
        
        let decoded: StoredRecord = bincode::deserialize(&retrieved.unwrap())?;
        assert_eq!(decoded.model_type, DataModel::Relational);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_cross_model_transaction() -> Result<(), TestError> {
        let cluster = TiKVTestCluster::new().await?;
        let backend = TiKVStorageBackend::new_with_client(cluster.client).await?;
        
        let mut txn = OrbitTransaction::begin(&cluster.client, IsolationLevel::ReadCommitted).await?;
        
        // Insert relational data
        let user_data = RelationalRecord {
            table_name: "users".to_string(),
            row_id: "456".to_string(),
            columns: {
                let mut cols = HashMap::new();
                cols.insert("name".to_string(), ColumnValue::String("Jane".to_string()));
                cols.insert("age".to_string(), ColumnValue::Int64(25));
                cols
            },
            schema_hash: 12345,
        };
        
        txn.put_relational("users", "456", &user_data).await?;
        
        // Insert vector data for the same user
        let vector_data = VectorRecord {
            collection: "user_embeddings".to_string(),
            vector_id: "456".to_string(),
            dimensions: 128,
            data: vec![0.1; 128],
            metadata: serde_json::json!({"user_id": "456", "model": "text-embedding-ada-002"}),
            index_hints: vec![],
        };
        
        txn.put_vector("user_embeddings", "456", &vector_data).await?;
        
        // Create graph relationship
        txn.create_graph_edge(
            "social_network",
            "456",
            "follows",
            "123",
            serde_json::json!({"created_at": chrono::Utc::now().timestamp()})
        ).await?;
        
        // Commit transaction
        txn.commit().await?;
        
        // Verify all data is present
        let user_key = MultiModelKeyBuilder::new().table_row_key("users", "456");
        let vector_key = MultiModelKeyBuilder::new().vector_key("user_embeddings", "456");
        let edge_key = MultiModelKeyBuilder::new().edge_key("social_network", "456", "follows", "123");
        
        let user_exists = backend.get(&user_key).await?.is_some();
        let vector_exists = backend.get(&vector_key).await?.is_some();
        let edge_exists = backend.get(&edge_key).await?.is_some();
        
        assert!(user_exists && vector_exists && edge_exists);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_actor_region_mapping() -> Result<(), TestError> {
        let cluster = TiKVTestCluster::new().await?;
        let backend = TiKVStorageBackend::new_with_client(cluster.client).await?;
        
        let region_manager = RegionManager::new(&cluster.client).await?;
        let mapper = ActorRegionMapper::new(region_manager)?;
        
        // Test consistent mapping
        let actor_id = ActorId::new("test_actor_123");
        let region1 = mapper.get_region(actor_id).await?;
        let region2 = mapper.get_region(actor_id).await?;
        
        assert_eq!(region1, region2, "Actor should always map to the same region");
        
        // Test different actors map to different regions (with high probability)
        let mut region_distribution = HashMap::new();
        for i in 0..100 {
            let actor_id = ActorId::new(&format!("test_actor_{}", i));
            let region = mapper.get_region(actor_id).await?;
            *region_distribution.entry(region).or_insert(0) += 1;
        }
        
        assert!(region_distribution.len() > 1, "Actors should distribute across multiple regions");
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_performance_benchmarks() -> Result<(), TestError> {
        let cluster = TiKVTestCluster::new().await?;
        let backend = TiKVStorageBackend::new_with_client(cluster.client).await?;
        
        const NUM_OPERATIONS: usize = 1000;
        
        // Benchmark writes
        let start = Instant::now();
        for i in 0..NUM_OPERATIONS {
            let key = format!("benchmark_key_{}", i).into_bytes().into();
            let value = format!("benchmark_value_{}", i).into_bytes();
            backend.put(key, value).await?;
        }
        let write_duration = start.elapsed();
        
        // Benchmark reads
        let start = Instant::now();
        for i in 0..NUM_OPERATIONS {
            let key = format!("benchmark_key_{}", i).into_bytes().into();
            backend.get(&key).await?;
        }
        let read_duration = start.elapsed();
        
        let writes_per_second = NUM_OPERATIONS as f64 / write_duration.as_secs_f64();
        let reads_per_second = NUM_OPERATIONS as f64 / read_duration.as_secs_f64();
        
        println!("TiKV Backend Performance:");
        println!("  Writes/second: {:.2}", writes_per_second);
        println!("  Reads/second: {:.2}", reads_per_second);
        
        // Assert minimum performance requirements
        assert!(writes_per_second > 100.0, "Write performance below minimum threshold");
        assert!(reads_per_second > 500.0, "Read performance below minimum threshold");
        
        Ok(())
    }
}
```

### Load Testing Framework

```rust
pub struct TiKVLoadTestFramework {
    backend: TiKVStorageBackend,
    test_config: LoadTestConfig,
    metrics_collector: MetricsCollector,
}

impl TiKVLoadTestFramework {
    pub async fn run_load_test(&self, test_name: &str) -> Result<LoadTestReport, LoadTestError> {
        println!("Starting load test: {}", test_name);
        
        let start_time = Instant::now();
        let mut test_report = LoadTestReport::new(test_name);
        
        // Spawn concurrent workers
        let mut handles = Vec::new();
        for worker_id in 0..self.test_config.worker_count {
            let backend = self.backend.clone();
            let config = self.test_config.clone();
            let collector = self.metrics_collector.clone();
            
            let handle = tokio::spawn(async move {
                Self::run_worker(worker_id, backend, config, collector).await
            });
            handles.push(handle);
        }
        
        // Wait for all workers to complete
        let worker_results = futures::future::join_all(handles).await;
        
        // Aggregate results
        for result in worker_results {
            let worker_report = result??;
            test_report.aggregate(worker_report);
        }
        
        test_report.duration = start_time.elapsed();
        test_report.calculate_statistics();
        
        println!("Load test completed: {}", test_name);
        println!("  Duration: {:?}", test_report.duration);
        println!("  Operations: {}", test_report.total_operations);
        println!("  Throughput: {:.2} ops/sec", test_report.throughput);
        println!("  Average Latency: {:?}", test_report.avg_latency);
        println!("  P99 Latency: {:?}", test_report.p99_latency);
        
        Ok(test_report)
    }
    
    async fn run_worker(
        worker_id: usize,
        backend: TiKVStorageBackend,
        config: LoadTestConfig,
        collector: MetricsCollector,
    ) -> Result<WorkerReport, LoadTestError> {
        let mut report = WorkerReport::new(worker_id);
        let operations_per_worker = config.total_operations / config.worker_count;
        
        for op_id in 0..operations_per_worker {
            let operation_start = Instant::now();
            
            match config.workload_pattern {
                WorkloadPattern::WriteHeavy => {
                    if op_id % 10 < 8 {  // 80% writes
                        Self::perform_write_operation(&backend, worker_id, op_id).await?;
                    } else {
                        Self::perform_read_operation(&backend, worker_id, op_id).await?;
                    }
                },
                WorkloadPattern::ReadHeavy => {
                    if op_id % 10 < 2 {  // 20% writes
                        Self::perform_write_operation(&backend, worker_id, op_id).await?;
                    } else {
                        Self::perform_read_operation(&backend, worker_id, op_id).await?;
                    }
                },
                WorkloadPattern::Mixed => {
                    if op_id % 2 == 0 {  // 50% writes
                        Self::perform_write_operation(&backend, worker_id, op_id).await?;
                    } else {
                        Self::perform_read_operation(&backend, worker_id, op_id).await?;
                    }
                },
            }
            
            let operation_duration = operation_start.elapsed();
            report.record_operation(operation_duration);
            
            collector.record_operation_latency(operation_duration).await;
        }
        
        Ok(report)
    }
}
```

This comprehensive TiKV integration specification provides a solid foundation for implementing TiKV as the persistence layer for Orbit-RS. The combination of TiKV's distributed ACID transactions with Orbit-RS's multi-model capabilities creates a powerful and scalable database platform.

The implementation roadmap spans 15 months with clear milestones and success criteria. The integration leverages both systems' strengths while maintaining Orbit-RS's unique multi-model, multi-protocol, and actor-native advantages.