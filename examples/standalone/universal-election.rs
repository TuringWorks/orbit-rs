use orbit_shared::k8s_election::{
    UniversalElectionManager, DeploymentMode, K8sElectionConfig, NodeDiscoveryMethod
};
use orbit_shared::{NodeId, OrbitResult};
use std::time::Duration;
use tracing::{info, warn, error};
use tokio::time::sleep;

/// Example demonstrating universal election manager usage across different deployment modes
#[tokio::main]
async fn main() -> OrbitResult<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("Starting Orbit-RS Universal Election Manager Example");
    
    // Detect deployment mode automatically
    let deployment_mode = UniversalElectionManager::detect_deployment_mode().await;
    info!("Detected deployment mode: {:?}", deployment_mode);
    
    // Create node ID
    let node_id = NodeId::new(
        std::env::var("HOSTNAME").unwrap_or_else(|_| "example-node".to_string()),
        "orbit-rs".to_string()
    );
    
    // Run example based on deployment mode
    match deployment_mode {
        DeploymentMode::Kubernetes { .. } => {
            run_kubernetes_example(node_id, deployment_mode).await?;
        },
        DeploymentMode::DockerCompose { .. } => {
            run_docker_compose_example(node_id, deployment_mode).await?;
        },
        DeploymentMode::Standalone { .. } => {
            run_standalone_example(node_id, deployment_mode).await?;
        },
    }
    
    Ok(())
}

/// Example for Kubernetes deployment
async fn run_kubernetes_example(node_id: NodeId, deployment_mode: DeploymentMode) -> OrbitResult<()> {
    info!("Running Kubernetes election example");
    
    // Configure Kubernetes-specific election settings
    let k8s_config = K8sElectionConfig {
        namespace: "orbit-rs".to_string(),
        lease_name: "orbit-leader-election".to_string(),
        lease_duration_secs: 30,
        renew_deadline_secs: 20,
        retry_period_secs: 5,
        enable_raft_fallback: true,
        pod_name: std::env::var("HOSTNAME").ok(),
        pod_namespace: std::env::var("POD_NAMESPACE").ok(),
    };
    
    // Create universal election manager
    let election_manager = UniversalElectionManager::new(
        node_id.clone(),
        deployment_mode,
        k8s_config,
        None, // Use default state path
    ).await?;
    
    // Start the election process
    election_manager.start().await?;
    
    info!("Kubernetes election manager started for node: {}", node_id);
    
    // Monitor election status
    monitor_election_status(&election_manager, "Kubernetes").await?;
    
    Ok(())
}

/// Example for Docker Compose deployment
async fn run_docker_compose_example(node_id: NodeId, deployment_mode: DeploymentMode) -> OrbitResult<()> {
    info!("Running Docker Compose election example");
    
    // For Docker Compose, we use Raft consensus
    let k8s_config = K8sElectionConfig {
        enable_raft_fallback: false,  // Use only Raft for Compose
        ..Default::default()
    };
    
    let election_manager = UniversalElectionManager::new(
        node_id.clone(),
        deployment_mode,
        k8s_config,
        None,
    ).await?;
    
    election_manager.start().await?;
    
    info!("Docker Compose election manager started for node: {}", node_id);
    
    monitor_election_status(&election_manager, "Docker Compose").await?;
    
    Ok(())
}

/// Example for standalone deployment  
async fn run_standalone_example(node_id: NodeId, deployment_mode: DeploymentMode) -> OrbitResult<()> {
    info!("Running standalone election example");
    
    // Standalone always uses Raft consensus
    let k8s_config = K8sElectionConfig {
        enable_raft_fallback: false,  // Use only Raft
        ..Default::default()
    };
    
    let election_manager = UniversalElectionManager::new(
        node_id.clone(),
        deployment_mode,
        k8s_config,
        None,
    ).await?;
    
    election_manager.start().await?;
    
    info!("Standalone election manager started for node: {}", node_id);
    
    monitor_election_status(&election_manager, "Standalone").await?;
    
    Ok(())
}

