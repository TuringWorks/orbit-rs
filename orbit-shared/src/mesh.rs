use orbit_util::RngUtils;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

pub type Namespace = String;
pub type NodeKey = String;

/// Unique identifier for a node in the cluster
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId {
    pub key: NodeKey,
    pub namespace: Namespace,
}

impl NodeId {
    /// Generate a new random NodeId for the given namespace
    pub fn generate(namespace: Namespace) -> Self {
        Self {
            key: RngUtils::random_string(),
            namespace,
        }
    }

    /// Create a NodeId with a specific key and namespace
    pub fn new(key: NodeKey, namespace: Namespace) -> Self {
        Self { key, namespace }
    }

    /// Parse NodeId from string format "namespace:key"
    pub fn from_string(s: &str) -> Self {
        if let Some((namespace, key)) = s.split_once(':') {
            Self {
                key: key.to_string(),
                namespace: namespace.to_string(),
            }
        } else {
            Self {
                key: s.to_string(),
                namespace: "default".to_string(),
            }
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}:{})", self.namespace, self.key)
    }
}

/// Capabilities that a node can provide
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeCapabilities {
    pub addressable_types: Vec<String>,
    pub max_addressables: Option<u32>,
    pub tags: HashMap<String, String>,
}

/// Current status of a node
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NodeStatus {
    #[default]
    Active,
    Draining,
    Stopped,
}

/// Information about a node in the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub url: String,
    pub port: u16,
    pub capabilities: NodeCapabilities,
    pub status: NodeStatus,
    pub lease: Option<NodeLease>,
}

impl NodeInfo {
    pub fn new(id: NodeId, url: String, port: u16) -> Self {
        Self {
            id,
            url,
            port,
            capabilities: NodeCapabilities::default(),
            status: NodeStatus::default(),
            lease: None,
        }
    }
}

/// A lease representing a node's claim to remain active
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLease {
    pub node_id: NodeId,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub renew_at: chrono::DateTime<chrono::Utc>,
    pub challenge_token: Option<String>,
}

impl NodeLease {
    pub fn new(
        node_id: NodeId,
        expires_at: chrono::DateTime<chrono::Utc>,
        renew_at: chrono::DateTime<chrono::Utc>,
    ) -> Self {
        Self {
            node_id,
            expires_at,
            renew_at,
            challenge_token: None,
        }
    }

    /// Check if the lease has expired
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }

    /// Check if the lease should be renewed
    pub fn should_renew(&self) -> bool {
        chrono::Utc::now() >= self.renew_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_node_id_generate() {
        let namespace = "test-namespace".to_string();
        let node_id = NodeId::generate(namespace.clone());
        assert_eq!(node_id.namespace, namespace);
        assert!(!node_id.key.is_empty());
    }

    #[test]
    fn test_node_id_display() {
        let node_id = NodeId::new("test-key".to_string(), "test-namespace".to_string());
        assert_eq!(node_id.to_string(), "(test-namespace:test-key)");
    }

    #[test]
    fn test_node_lease_expiry() {
        let node_id = NodeId::generate("test".to_string());
        let now = Utc::now();

        let expired_lease = NodeLease::new(
            node_id.clone(),
            now - Duration::minutes(1), // expired 1 minute ago
            now - Duration::minutes(2), // should have renewed 2 minutes ago
        );
        assert!(expired_lease.is_expired());
        assert!(expired_lease.should_renew());

        let active_lease = NodeLease::new(
            node_id,
            now + Duration::minutes(10), // expires in 10 minutes
            now + Duration::minutes(5),  // renew in 5 minutes
        );
        assert!(!active_lease.is_expired());
        assert!(!active_lease.should_renew());
    }

    #[test]
    fn test_node_info_creation() {
        let node_id = NodeId::generate("test".to_string());
        let node_info = NodeInfo::new(node_id.clone(), "localhost".to_string(), 8080);

        assert_eq!(node_info.id, node_id);
        assert_eq!(node_info.url, "localhost");
        assert_eq!(node_info.port, 8080);
        assert_eq!(node_info.status, NodeStatus::Active);
    }
}
