//! Replication slot management for logical replication
//!
//! This module provides replication slot management inspired by PostgreSQL's
//! logical replication, allowing consumers to track their position in the
//! CDC stream and ensure they don't miss any events.

use crate::cdc::CdcEvent;
use crate::error::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Replication slot represents a consumer's position in the event stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationSlot {
    /// Unique slot name
    pub name: String,
    /// Plugin/consumer type
    pub plugin: String,
    /// Current LSN (Log Sequence Number) position
    pub restart_lsn: u64,
    /// Confirmed flush LSN (consumer has processed up to this point)
    pub confirmed_flush_lsn: u64,
    /// Whether slot is active
    pub active: bool,
    /// Creation timestamp
    pub created_at: i64,
    /// Last activity timestamp
    pub last_activity: i64,
    /// Additional configuration
    pub config: HashMap<String, String>,
}

impl ReplicationSlot {
    /// Create a new replication slot
    pub fn new(name: String, plugin: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            name,
            plugin,
            restart_lsn: 0,
            confirmed_flush_lsn: 0,
            active: false,
            created_at: now,
            last_activity: now,
            config: HashMap::new(),
        }
    }

    /// Update slot position
    pub fn update_position(&mut self, lsn: u64) {
        self.confirmed_flush_lsn = lsn;
        self.restart_lsn = lsn;
        self.last_activity = chrono::Utc::now().timestamp_millis();
    }

    /// Mark slot as active
    pub fn activate(&mut self) {
        self.active = true;
        self.last_activity = chrono::Utc::now().timestamp_millis();
    }

    /// Mark slot as inactive
    pub fn deactivate(&mut self) {
        self.active = false;
        self.last_activity = chrono::Utc::now().timestamp_millis();
    }

    /// Check if slot has been inactive for too long
    pub fn is_stale(&self, stale_threshold_seconds: i64) -> bool {
        let now = chrono::Utc::now().timestamp_millis();
        let age_seconds = (now - self.last_activity) / 1000;
        age_seconds > stale_threshold_seconds
    }
}

/// Replication slot manager
pub struct ReplicationSlotManager {
    /// Active slots
    slots: Arc<RwLock<HashMap<String, ReplicationSlot>>>,
    /// Global LSN counter
    current_lsn: Arc<RwLock<u64>>,
    /// Configuration
    config: ReplicationConfig,
    /// Statistics
    stats: Arc<RwLock<ReplicationStats>>,
}

/// Replication configuration
#[derive(Debug, Clone)]
pub struct ReplicationConfig {
    /// Maximum number of replication slots
    pub max_slots: usize,
    /// Threshold for considering a slot stale (seconds)
    pub stale_threshold_seconds: i64,
    /// Enable automatic slot cleanup
    pub auto_cleanup: bool,
    /// Maximum lag allowed (in number of events)
    pub max_lag: u64,
}

impl Default for ReplicationConfig {
    fn default() -> Self {
        Self {
            max_slots: 10,
            stale_threshold_seconds: 3600, // 1 hour
            auto_cleanup: true,
            max_lag: 10000,
        }
    }
}

impl ReplicationSlotManager {
    /// Create a new replication slot manager
    pub fn new(config: ReplicationConfig) -> Self {
        Self {
            slots: Arc::new(RwLock::new(HashMap::new())),
            current_lsn: Arc::new(RwLock::new(0)),
            config,
            stats: Arc::new(RwLock::new(ReplicationStats::default())),
        }
    }

    /// Create a new replication slot
    pub async fn create_slot(&self, name: String, plugin: String) -> EngineResult<ReplicationSlot> {
        let mut slots = self.slots.write().await;

        if slots.contains_key(&name) {
            return Err(EngineError::internal(format!(
                "Replication slot '{name}' already exists"
            )));
        }

        if slots.len() >= self.config.max_slots {
            return Err(EngineError::internal(format!(
                "Maximum number of replication slots ({}) reached",
                self.config.max_slots
            )));
        }

        let slot = ReplicationSlot::new(name.clone(), plugin);
        slots.insert(name.clone(), slot.clone());

        info!("Created replication slot '{}'", name);

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_slots_created += 1;
            stats.active_slots = slots.len();
        }

