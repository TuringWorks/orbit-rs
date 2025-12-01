//! Actor-Tier Placement Integration
//!
//! This module provides integration between the actor memory profile system
//! and the tiered storage backend, enabling automatic data placement based
//! on actor requirements.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────────────┐
//! │                        Actor Tier Placement                               │
//! ├──────────────────────────────────────────────────────────────────────────┤
//! │                                                                           │
//! │  ┌─────────────────────────────────────────────────────────────────────┐ │
//! │  │                        Actor Types                                   │ │
//! │  ├─────────────┬─────────────┬─────────────┬────────────┬──────────────┤ │
//! │  │   Table     │   Extent    │   Column    │    Row     │    Index     │ │
//! │  │   Actor     │   Actor     │   Actor     │   Actor    │    Actor     │ │
//! │  └─────────────┴─────────────┴─────────────┴────────────┴──────────────┘ │
//! │         │             │             │            │             │          │
//! │         ▼             ▼             ▼            ▼             ▼          │
//! │  ┌─────────────────────────────────────────────────────────────────────┐ │
//! │  │                   Actor Tier Placement Policy                        │ │
//! │  │  - Maps actor types to storage tiers                                │ │
//! │  │  - Supports custom placement rules                                   │ │
//! │  │  - Handles tier migrations based on access patterns                  │ │
//! │  └─────────────────────────────────────────────────────────────────────┘ │
//! │         │             │             │            │             │          │
//! │         ▼             ▼             ▼            ▼             ▼          │
//! │  ┌───────────┐  ┌───────────┐  ┌───────────┐                             │
//! │  │ HOT TIER  │  │ WARM TIER │  │ COLD TIER │                             │
//! │  │ (Memory)  │  │ (RocksDB) │  │  (Cloud)  │                             │
//! │  │           │  │           │  │           │                             │
//! │  │ Row Actor │  │ Table     │  │ Archival  │                             │
//! │  │ Field     │  │ Extent    │  │ Data      │                             │
//! │  │ Index     │  │ Column    │  │           │                             │
//! │  └───────────┘  └───────────┘  └───────────┘                             │
//! │                                                                           │
//! └──────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Default Actor-Tier Mapping
//!
//! | Actor Type | Default Tier | Reason |
//! |------------|--------------|--------|
//! | Row Actor | Hot | Frequently accessed, low latency |
//! | Field Actor | Hot | Point lookups, immediate response |
//! | Index Actor | Hot | Query performance critical |
//! | Column Actor | Warm | Moderate access, analytical |
//! | Extent Actor | Warm | Block-level storage |
//! | Table Actor | Warm | Metadata, schema info |
//! | Archival Actor | Cold | Historical data |

use super::storage::UnifiedStorageBackend;
use super::tiered::{StorageTier, TieredStorageBackend, TieredStorageConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Actor type classification for tier placement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActorType {
    /// Table-level actor: manages table metadata and schema
    Table,
    /// Extent-level actor: manages data blocks
    Extent,
    /// Column-level actor: manages columnar data
    Column,
    /// Row-level actor: manages individual rows
    Row,
    /// Field-level actor: manages individual fields
    Field,
    /// Index actor: manages secondary indexes
    Index,
    /// Aggregation actor: manages pre-computed aggregates
    Aggregation,
    /// Cache actor: manages query result cache
    Cache,
    /// Custom actor type with specified name
    Custom(u32),
}

impl std::fmt::Display for ActorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActorType::Table => write!(f, "table"),
            ActorType::Extent => write!(f, "extent"),
            ActorType::Column => write!(f, "column"),
            ActorType::Row => write!(f, "row"),
            ActorType::Field => write!(f, "field"),
            ActorType::Index => write!(f, "index"),
            ActorType::Aggregation => write!(f, "aggregation"),
            ActorType::Cache => write!(f, "cache"),
            ActorType::Custom(id) => write!(f, "custom_{}", id),
        }
    }
}

