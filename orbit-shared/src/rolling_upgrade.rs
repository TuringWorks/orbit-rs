//! Rolling upgrade support for zero-downtime updates
//!
//! This module manages rolling upgrades across cluster nodes, ensuring
//! zero downtime and backward compatibility during version transitions.

use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Version information
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Check if this version is compatible with another
    pub fn is_compatible_with(&self, other: &Version) -> bool {
        // Major version must match, minor can differ by at most 1
        self.major == other.major && 
        (self.minor as i32 - other.minor as i32).abs() <= 1
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Upgrade strategy
#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeStrategy {
    /// One node at a time, wait for health check
    Sequential,
    /// Multiple nodes in parallel (with max parallelism)
    Parallel { max_parallel: usize },
    /// Canary deployment (upgrade one node, monitor, then proceed)
    Canary { canary_duration: Duration },
    /// Blue-green deployment
    BlueGreen,
}

/// Node upgrade status
#[derive(Debug, Clone, PartialEq)]
pub enum UpgradeStatus {
    /// Not started
    Pending,
    /// Draining connections
    Draining,
    /// Upgrading software
    Upgrading,
    /// Verifying health
    Verifying,
    /// Completed successfully
    Completed,
    /// Failed
    Failed { error: String },
    /// Rolled back
    RolledBack,
}

/// Upgrade plan for a node
#[derive(Debug, Clone)]
pub struct NodeUpgradePlan {
    pub node_id: NodeId,
    pub current_version: Version,
    pub target_version: Version,
    pub status: UpgradeStatus,
    pub started_at: Option<Instant>,
    pub completed_at: Option<Instant>,
}

/// Overall upgrade operation
#[derive(Debug, Clone)]
pub struct UpgradeOperation {
    pub operation_id: String,
    pub strategy: UpgradeStrategy,
    pub target_version: Version,
    pub node_plans: Vec<NodeUpgradePlan>,
    pub started_at: Instant,
    pub completed_at: Option<Instant>,
}

/// Configuration for rolling upgrades
#[derive(Debug, Clone)]
pub struct RollingUpgradeConfig {
    /// Strategy to use
    pub strategy: UpgradeStrategy,
    /// Timeout for draining connections
    pub drain_timeout: Duration,
    /// Timeout for upgrade process
    pub upgrade_timeout: Duration,
    /// Health check interval during upgrade
    pub health_check_interval: Duration,
    /// Number of consecutive successful health checks required
    pub health_check_threshold: u32,
    /// Enable automatic rollback on failure
    pub auto_rollback: bool,
}

impl Default for RollingUpgradeConfig {
    fn default() -> Self {
        Self {
            strategy: UpgradeStrategy::Sequential,
            drain_timeout: Duration::from_secs(30),
            upgrade_timeout: Duration::from_secs(300),
            health_check_interval: Duration::from_secs(5),
            health_check_threshold: 3,
            auto_rollback: true,
        }
    }
}

/// Connection draining manager
struct ConnectionDrainer {
    active_connections: Arc<RwLock<HashMap<NodeId, usize>>>,
}