        Ok(slot)
    }

    /// Drop a replication slot
    pub async fn drop_slot(&self, name: &str) -> EngineResult<()> {
        let mut slots = self.slots.write().await;

        if slots.remove(name).is_some() {
            info!("Dropped replication slot '{}'", name);

            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.total_slots_dropped += 1;
                stats.active_slots = slots.len();
            }

            Ok(())
        } else {
            Err(EngineError::AddressableNotFound(
                format!("Replication slot '{name}'")
            ))
        }
    }

    /// Get a replication slot
    pub async fn get_slot(&self, name: &str) -> Option<ReplicationSlot> {
        self.slots.read().await.get(name).cloned()
    }

    /// List all replication slots
    pub async fn list_slots(&self) -> Vec<ReplicationSlot> {
        self.slots.read().await.values().cloned().collect()
    }

    /// Advance slot position (consumer confirms processing)
    pub async fn advance_slot(&self, name: &str, lsn: u64) -> EngineResult<()> {
        let mut slots = self.slots.write().await;

        if let Some(slot) = slots.get_mut(name) {
            slot.update_position(lsn);
            debug!("Advanced slot '{}' to LSN {}", name, lsn);
            Ok(())
        } else {
            Err(EngineError::AddressableNotFound(
                format!("Replication slot '{name}'")
            ))
        }
    }

    /// Advance global LSN
    pub async fn advance_lsn(&self) -> u64 {
        let mut lsn = self.current_lsn.write().await;
        *lsn += 1;
        *lsn
    }

    /// Get current LSN
    pub async fn current_lsn(&self) -> u64 {
        *self.current_lsn.read().await
    }

    /// Get events for a slot since its position
    pub async fn get_pending_events(
        &self,
        slot_name: &str,
        events: &[CdcEvent],
    ) -> EngineResult<Vec<CdcEvent>> {
        let slot =
            self.get_slot(slot_name)
                .await
                .ok_or_else(|| EngineError::AddressableNotFound(
                    format!("Replication slot '{slot_name}'")
                ))?;

        // Return events with LSN greater than slot's confirmed position
        let pending: Vec<_> = events
            .iter()
            .filter(|e| e.lsn > slot.confirmed_flush_lsn)
            .cloned()
            .collect();

        Ok(pending)
    }

    /// Cleanup stale slots
    pub async fn cleanup_stale_slots(&self) -> usize {
        if !self.config.auto_cleanup {
            return 0;
        }

        let mut slots = self.slots.write().await;
        let initial_count = slots.len();

        slots.retain(|name, slot| {
            let is_stale = slot.is_stale(self.config.stale_threshold_seconds);
            if is_stale {
                warn!("Removing stale replication slot '{}'", name);
            }
            !is_stale
        });

        let removed = initial_count - slots.len();
        if removed > 0 {
            info!("Cleaned up {} stale replication slots", removed);

            // Update stats
            let mut stats = self.stats.write().await;
            stats.total_slots_dropped += removed as u64;
            stats.active_slots = slots.len();
        }

        removed
    }

    /// Check slot lag
    pub async fn get_slot_lag(&self, slot_name: &str) -> EngineResult<u64> {
        let slot =
            self.get_slot(slot_name)
                .await
                .ok_or_else(|| EngineError::AddressableNotFound(
                    format!("Replication slot '{slot_name}'")
                ))?;

        let current = self.current_lsn().await;
        Ok(current.saturating_sub(slot.confirmed_flush_lsn))
    }

    /// Check if slot is lagging too much
    pub async fn is_slot_lagging(&self, slot_name: &str) -> EngineResult<bool> {
        let lag = self.get_slot_lag(slot_name).await?;
        Ok(lag > self.config.max_lag)
    }

    /// Get replication statistics
    pub async fn get_stats(&self) -> ReplicationStats {
        let mut stats = self.stats.read().await.clone();
        stats.active_slots = self.slots.read().await.len();
        stats
    }
}

/// Replication statistics
#[derive(Debug, Clone, Default)]
pub struct ReplicationStats {
    pub active_slots: usize,
    pub total_slots_created: u64,
    pub total_slots_dropped: u64,
}

/// Replication stream for consuming events from a slot
pub struct ReplicationStream {
    slot_name: String,
    manager: Arc<ReplicationSlotManager>,
    #[allow(dead_code)] // Buffer will be used in future streaming implementations
    buffer: Vec<CdcEvent>,
}

impl ReplicationStream {
    /// Create a new replication stream
    pub fn new(slot_name: String, manager: Arc<ReplicationSlotManager>) -> Self {
        Self {
            slot_name,
            manager,
            buffer: Vec::new(),
        }
    }

