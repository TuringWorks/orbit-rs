//! Load balancing for connection pooling across multiple nodes

use crate::exception::OrbitResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Load balancing strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LoadBalancingStrategy {
    /// Round-robin distribution
    RoundRobin,
    /// Route to node with least connections
    #[default]
    LeastConnections,
    /// Random selection
    Random,
    /// Weighted round-robin based on node capacity
    WeightedRoundRobin,
    /// Route based on connection affinity
    AffinityBased,
}

/// Node health information
#[derive(Debug, Clone)]
pub struct NodeHealth {
    /// Node identifier
    pub node_id: String,
    /// Current active connections
    pub active_connections: usize,
    /// Maximum allowed connections
    pub max_connections: usize,
    /// Is the node healthy
    pub is_healthy: bool,
    /// Node weight (for weighted strategies)
    pub weight: u32,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
}

impl NodeHealth {
    pub fn new(node_id: String, max_connections: usize) -> Self {
        Self {
            node_id,
            active_connections: 0,
            max_connections,
            is_healthy: true,
            weight: 100,
            avg_response_time_ms: 0.0,
        }
    }

    pub fn utilization(&self) -> f64 {
        if self.max_connections == 0 {
            0.0
        } else {
            self.active_connections as f64 / self.max_connections as f64
        }
    }

    pub fn has_capacity(&self) -> bool {
        self.is_healthy && self.active_connections < self.max_connections
    }
}

/// Connection load balancer
pub struct ConnectionLoadBalancer {
    strategy: LoadBalancingStrategy,
    nodes: Arc<RwLock<HashMap<String, NodeHealth>>>,
    round_robin_counter: Arc<AtomicUsize>,
}

impl ConnectionLoadBalancer {
    /// Create a new load balancer with the specified strategy
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            nodes: Arc::new(RwLock::new(HashMap::new())),
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Add or update a node
    pub async fn add_node(&self, node: NodeHealth) {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.node_id.clone(), node);
    }

    /// Remove a node
    pub async fn remove_node(&self, node_id: &str) {
        let mut nodes = self.nodes.write().await;
        nodes.remove(node_id);
    }

    /// Update node health status
    pub async fn update_node_health(&self, node_id: &str, is_healthy: bool) {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.is_healthy = is_healthy;
        }
    }

    /// Update node connection count
    pub async fn update_node_connections(&self, node_id: &str, connections: usize) {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.active_connections = connections;
        }
    }

    /// Select a node for a new connection
    pub async fn select_node(&self) -> OrbitResult<Option<String>> {
        let nodes = self.nodes.read().await;

        // Filter healthy nodes with capacity
        let available_nodes: Vec<&NodeHealth> =
            nodes.values().filter(|n| n.has_capacity()).collect();

        if available_nodes.is_empty() {
            return Ok(None);
        }

        let selected_node = match self.strategy {
            LoadBalancingStrategy::RoundRobin => self.select_round_robin(&available_nodes),
            LoadBalancingStrategy::LeastConnections => {
                self.select_least_connections(&available_nodes)
            }
            LoadBalancingStrategy::Random => self.select_random(&available_nodes),
            LoadBalancingStrategy::WeightedRoundRobin => self.select_weighted(&available_nodes),
            LoadBalancingStrategy::AffinityBased => {
                // For affinity-based, fall back to least connections
                self.select_least_connections(&available_nodes)
            }
        };

        Ok(Some(selected_node.node_id.clone()))
    }

    /// Select a node using affinity key
    pub async fn select_node_with_affinity(
        &self,
        affinity_key: &str,
    ) -> OrbitResult<Option<String>> {
        let nodes = self.nodes.read().await;

        let available_nodes: Vec<&NodeHealth> =
            nodes.values().filter(|n| n.has_capacity()).collect();

        if available_nodes.is_empty() {
            return Ok(None);
        }

        // Use consistent hashing for affinity
        let hash = self.hash_affinity_key(affinity_key);
        let index = hash % available_nodes.len();

        Ok(Some(available_nodes[index].node_id.clone()))
    }

    fn hash_affinity_key(&self, key: &str) -> usize {
        // Simple hash function for affinity-based routing
        key.bytes().fold(0usize, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as usize)
        })
    }

    fn select_round_robin<'a>(&self, nodes: &[&'a NodeHealth]) -> &'a NodeHealth {
        let index = self.round_robin_counter.fetch_add(1, Ordering::SeqCst) % nodes.len();
        nodes[index]
    }

    fn select_least_connections<'a>(&self, nodes: &[&'a NodeHealth]) -> &'a NodeHealth {
        nodes.iter().min_by_key(|n| n.active_connections).unwrap()
    }

    fn select_random<'a>(&self, nodes: &[&'a NodeHealth]) -> &'a NodeHealth {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as usize;
        let index = now % nodes.len();
        nodes[index]
    }

    fn select_weighted<'a>(&self, nodes: &[&'a NodeHealth]) -> &'a NodeHealth {
        // Calculate total weight
        let total_weight: u32 = nodes.iter().map(|n| n.weight).sum();

        if total_weight == 0 {
            return self.select_round_robin(nodes);
        }

        // Select based on weight
        let counter = self.round_robin_counter.fetch_add(1, Ordering::SeqCst) as u32;
        let target = counter % total_weight;

        let mut cumulative = 0u32;
        for node in nodes {
            cumulative += node.weight;
            if target < cumulative {
                return node;
            }
        }

        // Fallback
        nodes[0]
    }

    /// Get statistics about all nodes
    pub async fn get_stats(&self) -> LoadBalancerStats {
        let nodes = self.nodes.read().await;

        let total_nodes = nodes.len();
        let healthy_nodes = nodes.values().filter(|n| n.is_healthy).count();
        let total_connections: usize = nodes.values().map(|n| n.active_connections).sum();
        let total_capacity: usize = nodes.values().map(|n| n.max_connections).sum();

        LoadBalancerStats {
            strategy: self.strategy,
            total_nodes,
            healthy_nodes,
            total_connections,
            total_capacity,
            average_utilization: if total_capacity > 0 {
                total_connections as f64 / total_capacity as f64
            } else {
                0.0
            },
        }
    }
}

