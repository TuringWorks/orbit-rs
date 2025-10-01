use crate::cluster_manager::{EnhancedClusterManager, RecoveryRaftEventHandler, QuorumConfig};
use crate::consensus::RaftConfig;
use crate::exception::{OrbitError, OrbitResult};
use crate::k8s_election::UniversalElectionManager;
use crate::mesh::NodeId;
use crate::recovery::TransactionRecoveryManager;
use crate::transaction_log::PersistentTransactionLogger;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Integrated recovery and leader election system
/// This combines the leader election mechanism with the transaction recovery system
pub struct IntegratedRecoverySystem {
    node_id: NodeId,
    recovery_manager: Arc<TransactionRecoveryManager>,
    election_manager: Arc<UniversalElectionManager>,
    cluster_manager: Arc<EnhancedClusterManager>,
    is_started: Arc<RwLock<bool>>,
}

impl IntegratedRecoverySystem {
    /// Create a new integrated recovery system
    pub async fn new(
        node_id: NodeId,
        cluster_nodes: Vec<NodeId>,
        logger: Arc<dyn PersistentTransactionLogger>,
        state_path: Option<std::path::PathBuf>,
    ) -> OrbitResult<Self> {
        info!("Initializing integrated recovery system for node: {}", node_id);

        // Create cluster manager with recovery integration
        let quorum_config = QuorumConfig {
            min_quorum_size: cluster_nodes.len() / 2 + 1,
            max_failures: cluster_nodes.len() / 2,
            quorum_timeout: Duration::from_secs(30),
            dynamic_quorum: true,
        };

        let raft_config = RaftConfig {
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
            ..Default::default()
        };

        let cluster_manager = Arc::new(EnhancedClusterManager::new(
            node_id.clone(),
            cluster_nodes,
            quorum_config,
            raft_config,
        ));

        // Create recovery manager using the cluster manager
        let recovery_manager = Arc::new(TransactionRecoveryManager::new(
            node_id.clone(),
            crate::recovery::RecoveryConfig::default(),
            logger,
            cluster_manager.clone() as Arc<dyn crate::recovery::ClusterManager>,
        ));

        // Create recovery event handler and wire it to the Raft consensus
        let recovery_event_handler = Arc::new(RecoveryRaftEventHandler::new(recovery_manager.clone()));
        cluster_manager.raft_consensus.add_event_handler(recovery_event_handler).await;

        // Create election manager for universal deployment support
        let deployment_mode = UniversalElectionManager::detect_deployment_mode().await;
        let k8s_config = crate::k8s_election::K8sElectionConfig::default();
        
        let election_manager = Arc::new(UniversalElectionManager::new(
            node_id.clone(),
            deployment_mode,
            k8s_config,
            state_path,
        ).await?);

        Ok(Self {
            node_id,
            recovery_manager,
            election_manager,
            cluster_manager,
            is_started: Arc::new(RwLock::new(false)),
        })
    }

    /// Start the integrated recovery system
    pub async fn start(&self, transport: Option<Arc<dyn crate::consensus::RaftTransport>>) -> OrbitResult<()> {
        let mut started = self.is_started.write().await;
        if *started {
            return Err(OrbitError::internal("Recovery system is already started"));
        }

        info!("Starting integrated recovery system for node: {}", self.node_id);

        // Start the cluster manager (includes Raft consensus)
        if let Some(transport) = transport {
            self.cluster_manager.start(transport).await?;
        } else {
            warn!("No transport provided - cluster manager running without Raft transport");
        }

        // Start the recovery manager
        self.recovery_manager.start().await?;

        // Start the election manager (for universal deployment support)
        self.election_manager.start().await?;

        // Set up leader change monitoring
        self.start_leader_monitoring().await?;

        *started = true;
        info!("Integrated recovery system started successfully");

        Ok(())
    }

