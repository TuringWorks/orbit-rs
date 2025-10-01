use orbit_shared::consensus::{RaftConsensus, RaftConfig, RaftEventHandler};
use orbit_shared::cluster_manager::{EnhancedClusterManager, QuorumConfig};
use orbit_shared::election_state::{ElectionStateManager, ElectionRecord};
use orbit_shared::election_metrics::{ElectionMetrics, ElectionOutcome};
use orbit_shared::raft_transport::GrpcRaftTransport;
use orbit_shared::{NodeId, OrbitResult, OrbitError};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::time::{timeout, sleep};
use tracing::{info, warn, error};
use tempfile::{tempdir, TempDir};

/// Test cluster configuration
#[derive(Debug, Clone)]
struct TestClusterConfig {
    pub node_count: usize,
    pub partition_size: usize,
    pub election_timeout_min: Duration,
    pub election_timeout_max: Duration,
    pub heartbeat_interval: Duration,
}

impl Default for TestClusterConfig {
    fn default() -> Self {
        Self {
            node_count: 5,
            partition_size: 2,
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
        }
    }
}

/// Test node wrapper with metrics and state
struct TestNode {
    pub node_id: NodeId,
    pub consensus: Arc<RaftConsensus>,
    pub cluster_manager: Arc<EnhancedClusterManager>,
    pub state_manager: ElectionStateManager,
    pub metrics: ElectionMetrics,
    pub transport: Arc<GrpcRaftTransport>,
    pub _temp_dir: TempDir,
    pub port: u16,
}

impl TestNode {
    pub async fn new(node_id: NodeId, cluster_nodes: Vec<NodeId>, port: u16) -> OrbitResult<Self> {
        let temp_dir = tempdir().map_err(|e| OrbitError::io(format!("Failed to create temp dir: {}", e)))?;
        let state_path = temp_dir.path().join("election_state.json");
        
        let raft_config = RaftConfig {
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
            ..Default::default()
        };
        
        let quorum_config = QuorumConfig {
            min_quorum_size: 3,
            max_failures: 2,
            quorum_timeout: Duration::from_secs(10),
            dynamic_quorum: false,
        };
        
        let consensus = Arc::new(RaftConsensus::new(
            node_id.clone(),
            cluster_nodes.clone(),
            raft_config.clone(),
        ));
        
        let cluster_manager = Arc::new(EnhancedClusterManager::new(
            node_id.clone(),
            cluster_nodes.clone(),
            quorum_config,
            raft_config,
        ));
        
        let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
        let metrics = ElectionMetrics::new();
        
        // Create node address mapping
        let mut node_addresses = HashMap::new();
        for (i, node) in cluster_nodes.iter().enumerate() {
            let node_port = 50051 + i as u16;
            node_addresses.insert(node.clone(), format!("http://127.0.0.1:{}", node_port));
        }
        
        let transport = Arc::new(GrpcRaftTransport::new(
            node_id.clone(),
            node_addresses,
            Duration::from_secs(5),
            Duration::from_secs(10),
        ));
        
        Ok(Self {
            node_id,
            consensus,
            cluster_manager,
            state_manager,
            metrics,
            transport,
            _temp_dir: temp_dir,
            port,
        })
    }
    
    pub async fn start(&self) -> OrbitResult<()> {
        self.state_manager.load().await?;
        self.consensus.start(self.transport.clone()).await?;
        self.cluster_manager.start(self.transport.clone()).await?;
        Ok(())
    }
    
    pub async fn is_leader(&self) -> bool {
        self.consensus.is_leader().await
    }
    
    pub async fn get_current_term(&self) -> u64 {
        self.consensus.get_current_term().await
    }
    
    pub async fn simulate_network_partition(&self, partitioned_nodes: &[NodeId]) {
        for node in partitioned_nodes {
            self.transport.update_node_address(node.clone(), "http://127.0.0.1:1".to_string()).await;
        }
    }
    