/// Monitor election status and demonstrate features
async fn monitor_election_status(
    election_manager: &UniversalElectionManager, 
    deployment_type: &str
) -> OrbitResult<()> {
    
    info!("Monitoring {} election status...", deployment_type);
    
    // Wait for leadership or timeout
    match election_manager.wait_for_leadership(Duration::from_secs(30)).await {
        Ok(_) => {
            info!("🎉 This node became the leader!");
            
            // Demonstrate leader operations
            demonstrate_leader_operations(election_manager).await?;
        },
        Err(_) => {
            info!("⏳ This node is a follower");
            
            // Show current leader
            if let Some(leader) = election_manager.get_current_leader().await {
                info!("Current leader: {}", leader);
            } else {
                warn!("No leader currently elected");
            }
        }
    }
    
    // Show election statistics
    let stats = election_manager.get_election_stats().await;
    info!("Election statistics:");
    info!("  Total elections: {}", stats.total_elections);
    info!("  Success rate: {:.2}%", stats.success_rate * 100.0);
    info!("  Current term: {}", stats.current_term);
    
    // Monitor for a while
    for i in 1..=10 {
        sleep(Duration::from_secs(5)).await;
        
        let is_leader = election_manager.is_leader().await;
        let current_leader = election_manager.get_current_leader().await;
        
        info!(
            "[{}s] Status: {} | Current leader: {:?}", 
            i * 5,
            if is_leader { "LEADER" } else { "FOLLOWER" },
            current_leader
        );
        
        // Simulate some leadership changes
        if i == 5 && is_leader {
            info!("Simulating leadership release...");
            election_manager.release_leadership().await?;
        }
    }
    
    Ok(())
}

/// Demonstrate operations available to the leader
async fn demonstrate_leader_operations(election_manager: &UniversalElectionManager) -> OrbitResult<()> {
    info!("Demonstrating leader operations:");
    
    // Leaders can coordinate distributed operations
    info!("  ✓ Can coordinate transactions");
    info!("  ✓ Can manage cluster membership");
    info!("  ✓ Can handle recovery operations");
    
    // Simulate some leader work
    for i in 1..=5 {
        if !election_manager.is_leader().await {
            warn!("Lost leadership during operations!");
            break;
        }
        
        info!("  📋 Performing leader task {}/5", i);
        sleep(Duration::from_millis(500)).await;
    }
    
    Ok(())
}

/// Configuration examples for different environments
fn show_configuration_examples() {
    info!("Configuration examples for different environments:");
    
    info!("Kubernetes (via environment variables):");
    info!("  export DEPLOYMENT_MODE=kubernetes");
    info!("  export POD_NAMESPACE=orbit-rs");
    info!("  export HOSTNAME=orbit-server-0");
    info!("  export ORBIT_ELECTION_METHOD=kubernetes");
    
    info!("Docker Compose (via environment variables):");
    info!("  export DEPLOYMENT_MODE=docker_compose");
    info!("  export COMPOSE_PROJECT_NAME=orbit-rs");
    info!("  export COMPOSE_SERVICE=orbit-server");
    info!("  export ORBIT_ELECTION_METHOD=raft");
    
    info!("Standalone (via environment variables):");
    info!("  export DEPLOYMENT_MODE=standalone");
    info!("  export ORBIT_DISCOVERY_DNS=orbit-cluster.local");
    info!("  export ORBIT_ELECTION_METHOD=raft");
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_shared::k8s_election::MockLeaseClient;
    use std::sync::Arc;
    use tempfile::tempdir;
    
    #[tokio::test]
    async fn test_kubernetes_election() {
        let node_id = NodeId::new("test-node".to_string(), "test".to_string());
        let deployment_mode = DeploymentMode::Kubernetes {
            namespace: "test".to_string(),
            pod_name: "test-pod".to_string(),
            service_account: "default".to_string(),
        };
        
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("test-state.json");
        
        let mut election_manager = UniversalElectionManager::new(
            node_id,
            deployment_mode,
            K8sElectionConfig::default(),
            Some(state_path),
        ).await.unwrap();
        
        // Use mock lease client for testing
        election_manager.lease_client = Some(Arc::new(MockLeaseClient::new()));
        
        // Start election
        election_manager.start().await.unwrap();
        
        // Should eventually become leader
        tokio::time::timeout(
            Duration::from_secs(5),
            election_manager.wait_for_leadership(Duration::from_secs(10))
        ).await.unwrap().unwrap();
        
        assert!(election_manager.is_leader().await);
    }
    
    #[tokio::test]
    async fn test_deployment_mode_detection() {
        // Test Kubernetes detection
        std::env::set_var("POD_NAMESPACE", "test-ns");
        std::env::set_var("HOSTNAME", "test-pod");
        
        let mode = UniversalElectionManager::detect_deployment_mode().await;
        matches!(mode, DeploymentMode::Kubernetes { .. });
        
        // Cleanup
        std::env::remove_var("POD_NAMESPACE");
        std::env::remove_var("HOSTNAME");
        
        // Test Docker Compose detection
        std::env::set_var("COMPOSE_PROJECT_NAME", "test-project");
        std::env::set_var("COMPOSE_SERVICE", "test-service");
        
        let mode = UniversalElectionManager::detect_deployment_mode().await;
        matches!(mode, DeploymentMode::DockerCompose { .. });
        
        // Cleanup
        std::env::remove_var("COMPOSE_PROJECT_NAME");
        std::env::remove_var("COMPOSE_SERVICE");
    }
}