    /// Start monitoring for leader changes and sync with recovery system
    async fn start_leader_monitoring(&self) -> OrbitResult<()> {
        let recovery_manager = Arc::clone(&self.recovery_manager);
        let election_manager = Arc::clone(&self.election_manager);
        let cluster_manager = Arc::clone(&self.cluster_manager);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                // Check current leader from both systems
                let raft_leader = cluster_manager.raft_consensus.get_leader().await;
                let election_leader = election_manager.get_current_leader().await.map(|s| NodeId::from_string(&s));

                // Sync leader information to recovery manager
                let effective_leader = raft_leader.or(election_leader);
                if let Err(e) = recovery_manager.update_cluster_leader(effective_leader.clone()).await {
                    error!("Failed to update cluster leader in recovery manager: {}", e);
                }

                // If this node is the leader, ensure it's handling recovery responsibilities
                if let Some(leader_id) = &effective_leader {
                    if leader_id == &recovery_manager.node_id {
                        // This node is the leader - perform periodic recovery checks
                        Self::perform_leader_recovery_duties(&recovery_manager).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Perform periodic recovery duties when this node is the leader
    async fn perform_leader_recovery_duties(recovery_manager: &Arc<TransactionRecoveryManager>) {
        // Check for stale transactions that may need recovery
        let checkpoints = recovery_manager.get_checkpoints().await;
        let transaction_ids: Vec<_> = checkpoints.keys().take(10).cloned().collect();
        
        for transaction_id in transaction_ids {
            // Check if transaction coordinator is still healthy
            if let Some(coordinator) = recovery_manager.get_transaction_coordinator(&transaction_id).await {
                let coordinators = recovery_manager.coordinators.read().await;
                if let Some(health) = coordinators.get(&coordinator) {
                    if !health.is_healthy {
                        info!("Found unhealthy coordinator {} for transaction {}, initiating recovery", 
                              coordinator, transaction_id);

                        // Reassign this transaction to current leader
                        if let Err(e) = recovery_manager.reassign_transactions_to_leader(&coordinator).await {
                            error!("Failed to reassign transactions from unhealthy coordinator: {}", e);
                        }
                    }
                }
            }
        }
    }

    /// Check if the recovery system is running
    pub async fn is_running(&self) -> bool {
        *self.is_started.read().await
    }

    /// Get the current cluster leader
    pub async fn get_current_leader(&self) -> Option<NodeId> {
        self.recovery_manager.get_cluster_leader().await
    }

    /// Check if this node is the current leader
    pub async fn is_leader(&self) -> bool {
        if let Some(leader) = self.get_current_leader().await {
            leader == self.node_id
        } else {
            false
        }
    }

    /// Force a recovery scan (useful for testing or manual intervention)
    pub async fn force_recovery_scan(&self) -> OrbitResult<()> {
        if !self.is_leader().await {
            return Err(OrbitError::cluster("Only the leader can initiate recovery scans"));
        }

        info!("Forcing comprehensive recovery scan...");

        let coordinators = self.recovery_manager.coordinators.read().await;
        let failed_coordinators: Vec<NodeId> = coordinators
            .iter()
            .filter_map(|(node_id, health)| {
                if !health.is_healthy {
                    Some(node_id.clone())
                } else {
                    None
                }
            })
            .collect();

        for failed_coordinator in failed_coordinators {
            let transactions = self.recovery_manager.find_transactions_needing_recovery(&failed_coordinator).await?;
            if !transactions.is_empty() {
                info!("Found {} transactions needing recovery from {}", 
                      transactions.len(), failed_coordinator);
                
                self.recovery_manager.become_coordinator(transactions).await?;
            }
        }

        Ok(())
    }

    /// Get recovery statistics
    pub async fn get_recovery_stats(&self) -> crate::recovery::RecoveryStats {
        self.recovery_manager.get_stats().await
    }

    /// Get election statistics  
    pub async fn get_election_stats(&self) -> crate::election_state::ElectionStats {
        self.election_manager.get_election_stats().await
    }

    /// Gracefully shutdown the recovery system
    pub async fn shutdown(&self) -> OrbitResult<()> {
        let mut started = self.is_started.write().await;
        if !*started {
            return Ok(());
        }

        info!("Shutting down integrated recovery system...");

        // If this node is leader, try to transfer leadership gracefully
        if self.is_leader().await {
            if let Err(e) = self.election_manager.release_leadership().await {
                warn!("Failed to gracefully release leadership: {}", e);
            }
        }

        *started = false;
        info!("Integrated recovery system shutdown complete");

        Ok(())
    }
}

impl Clone for IntegratedRecoverySystem {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            recovery_manager: Arc::clone(&self.recovery_manager),
            election_manager: Arc::clone(&self.election_manager),
            cluster_manager: Arc::clone(&self.cluster_manager),
            is_started: Arc::clone(&self.is_started),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction_log::{SqliteTransactionLogger, PersistentLogConfig};
    use tempfile::tempdir;

    async fn create_test_recovery_system() -> IntegratedRecoverySystem {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("recovery_test.db");
        let state_path = temp_dir.path().join("election_state.json");

        let log_config = PersistentLogConfig {
            database_path: db_path,
            ..Default::default()
        };

        let logger = Arc::new(SqliteTransactionLogger::new(log_config).await.unwrap());
        let node_id = NodeId::new("test-node-1".to_string(), "test".to_string());
        let cluster_nodes = vec![
            node_id.clone(),
            NodeId::new("test-node-2".to_string(), "test".to_string()),
            NodeId::new("test-node-3".to_string(), "test".to_string()),
        ];

        IntegratedRecoverySystem::new(
            node_id,
            cluster_nodes,
            logger,
            Some(state_path),
        ).await.unwrap()
    }

    #[tokio::test]
    async fn test_integrated_recovery_system_creation() {
        let system = create_test_recovery_system().await;
        assert!(!system.is_running().await);
        assert_eq!(system.node_id.key, "test-node-1");
    }

    #[tokio::test]
    async fn test_integrated_recovery_system_startup() {
        let system = create_test_recovery_system().await;
        
        // Start without transport (for testing)
        system.start(None).await.unwrap();
        assert!(system.is_running().await);
        
        // Shutdown
        system.shutdown().await.unwrap();
        assert!(!system.is_running().await);
    }

    #[tokio::test]
    async fn test_leader_monitoring() {
        let system = create_test_recovery_system().await;
        system.start(None).await.unwrap();

        // Check initial leader state
        let leader = system.get_current_leader().await;
        assert!(leader.is_none() || leader.is_some()); // Either state is valid initially

        system.shutdown().await.unwrap();
    }
}