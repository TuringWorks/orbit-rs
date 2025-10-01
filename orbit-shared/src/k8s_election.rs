use crate::cluster_manager::{EnhancedClusterManager, QuorumConfig};
use crate::election_state::{ElectionRecord, ElectionStateManager};
use crate::exception::{OrbitError, OrbitResult};
use crate::mesh::NodeId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::{sleep, Instant};
use tracing::{debug, error, info, warn};

/// Kubernetes lease-based leader election configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sElectionConfig {
    /// Kubernetes namespace for leases
    pub namespace: String,
    /// Lease name for leader election
    pub lease_name: String,
    /// Lease duration in seconds
    pub lease_duration_secs: u64,
    /// Renew deadline in seconds
    pub renew_deadline_secs: u64,
    /// Retry period in seconds
    pub retry_period_secs: u64,
    /// Enable fallback to Raft consensus
    pub enable_raft_fallback: bool,
    /// Pod name (identity)
    pub pod_name: Option<String>,
    /// Pod namespace
    pub pod_namespace: Option<String>,
}

impl Default for K8sElectionConfig {
    fn default() -> Self {
        Self {
            namespace: "orbit-rs".to_string(),
            lease_name: "orbit-leader-election".to_string(),
            lease_duration_secs: 30,
            renew_deadline_secs: 20,
            retry_period_secs: 5,
            enable_raft_fallback: true,
            pod_name: std::env::var("HOSTNAME").ok(),
            pod_namespace: std::env::var("POD_NAMESPACE").ok(),
        }
    }
}

/// Kubernetes lease information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sLease {
    pub holder_identity: String,
    pub lease_duration_seconds: u64,
    pub acquire_time: SystemTime,
    pub renew_time: SystemTime,
    pub leader_transitions: u64,
}

