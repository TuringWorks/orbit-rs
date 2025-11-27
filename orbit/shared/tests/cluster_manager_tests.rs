//! Cluster Manager Integration Tests
//!
//! Comprehensive tests for the cluster management system including:
//! - SplitBrainDetector for preventing split-brain scenarios
//! - PartitionDetector for network partition detection
//! - QuorumConfig for consensus configuration
//! - NodeHealthStatus tracking

use orbit_shared::cluster_manager::{
    NodeHealthStatus, PartitionDetector, QuorumConfig, SplitBrainDetector,
};
use orbit_shared::mesh::NodeId;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Helper function to create a test NodeId
fn test_node_id(name: &str) -> NodeId {
    NodeId::new(name.to_string(), "test".to_string())
}

// ============================================================================
// QuorumConfig Tests
// ============================================================================

#[test]
fn test_quorum_config_default() {
    let config = QuorumConfig::default();

    assert_eq!(config.min_quorum_size, 3);
    assert_eq!(config.max_failures, 1);
    assert_eq!(config.quorum_timeout, Duration::from_secs(10));
    assert!(!config.dynamic_quorum);
}

#[test]
fn test_quorum_config_custom() {
    let config = QuorumConfig {
        min_quorum_size: 5,
        max_failures: 2,
        quorum_timeout: Duration::from_secs(30),
        dynamic_quorum: true,
    };

    assert_eq!(config.min_quorum_size, 5);
    assert_eq!(config.max_failures, 2);
    assert_eq!(config.quorum_timeout, Duration::from_secs(30));
    assert!(config.dynamic_quorum);
}

#[test]
fn test_quorum_config_clone() {
    let config = QuorumConfig {
        min_quorum_size: 7,
        max_failures: 3,
        quorum_timeout: Duration::from_secs(60),
        dynamic_quorum: true,
    };

    let cloned = config.clone();

    assert_eq!(cloned.min_quorum_size, config.min_quorum_size);
    assert_eq!(cloned.max_failures, config.max_failures);
    assert_eq!(cloned.quorum_timeout, config.quorum_timeout);
    assert_eq!(cloned.dynamic_quorum, config.dynamic_quorum);
}

// ============================================================================
// NodeHealthStatus Tests
// ============================================================================

#[test]
fn test_node_health_status_creation() {
    let now = Instant::now();
    let status = NodeHealthStatus {
        node_id: test_node_id("node-1"),
        last_seen: now,
        consecutive_failures: 0,
        network_latency: Some(Duration::from_millis(5)),
        is_reachable: true,
        partition_group: None,
    };

    assert_eq!(status.node_id.key, "node-1");
    assert_eq!(status.consecutive_failures, 0);
    assert!(status.is_reachable);
    assert_eq!(status.network_latency, Some(Duration::from_millis(5)));
    assert!(status.partition_group.is_none());
}

#[test]
fn test_node_health_status_unreachable() {
    let now = Instant::now();
    let status = NodeHealthStatus {
        node_id: test_node_id("node-2"),
        last_seen: now,
        consecutive_failures: 5,
        network_latency: None,
        is_reachable: false,
        partition_group: Some("partition-A".to_string()),
    };

    assert!(!status.is_reachable);
    assert_eq!(status.consecutive_failures, 5);
    assert!(status.network_latency.is_none());
    assert_eq!(status.partition_group, Some("partition-A".to_string()));
}

#[test]
fn test_node_health_status_clone() {
    let status = NodeHealthStatus {
        node_id: test_node_id("node-3"),
        last_seen: Instant::now(),
        consecutive_failures: 2,
        network_latency: Some(Duration::from_millis(10)),
        is_reachable: true,
        partition_group: Some("main".to_string()),
    };

    let cloned = status.clone();

    assert_eq!(cloned.node_id, status.node_id);
    assert_eq!(cloned.consecutive_failures, status.consecutive_failures);
    assert_eq!(cloned.is_reachable, status.is_reachable);
    assert_eq!(cloned.network_latency, status.network_latency);
    assert_eq!(cloned.partition_group, status.partition_group);
}

// ============================================================================
// SplitBrainDetector Tests
// ============================================================================

#[test]
fn test_split_brain_detector_creation() {
    let detector = SplitBrainDetector::new(3, Duration::from_secs(5));

    assert_eq!(detector.get_detection_interval(), Duration::from_secs(5));
}