    /// Get next batch of events
    pub async fn next_batch(&mut self, events: &[CdcEvent]) -> EngineResult<Vec<CdcEvent>> {
        let pending = self
            .manager
            .get_pending_events(&self.slot_name, events)
            .await?;
        Ok(pending)
    }

    /// Confirm processing up to LSN
    pub async fn confirm_lsn(&self, lsn: u64) -> EngineResult<()> {
        self.manager.advance_slot(&self.slot_name, lsn).await
    }

    /// Get slot name
    pub fn slot_name(&self) -> &str {
        &self.slot_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_replication_slot_creation() {
        let manager = ReplicationSlotManager::new(ReplicationConfig::default());

        let slot = manager
            .create_slot("test_slot".to_string(), "test_plugin".to_string())
            .await
            .unwrap();

        assert_eq!(slot.name, "test_slot");
        assert_eq!(slot.plugin, "test_plugin");
        assert_eq!(slot.restart_lsn, 0);
        assert!(!slot.active);
    }

    #[tokio::test]
    async fn test_duplicate_slot_creation() {
        let manager = ReplicationSlotManager::new(ReplicationConfig::default());

        manager
            .create_slot("test_slot".to_string(), "test_plugin".to_string())
            .await
            .unwrap();

        let result = manager
            .create_slot("test_slot".to_string(), "test_plugin".to_string())
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_slot_advance() {
        let manager = ReplicationSlotManager::new(ReplicationConfig::default());

        manager
            .create_slot("test_slot".to_string(), "test_plugin".to_string())
            .await
            .unwrap();

        manager.advance_slot("test_slot", 10).await.unwrap();

        let slot = manager.get_slot("test_slot").await.unwrap();
        assert_eq!(slot.confirmed_flush_lsn, 10);
        assert_eq!(slot.restart_lsn, 10);
    }

    #[tokio::test]
    async fn test_slot_drop() {
        let manager = ReplicationSlotManager::new(ReplicationConfig::default());

        manager
            .create_slot("test_slot".to_string(), "test_plugin".to_string())
            .await
            .unwrap();

        manager.drop_slot("test_slot").await.unwrap();

        assert!(manager.get_slot("test_slot").await.is_none());
    }

    #[tokio::test]
    async fn test_list_slots() {
        let manager = ReplicationSlotManager::new(ReplicationConfig::default());

        manager
            .create_slot("slot1".to_string(), "plugin1".to_string())
            .await
            .unwrap();
        manager
            .create_slot("slot2".to_string(), "plugin2".to_string())
            .await
            .unwrap();

        let slots = manager.list_slots().await;
        assert_eq!(slots.len(), 2);
    }

    #[tokio::test]
    async fn test_slot_lag() {
        let manager = ReplicationSlotManager::new(ReplicationConfig::default());

        manager
            .create_slot("test_slot".to_string(), "test_plugin".to_string())
            .await
            .unwrap();

        // Advance global LSN
        for _ in 0..10 {
            manager.advance_lsn().await;
        }

        // Slot hasn't advanced, so it's lagging by 10
        let lag = manager.get_slot_lag("test_slot").await.unwrap();
        assert_eq!(lag, 10);

        // Advance slot
        manager.advance_slot("test_slot", 5).await.unwrap();
        let lag = manager.get_slot_lag("test_slot").await.unwrap();
        assert_eq!(lag, 5);
    }

    #[tokio::test]
    async fn test_max_slots_limit() {
        let config = ReplicationConfig {
            max_slots: 2,
            ..Default::default()
        };
        let manager = ReplicationSlotManager::new(config);

        manager
            .create_slot("slot1".to_string(), "plugin".to_string())
            .await
            .unwrap();
        manager
            .create_slot("slot2".to_string(), "plugin".to_string())
            .await
            .unwrap();

        let result = manager
            .create_slot("slot3".to_string(), "plugin".to_string())
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn test_slot_stale_check() {
        let mut slot = ReplicationSlot::new("test".to_string(), "plugin".to_string());

        // Fresh slot is not stale
        assert!(!slot.is_stale(3600));

        // Manually set old timestamp
        slot.last_activity = chrono::Utc::now().timestamp_millis() - (7200 * 1000); // 2 hours ago

        // Should be stale with 1 hour threshold
        assert!(slot.is_stale(3600));
    }
}
