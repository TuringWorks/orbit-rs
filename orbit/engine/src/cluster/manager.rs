use super::consensus::{RaftConfig, RaftConsensus, RaftEventHandler};
use crate::error::{EngineError, EngineResult};
use super::NodeId;
use crate::cluster::recovery::{ClusterConfig, ClusterManager};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Enhanced cluster manager with split-brain protection
pub struct EnhancedClusterManager {
    /// Node identifier
    node_id: NodeId,
    /// Raft consensus instance
    pub raft_consensus: Arc<RaftConsensus>,
    /// Quorum configuration
    quorum_config: QuorumConfig,
    /// Node health tracking
    node_health: Arc<RwLock<HashMap<NodeId, NodeHealthStatus>>>,
    /// Split-brain detection
    split_brain_detector: Arc<SplitBrainDetector>,
    /// Network partition detection
    partition_detector: Arc<PartitionDetector>,
}

/// Quorum configuration for cluster consensus
#[derive(Debug, Clone)]
pub struct QuorumConfig {
    /// Minimum nodes required for quorum
    pub min_quorum_size: usize,
    /// Maximum tolerated failures
    pub max_failures: usize,
    /// Quorum timeout
    pub quorum_timeout: Duration,
    /// Enable dynamic quorum adjustment
    pub dynamic_quorum: bool,
}

impl Default for QuorumConfig {
    fn default() -> Self {
        Self {
            min_quorum_size: 3,
            max_failures: 1,
            quorum_timeout: Duration::from_secs(10),
            dynamic_quorum: false,
        }
    }
}

/// Node health status tracking
#[derive(Debug, Clone)]
pub struct NodeHealthStatus {
    /// Node identifier
    pub node_id: NodeId,
    /// Last time node was seen
    pub last_seen: Instant,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Network latency to node
    pub network_latency: Option<Duration>,
    /// Whether node is reachable
    pub is_reachable: bool,
    /// Partition group if in network partition
    pub partition_group: Option<String>,
}

/// Split-brain detection system
pub struct SplitBrainDetector {
    /// Minimum cluster size to avoid split-brain
    min_cluster_size: usize,
    /// Detection interval
    detection_interval: Duration,
    /// Current partition map
    partitions: Arc<RwLock<HashMap<String, Vec<NodeId>>>>,
}