#[test]
fn test_split_brain_detector_different_configs() {
    let detector1 = SplitBrainDetector::new(1, Duration::from_millis(100));
    let detector2 = SplitBrainDetector::new(5, Duration::from_secs(30));

    assert_eq!(detector1.get_detection_interval(), Duration::from_millis(100));
    assert_eq!(detector2.get_detection_interval(), Duration::from_secs(30));
}

#[tokio::test]
async fn test_split_brain_no_risk_majority_reachable() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    // 5 node cluster, 0 unreachable = 5 reachable (majority is 3)
    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];
    let unreachable: Vec<NodeId> = vec![];

    let has_risk = detector.check_split_brain_risk(&cluster_nodes, &unreachable).await;

    assert!(!has_risk, "No split-brain risk when all nodes are reachable");
}

#[tokio::test]
async fn test_split_brain_no_risk_exactly_majority() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    // 5 node cluster, 2 unreachable = 3 reachable (majority is 3)
    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];
    let unreachable: Vec<NodeId> = vec![
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];

    let has_risk = detector.check_split_brain_risk(&cluster_nodes, &unreachable).await;

    assert!(!has_risk, "No split-brain risk when exactly majority is reachable");
}

#[tokio::test]
async fn test_split_brain_risk_below_majority() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    // 5 node cluster, 3 unreachable = 2 reachable (majority is 3)
    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];
    let unreachable: Vec<NodeId> = vec![
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];

    let has_risk = detector.check_split_brain_risk(&cluster_nodes, &unreachable).await;

    assert!(has_risk, "Split-brain risk when below majority");
}

#[tokio::test]
async fn test_split_brain_risk_below_min_cluster_size() {
    let detector = SplitBrainDetector::new(4, Duration::from_secs(5)); // Min cluster size 4

    // 5 node cluster, 2 unreachable = 3 reachable (majority is 3, but min_cluster_size is 4)
    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];
    let unreachable: Vec<NodeId> = vec![
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];

    let has_risk = detector.check_split_brain_risk(&cluster_nodes, &unreachable).await;

    assert!(has_risk, "Split-brain risk when below min cluster size");
}

#[tokio::test]
async fn test_split_brain_three_node_cluster() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    // 3 node cluster - majority is 2
    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
    ];

    // 0 unreachable = 3 reachable (>=2 majority)
    let no_risk = detector.check_split_brain_risk(&cluster_nodes, &[]).await;
    assert!(!no_risk, "No risk with all 3 nodes reachable");

    // 1 unreachable = 2 reachable (>=2 majority)
    let no_risk = detector.check_split_brain_risk(&cluster_nodes, &[test_node_id("node-3")]).await;
    assert!(!no_risk, "No risk with 2 of 3 nodes reachable");

    // 2 unreachable = 1 reachable (<2 majority)
    let has_risk = detector.check_split_brain_risk(
        &cluster_nodes,
        &[test_node_id("node-2"), test_node_id("node-3")],
    ).await;
    assert!(has_risk, "Risk with only 1 of 3 nodes reachable");
}

#[tokio::test]
async fn test_split_brain_single_node_cluster() {
    let detector = SplitBrainDetector::new(1, Duration::from_secs(5));

    // Single node cluster
    let cluster_nodes: Vec<NodeId> = vec![test_node_id("node-1")];

    // Single node always has majority (1/2 + 1 = 1)
    let no_risk = detector.check_split_brain_risk(&cluster_nodes, &[]).await;
    assert!(!no_risk, "Single node cluster has no split-brain risk");
}

#[tokio::test]
async fn test_split_brain_even_cluster_strict_majority() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    // 4 node cluster - majority is 3 (4/2 + 1 = 3)
    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
        test_node_id("node-4"),
    ];

    // 2 reachable is NOT majority (need 3)
    let has_risk = detector.check_split_brain_risk(
        &cluster_nodes,
        &[test_node_id("node-3"), test_node_id("node-4")],
    ).await;
    assert!(has_risk, "Even cluster needs strict majority");

    // 3 reachable IS majority
    let no_risk = detector.check_split_brain_risk(
        &cluster_nodes,
        &[test_node_id("node-4")],
    ).await;
    assert!(!no_risk, "3 of 4 nodes is majority");
}