    pub async fn heal_network_partition(&self, node_addresses: &HashMap<NodeId, String>) {
        for (node, address) in node_addresses {
            self.transport.update_node_address(node.clone(), address.clone()).await;
        }
    }
}

/// Test cluster manager for orchestrating multi-node tests
struct TestCluster {
    nodes: Vec<TestNode>,
    config: TestClusterConfig,
    original_addresses: HashMap<NodeId, String>,
}

impl TestCluster {
    pub async fn new(config: TestClusterConfig) -> OrbitResult<Self> {
        let mut nodes = Vec::new();
        let mut node_ids = Vec::new();
        
        // Generate node IDs
        for i in 0..config.node_count {
            let node_id = NodeId::new(format!("node-{}", i), "test-cluster".to_string());
            node_ids.push(node_id);
        }
        
        let mut original_addresses = HashMap::new();
        
        // Create test nodes
        for (i, node_id) in node_ids.iter().enumerate() {
            let port = 50051 + i as u16;
            original_addresses.insert(node_id.clone(), format!("http://127.0.0.1:{}", port));
            
            let node = TestNode::new(node_id.clone(), node_ids.clone(), port).await?;
            nodes.push(node);
        }
        
        Ok(Self {
            nodes,
            config,
            original_addresses,
        })
    }
    
    pub async fn start_all(&mut self) -> OrbitResult<()> {
        for node in &self.nodes {
            node.start().await?;
        }
        
        // Give nodes time to initialize
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }
    
