use crate::mesh::NodeId;
use serde::{Deserialize, Serialize};

/// A route representing a path through the cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub hops: Vec<NodeId>,
}

impl Route {
    pub fn new(hops: Vec<NodeId>) -> Self {
        Self { hops }
    }

    pub fn direct(target: NodeId) -> Self {
        Self { hops: vec![target] }
    }

    pub fn is_empty(&self) -> bool {
        self.hops.is_empty()
    }

    pub fn len(&self) -> usize {
        self.hops.len()
    }

    pub fn next_hop(&self) -> Option<&NodeId> {
        self.hops.first()
    }

    pub fn advance(&mut self) -> Option<NodeId> {
        if !self.hops.is_empty() {
            Some(self.hops.remove(0))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_route() {
        let node = NodeId::generate("test".to_string());
        let route = Route::direct(node.clone());

        assert_eq!(route.len(), 1);
        assert_eq!(route.next_hop(), Some(&node));
    }

    #[test]
    fn test_route_advance() {
        let node1 = NodeId::generate("test".to_string());
        let node2 = NodeId::generate("test".to_string());
        let mut route = Route::new(vec![node1.clone(), node2.clone()]);

        assert_eq!(route.advance(), Some(node1));
        assert_eq!(route.len(), 1);
        assert_eq!(route.next_hop(), Some(&node2));
    }

    #[test]
    fn test_empty_route() {
        let route = Route::new(vec![]);
        assert!(route.is_empty());
        assert_eq!(route.len(), 0);
        assert_eq!(route.next_hop(), None);
    }
}