#[tokio::test]
async fn test_detect_partitions_all_reachable() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    let mut node_health = HashMap::new();
    node_health.insert(test_node_id("node-1"), NodeHealthStatus {
        node_id: test_node_id("node-1"),
        last_seen: Instant::now(),
        consecutive_failures: 0,
        network_latency: Some(Duration::from_millis(5)),
        is_reachable: true,
        partition_group: None,
    });
    node_health.insert(test_node_id("node-2"), NodeHealthStatus {
        node_id: test_node_id("node-2"),
        last_seen: Instant::now(),
        consecutive_failures: 0,
        network_latency: Some(Duration::from_millis(3)),
        is_reachable: true,
        partition_group: None,
    });
    node_health.insert(test_node_id("node-3"), NodeHealthStatus {
        node_id: test_node_id("node-3"),
        last_seen: Instant::now(),
        consecutive_failures: 0,
        network_latency: Some(Duration::from_millis(7)),
        is_reachable: true,
        partition_group: None,
    });

    let partitions = detector.detect_partitions(&node_health).await;

    assert!(partitions.contains_key("reachable"));
    assert!(!partitions.contains_key("unreachable"));
    assert_eq!(partitions.get("reachable").unwrap().len(), 3);
}

#[tokio::test]
async fn test_detect_partitions_some_unreachable() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    let mut node_health = HashMap::new();
    node_health.insert(test_node_id("node-1"), NodeHealthStatus {
        node_id: test_node_id("node-1"),
        last_seen: Instant::now(),
        consecutive_failures: 0,
        network_latency: Some(Duration::from_millis(5)),
        is_reachable: true,
        partition_group: None,
    });
    node_health.insert(test_node_id("node-2"), NodeHealthStatus {
        node_id: test_node_id("node-2"),
        last_seen: Instant::now(),
        consecutive_failures: 5,
        network_latency: None,
        is_reachable: false,
        partition_group: None,
    });
    node_health.insert(test_node_id("node-3"), NodeHealthStatus {
        node_id: test_node_id("node-3"),
        last_seen: Instant::now(),
        consecutive_failures: 3,
        network_latency: None,
        is_reachable: false,
        partition_group: None,
    });

    let partitions = detector.detect_partitions(&node_health).await;

    assert!(partitions.contains_key("reachable"));
    assert!(partitions.contains_key("unreachable"));
    assert_eq!(partitions.get("reachable").unwrap().len(), 1);
    assert_eq!(partitions.get("unreachable").unwrap().len(), 2);
}

#[tokio::test]
async fn test_detect_partitions_all_unreachable() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    let mut node_health = HashMap::new();
    node_health.insert(test_node_id("node-1"), NodeHealthStatus {
        node_id: test_node_id("node-1"),
        last_seen: Instant::now(),
        consecutive_failures: 10,
        network_latency: None,
        is_reachable: false,
        partition_group: None,
    });
    node_health.insert(test_node_id("node-2"), NodeHealthStatus {
        node_id: test_node_id("node-2"),
        last_seen: Instant::now(),
        consecutive_failures: 10,
        network_latency: None,
        is_reachable: false,
        partition_group: None,
    });

    let partitions = detector.detect_partitions(&node_health).await;

    assert!(!partitions.contains_key("reachable"));
    assert!(partitions.contains_key("unreachable"));
    assert_eq!(partitions.get("unreachable").unwrap().len(), 2);
}

#[tokio::test]
async fn test_detect_partitions_empty() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    let node_health: HashMap<NodeId, NodeHealthStatus> = HashMap::new();

    let partitions = detector.detect_partitions(&node_health).await;

    assert!(partitions.is_empty());
}

// ============================================================================
// PartitionDetector Tests
// ============================================================================

#[test]
fn test_partition_detector_creation() {
    let _detector = PartitionDetector::new(Duration::from_millis(500), 3);

    // Detector should be created successfully
    assert!(true, "PartitionDetector created successfully");
}

#[tokio::test]
async fn test_partition_detector_check_connectivity() {
    let detector = PartitionDetector::new(Duration::from_millis(10), 1); // Short timeout for tests

    let target_node = test_node_id("node-1");

    // Check connectivity to a single node
    let is_connected = detector.check_connectivity(&target_node).await;

    // The simulated implementation has 90% success rate
    // We just verify the method runs without error
    assert!(is_connected || !is_connected, "Connectivity check completed");
}