/// Configuration for actor tier placement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorTierPlacementConfig {
    /// Enable tier-aware placement
    pub enabled: bool,
    /// Default tier for actors without explicit mapping
    pub default_tier: StorageTier,
    /// Actor type to tier mappings
    pub actor_tier_mapping: HashMap<String, StorageTier>,
    /// Enable automatic promotion based on access patterns
    pub enable_auto_promotion: bool,
    /// Access count threshold for promotion
    pub promotion_threshold: u32,
    /// Enable automatic demotion based on idle time
    pub enable_auto_demotion: bool,
    /// Idle time threshold for demotion (seconds)
    pub demotion_idle_secs: u64,
}

impl Default for ActorTierPlacementConfig {
    fn default() -> Self {
        let mut actor_tier_mapping = HashMap::new();

        // Hot tier: Row, Field, Index, Cache actors (frequent access)
        actor_tier_mapping.insert("row".to_string(), StorageTier::Hot);
        actor_tier_mapping.insert("field".to_string(), StorageTier::Hot);
        actor_tier_mapping.insert("index".to_string(), StorageTier::Hot);
        actor_tier_mapping.insert("cache".to_string(), StorageTier::Hot);

        // Warm tier: Table, Extent, Column, Aggregation actors
        actor_tier_mapping.insert("table".to_string(), StorageTier::Warm);
        actor_tier_mapping.insert("extent".to_string(), StorageTier::Warm);
        actor_tier_mapping.insert("column".to_string(), StorageTier::Warm);
        actor_tier_mapping.insert("aggregation".to_string(), StorageTier::Warm);

        Self {
            enabled: true,
            default_tier: StorageTier::Warm,
            actor_tier_mapping,
            enable_auto_promotion: true,
            promotion_threshold: 10,
            enable_auto_demotion: true,
            demotion_idle_secs: 1800, // 30 minutes
        }
    }
}

/// Statistics for actor tier placement
#[derive(Debug, Clone, Default)]
pub struct ActorTierStats {
    /// Number of actors in hot tier
    pub hot_tier_actors: u64,
    /// Number of actors in warm tier
    pub warm_tier_actors: u64,
    /// Number of actors in cold tier
    pub cold_tier_actors: u64,
    /// Total promotions
    pub promotions: u64,
    /// Total demotions
    pub demotions: u64,
    /// Access count by actor type
    pub access_by_type: HashMap<String, u64>,
}

/// Tier recommendation for an actor
#[derive(Debug, Clone)]
pub struct TierRecommendation {
    /// Recommended storage tier
    pub tier: StorageTier,
    /// Reason for the recommendation
    pub reason: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
}

/// Actor tier placement manager
///
/// This component manages the placement of actor data across storage tiers
/// based on actor types, access patterns, and configuration.
pub struct ActorTierPlacement {
    /// Configuration
    config: ActorTierPlacementConfig,
    /// Underlying tiered storage backend
    storage: Arc<TieredStorageBackend>,
    /// Actor tier tracking (actor_id -> current_tier)
    actor_tiers: Arc<RwLock<HashMap<String, StorageTier>>>,
    /// Access tracking (actor_id -> access_count)
    access_counts: Arc<RwLock<HashMap<String, u64>>>,
    /// Statistics
    stats: Arc<RwLock<ActorTierStats>>,
}

