//! Server-side mesh management and cluster coordination

use dashmap::DashMap;
use orbit_shared::*;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Manages the distributed directory of addressables
#[derive(Debug, Clone)]
pub struct AddressableDirectory {
    // Map of addressable reference to its lease information
    leases: Arc<DashMap<AddressableReference, AddressableLease>>,
    // Map of node ID to the addressables it hosts
    node_addressables: Arc<DashMap<NodeId, Vec<AddressableReference>>>,
}

impl AddressableDirectory {
    pub fn new() -> Self {
        Self {
            leases: Arc::new(DashMap::new()),
            node_addressables: Arc::new(DashMap::new()),
        }
    }

    /// Register an addressable lease
    pub async fn register_lease(&self, lease: AddressableLease) -> OrbitResult<()> {
        let reference = lease.reference.clone();
        let node_id = lease.node_id.clone();

        // Store the lease
        self.leases.insert(reference.clone(), lease);

        // Update node addressables mapping
        self.node_addressables
            .entry(node_id)
            .or_insert_with(Vec::new)
            .push(reference.clone());

        tracing::debug!("Registered lease for {}", reference);
        Ok(())
    }

    /// Find which node hosts a specific addressable
    pub async fn find_addressable(&self, reference: &AddressableReference) -> Option<NodeId> {
        self.leases
            .get(reference)
            .map(|lease| lease.node_id.clone())
    }

    /// Remove an addressable lease
    pub async fn remove_lease(&self, reference: &AddressableReference) -> OrbitResult<()> {
        if let Some((_, lease)) = self.leases.remove(reference) {
            // Remove from node mapping
            if let Some(mut addressables) = self.node_addressables.get_mut(&lease.node_id) {
                addressables.retain(|addr| addr != reference);
            }
            tracing::debug!("Removed lease for {}", reference);
        }
        Ok(())
    }

    /// Clean up expired leases
    pub async fn cleanup_expired_leases(&self) -> OrbitResult<()> {
        let now = chrono::Utc::now();
        let mut expired = Vec::new();

        // Find expired leases
        for entry in self.leases.iter() {
            if entry.expires_at < now {
                expired.push(entry.reference.clone());
            }
        }

        // Remove expired leases
        for reference in expired {
            self.remove_lease(&reference).await?;
        }

        Ok(())
    }

    /// Get all addressables hosted by a node
    pub async fn get_node_addressables(&self, node_id: &NodeId) -> Vec<AddressableReference> {
        self.node_addressables
            .get(node_id)
            .map(|refs| refs.clone())
            .unwrap_or_default()
    }

    /// Get statistics about the directory
    pub async fn stats(&self) -> DirectoryStats {
        DirectoryStats {
            total_leases: self.leases.len(),
            active_nodes: self.node_addressables.len(),
        }
    }
}

impl Default for AddressableDirectory {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about the addressable directory
#[derive(Debug, Clone)]
pub struct DirectoryStats {
    pub total_leases: usize,
    pub active_nodes: usize,
}

/// Manages cluster node membership and health
#[derive(Debug, Clone)]
pub struct ClusterManager {
    nodes: Arc<DashMap<NodeId, NodeInfo>>,
    local_node: Arc<RwLock<Option<NodeInfo>>>,
    lease_duration: Duration,
}

impl ClusterManager {
    pub fn new(lease_duration: Duration) -> Self {
        Self {
            nodes: Arc::new(DashMap::new()),
            local_node: Arc::new(RwLock::new(None)),
            lease_duration,
        }
    }

    /// Register this node in the cluster
    pub async fn register_node(&self, mut node_info: NodeInfo) -> OrbitResult<()> {
        let now = chrono::Utc::now();
        let expires_at = now
            + chrono::Duration::from_std(self.lease_duration)
                .map_err(|e| OrbitError::internal(format!("Invalid lease duration: {}", e)))?;
        let renew_at = now
            + chrono::Duration::from_std(self.lease_duration / 2)
                .map_err(|e| OrbitError::internal(format!("Invalid renew duration: {}", e)))?;

        let lease = NodeLease::new(node_info.id.clone(), expires_at, renew_at);

        node_info.lease = Some(lease);
        node_info.status = NodeStatus::Active;

        // Store locally
        {
            let mut local_node = self.local_node.write().await;
            *local_node = Some(node_info.clone());
        }

        // Add to cluster
        self.nodes.insert(node_info.id.clone(), node_info.clone());

        tracing::info!("Node {} registered in cluster", node_info.id);
        Ok(())
    }

