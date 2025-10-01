//! Load balancing and actor placement algorithms for the Orbit cluster

use orbit_shared::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Load balancing strategies for actor placement
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LoadBalancingStrategy {
    #[default]
    RoundRobin,
    LeastConnections,
    Random,
}

/// Manages load balancing and actor placement decisions
#[derive(Debug)]
pub struct LoadBalancer {
    strategy: LoadBalancingStrategy,
    node_loads: Arc<RwLock<HashMap<NodeId, NodeLoad>>>,
    round_robin_counter: Arc<std::sync::atomic::AtomicUsize>,
}

impl LoadBalancer {
    pub fn new() -> Self {
        Self {
            strategy: LoadBalancingStrategy::default(),
            node_loads: Arc::new(RwLock::new(HashMap::new())),
            round_robin_counter: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    pub fn with_strategy(mut self, strategy: LoadBalancingStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Select the best node to host a new actor instance
    pub async fn select_node(
        &self,
        reference: &AddressableReference,
        available_nodes: &[NodeInfo],
    ) -> OrbitResult<Option<NodeId>> {
        if available_nodes.is_empty() {
            return Ok(None);
        }

        // Filter nodes that can host this addressable type
        let eligible_nodes: Vec<&NodeInfo> = available_nodes
            .iter()
            .filter(|node| {
                node.capabilities.addressable_types.is_empty() || // Accept all types
                node.capabilities.addressable_types.contains(&reference.addressable_type)
            })
            .filter(|node| node.status == NodeStatus::Active)
            .collect();

        if eligible_nodes.is_empty() {
            return Ok(None);
        }

        let selected_node = match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.select_round_robin(&eligible_nodes).await,
            LoadBalancingStrategy::LeastConnections => {
                self.select_least_connections(&eligible_nodes).await
            }
            LoadBalancingStrategy::Random => {
                self.select_random(&eligible_nodes).await
            }
        };

        Ok(Some(selected_node.id.clone()))
    }

    /// Update load information for a node
    pub async fn update_node_load(&self, node_id: NodeId, load: NodeLoad) {
        let mut node_loads = self.node_loads.write().await;
        node_loads.insert(node_id, load);
    }

    /// Remove a node from load tracking
    pub async fn remove_node(&self, node_id: &NodeId) {
        let mut node_loads = self.node_loads.write().await;
        node_loads.remove(node_id);
    }

    /// Get current load statistics
    pub async fn get_load_stats(&self) -> LoadBalancerStats {
        let node_loads = self.node_loads.read().await;
        let total_nodes = node_loads.len();
        let total_load: u32 = node_loads
            .values()
            .map(|load| load.active_addressables)
            .sum();

        LoadBalancerStats {
            strategy: self.strategy.clone(),
            total_nodes,
            total_load,
            average_load: if total_nodes > 0 {
                total_load / total_nodes as u32
            } else {
                0
            },
        }
    }

    async fn select_round_robin<'a>(&self, nodes: &[&'a NodeInfo]) -> &'a NodeInfo {
        let index = self
            .round_robin_counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            % nodes.len();
        nodes[index]
    }

    async fn select_least_connections<'a>(&self, nodes: &[&'a NodeInfo]) -> &'a NodeInfo {
        let node_loads = self.node_loads.read().await;

        nodes
            .iter()
            .min_by_key(|node| {
                node_loads
                    .get(&node.id)
                    .map(|load| load.active_addressables)
                    .unwrap_or(0)
            })
            .unwrap()
    }

    async fn select_random<'a>(&self, nodes: &[&'a NodeInfo]) -> &'a NodeInfo {
        // Simple random selection using the hash of current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize;
        let index = now % nodes.len();
        nodes[index]
    }

    #[allow(dead_code)]
    async fn select_resource_aware<'a>(&self, nodes: &[&'a NodeInfo]) -> &'a NodeInfo {
        let node_loads = self.node_loads.read().await;

        nodes
            .iter()
            .min_by(|a, b| {
                let load_a = node_loads.get(&a.id).cloned().unwrap_or_default();
                let load_b = node_loads.get(&b.id).cloned().unwrap_or_default();

                // Compare based on resource utilization score
                let score_a = load_a.cpu_usage
                    + (load_a.memory_usage * 0.5)
                    + (load_a.active_addressables as f64 * 0.1);
                let score_b = load_b.cpu_usage
                    + (load_b.memory_usage * 0.5)
                    + (load_b.active_addressables as f64 * 0.1);

                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap()
    }

    #[allow(dead_code)]
    async fn select_hash<'a>(
        &self,
        reference: &AddressableReference,
        nodes: &[&'a NodeInfo],
    ) -> &'a NodeInfo {
        // Use consistent hashing based on the addressable reference
        let hash = self.hash_reference(reference);
        let index = hash % nodes.len();
        nodes[index]
    }

    fn hash_reference(&self, reference: &AddressableReference) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        reference.hash(&mut hasher);
        hasher.finish() as usize
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LoadBalancer {
    fn clone(&self) -> Self {
        Self {
            strategy: self.strategy.clone(),
            node_loads: self.node_loads.clone(),
            round_robin_counter: Arc::new(std::sync::atomic::AtomicUsize::new(
                self.round_robin_counter
                    .load(std::sync::atomic::Ordering::SeqCst),
            )),
        }
    }
}