impl Clone for ConnectionLoadBalancer {
    fn clone(&self) -> Self {
        Self {
            strategy: self.strategy,
            nodes: Arc::clone(&self.nodes),
            round_robin_counter: Arc::clone(&self.round_robin_counter),
        }
    }
}

/// Load balancer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancerStats {
    pub strategy: LoadBalancingStrategy,
    pub total_nodes: usize,
    pub healthy_nodes: usize,
    pub total_connections: usize,
    pub total_capacity: usize,
    pub average_utilization: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_round_robin() {
        let lb = ConnectionLoadBalancer::new(LoadBalancingStrategy::RoundRobin);

        lb.add_node(NodeHealth::new("node1".to_string(), 10)).await;
        lb.add_node(NodeHealth::new("node2".to_string(), 10)).await;
        lb.add_node(NodeHealth::new("node3".to_string(), 10)).await;

        // Select multiple times and verify round-robin
        let mut selected = Vec::new();
        for _ in 0..6 {
            if let Some(node_id) = lb.select_node().await.unwrap() {
                selected.push(node_id);
            }
        }

        // Should cycle through nodes
        assert_eq!(selected.len(), 6);
    }

    #[tokio::test]
    async fn test_least_connections() {
        let lb = ConnectionLoadBalancer::new(LoadBalancingStrategy::LeastConnections);

        let mut node1 = NodeHealth::new("node1".to_string(), 10);
        node1.active_connections = 5;
        let mut node2 = NodeHealth::new("node2".to_string(), 10);
        node2.active_connections = 2;
        let mut node3 = NodeHealth::new("node3".to_string(), 10);
        node3.active_connections = 8;

        lb.add_node(node1).await;
        lb.add_node(node2).await;
        lb.add_node(node3).await;

        let selected = lb.select_node().await.unwrap();
        assert_eq!(selected, Some("node2".to_string()));
    }

    #[tokio::test]
    async fn test_node_health_filtering() {
        let lb = ConnectionLoadBalancer::new(LoadBalancingStrategy::LeastConnections);

        let mut node1 = NodeHealth::new("node1".to_string(), 10);
        node1.is_healthy = false;
        let node2 = NodeHealth::new("node2".to_string(), 10);

        lb.add_node(node1).await;
        lb.add_node(node2).await;

        let selected = lb.select_node().await.unwrap();
        assert_eq!(selected, Some("node2".to_string()));
    }

    #[tokio::test]
    async fn test_affinity_routing() {
        let lb = ConnectionLoadBalancer::new(LoadBalancingStrategy::AffinityBased);

        lb.add_node(NodeHealth::new("node1".to_string(), 10)).await;
        lb.add_node(NodeHealth::new("node2".to_string(), 10)).await;
        lb.add_node(NodeHealth::new("node3".to_string(), 10)).await;

        // Same affinity key should always route to same node
        let node1 = lb.select_node_with_affinity("user123").await.unwrap();
        let node2 = lb.select_node_with_affinity("user123").await.unwrap();
        assert_eq!(node1, node2);
    }
}