    /// Update information about a cluster node
    pub async fn update_node(&self, node_info: NodeInfo) -> OrbitResult<()> {
        let node_id = node_info.id.clone();
        self.nodes.insert(node_id.clone(), node_info);
        tracing::debug!("Updated node info for {}", node_id);
        Ok(())
    }

    /// Remove a node from the cluster
    pub async fn remove_node(&self, node_id: &NodeId) -> OrbitResult<()> {
        if let Some((_, _node_info)) = self.nodes.remove(node_id) {
            tracing::info!("Node {} removed from cluster", node_id);
        }
        Ok(())
    }

    /// Get information about a specific node
    pub async fn get_node(&self, node_id: &NodeId) -> Option<NodeInfo> {
        self.nodes.get(node_id).map(|entry| entry.clone())
    }

    /// Get all active nodes in the cluster
    pub async fn get_active_nodes(&self) -> Vec<NodeInfo> {
        self.nodes
            .iter()
            .filter(|entry| entry.status == NodeStatus::Active)
            .map(|entry| entry.clone())
            .collect()
    }

    /// Get the local node information
    pub async fn get_local_node(&self) -> Option<NodeInfo> {
        let local_node = self.local_node.read().await;
        local_node.clone()
    }

    /// Check if a node's lease should be renewed
    pub async fn should_renew_lease(&self, node_id: &NodeId) -> bool {
        if let Some(node) = self.nodes.get(node_id) {
            if let Some(lease) = &node.lease {
                return lease.should_renew();
            }
        }
        false
    }

    /// Clean up nodes with expired leases
    pub async fn cleanup_expired_nodes(&self) -> OrbitResult<()> {
        let mut expired = Vec::new();

        for entry in self.nodes.iter() {
            if let Some(lease) = &entry.lease {
                if lease.is_expired() {
                    expired.push(entry.id.clone());
                }
            }
        }

        for node_id in expired {
            self.remove_node(&node_id).await?;
        }

        Ok(())
    }

    /// Get cluster statistics
    pub async fn stats(&self) -> ClusterStats {
        let active_nodes = self.get_active_nodes().await.len();
        let total_nodes = self.nodes.len();

        ClusterStats {
            total_nodes,
            active_nodes,
        }
    }

    /// Start background tasks for cluster management
    pub async fn start_background_tasks(&self) {
        let _nodes = self.nodes.clone();
        let cleanup_interval = self.lease_duration / 4;

        // Node cleanup task
        tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);
            loop {
                interval.tick().await;
                // Implementation would clean up expired nodes
                tracing::debug!("Running cluster cleanup task");
            }
        });
    }
}

/// Statistics about the cluster
#[derive(Debug, Clone)]
pub struct ClusterStats {
    pub total_nodes: usize,
    pub active_nodes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_addressable_directory() {
        let directory = AddressableDirectory::new();

        let reference = AddressableReference {
            addressable_type: "TestActor".to_string(),
            key: Key::StringKey {
                key: "test".to_string(),
            },
        };

        let node_id = NodeId::generate("test".to_string());
        let now = chrono::Utc::now();

        let lease = AddressableLease {
            reference: reference.clone(),
            node_id: node_id.clone(),
            expires_at: now + chrono::Duration::minutes(5),
            renew_at: now + chrono::Duration::minutes(2),
        };

        directory.register_lease(lease).await.unwrap();

        let found_node = directory.find_addressable(&reference).await;
        assert_eq!(found_node, Some(node_id));
    }

    #[tokio::test]
    async fn test_cluster_manager() {
        let manager = ClusterManager::new(Duration::from_secs(60));

        let node_id = NodeId::generate("test".to_string());
        let node_info = NodeInfo::new(node_id.clone(), "localhost".to_string(), 50051);

        manager.register_node(node_info).await.unwrap();

        let retrieved = manager.get_node(&node_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().status, NodeStatus::Active);
    }
}