    pub async fn wait_for_leader(&self, timeout_duration: Duration) -> OrbitResult<NodeId> {
        let start = Instant::now();
        
        while start.elapsed() < timeout_duration {
            for node in &self.nodes {
                if node.is_leader().await {
                    return Ok(node.node_id.clone());
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
        
        Err(OrbitError::timeout("No leader elected within timeout"))
    }
    
    pub async fn get_leaders(&self) -> Vec<&TestNode> {
        let mut leaders = Vec::new();
        for node in &self.nodes {
            if node.is_leader().await {
                leaders.push(node);
            }
        }
        leaders
    }
    
    pub async fn simulate_network_partition(&self, partition1: &[usize], partition2: &[usize]) {
        // Partition 1 nodes can't reach partition 2 nodes
        for &i in partition1 {
            let partitioned_nodes: Vec<NodeId> = partition2.iter()
                .map(|&j| self.nodes[j].node_id.clone())
                .collect();
            self.nodes[i].simulate_network_partition(&partitioned_nodes).await;
        }
        
        // Partition 2 nodes can't reach partition 1 nodes  
        for &i in partition2 {
            let partitioned_nodes: Vec<NodeId> = partition1.iter()
                .map(|&j| self.nodes[j].node_id.clone())
                .collect();
            self.nodes[i].simulate_network_partition(&partitioned_nodes).await;
        }
    }
    
    pub async fn heal_network_partition(&self) {
        for node in &self.nodes {
            node.heal_network_partition(&self.original_addresses).await;
        }
    }
    
    pub async fn stop_node(&self, node_index: usize) {
        // In a real implementation, this would stop the node's processes
        // For now, we simulate by creating network partition
        let node_id = &self.nodes[node_index].node_id;
        let other_nodes: Vec<NodeId> = self.nodes.iter()
            .enumerate()
            .filter(|(i, _)| *i != node_index)
            .map(|(_, n)| n.node_id.clone())
            .collect();
        
        self.nodes[node_index].simulate_network_partition(&other_nodes).await;
    }
}

/// Basic leader election test
#[tokio::test]
async fn test_basic_leader_election() {
    tracing_subscriber::fmt::init();
    
    let config = TestClusterConfig::default();
    let mut cluster = TestCluster::new(config).await.unwrap();
    
    // Start all nodes
    cluster.start_all().await.unwrap();
    
    // Wait for a leader to be elected
    let leader_id = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();
    info!("Leader elected: {}", leader_id);
    
    // Verify exactly one leader
    let leaders = cluster.get_leaders().await;
    assert_eq!(leaders.len(), 1, "Should have exactly one leader");
    assert_eq!(leaders[0].node_id, leader_id, "Leader should match expected");
    
    // Verify all nodes have the same term
    let leader_term = leaders[0].get_current_term().await;
    for node in &cluster.nodes {
        assert_eq!(
            node.get_current_term().await,
            leader_term,
            "All nodes should have same term"
        );
    }
}

/// Test leader failure and re-election
#[tokio::test]
async fn test_leader_failure_recovery() {
    tracing_subscriber::fmt::init();
    
    let config = TestClusterConfig::default();
    let mut cluster = TestCluster::new(config).await.unwrap();
    
    cluster.start_all().await.unwrap();
    
    // Wait for initial leader
    let initial_leader = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();
    info!("Initial leader: {}", initial_leader);
    
    // Find the leader node index
    let leader_index = cluster.nodes.iter()
        .position(|n| n.node_id == initial_leader)
        .unwrap();
    
    // Simulate leader failure
    cluster.stop_node(leader_index).await;
    info!("Stopped leader node: {}", initial_leader);
    
    // Wait for new leader election
    sleep(Duration::from_secs(2)).await;
    
    let new_leader = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();
    info!("New leader elected: {}", new_leader);
    
    // Verify new leader is different
    assert_ne!(initial_leader, new_leader, "New leader should be different");
    
    // Verify exactly one leader
    let leaders = cluster.get_leaders().await;
    assert_eq!(leaders.len(), 1, "Should have exactly one leader after recovery");
}

/// Test split-brain prevention
#[tokio::test]
async fn test_split_brain_prevention() {
    tracing_subscriber::fmt::init();
    
    let config = TestClusterConfig::default();
    let mut cluster = TestCluster::new(config).await.unwrap();
    
    cluster.start_all().await.unwrap();
    
    // Wait for initial leader
    let _initial_leader = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();
    
    // Create network partition: [0,1] vs [2,3,4]
    cluster.simulate_network_partition(&[0, 1], &[2, 3, 4]).await;
    info!("Created network partition: [0,1] vs [2,3,4]");
    
    // Wait for partition to take effect
    sleep(Duration::from_secs(5)).await;
    
    // Check leaders in each partition
    let partition1_leaders = cluster.nodes[0..2].iter()
        .filter(|n| futures::executor::block_on(n.is_leader()))
        .count();
    
    let partition2_leaders = cluster.nodes[2..5].iter()
        .filter(|n| futures::executor::block_on(n.is_leader()))
        .count();
    
    info!("Partition 1 leaders: {}, Partition 2 leaders: {}", partition1_leaders, partition2_leaders);
    
    // The minority partition (2 nodes) should not elect a leader
    // The majority partition (3 nodes) should have exactly 1 leader
    assert_eq!(partition1_leaders, 0, "Minority partition should have no leader");
    assert_eq!(partition2_leaders, 1, "Majority partition should have exactly one leader");
    
    // Heal the network partition
    cluster.heal_network_partition().await;
    info!("Healed network partition");
    
    // Wait for convergence
    sleep(Duration::from_secs(3)).await;
    
    // Verify exactly one leader across the whole cluster
    let leaders = cluster.get_leaders().await;
    assert_eq!(leaders.len(), 1, "Should have exactly one leader after healing");
}

/// Test multiple concurrent elections
#[tokio::test]
async fn test_concurrent_elections() {
    tracing_subscriber::fmt::init();
    
    let config = TestClusterConfig {
        node_count: 7,
        election_timeout_min: Duration::from_millis(100),
        election_timeout_max: Duration::from_millis(200),
        ..Default::default()
    };
    
    let mut cluster = TestCluster::new(config).await.unwrap();
    cluster.start_all().await.unwrap();
    
    // Wait for initial leader
    let _initial_leader = cluster.wait_for_leader(Duration::from_secs(15)).await.unwrap();
    
    // Verify exactly one leader despite concurrent elections
    let leaders = cluster.get_leaders().await;
    assert_eq!(leaders.len(), 1, "Should converge to exactly one leader");
    
    // Check that all nodes agree on the same term
    let leader_term = leaders[0].get_current_term().await;
    for node in &cluster.nodes {
        assert_eq!(
            node.get_current_term().await,
            leader_term,
            "All nodes should converge to same term"
        );
    }
}

/// Test election with network delays
#[tokio::test]
async fn test_election_with_network_delays() {
    tracing_subscriber::fmt::init();
    
    let config = TestClusterConfig::default();
    let mut cluster = TestCluster::new(config).await.unwrap();
    
    cluster.start_all().await.unwrap();
    
    // Simulate network delays by creating temporary partitions
    for i in 0..3 {
        // Create temporary partition
        cluster.simulate_network_partition(&[i], &[(i+1) % 5, (i+2) % 5]).await;
        sleep(Duration::from_millis(100)).await;
        
        // Heal partition
        cluster.heal_network_partition().await;
        sleep(Duration::from_millis(100)).await;
    }
    
    // Wait for election to complete despite network instability
    let leader = cluster.wait_for_leader(Duration::from_secs(15)).await.unwrap();
    info!("Leader elected despite network delays: {}", leader);
    
    let leaders = cluster.get_leaders().await;
    assert_eq!(leaders.len(), 1, "Should have exactly one leader despite network issues");
}

/// Test election state persistence
#[tokio::test]
async fn test_election_state_persistence() {
    let temp_dir = tempdir().unwrap();
    let state_path = temp_dir.path().join("test_election_state.json");
    let node_id = NodeId::new("persistence-test".to_string(), "test".to_string());
    
    // Create state manager and record some elections
    {
        let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
        
        state_manager.update_term(5).await.unwrap();
        state_manager.vote_for(node_id.clone(), 5).await.unwrap();
        
        let record = ElectionRecord {
            term: 5,
            candidate: node_id.clone(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            successful: true,
            vote_count: 3,
            total_nodes: 5,
        };
        
        state_manager.record_election(record).await.unwrap();
    }
    
    // Verify persistence by loading in new instance
    {
        let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
        state_manager.load().await.unwrap();
        
        assert_eq!(state_manager.get_current_term().await, 5);
        assert_eq!(state_manager.get_voted_for().await, Some(node_id));
        
        let stats = state_manager.get_stats().await;
        assert_eq!(stats.total_elections, 1);
        assert_eq!(stats.successful_elections, 1);
    }
}

/// Test metrics collection during elections
#[tokio::test]
async fn test_election_metrics_collection() {
    let metrics = ElectionMetrics::new();
    let node_id = NodeId::new("metrics-test".to_string(), "test".to_string());
    
    // Simulate an election
    let tracker = metrics.start_election(1).await;
    
    // Record some metrics
    metrics.record_vote_latency(&node_id, Duration::from_millis(50)).await;
    metrics.record_message_loss(&node_id, 10, 9).await;
    metrics.record_performance(25.0, 1024*1024).await;
    
    // Complete the election
    tracker.complete(ElectionOutcome::Won, 3, 5).await;
    
    // Verify metrics
    let summary = metrics.get_summary().await;
    assert_eq!(summary.total_elections, 1);
    assert_eq!(summary.win_rate, 1.0);
    assert!(summary.avg_election_duration > Duration::ZERO);
    assert!(summary.avg_vote_latency > Duration::ZERO);
    assert!(summary.avg_message_loss_rate > 0.0);
    assert!(summary.avg_cpu_during_election > 0.0);
    assert!(summary.avg_memory_during_election > 0);
}

/// Benchmark election performance under load
#[tokio::test]
async fn bench_election_performance() {
    let config = TestClusterConfig {
        node_count: 5,
        election_timeout_min: Duration::from_millis(50),
        election_timeout_max: Duration::from_millis(100),
        heartbeat_interval: Duration::from_millis(25),
        ..Default::default()
    };
    
    let mut cluster = TestCluster::new(config).await.unwrap();
    cluster.start_all().await.unwrap();
    
    let start = Instant::now();
    
    // Force multiple elections by simulating leader failures
    for i in 0..5 {
        if let Ok(leader) = cluster.wait_for_leader(Duration::from_secs(5)).await {
            let leader_index = cluster.nodes.iter()
                .position(|n| n.node_id == leader)
                .unwrap();
            
            cluster.stop_node(leader_index).await;
            sleep(Duration::from_millis(200)).await;
            cluster.heal_network_partition().await;
        }
    }
    
    let elapsed = start.elapsed();
    info!("Completed 5 election cycles in {:?}", elapsed);
    
    // Verify final state
    let final_leader = cluster.wait_for_leader(Duration::from_secs(5)).await.unwrap();
    let leaders = cluster.get_leaders().await;
    
    assert_eq!(leaders.len(), 1);
    info!("Final leader: {} after {} elections", final_leader, 5);
    
    // Performance assertion: should complete within reasonable time
    assert!(elapsed < Duration::from_secs(30), "Elections took too long: {:?}", elapsed);
}

/// Integration test with the recovery system
#[tokio::test] 
async fn test_integration_with_recovery_system() {
    // This test would integrate the new election system with the existing
    // TransactionRecoveryManager to verify end-to-end functionality
    
    let config = TestClusterConfig::default();
    let mut cluster = TestCluster::new(config).await.unwrap();
    
    cluster.start_all().await.unwrap();
    
    // Wait for leader election
    let leader_id = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();
    
    // In a full integration test, we would:
    // 1. Create some distributed transactions
    // 2. Simulate coordinator failure
    // 3. Verify new coordinator is elected
    // 4. Verify transaction recovery works
    
    // For now, just verify the election system works
    let leaders = cluster.get_leaders().await;
    assert_eq!(leaders.len(), 1);
    assert_eq!(leaders[0].node_id, leader_id);
    
    info!("Election system ready for transaction recovery integration");
}

// Helper macros for test setup
macro_rules! assert_eventually {
    ($condition:expr, $timeout:expr, $message:expr) => {
        let start = Instant::now();
        while start.elapsed() < $timeout {
            if $condition {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
        assert!($condition, $message);
    };
}

// Test utility functions
async fn wait_for_condition<F, Fut>(condition: F, timeout: Duration) -> bool 
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition().await {
            return true;
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

#[cfg(test)]
mod cluster_simulation {
    use super::*;
    
    /// Simulate various failure scenarios
    #[tokio::test]
    async fn test_cascading_failures() {
        let config = TestClusterConfig {
            node_count: 7,
            ..Default::default()
        };
        
        let mut cluster = TestCluster::new(config).await.unwrap();
        cluster.start_all().await.unwrap();
        
        // Initial leader
        let leader1 = cluster.wait_for_leader(Duration::from_secs(10)).await.unwrap();
        
        // Simulate cascading failures
        for i in 0..3 {
            if let Some(leader_idx) = cluster.nodes.iter()
                .position(|n| futures::executor::block_on(n.is_leader())) 
            {
                cluster.stop_node(leader_idx).await;
                sleep(Duration::from_millis(500)).await;
                
                // Should elect new leader within reasonable time
                let _ = cluster.wait_for_leader(Duration::from_secs(5)).await.unwrap();
            }
        }
        
        // Final verification
        let leaders = cluster.get_leaders().await;
        assert_eq!(leaders.len(), 1, "Should maintain single leader despite cascading failures");
    }
}