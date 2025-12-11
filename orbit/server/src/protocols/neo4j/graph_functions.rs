//! Graph pathfinding functions for Neo4j Cypher queries
//!
//! Implements graph algorithms like shortestPath() and allShortestPaths()

use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_shared::graph::{GraphNode, GraphRelationship, NodeId};
use std::collections::{HashMap, HashSet, VecDeque};

/// Represents a path in the graph
#[derive(Debug, Clone)]
pub struct GraphPath {
    /// Nodes in the path (in order)
    pub nodes: Vec<GraphNode>,
    /// Relationships connecting the nodes (in order)
    pub relationships: Vec<GraphRelationship>,
    /// Total length of the path
    pub length: usize,
}

impl GraphPath {
    /// Create a new empty path
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            relationships: Vec::new(),
            length: 0,
        }
    }

    /// Add a node and relationship to the path
    pub fn add_step(&mut self, node: GraphNode, relationship: Option<GraphRelationship>) {
        self.nodes.push(node);
        if let Some(rel) = relationship {
            self.relationships.push(rel);
        }
        self.length = self.relationships.len();
    }

    /// Get the start node
    pub fn start_node(&self) -> Option<&GraphNode> {
        self.nodes.first()
    }

    /// Get the end node
    pub fn end_node(&self) -> Option<&GraphNode> {
        self.nodes.last()
    }
}