/// Load information for a cluster node
#[derive(Debug, Clone, Default)]
pub struct NodeLoad {
    pub active_addressables: u32,
    pub cpu_usage: f64,     // 0.0 to 1.0
    pub memory_usage: f64,  // 0.0 to 1.0
    pub network_usage: f64, // 0.0 to 1.0
}

/// Statistics about the load balancer
#[derive(Debug, Clone)]
pub struct LoadBalancerStats {
    pub strategy: LoadBalancingStrategy,
    pub total_nodes: usize,
    pub total_load: u32,
    pub average_load: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_nodes() -> Vec<NodeInfo> {
        vec![
            NodeInfo {
                id: NodeId::new("node1".to_string(), "test".to_string()),
                url: "localhost".to_string(),
                port: 50051,
                capabilities: NodeCapabilities {
                    addressable_types: vec!["TestActor".to_string()],
                    max_addressables: Some(100),
                    tags: HashMap::new(),
                },
                status: NodeStatus::Active,
                lease: None,
            },
            NodeInfo {
                id: NodeId::new("node2".to_string(), "test".to_string()),
                url: "localhost".to_string(),
                port: 50052,
                capabilities: NodeCapabilities {
                    addressable_types: vec!["TestActor".to_string()],
                    max_addressables: Some(100),
                    tags: HashMap::new(),
                },
                status: NodeStatus::Active,
                lease: None,
            },
        ]
    }

    #[tokio::test]
    async fn test_round_robin_selection() {
        let load_balancer = LoadBalancer::new().with_strategy(LoadBalancingStrategy::RoundRobin);
        let nodes = create_test_nodes();

        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test1".to_string(),
            },
        };

        let selected1 = load_balancer
            .select_node(&reference, &nodes)
            .await
            .unwrap()
            .unwrap();
        let selected2 = load_balancer
            .select_node(&reference, &nodes)
            .await
            .unwrap()
            .unwrap();

        // Should alternate between nodes
        assert_ne!(selected1, selected2);
    }

    #[tokio::test]
    async fn test_least_connections_selection() {
        let load_balancer =
            LoadBalancer::new().with_strategy(LoadBalancingStrategy::LeastConnections);
        let nodes = create_test_nodes();

        // Set different loads
        let node1_id = nodes[0].id.clone();
        let node2_id = nodes[1].id.clone();

        load_balancer
            .update_node_load(
                node1_id.clone(),
                NodeLoad {
                    active_addressables: 10,
                    ..Default::default()
                },
            )
            .await;

        load_balancer
            .update_node_load(
                node2_id.clone(),
                NodeLoad {
                    active_addressables: 5,
                    ..Default::default()
                },
            )
            .await;

        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let selected = load_balancer
            .select_node(&reference, &nodes)
            .await
            .unwrap()
            .unwrap();

        // Should select node2 (lower load)
        assert_eq!(selected, node2_id);
    }

    #[tokio::test]
    async fn test_random_selection() {
        let load_balancer = LoadBalancer::new().with_strategy(LoadBalancingStrategy::Random);
        let nodes = create_test_nodes();

        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "consistent-key".to_string(),
            },
        };

        let selected1 = load_balancer
            .select_node(&reference, &nodes)
            .await
            .unwrap()
            .unwrap();
        let selected2 = load_balancer
            .select_node(&reference, &nodes)
            .await
            .unwrap()
            .unwrap();

        // For random selection, we just verify both selections are valid nodes
        assert!(nodes.iter().any(|n| n.id == selected1));
        assert!(nodes.iter().any(|n| n.id == selected2));
    }

    #[tokio::test]
    async fn test_node_capability_filtering() {
        let load_balancer = LoadBalancer::new();

        let mut nodes = create_test_nodes();
        nodes[1].capabilities.addressable_types = vec!["OtherActor".to_string()];

        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let selected = load_balancer
            .select_node(&reference, &nodes)
            .await
            .unwrap()
            .unwrap();

        // Should only select node1 (which supports TestActor)
        assert_eq!(selected, nodes[0].id);
    }

    #[tokio::test]
    async fn test_load_balancer_stats() {
        let load_balancer = LoadBalancer::new();

        let node1_id = NodeId::new("node1".to_string(), "test".to_string());
        let node2_id = NodeId::new("node2".to_string(), "test".to_string());

        load_balancer
            .update_node_load(
                node1_id,
                NodeLoad {
                    active_addressables: 10,
                    ..Default::default()
                },
            )
            .await;

        load_balancer
            .update_node_load(
                node2_id,
                NodeLoad {
                    active_addressables: 20,
                    ..Default::default()
                },
            )
            .await;

        let stats = load_balancer.get_load_stats().await;
        assert_eq!(stats.total_nodes, 2);
        assert_eq!(stats.total_load, 30);
        assert_eq!(stats.average_load, 15);
    }
}