impl ActorTierPlacement {
    /// Create a new actor tier placement manager
    pub fn new(
        config: ActorTierPlacementConfig,
        storage_config: TieredStorageConfig,
    ) -> Self {
        let storage = Arc::new(TieredStorageBackend::new(storage_config));

        info!(
            "[ActorTierPlacement] Initialized with {} tier mappings",
            config.actor_tier_mapping.len()
        );

        Self {
            config,
            storage,
            actor_tiers: Arc::new(RwLock::new(HashMap::new())),
            access_counts: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ActorTierStats::default())),
        }
    }

    /// Create with existing tiered storage backend
    pub fn with_storage(
        config: ActorTierPlacementConfig,
        storage: Arc<TieredStorageBackend>,
    ) -> Self {
        Self {
            config,
            storage,
            actor_tiers: Arc::new(RwLock::new(HashMap::new())),
            access_counts: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(ActorTierStats::default())),
        }
    }

    /// Get the underlying tiered storage backend
    pub fn storage(&self) -> Arc<TieredStorageBackend> {
        Arc::clone(&self.storage)
    }

    /// Initialize the placement manager
    pub async fn initialize(&self) -> Result<(), String> {
        self.storage.initialize().await.map_err(|e| e.to_string())?;
        info!("[ActorTierPlacement] Storage initialized");
        Ok(())
    }

    /// Shutdown the placement manager
    pub async fn shutdown(&self) -> Result<(), String> {
        self.storage.shutdown().await.map_err(|e| e.to_string())?;
        info!("[ActorTierPlacement] Storage shutdown");
        Ok(())
    }

    /// Get the recommended tier for an actor type
    pub fn get_tier_for_actor_type(&self, actor_type: ActorType) -> StorageTier {
        if !self.config.enabled {
            return self.config.default_tier;
        }

        let type_name = actor_type.to_string();
        self.config
            .actor_tier_mapping
            .get(&type_name)
            .copied()
            .unwrap_or(self.config.default_tier)
    }

    /// Get tier recommendation with reasoning
    pub fn recommend_tier(&self, actor_type: ActorType, access_count: u64) -> TierRecommendation {
        let base_tier = self.get_tier_for_actor_type(actor_type);

        // Check if access pattern suggests promotion
        if self.config.enable_auto_promotion && access_count >= self.config.promotion_threshold as u64 {
            match base_tier {
                StorageTier::Cold => {
                    return TierRecommendation {
                        tier: StorageTier::Warm,
                        reason: format!("High access count ({}) suggests promotion from cold to warm", access_count),
                        confidence: 0.8,
                    };
                }
                StorageTier::Warm => {
                    if access_count >= (self.config.promotion_threshold * 2) as u64 {
                        return TierRecommendation {
                            tier: StorageTier::Hot,
                            reason: format!("Very high access count ({}) suggests promotion to hot tier", access_count),
                            confidence: 0.9,
                        };
                    }
                }
                StorageTier::Hot => {
                    // Already in hot tier
                }
            }
        }

        TierRecommendation {
            tier: base_tier,
            reason: format!("Default tier for {} actor type", actor_type),
            confidence: 0.7,
        }
    }

    /// Register an actor with tier placement
    pub async fn register_actor(
        &self,
        actor_id: &str,
        actor_type: ActorType,
    ) -> Result<StorageTier, String> {
        let tier = self.get_tier_for_actor_type(actor_type);

        {
            let mut actor_tiers = self.actor_tiers.write().await;
            actor_tiers.insert(actor_id.to_string(), tier);
        }

        {
            let mut stats = self.stats.write().await;
            match tier {
                StorageTier::Hot => stats.hot_tier_actors += 1,
                StorageTier::Warm => stats.warm_tier_actors += 1,
                StorageTier::Cold => stats.cold_tier_actors += 1,
            }
        }

        debug!(
            "[ActorTierPlacement] Registered actor {} ({}) in {:?} tier",
            actor_id, actor_type, tier
        );

        Ok(tier)
    }

    /// Record an access to an actor
    pub async fn record_access(&self, actor_id: &str) {
        let mut access_counts = self.access_counts.write().await;
        let count = access_counts.entry(actor_id.to_string()).or_insert(0);
        *count += 1;
    }

    /// Get the current tier for an actor
    pub async fn get_actor_tier(&self, actor_id: &str) -> Option<StorageTier> {
        let actor_tiers = self.actor_tiers.read().await;
        actor_tiers.get(actor_id).copied()
    }

    /// Promote an actor to a higher tier
    pub async fn promote_actor(
        &self,
        actor_id: &str,
        target_tier: StorageTier,
    ) -> Result<(), String> {
        let current_tier = {
            let actor_tiers = self.actor_tiers.read().await;
            actor_tiers.get(actor_id).copied()
        };

        match current_tier {
            Some(current) if current == target_tier => {
                return Ok(()); // Already in target tier
            }
            Some(current) => {
                // Update tier mapping
                {
                    let mut actor_tiers = self.actor_tiers.write().await;
                    actor_tiers.insert(actor_id.to_string(), target_tier);
                }

                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    match current {
                        StorageTier::Hot => stats.hot_tier_actors = stats.hot_tier_actors.saturating_sub(1),
                        StorageTier::Warm => stats.warm_tier_actors = stats.warm_tier_actors.saturating_sub(1),
                        StorageTier::Cold => stats.cold_tier_actors = stats.cold_tier_actors.saturating_sub(1),
                    }
                    match target_tier {
                        StorageTier::Hot => stats.hot_tier_actors += 1,
                        StorageTier::Warm => stats.warm_tier_actors += 1,
                        StorageTier::Cold => stats.cold_tier_actors += 1,
                    }
                    stats.promotions += 1;
                }

                debug!(
                    "[ActorTierPlacement] Promoted actor {} from {:?} to {:?}",
                    actor_id, current, target_tier
                );
            }
            None => {
                return Err(format!("Actor {} not registered", actor_id));
            }
        }

        Ok(())
    }

    /// Demote an actor to a lower tier
    pub async fn demote_actor(
        &self,
        actor_id: &str,
        target_tier: StorageTier,
    ) -> Result<(), String> {
        let current_tier = {
            let actor_tiers = self.actor_tiers.read().await;
            actor_tiers.get(actor_id).copied()
        };

        match current_tier {
            Some(current) if current == target_tier => {
                return Ok(()); // Already in target tier
            }
            Some(current) => {
                // Update tier mapping
                {
                    let mut actor_tiers = self.actor_tiers.write().await;
                    actor_tiers.insert(actor_id.to_string(), target_tier);
                }

                // Update stats
                {
                    let mut stats = self.stats.write().await;
                    match current {
                        StorageTier::Hot => stats.hot_tier_actors = stats.hot_tier_actors.saturating_sub(1),
                        StorageTier::Warm => stats.warm_tier_actors = stats.warm_tier_actors.saturating_sub(1),
                        StorageTier::Cold => stats.cold_tier_actors = stats.cold_tier_actors.saturating_sub(1),
                    }
                    match target_tier {
                        StorageTier::Hot => stats.hot_tier_actors += 1,
                        StorageTier::Warm => stats.warm_tier_actors += 1,
                        StorageTier::Cold => stats.cold_tier_actors += 1,
                    }
                    stats.demotions += 1;
                }

                debug!(
                    "[ActorTierPlacement] Demoted actor {} from {:?} to {:?}",
                    actor_id, current, target_tier
                );
            }
            None => {
                return Err(format!("Actor {} not registered", actor_id));
            }
        }

        Ok(())
    }

    /// Get current placement statistics
    pub async fn stats(&self) -> ActorTierStats {
        self.stats.read().await.clone()
    }

    /// Get storage key prefix for an actor in its tier
    pub fn get_storage_key(&self, actor_id: &str, namespace: &str) -> String {
        format!("actor:{}:{}", namespace, actor_id)
    }
}

