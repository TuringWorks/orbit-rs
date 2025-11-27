//! Feature flag detection and runtime configuration.
//!
//! This module provides compile-time and runtime feature detection based on
//! Cargo feature flags. Use this to conditionally enable/disable functionality
//! based on the build configuration.

use std::collections::HashSet;

/// Feature detection and configuration system.
#[derive(Debug, Clone)]
pub struct Features {
    enabled_features: HashSet<String>,
}

impl Features {
    /// Create a new Features instance with compile-time detected features.
    pub fn new() -> Self {
        let mut enabled_features = HashSet::new();

        // Core System Features
        #[cfg(feature = "actor-system")]
        enabled_features.insert("actor-system".to_string());

        #[cfg(feature = "clustering")]
        enabled_features.insert("clustering".to_string());

        #[cfg(feature = "persistence")]
        enabled_features.insert("persistence".to_string());

        #[cfg(feature = "networking")]
        enabled_features.insert("networking".to_string());

        #[cfg(feature = "metrics")]
        enabled_features.insert("metrics".to_string());

        // Protocol Support
        #[cfg(feature = "protocol-redis")]
        enabled_features.insert("protocol-redis".to_string());

        #[cfg(feature = "protocol-postgres")]
        enabled_features.insert("protocol-postgres".to_string());

        #[cfg(feature = "protocol-mysql")]
        enabled_features.insert("protocol-mysql".to_string());

        #[cfg(feature = "protocol-cassandra")]
        enabled_features.insert("protocol-cassandra".to_string());

        #[cfg(feature = "protocol-neo4j")]
        enabled_features.insert("protocol-neo4j".to_string());

        #[cfg(feature = "protocol-arangodb")]
        enabled_features.insert("protocol-arangodb".to_string());

        #[cfg(feature = "protocol-grpc")]
        enabled_features.insert("protocol-grpc".to_string());

        #[cfg(feature = "protocol-rest")]
        enabled_features.insert("protocol-rest".to_string());

        #[cfg(feature = "protocol-mcp")]
        enabled_features.insert("protocol-mcp".to_string());

        // Storage Engines
        #[cfg(feature = "storage-memory")]
        enabled_features.insert("storage-memory".to_string());

        #[cfg(feature = "storage-rocksdb")]
        enabled_features.insert("storage-rocksdb".to_string());

        #[cfg(feature = "storage-lsm")]
        enabled_features.insert("storage-lsm".to_string());

        #[cfg(feature = "storage-btree")]
        enabled_features.insert("storage-btree".to_string());

        #[cfg(feature = "storage-iceberg")]
        enabled_features.insert("storage-iceberg".to_string());

        #[cfg(feature = "storage-cloud")]
        enabled_features.insert("storage-cloud".to_string());

        // GPU/Heterogeneous Compute Features
        #[cfg(feature = "gpu-acceleration")]
        enabled_features.insert("gpu-acceleration".to_string());

        #[cfg(feature = "gpu-graph-traversal")]
        enabled_features.insert("gpu-graph-traversal".to_string());

        // AI-Native Features
        #[cfg(feature = "ai-native")]
        enabled_features.insert("ai-native".to_string());

        #[cfg(feature = "ai-query-optimizer")]
        enabled_features.insert("ai-query-optimizer".to_string());

        #[cfg(feature = "ai-resource-manager")]
        enabled_features.insert("ai-resource-manager".to_string());

        #[cfg(feature = "ai-storage-manager")]
        enabled_features.insert("ai-storage-manager".to_string());

        #[cfg(feature = "ai-transaction-manager")]
        enabled_features.insert("ai-transaction-manager".to_string());

        #[cfg(feature = "ai-learning-engine")]
        enabled_features.insert("ai-learning-engine".to_string());

        #[cfg(feature = "ai-decision-engine")]
        enabled_features.insert("ai-decision-engine".to_string());

        #[cfg(feature = "ai-knowledge-base")]
        enabled_features.insert("ai-knowledge-base".to_string());

        // Data Model Support
        #[cfg(feature = "model-relational")]
        enabled_features.insert("model-relational".to_string());

        #[cfg(feature = "model-document")]
        enabled_features.insert("model-document".to_string());

        #[cfg(feature = "model-kv")]
        enabled_features.insert("model-kv".to_string());

        #[cfg(feature = "model-graph")]
        enabled_features.insert("model-graph".to_string());

        #[cfg(feature = "model-timeseries")]
        enabled_features.insert("model-timeseries".to_string());

        #[cfg(feature = "model-vector")]
        enabled_features.insert("model-vector".to_string());

        #[cfg(feature = "model-spatial")]
        enabled_features.insert("model-spatial".to_string());

        #[cfg(feature = "model-columnar")]
        enabled_features.insert("model-columnar".to_string());

        // Advanced Features
        #[cfg(feature = "transactions")]
        enabled_features.insert("transactions".to_string());

        #[cfg(feature = "distributed-tx")]
        enabled_features.insert("distributed-tx".to_string());

        #[cfg(feature = "sagas")]
        enabled_features.insert("sagas".to_string());

        #[cfg(feature = "cdc")]
        enabled_features.insert("cdc".to_string());

        #[cfg(feature = "streaming")]
        enabled_features.insert("streaming".to_string());

        #[cfg(feature = "replication")]
        enabled_features.insert("replication".to_string());

        #[cfg(feature = "sharding")]
        enabled_features.insert("sharding".to_string());

        #[cfg(feature = "compression")]
        enabled_features.insert("compression".to_string());

        #[cfg(feature = "encryption")]
        enabled_features.insert("encryption".to_string());

        // Integration Features
        #[cfg(feature = "kubernetes")]
        enabled_features.insert("kubernetes".to_string());

        #[cfg(feature = "prometheus")]
        enabled_features.insert("prometheus".to_string());

        #[cfg(feature = "etcd")]
        enabled_features.insert("etcd".to_string());

        #[cfg(feature = "graphrag")]
        enabled_features.insert("graphrag".to_string());

        #[cfg(feature = "neural-engine")]
        enabled_features.insert("neural-engine".to_string());

        #[cfg(feature = "heterogeneous-compute")]
        enabled_features.insert("heterogeneous-compute".to_string());

        // Query Languages
        #[cfg(feature = "query-sql")]
        enabled_features.insert("query-sql".to_string());

        #[cfg(feature = "query-cypher")]
        enabled_features.insert("query-cypher".to_string());

        #[cfg(feature = "query-aql")]
        enabled_features.insert("query-aql".to_string());

        #[cfg(feature = "query-orbitql")]
        enabled_features.insert("query-orbitql".to_string());

        #[cfg(feature = "query-redis")]
        enabled_features.insert("query-redis".to_string());

        #[cfg(feature = "query-natural")]
        enabled_features.insert("query-natural".to_string());

        // Development & Testing
        #[cfg(feature = "benchmarks")]
        enabled_features.insert("benchmarks".to_string());

        #[cfg(feature = "testing")]
        enabled_features.insert("testing".to_string());

        #[cfg(feature = "tracing")]
        enabled_features.insert("tracing".to_string());

        #[cfg(feature = "debug-tools")]
        enabled_features.insert("debug-tools".to_string());

        Self { enabled_features }
    }