impl Default for GraphPath {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the shortest path between two nodes using BFS
///
/// # Arguments
/// * `start_node` - Starting node
/// * `end_node` - Target node
/// * `get_neighbors` - Function to get neighboring nodes and relationships
/// * `max_depth` - Optional maximum path length
///
/// # Returns
/// The shortest path, or None if no path exists
pub fn shortest_path<F>(
    start_node: &GraphNode,
    end_node: &GraphNode,
    mut get_neighbors: F,
    max_depth: Option<usize>,
) -> ProtocolResult<Option<GraphPath>>
where
    F: FnMut(&NodeId) -> ProtocolResult<Vec<(GraphNode, GraphRelationship)>>,
{
    if start_node.id == end_node.id {
        let mut path = GraphPath::new();
        path.add_step(start_node.clone(), None);
        return Ok(Some(path));
    }

    let max_depth = max_depth.unwrap_or(usize::MAX);
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent_map: HashMap<NodeId, (NodeId, GraphRelationship)> = HashMap::new();

    queue.push_back((start_node.id.clone(), 0));
    visited.insert(start_node.id.clone());

    while let Some((current_id, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        // Get neighbors
        let neighbors = get_neighbors(&current_id)?;

        for (neighbor_node, relationship) in neighbors {
            if visited.contains(&neighbor_node.id) {
                continue;
            }

            visited.insert(neighbor_node.id.clone());
            parent_map.insert(
                neighbor_node.id.clone(),
                (current_id.clone(), relationship),
            );

            // Check if we reached the target
            if neighbor_node.id == end_node.id {
                return Ok(Some(reconstruct_path(
                    start_node,
                    end_node,
                    &parent_map,
                    &mut get_neighbors,
                )?));
            }

            queue.push_back((neighbor_node.id.clone(), depth + 1));
        }
    }

    Ok(None) // No path found
}

/// Find all shortest paths between two nodes
///
/// # Arguments
/// * `start_node` - Starting node
/// * `end_node` - Target node
/// * `get_neighbors` - Function to get neighboring nodes and relationships
/// * `max_depth` - Optional maximum path length
///
/// # Returns
/// All shortest paths (all with the same minimum length)
pub fn all_shortest_paths<F>(
    start_node: &GraphNode,
    end_node: &GraphNode,
    mut get_neighbors: F,
    max_depth: Option<usize>,
) -> ProtocolResult<Vec<GraphPath>>
where
    F: FnMut(&NodeId) -> ProtocolResult<Vec<(GraphNode, GraphRelationship)>>,
{
    if start_node.id == end_node.id {
        let mut path = GraphPath::new();
        path.add_step(start_node.clone(), None);
        return Ok(vec![path]);
    }

    let max_depth = max_depth.unwrap_or(usize::MAX);
    let mut queue = VecDeque::new();
    let mut visited_at_depth: HashMap<NodeId, usize> = HashMap::new();
    let mut parent_map: HashMap<NodeId, Vec<(NodeId, GraphRelationship)>> = HashMap::new();
    let mut shortest_length: Option<usize> = None;

    queue.push_back((start_node.id.clone(), 0));
    visited_at_depth.insert(start_node.id.clone(), 0);

    while let Some((current_id, depth)) = queue.pop_front() {
        // If we've found a shortest path and we're beyond that depth, stop
        if let Some(shortest) = shortest_length {
            if depth > shortest {
                break;
            }
        }

        if depth >= max_depth {
            continue;
        }

        // Get neighbors
        let neighbors = get_neighbors(&current_id)?;

        for (neighbor_node, relationship) in neighbors {
            let next_depth = depth + 1;

            // Check if we've visited this node at a shallower depth
            if let Some(&prev_depth) = visited_at_depth.get(&neighbor_node.id) {
                if next_depth > prev_depth {
                    continue; // Skip, we've already found a shorter path
                }
            }

            visited_at_depth.insert(neighbor_node.id.clone(), next_depth);

            // Add to parent map
            parent_map
                .entry(neighbor_node.id.clone())
                .or_insert_with(Vec::new)
                .push((current_id.clone(), relationship));

            // Check if we reached the target
            if neighbor_node.id == end_node.id {
                if shortest_length.is_none() {
                    shortest_length = Some(next_depth);
                }
            } else if shortest_length.is_none() || next_depth < shortest_length.unwrap() {
                queue.push_back((neighbor_node.id.clone(), next_depth));
            }
        }
    }

    if shortest_length.is_none() {
        return Ok(Vec::new()); // No paths found
    }

    // Reconstruct all paths
    let paths = reconstruct_all_paths(start_node, end_node, &parent_map, &mut get_neighbors)?;

    Ok(paths)
}

/// Reconstruct a single path from the parent map
fn reconstruct_path<F>(
    start_node: &GraphNode,
    end_node: &GraphNode,
    parent_map: &HashMap<NodeId, (NodeId, GraphRelationship)>,
    get_node: &mut F,
) -> ProtocolResult<GraphPath>
where
    F: FnMut(&NodeId) -> ProtocolResult<Vec<(GraphNode, GraphRelationship)>>,
{
    let mut path = GraphPath::new();
    let mut current_id = end_node.id.clone();
    let mut nodes = vec![end_node.clone()];
    let mut relationships = Vec::new();

    // Trace back from end to start
    while current_id != start_node.id {
        if let Some((parent_id, relationship)) = parent_map.get(&current_id) {
            relationships.push(relationship.clone());
            
            // Get the parent node
            let parent_neighbors = get_node(parent_id)?;
            let parent_node = parent_neighbors
                .into_iter()
                .find(|(n, _)| &n.id == parent_id)
                .map(|(n, _)| n)
                .unwrap_or_else(|| GraphNode::new(vec![], HashMap::new()));
            
            nodes.push(parent_node);
            current_id = parent_id.clone();
        } else {
            return Err(ProtocolError::CypherError(
                "Path reconstruction failed".to_string(),
            ));
        }
    }

    // Reverse to get start -> end order
    nodes.reverse();
    relationships.reverse();

    for (i, node) in nodes.iter().enumerate() {
        let rel = if i < relationships.len() {
            Some(relationships[i].clone())
        } else {
            None
        };
        path.add_step(node.clone(), rel);
    }

    Ok(path)
}

/// Reconstruct all paths from the parent map (for all_shortest_paths)
fn reconstruct_all_paths<F>(
    start_node: &GraphNode,
    end_node: &GraphNode,
    parent_map: &HashMap<NodeId, Vec<(NodeId, GraphRelationship)>>,
    _get_node: &mut F,
) -> ProtocolResult<Vec<GraphPath>>
where
    F: FnMut(&NodeId) -> ProtocolResult<Vec<(GraphNode, GraphRelationship)>>,
{
    let mut all_paths = Vec::new();
    let mut current_paths = vec![vec![(end_node.clone(), None)]];

    // Build paths backwards from end to start
    while !current_paths.is_empty() {
        let mut next_paths = Vec::new();

        for path in current_paths {
            let (last_node, _) = &path[path.len() - 1];

            if last_node.id == start_node.id {
                // Complete path found
                let mut complete_path = GraphPath::new();
                for (node, rel) in path.iter().rev() {
                    complete_path.add_step(node.clone(), rel.clone());
                }
                all_paths.push(complete_path);
            } else if let Some(parents) = parent_map.get(&last_node.id) {
                // Extend path with all possible parents
                for (parent_id, relationship) in parents {
                    let mut new_path = path.clone();
                    // Create a simple node for the parent (in real implementation, would fetch from graph)
                    let parent_node = GraphNode::with_id(
                        parent_id.clone(),
                        vec![],
                        HashMap::new(),
                    );
                    new_path.push((parent_node, Some(relationship.clone())));
                    next_paths.push(new_path);
                }
            }
        }

        current_paths = next_paths;
    }

    Ok(all_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn create_test_node(id: &str, label: &str) -> GraphNode {
        GraphNode::with_id(
            NodeId::new(id.to_string()),
            vec![label.to_string()],
            HashMap::new(),
        )
    }

    fn create_test_relationship(
        start: &str,
        end: &str,
        rel_type: &str,
    ) -> GraphRelationship {
        GraphRelationship::new(
            NodeId::new(start.to_string()),
            NodeId::new(end.to_string()),
            rel_type.to_string(),
            HashMap::new(),
        )
    }

    #[test]
    fn test_graph_path_creation() {
        let mut path = GraphPath::new();
        assert_eq!(path.length, 0);
        assert!(path.nodes.is_empty());

        let node1 = create_test_node("node1", "Person");
        path.add_step(node1.clone(), None);
        assert_eq!(path.length, 0);
        assert_eq!(path.nodes.len(), 1);

        let node2 = create_test_node("node2", "Person");
        let rel = create_test_relationship("node1", "node2", "KNOWS");
        path.add_step(node2, Some(rel));
        assert_eq!(path.length, 1);
        assert_eq!(path.nodes.len(), 2);
    }

    #[test]
    fn test_shortest_path_simple() {
        // Create a simple graph: A -> B -> C
        let node_a = create_test_node("A", "Node");
        let node_b = create_test_node("B", "Node");
        let node_c = create_test_node("C", "Node");

        let rel_ab = create_test_relationship("A", "B", "CONNECTS");
        let rel_bc = create_test_relationship("B", "C", "CONNECTS");

        let get_neighbors = |node_id: &NodeId| -> ProtocolResult<Vec<(GraphNode, GraphRelationship)>> {
            match node_id.as_str() {
                "A" => Ok(vec![(node_b.clone(), rel_ab.clone())]),
                "B" => Ok(vec![(node_c.clone(), rel_bc.clone())]),
                "C" => Ok(vec![]),
                _ => Ok(vec![]),
            }
        };

        let result = shortest_path(&node_a, &node_c, get_neighbors, None).unwrap();
        assert!(result.is_some());

        let path = result.unwrap();
        assert_eq!(path.length, 2);
        assert_eq!(path.nodes.len(), 3);
    }

    #[test]
    fn test_shortest_path_no_path() {
        let node_a = create_test_node("A", "Node");
        let node_z = create_test_node("Z", "Node");

        let get_neighbors = |_node_id: &NodeId| -> ProtocolResult<Vec<(GraphNode, GraphRelationship)>> {
            Ok(vec![])
        };

        let result = shortest_path(&node_a, &node_z, get_neighbors, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_shortest_path_same_node() {
        let node_a = create_test_node("A", "Node");

        let get_neighbors = |_node_id: &NodeId| -> ProtocolResult<Vec<(GraphNode, GraphRelationship)>> {
            Ok(vec![])
        };

        let result = shortest_path(&node_a, &node_a, get_neighbors, None).unwrap();
        assert!(result.is_some());

        let path = result.unwrap();
        assert_eq!(path.length, 0);
        assert_eq!(path.nodes.len(), 1);
    }
}