/// Builder for actor tier placement configuration
pub struct ActorTierPlacementBuilder {
    config: ActorTierPlacementConfig,
}

impl ActorTierPlacementBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: ActorTierPlacementConfig::default(),
        }
    }

    /// Enable or disable tier-aware placement
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    /// Set the default tier
    pub fn default_tier(mut self, tier: StorageTier) -> Self {
        self.config.default_tier = tier;
        self
    }

    /// Map an actor type to a specific tier
    pub fn map_actor_type(mut self, actor_type: ActorType, tier: StorageTier) -> Self {
        self.config
            .actor_tier_mapping
            .insert(actor_type.to_string(), tier);
        self
    }

    /// Enable automatic promotion
    pub fn auto_promotion(mut self, enabled: bool, threshold: u32) -> Self {
        self.config.enable_auto_promotion = enabled;
        self.config.promotion_threshold = threshold;
        self
    }

    /// Enable automatic demotion
    pub fn auto_demotion(mut self, enabled: bool, idle_secs: u64) -> Self {
        self.config.enable_auto_demotion = enabled;
        self.config.demotion_idle_secs = idle_secs;
        self
    }

    /// Build the configuration
    pub fn build(self) -> ActorTierPlacementConfig {
        self.config
    }
}

impl Default for ActorTierPlacementBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_type_to_string() {
        assert_eq!(ActorType::Table.to_string(), "table");
        assert_eq!(ActorType::Row.to_string(), "row");
        assert_eq!(ActorType::Custom(42).to_string(), "custom_42");
    }

    #[test]
    fn test_default_tier_mapping() {
        let config = ActorTierPlacementConfig::default();

        // Hot tier actors
        assert_eq!(config.actor_tier_mapping.get("row"), Some(&StorageTier::Hot));
        assert_eq!(config.actor_tier_mapping.get("field"), Some(&StorageTier::Hot));
        assert_eq!(config.actor_tier_mapping.get("index"), Some(&StorageTier::Hot));

        // Warm tier actors
        assert_eq!(config.actor_tier_mapping.get("table"), Some(&StorageTier::Warm));
        assert_eq!(config.actor_tier_mapping.get("extent"), Some(&StorageTier::Warm));
        assert_eq!(config.actor_tier_mapping.get("column"), Some(&StorageTier::Warm));
    }

    #[test]
    fn test_tier_recommendation() {
        let config = ActorTierPlacementConfig::default();
        let storage_config = TieredStorageConfig::default();
        let placement = ActorTierPlacement::new(config, storage_config);

        // Low access count - use default tier
        let rec = placement.recommend_tier(ActorType::Table, 5);
        assert_eq!(rec.tier, StorageTier::Warm);

        // High access count - suggest promotion
        let rec = placement.recommend_tier(ActorType::Table, 20);
        assert_eq!(rec.tier, StorageTier::Hot);
    }

    #[tokio::test]
    async fn test_actor_registration() {
        let config = ActorTierPlacementConfig::default();
        let storage_config = TieredStorageConfig::default();
        let placement = ActorTierPlacement::new(config, storage_config);
        placement.initialize().await.unwrap();

        // Register row actor (should be hot)
        let tier = placement.register_actor("actor1", ActorType::Row).await.unwrap();
        assert_eq!(tier, StorageTier::Hot);

        // Register table actor (should be warm)
        let tier = placement.register_actor("actor2", ActorType::Table).await.unwrap();
        assert_eq!(tier, StorageTier::Warm);

        // Check stats
        let stats = placement.stats().await;
        assert_eq!(stats.hot_tier_actors, 1);
        assert_eq!(stats.warm_tier_actors, 1);

        placement.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_promotion() {
        let config = ActorTierPlacementConfig::default();
        let storage_config = TieredStorageConfig::default();
        let placement = ActorTierPlacement::new(config, storage_config);
        placement.initialize().await.unwrap();

        // Register table actor (warm tier)
        placement.register_actor("actor1", ActorType::Table).await.unwrap();
        assert_eq!(placement.get_actor_tier("actor1").await, Some(StorageTier::Warm));

        // Promote to hot tier
        placement.promote_actor("actor1", StorageTier::Hot).await.unwrap();
        assert_eq!(placement.get_actor_tier("actor1").await, Some(StorageTier::Hot));

        // Check stats
        let stats = placement.stats().await;
        assert_eq!(stats.hot_tier_actors, 1);
        assert_eq!(stats.warm_tier_actors, 0);
        assert_eq!(stats.promotions, 1);

        placement.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_actor_demotion() {
        let config = ActorTierPlacementConfig::default();
        let storage_config = TieredStorageConfig::default();
        let placement = ActorTierPlacement::new(config, storage_config);
        placement.initialize().await.unwrap();

        // Register row actor (hot tier)
        placement.register_actor("actor1", ActorType::Row).await.unwrap();
        assert_eq!(placement.get_actor_tier("actor1").await, Some(StorageTier::Hot));

        // Demote to warm tier
        placement.demote_actor("actor1", StorageTier::Warm).await.unwrap();
        assert_eq!(placement.get_actor_tier("actor1").await, Some(StorageTier::Warm));

        // Check stats
        let stats = placement.stats().await;
        assert_eq!(stats.hot_tier_actors, 0);
        assert_eq!(stats.warm_tier_actors, 1);
        assert_eq!(stats.demotions, 1);

        placement.shutdown().await.unwrap();
    }

    #[test]
    fn test_builder() {
        let config = ActorTierPlacementBuilder::new()
            .enabled(true)
            .default_tier(StorageTier::Cold)
            .map_actor_type(ActorType::Cache, StorageTier::Hot)
            .auto_promotion(true, 5)
            .auto_demotion(true, 300)
            .build();

        assert!(config.enabled);
        assert_eq!(config.default_tier, StorageTier::Cold);
        assert_eq!(config.actor_tier_mapping.get("cache"), Some(&StorageTier::Hot));
        assert_eq!(config.promotion_threshold, 5);
        assert_eq!(config.demotion_idle_secs, 300);
    }
}