impl ConnectionDrainer {
    fn new() -> Self {
        Self {
            active_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start draining connections for a node
    async fn start_draining(&self, node_id: &NodeId) {
        info!("Starting connection drain for node: {}", node_id);
        // In real implementation: stop accepting new connections
    }

    /// Check if a node has finished draining
    async fn is_drained(&self, node_id: &NodeId) -> bool {
        let connections = self.active_connections.read().await;
        connections.get(node_id).map(|&count| count == 0).unwrap_or(true)
    }

    /// Wait for connections to drain
    async fn wait_for_drain(&self, node_id: &NodeId, timeout: Duration) -> OrbitResult<()> {
        let start = Instant::now();
        
        while !self.is_drained(node_id).await {
            if start.elapsed() > timeout {
                return Err(OrbitError::internal("Connection drain timeout"));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        
        info!("Node {} connections drained", node_id);
        Ok(())
    }
}

/// Health checker for nodes
struct HealthChecker {
    healthy_checks: Arc<RwLock<HashMap<NodeId, u32>>>,
}

impl HealthChecker {
    fn new() -> Self {
        Self {
            healthy_checks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a node is healthy
    async fn check_health(&self, node_id: &NodeId) -> bool {
        // In real implementation: perform actual health checks
        // For now, simulate 90% success rate
        fastrand::f32() < 0.9
    }

    /// Verify node health meets threshold
    async fn verify_health(
        &self,
        node_id: &NodeId,
        threshold: u32,
        interval: Duration,
    ) -> OrbitResult<()> {
        let mut checks = self.healthy_checks.write().await;
        checks.insert(node_id.clone(), 0);
        drop(checks);

        loop {
            let is_healthy = self.check_health(node_id).await;
            
            let mut checks = self.healthy_checks.write().await;
            let count = checks.entry(node_id.clone()).or_insert(0);
            
            if is_healthy {
                *count += 1;
                if *count >= threshold {
                    info!("Node {} health verified", node_id);
                    return Ok(());
                }
            } else {
                *count = 0;
                warn!("Node {} health check failed, resetting counter", node_id);
            }
            
            drop(checks);
            tokio::time::sleep(interval).await;
        }
    }
}

/// Rolling upgrade manager
pub struct RollingUpgradeManager {
    config: RollingUpgradeConfig,
    connection_drainer: Arc<ConnectionDrainer>,
    health_checker: Arc<HealthChecker>,
    active_operation: Arc<RwLock<Option<UpgradeOperation>>>,
    node_versions: Arc<RwLock<HashMap<NodeId, Version>>>,
}

impl RollingUpgradeManager {
    /// Create a new rolling upgrade manager
    pub fn new(config: RollingUpgradeConfig) -> Self {
        Self {
            config,
            connection_drainer: Arc::new(ConnectionDrainer::new()),
            health_checker: Arc::new(HealthChecker::new()),
            active_operation: Arc::new(RwLock::new(None)),
            node_versions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a node with its current version
    pub async fn register_node(&self, node_id: NodeId, version: Version) {
        let mut versions = self.node_versions.write().await;
        versions.insert(node_id, version);
    }

    /// Start a rolling upgrade
    pub async fn start_upgrade(
        &self,
        target_version: Version,
        nodes: Vec<NodeId>,
    ) -> OrbitResult<String> {
        // Check if an upgrade is already in progress
        {
            let active = self.active_operation.read().await;
            if active.is_some() {
                return Err(OrbitError::internal("Upgrade already in progress"));
            }
        }

        // Verify version compatibility
        let versions = self.node_versions.read().await;
        for node_id in &nodes {
            if let Some(current_version) = versions.get(node_id) {
                if !target_version.is_compatible_with(current_version) {
                    return Err(OrbitError::internal(format!(
                        "Target version {} incompatible with node {} version {}",
                        target_version, node_id, current_version
                    )));
                }
            }
        }
        drop(versions);

        let operation_id = format!("upgrade-{}", uuid::Uuid::new_v4());
        
        // Create upgrade plans
        let mut node_plans = Vec::new();
        for node_id in nodes {
            let current_version = self.node_versions.read().await
                .get(&node_id)
                .cloned()
                .unwrap_or_else(|| Version::new(0, 0, 0));
            
            node_plans.push(NodeUpgradePlan {
                node_id,
                current_version,
                target_version: target_version.clone(),
                status: UpgradeStatus::Pending,
                started_at: None,
                completed_at: None,
            });
        }

        let operation = UpgradeOperation {
            operation_id: operation_id.clone(),
            strategy: self.config.strategy.clone(),
            target_version: target_version.clone(),
            node_plans,
            started_at: Instant::now(),
            completed_at: None,
        };

        info!("Starting rolling upgrade to version {} with {} nodes", 
              target_version, operation.node_plans.len());

        // Store operation
        {
            let mut active = self.active_operation.write().await;
            *active = Some(operation);
        }

        // Execute upgrade in background
        let manager = self.clone();
        let op_id = operation_id.clone();
        tokio::spawn(async move {
            manager.execute_upgrade(op_id).await
        });

        Ok(operation_id)
    }

    /// Execute the upgrade operation
    async fn execute_upgrade(&self, operation_id: String) -> OrbitResult<()> {
        let strategy = self.config.strategy.clone();
        
        match strategy {
            UpgradeStrategy::Sequential => self.execute_sequential_upgrade().await,
            UpgradeStrategy::Parallel { max_parallel } => {
                self.execute_parallel_upgrade(max_parallel).await
            }
            UpgradeStrategy::Canary { canary_duration } => {
                self.execute_canary_upgrade(canary_duration).await
            }
            UpgradeStrategy::BlueGreen => self.execute_blue_green_upgrade().await,
        }
    }

    /// Execute sequential upgrade (one node at a time)
    async fn execute_sequential_upgrade(&self) -> OrbitResult<()> {
        loop {
            // Get next node to upgrade
            let next_node = {
                let mut active = self.active_operation.write().await;
                if let Some(operation) = active.as_mut() {
                    operation.node_plans.iter_mut()
                        .find(|plan| plan.status == UpgradeStatus::Pending)
                        .cloned()
                } else {
                    None
                }
            };

            match next_node {
                Some(mut plan) => {
                    info!("Upgrading node: {}", plan.node_id);
                    
                    match self.upgrade_node(&mut plan).await {
                        Ok(_) => {
                            info!("Node {} upgraded successfully", plan.node_id);
                            plan.status = UpgradeStatus::Completed;
                            plan.completed_at = Some(Instant::now());
                            
                            // Update node version
                            let mut versions = self.node_versions.write().await;
                            versions.insert(plan.node_id.clone(), plan.target_version.clone());
                        }
                        Err(e) => {
                            error!("Failed to upgrade node {}: {}", plan.node_id, e);
                            plan.status = UpgradeStatus::Failed {
                                error: e.to_string(),
                            };
                            
                            if self.config.auto_rollback {
                                self.rollback_upgrade().await?;
                            }
                            return Err(e);
                        }
                    }

                    // Update plan in operation
                    let mut active = self.active_operation.write().await;
                    if let Some(operation) = active.as_mut() {
                        if let Some(stored_plan) = operation.node_plans.iter_mut()
                            .find(|p| p.node_id == plan.node_id) {
                            *stored_plan = plan;
                        }
                    }
                }
                None => {
                    // All nodes upgraded
                    info!("All nodes upgraded successfully");
                    let mut active = self.active_operation.write().await;
                    if let Some(operation) = active.as_mut() {
                        operation.completed_at = Some(Instant::now());
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    /// Execute parallel upgrade
    async fn execute_parallel_upgrade(&self, max_parallel: usize) -> OrbitResult<()> {
        info!("Executing parallel upgrade with max_parallel: {}", max_parallel);
        
        // In a real implementation, this would:
        // 1. Identify nodes to upgrade in parallel
        // 2. Ensure load balancing
        // 3. Coordinate upgrades
        // 4. Monitor progress
        
        // For now, fall back to sequential
        self.execute_sequential_upgrade().await
    }

    /// Execute canary upgrade
    async fn execute_canary_upgrade(&self, canary_duration: Duration) -> OrbitResult<()> {
        info!("Executing canary upgrade with duration: {:?}", canary_duration);
        
        // Upgrade first node (canary)
        // Monitor for canary_duration
        // If successful, upgrade remaining nodes
        
        self.execute_sequential_upgrade().await
    }

    /// Execute blue-green upgrade
    async fn execute_blue_green_upgrade(&self) -> OrbitResult<()> {
        info!("Executing blue-green upgrade");
        
        // In a real implementation:
        // 1. Set up "green" environment with new version
        // 2. Test green environment
        // 3. Switch traffic to green
        // 4. Keep blue as fallback
        
        self.execute_sequential_upgrade().await
    }

    /// Upgrade a single node
    async fn upgrade_node(&self, plan: &mut NodeUpgradePlan) -> OrbitResult<()> {
        plan.started_at = Some(Instant::now());
        
        // 1. Drain connections
        plan.status = UpgradeStatus::Draining;
        self.connection_drainer.start_draining(&plan.node_id).await;
        self.connection_drainer.wait_for_drain(&plan.node_id, self.config.drain_timeout).await?;
        
        // 2. Perform upgrade
        plan.status = UpgradeStatus::Upgrading;
        self.perform_upgrade(&plan.node_id, &plan.target_version).await?;
        
        // 3. Verify health
        plan.status = UpgradeStatus::Verifying;
        self.health_checker.verify_health(
            &plan.node_id,
            self.config.health_check_threshold,
            self.config.health_check_interval,
        ).await?;
        
        Ok(())
    }

    /// Perform the actual upgrade
    async fn perform_upgrade(&self, node_id: &NodeId, version: &Version) -> OrbitResult<()> {
        info!("Performing upgrade of node {} to version {}", node_id, version);
        
        // In a real implementation:
        // 1. Download new version binary
        // 2. Stop old process
        // 3. Start new process
        // 4. Verify startup
        
        // Simulate upgrade time
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        Ok(())
    }

    /// Rollback the upgrade
    async fn rollback_upgrade(&self) -> OrbitResult<()> {
        warn!("Rolling back upgrade");
        
        let mut active = self.active_operation.write().await;
        if let Some(operation) = active.as_mut() {
            for plan in &mut operation.node_plans {
                if plan.status == UpgradeStatus::Completed {
                    info!("Rolling back node: {}", plan.node_id);
                    plan.status = UpgradeStatus::RolledBack;
                    
                    // Restore original version
                    let mut versions = self.node_versions.write().await;
                    versions.insert(plan.node_id.clone(), plan.current_version.clone());
                }
            }
        }
        
        Ok(())
    }

    /// Get current upgrade status
    pub async fn get_upgrade_status(&self) -> Option<UpgradeOperation> {
        self.active_operation.read().await.clone()
    }

    /// Cancel ongoing upgrade
    pub async fn cancel_upgrade(&self) -> OrbitResult<()> {
        let mut active = self.active_operation.write().await;
        *active = None;
        info!("Upgrade cancelled");
        Ok(())
    }
}

impl Clone for RollingUpgradeManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            connection_drainer: Arc::clone(&self.connection_drainer),
            health_checker: Arc::clone(&self.health_checker),
            active_operation: Arc::clone(&self.active_operation),
            node_versions: Arc::clone(&self.node_versions),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compatibility() {
        let v1 = Version::new(1, 2, 3);
        let v2 = Version::new(1, 3, 0);
        let v3 = Version::new(2, 0, 0);
        
        assert!(v1.is_compatible_with(&v2));
        assert!(!v1.is_compatible_with(&v3));
    }

    #[tokio::test]
    async fn test_rolling_upgrade_manager_creation() {
        let config = RollingUpgradeConfig::default();
        let manager = RollingUpgradeManager::new(config);
        
        let status = manager.get_upgrade_status().await;
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_register_node() {
        let manager = RollingUpgradeManager::new(RollingUpgradeConfig::default());
        let node_id = NodeId::new("node-1".to_string(), "default".to_string());
        let version = Version::new(1, 0, 0);
        
        manager.register_node(node_id.clone(), version.clone()).await;
        
        let versions = manager.node_versions.read().await;
        assert_eq!(versions.get(&node_id), Some(&version));
    }
}