    /// Check if a specific feature is enabled.
    pub fn is_enabled(&self, feature: &str) -> bool {
        self.enabled_features.contains(feature)
    }

    /// Get all enabled features.
    pub fn enabled(&self) -> Vec<String> {
        let mut features: Vec<String> = self.enabled_features.iter().cloned().collect();
        features.sort();
        features
    }

    /// Get the count of enabled features.
    pub fn count(&self) -> usize {
        self.enabled_features.len()
    }

    /// Check if actor system is enabled.
    #[inline]
    pub fn has_actor_system(&self) -> bool {
        cfg!(feature = "actor-system")
    }

    /// Check if clustering is enabled.
    #[inline]
    pub fn has_clustering(&self) -> bool {
        cfg!(feature = "clustering")
    }

    /// Check if persistence is enabled.
    #[inline]
    pub fn has_persistence(&self) -> bool {
        cfg!(feature = "persistence")
    }

    /// Check if AI-native features are enabled.
    #[inline]
    pub fn has_ai_native(&self) -> bool {
        cfg!(feature = "ai-native")
    }

    /// Check if Redis protocol is enabled.
    #[inline]
    pub fn has_redis(&self) -> bool {
        cfg!(feature = "protocol-redis")
    }

    /// Check if PostgreSQL protocol is enabled.
    #[inline]
    pub fn has_postgres(&self) -> bool {
        cfg!(feature = "protocol-postgres")
    }

    /// Check if transactions are enabled.
    #[inline]
    pub fn has_transactions(&self) -> bool {
        cfg!(feature = "transactions")
    }

    /// Check if distributed transactions are enabled.
    #[inline]
    pub fn has_distributed_tx(&self) -> bool {
        cfg!(feature = "distributed-tx")
    }

    /// Check if GraphRAG is enabled.
    #[inline]
    pub fn has_graphrag(&self) -> bool {
        cfg!(feature = "graphrag")
    }

    /// Check if GPU acceleration is enabled.
    #[inline]
    pub fn has_gpu_acceleration(&self) -> bool {
        cfg!(feature = "gpu-acceleration")
    }

    /// Check if GPU graph traversal is enabled.
    #[inline]
    pub fn has_gpu_graph_traversal(&self) -> bool {
        cfg!(feature = "gpu-graph-traversal")
    }

    /// Check if heterogeneous compute is enabled.
    #[inline]
    pub fn has_heterogeneous_compute(&self) -> bool {
        cfg!(feature = "heterogeneous-compute")
    }
}

impl Default for Features {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_features_creation() {
        let features = Features::new();
        assert!(features.count() > 0);
    }

    #[test]
    fn test_default_features_enabled() {
        let features = Features::new();

        // These should be enabled by default
        assert!(features.has_actor_system());
        assert!(features.has_clustering());
        assert!(features.has_persistence());
        assert!(features.has_redis());
        assert!(features.has_postgres());
        assert!(features.has_transactions());
    }

    #[test]
    fn test_ai_features_enabled() {
        let features = Features::new();
        assert!(features.has_ai_native());
        assert!(features.is_enabled("ai-query-optimizer"));
        assert!(features.is_enabled("ai-resource-manager"));
    }

    #[test]
    fn test_enabled_features_list() {
        let features = Features::new();
        let enabled = features.enabled();

        // Should be sorted
        for i in 1..enabled.len() {
            assert!(enabled[i - 1] < enabled[i]);
        }

        // Should contain some default features
        assert!(enabled.contains(&"actor-system".to_string()));
    }
}