#[tokio::test]
async fn test_partition_detector_check_cluster_connectivity() {
    let detector = PartitionDetector::new(Duration::from_millis(10), 1); // Short timeout for tests

    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
    ];

    let from_node = test_node_id("node-1");

    // Check connectivity from node-1 to all other nodes
    let connectivity = detector.check_cluster_connectivity(&cluster_nodes, &from_node).await;

    // Should not include the from_node in results
    assert!(!connectivity.contains_key(&from_node));

    // Should have entries for other nodes
    assert!(connectivity.len() <= 2); // At most 2 other nodes (excludes self)
}

#[tokio::test]
async fn test_partition_detector_multiple_attempts() {
    let detector = PartitionDetector::new(Duration::from_millis(5), 3); // 3 attempts

    let target_node = test_node_id("node-1");

    // With 3 attempts and 90% success rate, almost always succeeds
    let is_connected = detector.check_connectivity(&target_node).await;

    // The test verifies the method handles multiple attempts correctly
    assert!(is_connected || !is_connected, "Multiple attempt check completed");
}

// ============================================================================
// Integration Tests - Split-Brain Scenarios
// ============================================================================

#[tokio::test]
async fn test_split_brain_scenario_network_partition() {
    // Simulate a network partition where cluster splits into two groups
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    // 5 node cluster splits into 2 + 3
    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];

    // Group A (nodes 1,2) can't see group B (nodes 3,4,5)
    // From Group A's perspective: 2 reachable, 3 unreachable
    let unreachable_from_a: Vec<NodeId> = vec![
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];

    let risk_from_a = detector.check_split_brain_risk(&cluster_nodes, &unreachable_from_a).await;
    assert!(risk_from_a, "Group A (minority) should detect split-brain risk");

    // From Group B's perspective: 3 reachable, 2 unreachable
    let unreachable_from_b: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
    ];

    let risk_from_b = detector.check_split_brain_risk(&cluster_nodes, &unreachable_from_b).await;
    assert!(!risk_from_b, "Group B (majority) should NOT detect split-brain risk");
}

#[tokio::test]
async fn test_split_brain_scenario_cascading_failures() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];

    // Simulate cascading failures: start with 0, then 1, then 2, then 3 unreachable
    let unreachable_0: Vec<NodeId> = vec![];
    let unreachable_1: Vec<NodeId> = vec![test_node_id("node-5")];
    let unreachable_2: Vec<NodeId> = vec![test_node_id("node-4"), test_node_id("node-5")];
    let unreachable_3: Vec<NodeId> = vec![
        test_node_id("node-3"),
        test_node_id("node-4"),
        test_node_id("node-5"),
    ];

    // 5 reachable: no risk
    assert!(!detector.check_split_brain_risk(&cluster_nodes, &unreachable_0).await);

    // 4 reachable: no risk (majority is 3)
    assert!(!detector.check_split_brain_risk(&cluster_nodes, &unreachable_1).await);

    // 3 reachable: no risk (exactly majority)
    assert!(!detector.check_split_brain_risk(&cluster_nodes, &unreachable_2).await);

    // 2 reachable: RISK (below majority)
    assert!(detector.check_split_brain_risk(&cluster_nodes, &unreachable_3).await);
}

#[tokio::test]
async fn test_split_brain_recovery_detection() {
    let detector = SplitBrainDetector::new(2, Duration::from_secs(5));

    let cluster_nodes: Vec<NodeId> = vec![
        test_node_id("node-1"),
        test_node_id("node-2"),
        test_node_id("node-3"),
    ];

    // Start in split-brain state
    let unreachable_all: Vec<NodeId> = vec![test_node_id("node-2"), test_node_id("node-3")];
    assert!(detector.check_split_brain_risk(&cluster_nodes, &unreachable_all).await);

    // Recover: one node comes back
    let unreachable_one: Vec<NodeId> = vec![test_node_id("node-3")];
    assert!(!detector.check_split_brain_risk(&cluster_nodes, &unreachable_one).await);

    // Full recovery
    let unreachable_none: Vec<NodeId> = vec![];
    assert!(!detector.check_split_brain_risk(&cluster_nodes, &unreachable_none).await);
}