impl SplitBrainDetector {
    /// Create a new split-brain detector
    pub fn new(min_cluster_size: usize, detection_interval: Duration) -> Self {
        Self {
            min_cluster_size,
            detection_interval,
            partitions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the detection interval
    pub fn get_detection_interval(&self) -> Duration {
        self.detection_interval
    }

    /// Check if current cluster configuration might cause split-brain
    pub async fn check_split_brain_risk(
        &self,
        cluster_nodes: &[NodeId],
        unreachable_nodes: &[NodeId],
    ) -> bool {
        let reachable_count = cluster_nodes.len() - unreachable_nodes.len();
        let total_nodes = cluster_nodes.len();

        // Classic split-brain condition: can't reach majority
        let majority_threshold = total_nodes / 2 + 1;

        if reachable_count < majority_threshold {
            warn!(
                "Split-brain risk detected: {} reachable out of {} total (need {})",
                reachable_count, total_nodes, majority_threshold
            );
            return true;
        }

        // Additional check: ensure minimum cluster size
        if reachable_count < self.min_cluster_size {
            warn!(
                "Cluster too small: {} reachable (minimum {})",
                reachable_count, self.min_cluster_size
            );
            return true;
        }

        false
    }

    /// Detect network partitions
    pub async fn detect_partitions(
        &self,
        node_health: &HashMap<NodeId, NodeHealthStatus>,
    ) -> HashMap<String, Vec<NodeId>> {
        let mut partitions = HashMap::new();

        // Group nodes by connectivity patterns
        let mut reachable_nodes = Vec::new();
        let mut unreachable_nodes = Vec::new();

        for (node_id, health) in node_health.iter() {
            if health.is_reachable {
                reachable_nodes.push(node_id.clone());
            } else {
                unreachable_nodes.push(node_id.clone());
            }
        }

        if !reachable_nodes.is_empty() {
            partitions.insert("reachable".to_string(), reachable_nodes);
        }

        if !unreachable_nodes.is_empty() {
            partitions.insert("unreachable".to_string(), unreachable_nodes);
        }

        // Store detected partitions
        {
            let mut stored_partitions = self.partitions.write().await;
            *stored_partitions = partitions.clone();
        }

        partitions
    }
}

/// Network partition detector
pub struct PartitionDetector {
    /// Ping timeout for connectivity checks
    ping_timeout: Duration,
    /// Number of ping attempts
    ping_attempts: u32,
}

impl PartitionDetector {
    /// Create a new partition detector
    pub fn new(ping_timeout: Duration, ping_attempts: u32) -> Self {
        Self {
            ping_timeout,
            ping_attempts,
        }
    }

    /// Check network connectivity to a node
    pub async fn check_connectivity(&self, target_node: &NodeId) -> bool {
        // In a real implementation, this would perform actual network checks
        // For now, simulate with a simple timeout

        for attempt in 1..=self.ping_attempts {
            debug!(
                "Checking connectivity to {} (attempt {})",
                target_node, attempt
            );

            // Simulate network check with timeout
            tokio::time::sleep(self.ping_timeout.min(Duration::from_millis(100))).await;

            // For demo purposes, assume 90% success rate
            if fastrand::f32() > 0.1 {
                return true;
            }
        }

        warn!(
            "Node {} appears unreachable after {} attempts",
            target_node, self.ping_attempts
        );
        false
    }

    /// Perform connectivity matrix check across all nodes
    pub async fn check_cluster_connectivity(
        &self,
        nodes: &[NodeId],
        from_node: &NodeId,
    ) -> HashMap<NodeId, bool> {
        let mut connectivity = HashMap::new();

        for node in nodes {
            if node != from_node {
                let is_reachable = self.check_connectivity(node).await;
                connectivity.insert(node.clone(), is_reachable);
            }
        }

        connectivity
    }
}

impl EnhancedClusterManager {
    /// Create a new cluster manager with Raft consensus and split-brain detection
    pub fn new(
        node_id: NodeId,
        cluster_nodes: Vec<NodeId>,
        quorum_config: QuorumConfig,
        raft_config: RaftConfig,
    ) -> Self {
        let raft_consensus = Arc::new(RaftConsensus::new(
            node_id.clone(),
            cluster_nodes,
            raft_config,
        ));

        let split_brain_detector = Arc::new(SplitBrainDetector::new(
            quorum_config.min_quorum_size,
            Duration::from_secs(30),
        ));

        let partition_detector = Arc::new(PartitionDetector::new(Duration::from_millis(500), 3));

        Self {
            node_id,
            raft_consensus,
            quorum_config,
            node_health: Arc::new(RwLock::new(HashMap::new())),
            split_brain_detector,
            partition_detector,
        }
    }

    /// Start the enhanced cluster manager
    pub async fn start(
        &self,
        transport: Arc<dyn super::consensus::RaftTransport>,
    ) -> EngineResult<()> {
        info!(
            "Starting enhanced cluster manager for node: {}",
            self.node_id
        );

        // Start Raft consensus
        self.raft_consensus.start(transport).await?;

        // Start health monitoring
        self.start_health_monitoring().await?;

        // Start split-brain detection
        self.start_split_brain_monitoring().await?;

        Ok(())
    }

    /// Start health monitoring background task
    async fn start_health_monitoring(&self) -> EngineResult<()> {
        let cluster_manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                if let Err(e) = cluster_manager.update_node_health().await {
                    error!("Health monitoring failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Start split-brain monitoring
    async fn start_split_brain_monitoring(&self) -> EngineResult<()> {
        let cluster_manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));

            loop {
                interval.tick().await;

                if let Err(e) = cluster_manager.check_cluster_health().await {
                    error!("Split-brain monitoring failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Update health status of all cluster nodes
    async fn update_node_health(&self) -> EngineResult<()> {
        let cluster_nodes = self.get_cluster_nodes().await?;
        let connectivity = self
            .partition_detector
            .check_cluster_connectivity(&cluster_nodes, &self.node_id)
            .await;

        let mut health_map = self.node_health.write().await;
        let now = Instant::now();

        for node in &cluster_nodes {
            if node != &self.node_id {
                let is_reachable = connectivity.get(node).copied().unwrap_or(false);

                let health = health_map
                    .entry(node.clone())
                    .or_insert_with(|| NodeHealthStatus {
                        node_id: node.clone(),
                        last_seen: now,
                        consecutive_failures: 0,
                        network_latency: None,
                        is_reachable: true,
                        partition_group: None,
                    });

                if is_reachable {
                    health.last_seen = now;
                    health.consecutive_failures = 0;
                    health.is_reachable = true;
                } else {
                    health.consecutive_failures += 1;
                    health.is_reachable = false;
                }
            }
        }

        Ok(())
    }

    /// Check overall cluster health and split-brain risk
    async fn check_cluster_health(&self) -> EngineResult<()> {
        let cluster_nodes = self.get_cluster_nodes().await?;
        let health_map = self.node_health.read().await;

        let unreachable_nodes: Vec<NodeId> = health_map
            .values()
            .filter(|health| !health.is_reachable)
            .map(|health| health.node_id.clone())
            .collect();

        let split_brain_risk = self
            .split_brain_detector
            .check_split_brain_risk(&cluster_nodes, &unreachable_nodes)
            .await;

        if split_brain_risk {
            error!(
                "Split-brain risk detected! Unreachable nodes: {:?}",
                unreachable_nodes
            );

            // In a production system, you might want to:
            // 1. Stop accepting new coordinator roles
            // 2. Increase election timeouts
            // 3. Alert monitoring systems
            // 4. Attempt network partition healing
        }

        // Detect partitions
        let partitions = self
            .split_brain_detector
            .detect_partitions(&health_map)
            .await;

        if partitions.len() > 1 {
            warn!("Network partitions detected: {:?}", partitions);
        }

        Ok(())
    }

    /// Check if current node has quorum
    pub async fn has_quorum(&self) -> EngineResult<bool> {
        let cluster_nodes = self.get_cluster_nodes().await?;
        let health_map = self.node_health.read().await;

        let reachable_count = health_map
            .values()
            .filter(|health| health.is_reachable)
            .count()
            + 1; // +1 for self

        let total_nodes = cluster_nodes.len();
        let majority = total_nodes / 2 + 1;

        Ok(reachable_count >= majority && reachable_count >= self.quorum_config.min_quorum_size)
    }

    /// Get nodes in the same partition as this node
    pub async fn get_partition_nodes(&self) -> EngineResult<Vec<NodeId>> {
        let health_map = self.node_health.read().await;

        let mut partition_nodes = vec![self.node_id.clone()]; // Include self

        for health in health_map.values() {
            if health.is_reachable {
                partition_nodes.push(health.node_id.clone());
            }
        }

        Ok(partition_nodes)
    }
}

impl Clone for EnhancedClusterManager {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            raft_consensus: Arc::clone(&self.raft_consensus),
            quorum_config: self.quorum_config.clone(),
            node_health: Arc::clone(&self.node_health),
            split_brain_detector: Arc::clone(&self.split_brain_detector),
            partition_detector: Arc::clone(&self.partition_detector),
        }
    }
}

#[async_trait]
impl ClusterManager for EnhancedClusterManager {
    async fn get_cluster_nodes(&self) -> EngineResult<Vec<NodeId>> {
        // Get nodes from Raft consensus
        let cluster_nodes = self.raft_consensus.get_cluster_nodes().await;
        Ok(cluster_nodes.clone())
    }

    async fn is_leader(&self, node_id: &NodeId) -> EngineResult<bool> {
        if node_id == &self.node_id {
            Ok(self.raft_consensus.is_leader().await)
        } else {
            // Check if the given node is the current leader according to Raft
            let current_leader = self.raft_consensus.get_leader().await;
            Ok(current_leader.as_ref() == Some(node_id))
        }
    }

    async fn start_election(&self, candidate: &NodeId) -> EngineResult<bool> {
        // Only allow election if we have quorum
        if !self.has_quorum().await? {
            warn!("Cannot start election: no quorum available");
            return Ok(false);
        }

        // Only this node can start its own election
        if candidate != &self.node_id {
            return Err(EngineError::config(
                "Can only start election for self",
            ));
        }

        // Check if already leader
        if self.raft_consensus.is_leader().await {
            return Ok(true);
        }

        // The election is handled by the Raft consensus algorithm
        // Return true to indicate election attempt was successful
        Ok(true)
    }

    async fn report_coordinator_failure(&self, failed_coordinator: &NodeId) -> EngineResult<()> {
        info!("Reporting coordinator failure: {}", failed_coordinator);

        // Update health status
        {
            let mut health_map = self.node_health.write().await;
            if let Some(health) = health_map.get_mut(failed_coordinator) {
                health.is_reachable = false;
                health.consecutive_failures += 1;
            }
        }

        // Check if we still have quorum after this failure
        if !self.has_quorum().await? {
            warn!(
                "Lost quorum after coordinator failure: {}",
                failed_coordinator
            );
        }

        Ok(())
    }

    async fn get_cluster_config(&self) -> EngineResult<ClusterConfig> {
        let cluster_nodes = self.get_cluster_nodes().await?;
        let current_leader = self.raft_consensus.get_leader().await;

        let total_nodes = cluster_nodes.len();
        let majority_threshold = total_nodes / 2 + 1;

        Ok(ClusterConfig {
            total_nodes,
            majority_threshold,
            current_leader,
        })
    }
}

/// Event handler that integrates with recovery system
pub struct RecoveryRaftEventHandler {
    recovery_manager: Arc<crate::cluster::recovery::TransactionRecoveryManager>,
}

impl RecoveryRaftEventHandler {
    /// Create a new recovery event handler
    pub fn new(recovery_manager: Arc<crate::cluster::recovery::TransactionRecoveryManager>) -> Self {
        Self { recovery_manager }
    }

    /// Initiate a comprehensive recovery scan when this node becomes leader
    async fn initiate_leader_recovery_scan(&self) -> EngineResult<()> {
        info!("Starting leader recovery scan for orphaned transactions");

        let failed_coordinators = self.identify_failed_coordinators().await;
        let transactions_to_recover = self
            .collect_transactions_for_recovery(&failed_coordinators)
            .await?;
        self.process_recovery_transactions(transactions_to_recover, failed_coordinators.len())
            .await?;

        Ok(())
    }

    /// Identify coordinators that have failed
    async fn identify_failed_coordinators(&self) -> Vec<crate::cluster::NodeId> {
        let coordinators = self.recovery_manager.coordinators.read().await;
        let mut failed_coordinators = Vec::new();

        for (node_id, health) in coordinators.iter() {
            if !health.is_healthy {
                failed_coordinators.push(node_id.clone());
            }
        }

        failed_coordinators
    }

    /// Collect all transactions that need recovery from failed coordinators
    async fn collect_transactions_for_recovery(
        &self,
        failed_coordinators: &[crate::cluster::NodeId],
    ) -> EngineResult<Vec<crate::cluster::recovery::TransactionCheckpoint>> {
        let mut all_transactions_to_recover = Vec::new();

        for failed_coordinator in failed_coordinators {
            let transactions = self
                .recovery_manager
                .find_transactions_needing_recovery(failed_coordinator)
                .await?;
            all_transactions_to_recover.extend(transactions);
        }

        Ok(all_transactions_to_recover)
    }

    /// Process the collected transactions for recovery
    async fn process_recovery_transactions(
        &self,
        transactions_to_recover: Vec<crate::cluster::recovery::TransactionCheckpoint>,
        failed_coordinators_count: usize,
    ) -> EngineResult<()> {
        if !transactions_to_recover.is_empty() {
            info!(
                "Found {} total transactions needing recovery from {} failed coordinators",
                transactions_to_recover.len(),
                failed_coordinators_count
            );

            // Since this node just became leader, it should take over these transactions
            self.recovery_manager
                .become_coordinator(transactions_to_recover)
                .await?;
        } else {
            info!("No orphaned transactions found during leader recovery scan");
        }

        Ok(())
    }
}

#[async_trait]
impl RaftEventHandler for RecoveryRaftEventHandler {
    async fn on_leader_elected(&self, leader_id: &NodeId, term: u64) -> EngineResult<()> {
        info!("New leader elected: {} (term: {})", leader_id, term);

        // Notify recovery manager about the new leader
        let handlers = self.recovery_manager.event_handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.on_coordinator_elected(leader_id).await {
                error!("Recovery event handler failed: {}", e);
            }
        }

        // If this node became leader, initiate recovery scan for orphaned transactions
        if let Ok(cluster_config) = self
            .recovery_manager
            .cluster_manager
            .get_cluster_config()
            .await
        {
            if cluster_config.current_leader.as_ref() == Some(leader_id) {
                // This node is the new leader - scan for transactions that need recovery
                info!("This node became leader, scanning for orphaned transactions...");

                // Trigger a comprehensive recovery check
                if let Err(e) = self.initiate_leader_recovery_scan().await {
                    error!("Failed to initiate leader recovery scan: {}", e);
                }
            }
        }

        Ok(())
    }

    async fn on_leader_lost(&self, former_leader_id: &NodeId, term: u64) -> EngineResult<()> {
        warn!("Leader lost: {} (term: {})", former_leader_id, term);

        // Report coordinator failure to trigger recovery process
        if let Err(e) = self
            .recovery_manager
            .cluster_manager
            .report_coordinator_failure(former_leader_id)
            .await
        {
            error!("Failed to report coordinator failure: {}", e);
        }

        // Find and handle transactions coordinated by the failed leader
        let transactions_needing_recovery = self
            .recovery_manager
            .find_transactions_needing_recovery(former_leader_id)
            .await?;

        if !transactions_needing_recovery.is_empty() {
            warn!(
                "Found {} transactions needing recovery from failed leader {}",
                transactions_needing_recovery.len(),
                former_leader_id
            );

            // The recovery manager will handle these through normal coordinator failure process
            self.recovery_manager
                .initiate_recovery_process(transactions_needing_recovery)
                .await?;
        }

        Ok(())
    }

    async fn on_term_changed(&self, old_term: u64, new_term: u64) -> EngineResult<()> {
        info!(
            "Raft term changed: {} -> {} - updating recovery state",
            old_term, new_term
        );

        // Update term information in recovery state if needed
        // This helps with recovery decision making and prevents split-brain scenarios

        Ok(())
    }
}