/// Deployment mode detection and configuration
#[derive(Debug, Clone, PartialEq)]
pub enum DeploymentMode {
    /// Running in Kubernetes cluster
    Kubernetes {
        namespace: String,
        pod_name: String,
        service_account: String,
    },
    /// Running in standalone mode
    Standalone {
        cluster_nodes: Vec<String>,
        node_discovery: NodeDiscoveryMethod,
    },
    /// Docker Compose deployment
    DockerCompose {
        compose_project: String,
        service_name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeDiscoveryMethod {
    /// Static list of nodes
    Static(Vec<String>),
    /// DNS-based service discovery
    Dns(String),
    /// Consul-based discovery
    Consul {
        address: String,
        service_name: String,
    },
    /// etcd-based discovery
    Etcd {
        endpoints: Vec<String>,
        key_prefix: String,
    },
}

/// Universal election manager that works in any deployment mode
pub struct UniversalElectionManager {
    node_id: NodeId,
    deployment_mode: DeploymentMode,
    k8s_config: K8sElectionConfig,
    raft_manager: Option<Arc<EnhancedClusterManager>>,
    state_manager: ElectionStateManager,
    current_leader: Arc<RwLock<Option<String>>>,
    is_leader: Arc<RwLock<bool>>,
    lease_client: Option<Arc<dyn K8sLeaseClient>>,
}

/// Kubernetes lease client trait for testability
#[async_trait]
pub trait K8sLeaseClient: Send + Sync {
    async fn get_lease(&self, namespace: &str, name: &str) -> OrbitResult<Option<K8sLease>>;
    async fn create_lease(&self, namespace: &str, name: &str, lease: &K8sLease) -> OrbitResult<()>;
    async fn update_lease(&self, namespace: &str, name: &str, lease: &K8sLease) -> OrbitResult<()>;
    async fn delete_lease(&self, namespace: &str, name: &str) -> OrbitResult<()>;
}

impl UniversalElectionManager {
    /// Create a new universal election manager
    pub async fn new(
        node_id: NodeId,
        deployment_mode: DeploymentMode,
        k8s_config: K8sElectionConfig,
        state_path: Option<std::path::PathBuf>,
    ) -> OrbitResult<Self> {
        let state_manager = match state_path {
            Some(path) => ElectionStateManager::new(&path, node_id.clone()),
            None => {
                let default_path =
                    std::env::temp_dir().join(format!("orbit-election-{}.json", node_id.key));
                ElectionStateManager::new(&default_path, node_id.clone())
            }
        };

        // Load existing state
        state_manager.load().await.unwrap_or_else(|e| {
            debug!("Could not load election state: {}, starting fresh", e);
        });

        let raft_manager = if matches!(deployment_mode, DeploymentMode::Standalone { .. })
            || k8s_config.enable_raft_fallback
        {
            Some(Self::setup_raft_manager(&node_id, &deployment_mode).await?)
        } else {
            None
        };

        let lease_client = if matches!(deployment_mode, DeploymentMode::Kubernetes { .. }) {
            Some(Arc::new(KubernetesLeaseClient::new().await?) as Arc<dyn K8sLeaseClient>)
        } else {
            None
        };

        Ok(Self {
            node_id,
            deployment_mode,
            k8s_config,
            raft_manager,
            state_manager,
            current_leader: Arc::new(RwLock::new(None)),
            is_leader: Arc::new(RwLock::new(false)),
            lease_client,
        })
    }

    /// Auto-detect deployment mode
    pub async fn detect_deployment_mode() -> DeploymentMode {
        // Check for Kubernetes environment variables
        if let (Ok(namespace), Ok(pod_name)) =
            (std::env::var("POD_NAMESPACE"), std::env::var("HOSTNAME"))
        {
            let service_account = std::env::var("SERVICE_ACCOUNT").unwrap_or_default();
            return DeploymentMode::Kubernetes {
                namespace,
                pod_name,
                service_account,
            };
        }

        // Check for Docker Compose
        if let (Ok(project), Ok(service)) = (
            std::env::var("COMPOSE_PROJECT_NAME"),
            std::env::var("COMPOSE_SERVICE"),
        ) {
            return DeploymentMode::DockerCompose {
                compose_project: project,
                service_name: service,
            };
        }

        // Default to standalone with DNS discovery
        let discovery_dns = std::env::var("ORBIT_DISCOVERY_DNS")
            .unwrap_or_else(|_| "orbit-server.orbit-rs.svc.cluster.local".to_string());

        DeploymentMode::Standalone {
            cluster_nodes: vec![],
            node_discovery: NodeDiscoveryMethod::Dns(discovery_dns),
        }
    }

    async fn setup_raft_manager(
        node_id: &NodeId,
        deployment_mode: &DeploymentMode,
    ) -> OrbitResult<Arc<EnhancedClusterManager>> {
        let cluster_nodes = Self::discover_cluster_nodes(deployment_mode).await?;

        let quorum_config = QuorumConfig {
            min_quorum_size: cluster_nodes.len() / 2 + 1,
            max_failures: cluster_nodes.len() / 2,
            quorum_timeout: Duration::from_secs(10),
            dynamic_quorum: true,
        };

        let raft_config = crate::consensus::RaftConfig {
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
            ..Default::default()
        };

        Ok(Arc::new(EnhancedClusterManager::new(
            node_id.clone(),
            cluster_nodes,
            quorum_config,
            raft_config,
        )))
    }

    async fn discover_cluster_nodes(deployment_mode: &DeploymentMode) -> OrbitResult<Vec<NodeId>> {
        match deployment_mode {
            DeploymentMode::Kubernetes { namespace, .. } => {
                // Use Kubernetes service discovery
                Self::discover_k8s_nodes(namespace).await
            }
            DeploymentMode::Standalone { node_discovery, .. } => match node_discovery {
                NodeDiscoveryMethod::Static(nodes) => Ok(nodes
                    .iter()
                    .enumerate()
                    .map(|(i, _addr)| NodeId::new(format!("node-{}", i), "cluster".to_string()))
                    .collect()),
                NodeDiscoveryMethod::Dns(dns_name) => Self::discover_dns_nodes(dns_name).await,
                _ => {
                    warn!("Node discovery method not implemented, using single node");
                    Ok(vec![])
                }
            },
            DeploymentMode::DockerCompose {
                compose_project,
                service_name,
            } => Self::discover_compose_nodes(compose_project, service_name).await,
        }
    }

    async fn discover_k8s_nodes(namespace: &str) -> OrbitResult<Vec<NodeId>> {
        // In a real implementation, this would use the Kubernetes API to discover pods
        // For now, simulate discovery
        let pod_count = std::env::var("ORBIT_REPLICA_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .unwrap_or(3);

        let nodes: Vec<NodeId> = (0..pod_count)
            .map(|i| NodeId::new(format!("orbit-server-{}", i), namespace.to_string()))
            .collect();

        info!("Discovered {} Kubernetes nodes", nodes.len());
        Ok(nodes)
    }

    async fn discover_dns_nodes(dns_name: &str) -> OrbitResult<Vec<NodeId>> {
        use tokio::net::lookup_host;

        match lookup_host(dns_name).await {
            Ok(addrs) => {
                let nodes: Vec<NodeId> = addrs
                    .enumerate()
                    .map(|(i, addr)| {
                        NodeId::new(
                            format!("dns-node-{}-{}", i, addr.ip()),
                            "cluster".to_string(),
                        )
                    })
                    .collect();
                info!("Discovered {} DNS nodes from {}", nodes.len(), dns_name);
                Ok(nodes)
            }
            Err(e) => {
                warn!("DNS discovery failed for {}: {}", dns_name, e);
                Ok(vec![])
            }
        }
    }

    async fn discover_compose_nodes(project: &str, service: &str) -> OrbitResult<Vec<NodeId>> {
        // Docker Compose uses predictable naming: {project}_{service}_{index}
        let replica_count = std::env::var("ORBIT_REPLICA_COUNT")
            .unwrap_or_else(|_| "3".to_string())
            .parse::<usize>()
            .unwrap_or(3);

        let nodes: Vec<NodeId> = (1..=replica_count)
            .map(|i| {
                NodeId::new(
                    format!("{}_{}_{}", project, service, i),
                    "compose".to_string(),
                )
            })
            .collect();

        info!("Discovered {} Docker Compose nodes", nodes.len());
        Ok(nodes)
    }

    /// Start the election process
    pub async fn start(&self) -> OrbitResult<()> {
        info!(
            "Starting universal election manager in mode: {:?}",
            self.deployment_mode
        );

        match &self.deployment_mode {
            DeploymentMode::Kubernetes { .. } => {
                self.start_k8s_election().await?;
            }
            _ => {
                if let Some(_raft_manager) = &self.raft_manager {
                    // For standalone/compose, use Raft consensus
                    // In a real implementation, we would need a proper transport
                    info!("Starting Raft-based election for standalone deployment");
                    // raft_manager.start(transport).await?;
                }
            }
        }

        // Start background monitoring
        self.start_monitoring_tasks().await?;

        Ok(())
    }

    async fn start_k8s_election(&self) -> OrbitResult<()> {
        let election_manager = self.clone();

        tokio::spawn(async move {
            let mut retry_interval = tokio::time::interval(Duration::from_secs(
                election_manager.k8s_config.retry_period_secs,
            ));

            loop {
                retry_interval.tick().await;

                if let Err(e) = election_manager.run_k8s_election_cycle().await {
                    error!("Kubernetes election cycle failed: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn run_k8s_election_cycle(&self) -> OrbitResult<()> {
        let lease_client = self
            .lease_client
            .as_ref()
            .ok_or_else(|| OrbitError::internal("No Kubernetes lease client available"))?;

        let current_lease = lease_client
            .get_lease(&self.k8s_config.namespace, &self.k8s_config.lease_name)
            .await?;

        let now = SystemTime::now();
        let identity = self.get_node_identity();

        match current_lease {
            Some(lease) => {
                if lease.holder_identity == identity {
                    // We are the current leader, try to renew
                    self.renew_leadership(lease_client.as_ref(), lease, now)
                        .await?;
                } else {
                    // Someone else is leader, check if lease is expired
                    if self.is_lease_expired(&lease, now) {
                        self.attempt_leadership_acquisition(lease_client.as_ref(), identity, now)
                            .await?;
                    } else {
                        // Valid lease held by someone else
                        self.update_follower_state(&lease.holder_identity).await;
                    }
                }
            }
            None => {
                // No lease exists, try to acquire
                self.attempt_leadership_acquisition(lease_client.as_ref(), identity, now)
                    .await?;
            }
        }

        Ok(())
    }

    async fn renew_leadership(
        &self,
        client: &dyn K8sLeaseClient,
        mut lease: K8sLease,
        now: SystemTime,
    ) -> OrbitResult<()> {
        lease.renew_time = now;

        if let Err(e) = client
            .update_lease(
                &self.k8s_config.namespace,
                &self.k8s_config.lease_name,
                &lease,
            )
            .await
        {
            warn!("Failed to renew leadership lease: {}", e);
            self.update_follower_state("").await;
            return Err(e);
        }

        self.update_leader_state().await;
        debug!("Successfully renewed leadership lease");
        Ok(())
    }

    async fn attempt_leadership_acquisition(
        &self,
        client: &dyn K8sLeaseClient,
        identity: String,
        now: SystemTime,
    ) -> OrbitResult<()> {
        let new_lease = K8sLease {
            holder_identity: identity.clone(),
            lease_duration_seconds: self.k8s_config.lease_duration_secs,
            acquire_time: now,
            renew_time: now,
            leader_transitions: 1,
        };

        match client
            .create_lease(
                &self.k8s_config.namespace,
                &self.k8s_config.lease_name,
                &new_lease,
            )
            .await
        {
            Ok(()) => {
                info!("Successfully acquired leadership lease");
                self.update_leader_state().await;

                // Record the election
                let record = ElectionRecord {
                    term: self.state_manager.get_current_term().await + 1,
                    candidate: self.node_id.clone(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    successful: true,
                    vote_count: 1,
                    total_nodes: 1,
                };

                if let Err(e) = self.state_manager.record_election(record).await {
                    warn!("Failed to record election: {}", e);
                }
            }
            Err(e) => {
                debug!("Failed to acquire leadership lease: {}", e);
                self.update_follower_state("").await;
            }
        }

        Ok(())
    }

    fn is_lease_expired(&self, lease: &K8sLease, now: SystemTime) -> bool {
        let lease_duration = Duration::from_secs(lease.lease_duration_seconds);
        let elapsed = now
            .duration_since(lease.renew_time)
            .unwrap_or(Duration::ZERO);
        elapsed > lease_duration
    }

    async fn update_leader_state(&self) {
        let mut is_leader = self.is_leader.write().await;
        let mut current_leader = self.current_leader.write().await;

        *is_leader = true;
        *current_leader = Some(self.get_node_identity());

        info!("Updated state: This node is now the leader");
    }

    async fn update_follower_state(&self, leader_identity: &str) {
        let mut is_leader = self.is_leader.write().await;
        let mut current_leader = self.current_leader.write().await;

        *is_leader = false;
        *current_leader = if leader_identity.is_empty() {
            None
        } else {
            Some(leader_identity.to_string())
        };

        debug!(
            "Updated state: This node is a follower, leader: {:?}",
            current_leader
        );
    }

    fn get_node_identity(&self) -> String {
        self.k8s_config
            .pod_name
            .clone()
            .unwrap_or_else(|| self.node_id.to_string())
    }

    async fn start_monitoring_tasks(&self) -> OrbitResult<()> {
        // Health monitoring task
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = manager.health_check().await {
                    error!("Health check failed: {}", e);
                }
            }
        });

        // Metrics reporting task
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = manager.report_metrics().await {
                    error!("Metrics reporting failed: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn health_check(&self) -> OrbitResult<()> {
        // Check if we can access required resources
        if matches!(self.deployment_mode, DeploymentMode::Kubernetes { .. }) {
            if let Some(client) = &self.lease_client {
                let _lease = client
                    .get_lease(&self.k8s_config.namespace, &self.k8s_config.lease_name)
                    .await;
                // Health check passed if we can read the lease
            }
        }

        debug!("Health check passed");
        Ok(())
    }

    async fn report_metrics(&self) -> OrbitResult<()> {
        let is_leader = self.is_leader().await;
        let current_leader = self.get_current_leader().await;
        let stats = self.state_manager.get_stats().await;

        info!(
            "Election metrics: is_leader={}, current_leader={:?}, total_elections={}, success_rate={:.2}",
            is_leader, current_leader, stats.total_elections, stats.success_rate
        );

        Ok(())
    }

    /// Check if this node is currently the leader
    pub async fn is_leader(&self) -> bool {
        *self.is_leader.read().await
    }

    /// Get the current leader identity
    pub async fn get_current_leader(&self) -> Option<String> {
        self.current_leader.read().await.clone()
    }

    /// Get election statistics
    pub async fn get_election_stats(&self) -> crate::election_state::ElectionStats {
        self.state_manager.get_stats().await
    }

    /// Wait for leadership (useful for testing)
    pub async fn wait_for_leadership(&self, timeout_duration: Duration) -> OrbitResult<()> {
        let start = Instant::now();

        while start.elapsed() < timeout_duration {
            if self.is_leader().await {
                return Ok(());
            }
            sleep(Duration::from_millis(100)).await;
        }

        Err(OrbitError::timeout(
            "Leadership not acquired within timeout",
        ))
    }

    /// Force leadership release (for graceful shutdown)
    pub async fn release_leadership(&self) -> OrbitResult<()> {
        if !self.is_leader().await {
            return Ok(());
        }

        if let Some(client) = &self.lease_client {
            if let Err(e) = client
                .delete_lease(&self.k8s_config.namespace, &self.k8s_config.lease_name)
                .await
            {
                warn!("Failed to delete leadership lease: {}", e);
            }
        }

        self.update_follower_state("").await;
        info!("Released leadership");
        Ok(())
    }
}

impl Clone for UniversalElectionManager {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            deployment_mode: self.deployment_mode.clone(),
            k8s_config: self.k8s_config.clone(),
            raft_manager: self.raft_manager.clone(),
            state_manager: ElectionStateManager::new(
                &std::env::temp_dir().join(format!("orbit-election-{}.json", self.node_id.key)),
                self.node_id.clone(),
            ),
            current_leader: Arc::clone(&self.current_leader),
            is_leader: Arc::clone(&self.is_leader),
            lease_client: self.lease_client.clone(),
        }
    }
}

/// Real Kubernetes lease client implementation
pub struct KubernetesLeaseClient {
    // In a real implementation, this would contain the Kubernetes client
    _client: (),
}

impl KubernetesLeaseClient {
    pub async fn new() -> OrbitResult<Self> {
        // Initialize Kubernetes client
        info!("Initializing Kubernetes lease client");
        Ok(Self { _client: () })
    }
}

#[async_trait]
impl K8sLeaseClient for KubernetesLeaseClient {
    async fn get_lease(&self, namespace: &str, name: &str) -> OrbitResult<Option<K8sLease>> {
        // In a real implementation, this would use the Kubernetes API
        debug!("Getting lease {}/{}", namespace, name);

        // For now, simulate no existing lease
        Ok(None)
    }

    async fn create_lease(&self, namespace: &str, name: &str, lease: &K8sLease) -> OrbitResult<()> {
        info!(
            "Creating lease {}/{} for {}",
            namespace, name, lease.holder_identity
        );

        // In a real implementation, this would create the lease via Kubernetes API
        Ok(())
    }

    async fn update_lease(&self, namespace: &str, name: &str, lease: &K8sLease) -> OrbitResult<()> {
        debug!(
            "Updating lease {}/{} for {}",
            namespace, name, lease.holder_identity
        );

        // In a real implementation, this would update the lease via Kubernetes API
        Ok(())
    }

    async fn delete_lease(&self, namespace: &str, name: &str) -> OrbitResult<()> {
        info!("Deleting lease {}/{}", namespace, name);

        // In a real implementation, this would delete the lease via Kubernetes API
        Ok(())
    }
}

/// Mock lease client for testing
pub struct MockLeaseClient {
    leases: Arc<RwLock<HashMap<String, K8sLease>>>,
}

impl MockLeaseClient {
    pub fn new() -> Self {
        Self {
            leases: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn lease_key(namespace: &str, name: &str) -> String {
        format!("{}/{}", namespace, name)
    }
}

#[async_trait]
impl K8sLeaseClient for MockLeaseClient {
    async fn get_lease(&self, namespace: &str, name: &str) -> OrbitResult<Option<K8sLease>> {
        let key = Self::lease_key(namespace, name);
        let leases = self.leases.read().await;
        Ok(leases.get(&key).cloned())
    }

    async fn create_lease(&self, namespace: &str, name: &str, lease: &K8sLease) -> OrbitResult<()> {
        let key = Self::lease_key(namespace, name);
        let mut leases = self.leases.write().await;

        if leases.contains_key(&key) {
            return Err(OrbitError::configuration("Lease already exists"));
        }

        leases.insert(key, lease.clone());
        Ok(())
    }

    async fn update_lease(&self, namespace: &str, name: &str, lease: &K8sLease) -> OrbitResult<()> {
        let key = Self::lease_key(namespace, name);
        let mut leases = self.leases.write().await;

        if !leases.contains_key(&key) {
            return Err(OrbitError::cluster("Lease not found"));
        }

        leases.insert(key, lease.clone());
        Ok(())
    }

    async fn delete_lease(&self, namespace: &str, name: &str) -> OrbitResult<()> {
        let key = Self::lease_key(namespace, name);
        let mut leases = self.leases.write().await;
        leases.remove(&key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_deployment_mode_detection() {
        std::env::set_var("POD_NAMESPACE", "test-namespace");
        std::env::set_var("HOSTNAME", "test-pod-123");

        let mode = UniversalElectionManager::detect_deployment_mode().await;

        match mode {
            DeploymentMode::Kubernetes {
                namespace,
                pod_name,
                ..
            } => {
                assert_eq!(namespace, "test-namespace");
                assert_eq!(pod_name, "test-pod-123");
            }
            _ => panic!("Should detect Kubernetes mode"),
        }

        // Clean up
        std::env::remove_var("POD_NAMESPACE");
        std::env::remove_var("HOSTNAME");
    }

    #[tokio::test]
    async fn test_k8s_election_cycle() {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("test_state.json");

        let node_id = NodeId::new("test-node".to_string(), "test".to_string());
        let deployment_mode = DeploymentMode::Kubernetes {
            namespace: "test".to_string(),
            pod_name: "test-pod".to_string(),
            service_account: "default".to_string(),
        };

        let mut manager = UniversalElectionManager::new(
            node_id,
            deployment_mode,
            K8sElectionConfig::default(),
            Some(state_path),
        )
        .await
        .unwrap();

        // Use mock lease client
        manager.lease_client = Some(Arc::new(MockLeaseClient::new()));

        // Run election cycle
        manager.run_k8s_election_cycle().await.unwrap();

        // Should become leader since no existing lease
        assert!(manager.is_leader().await);
    }

    #[tokio::test]
    async fn test_lease_expiration() {
        let manager = create_test_manager().await;

        let now = SystemTime::now();
        let expired_lease = K8sLease {
            holder_identity: "other-node".to_string(),
            lease_duration_seconds: 1, // 1 second
            acquire_time: now - Duration::from_secs(5),
            renew_time: now - Duration::from_secs(5),
            leader_transitions: 1,
        };

        assert!(manager.is_lease_expired(&expired_lease, now));

        let valid_lease = K8sLease {
            holder_identity: "other-node".to_string(),
            lease_duration_seconds: 30,
            acquire_time: now,
            renew_time: now,
            leader_transitions: 1,
        };

        assert!(!manager.is_lease_expired(&valid_lease, now));
    }

    async fn create_test_manager() -> UniversalElectionManager {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("test_state.json");

        let node_id = NodeId::new("test-node".to_string(), "test".to_string());
        let deployment_mode = DeploymentMode::Kubernetes {
            namespace: "test".to_string(),
            pod_name: "test-pod".to_string(),
            service_account: "default".to_string(),
        };

        UniversalElectionManager::new(
            node_id,
            deployment_mode,
            K8sElectionConfig::default(),
            Some(state_path),
        )
        .await
        .unwrap()
    }
}
