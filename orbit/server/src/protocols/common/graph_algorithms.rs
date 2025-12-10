//! Shared Graph Algorithms for Cross-Protocol Use
//!
//! This module provides protocol-agnostic graph algorithm implementations that can be
//! used by AQL, Cypher, and other protocols. The algorithms operate on generic graph
//! representations and return protocol-agnostic results.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

/// A node in the graph representation for algorithms
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub properties: HashMap<String, serde_json::Value>,
}

/// An edge in the graph representation for algorithms
#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub weight: f64,
    pub edge_type: Option<String>,
    pub properties: HashMap<String, serde_json::Value>,
}

/// Result of a shortest path algorithm
#[derive(Debug, Clone)]
pub struct ShortestPathResult {
    /// Node IDs in the path from source to target
    pub path: Vec<String>,
    /// Total cost/distance of the path
    pub cost: f64,
    /// Number of edges in the path
    pub length: usize,
    /// Whether a path was found
    pub found: bool,
}

impl Default for ShortestPathResult {
    fn default() -> Self {
        Self {
            path: Vec::new(),
            cost: f64::INFINITY,
            length: 0,
            found: false,
        }
    }
}

/// Result of a BFS/DFS traversal
#[derive(Debug, Clone)]
pub struct TraversalResult {
    /// Visited node IDs in order of visitation
    pub visited: Vec<String>,
    /// Depth of each visited node from the start
    pub depths: HashMap<String, usize>,
    /// Parent of each node in the traversal tree
    pub parents: HashMap<String, String>,
}

/// Result of finding all paths
#[derive(Debug, Clone)]
pub struct AllPathsResult {
    /// All paths found, each path is a vector of node IDs
    pub paths: Vec<Vec<String>>,
}

/// Result of k shortest paths
#[derive(Debug, Clone)]
pub struct KShortestPathsResult {
    /// K shortest paths with their costs
    pub paths: Vec<ShortestPathResult>,
}

/// Result of getting neighbors
#[derive(Debug, Clone)]
pub struct NeighborsResult {
    /// Neighbor node IDs
    pub neighbors: Vec<String>,
    /// Edge information for each neighbor
    pub edges: Vec<GraphEdge>,
}

/// Graph representation for algorithm execution
#[derive(Debug, Clone, Default)]
pub struct Graph {
    /// All nodes in the graph
    pub nodes: HashMap<String, GraphNode>,
    /// Adjacency list: node_id -> [(neighbor_id, weight, edge_type)]
    pub adjacency: HashMap<String, Vec<(String, f64, Option<String>)>>,
    /// Reverse adjacency for incoming edges
    pub reverse_adjacency: HashMap<String, Vec<(String, f64, Option<String>)>>,
}

impl Graph {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, id: String, properties: HashMap<String, serde_json::Value>) {
        self.nodes.insert(id.clone(), GraphNode { id, properties });
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, from: String, to: String, weight: f64, edge_type: Option<String>) {
        self.adjacency.entry(from.clone()).or_default().push((
            to.clone(),
            weight,
            edge_type.clone(),
        ));
        self.reverse_adjacency
            .entry(to)
            .or_default()
            .push((from, weight, edge_type));
    }

    /// Get outgoing neighbors of a node
    pub fn get_outgoing_neighbors(&self, node_id: &str) -> Vec<(String, f64, Option<String>)> {
        self.adjacency.get(node_id).cloned().unwrap_or_default()
    }

    /// Get incoming neighbors of a node
    pub fn get_incoming_neighbors(&self, node_id: &str) -> Vec<(String, f64, Option<String>)> {
        self.reverse_adjacency
            .get(node_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all neighbors (both directions)
    pub fn get_all_neighbors(&self, node_id: &str) -> Vec<(String, f64, Option<String>)> {
        let mut neighbors = self.get_outgoing_neighbors(node_id);
        neighbors.extend(self.get_incoming_neighbors(node_id));
        neighbors
    }
}

/// State for Dijkstra's algorithm priority queue
#[derive(Debug, Clone)]
struct DijkstraState {
    cost: f64,
    node: String,
}

impl PartialEq for DijkstraState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Eq for DijkstraState {}

impl Ord for DijkstraState {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for DijkstraState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Find shortest path using Dijkstra's algorithm
pub fn dijkstra(graph: &Graph, source: &str, target: &str) -> ShortestPathResult {
    if !graph.nodes.contains_key(source) || !graph.nodes.contains_key(target) {
        return ShortestPathResult::default();
    }

    let mut dist: HashMap<String, f64> = HashMap::new();
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut heap = BinaryHeap::new();

    dist.insert(source.to_string(), 0.0);
    heap.push(DijkstraState {
        cost: 0.0,
        node: source.to_string(),
    });

    while let Some(DijkstraState { cost, node }) = heap.pop() {
        if node == target {
            // Reconstruct path
            let mut path = Vec::new();
            let mut current = target.to_string();
            while current != source {
                path.push(current.clone());
                if let Some(p) = prev.get(&current) {
                    current = p.clone();
                } else {
                    break;
                }
            }
            path.push(source.to_string());
            path.reverse();

            return ShortestPathResult {
                length: path.len() - 1,
                path,
                cost,
                found: true,
            };
        }

        if cost > *dist.get(&node).unwrap_or(&f64::INFINITY) {
            continue;
        }

        for (neighbor, weight, _) in graph.get_outgoing_neighbors(&node) {
            let next_cost = cost + weight;
            if next_cost < *dist.get(&neighbor).unwrap_or(&f64::INFINITY) {
                dist.insert(neighbor.clone(), next_cost);
                prev.insert(neighbor.clone(), node.clone());
                heap.push(DijkstraState {
                    cost: next_cost,
                    node: neighbor,
                });
            }
        }
    }

    ShortestPathResult::default()
}

/// Find shortest path using BFS (for unweighted graphs)
pub fn bfs_shortest_path(graph: &Graph, source: &str, target: &str) -> ShortestPathResult {
    if !graph.nodes.contains_key(source) || !graph.nodes.contains_key(target) {
        return ShortestPathResult::default();
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut queue = VecDeque::new();

    visited.insert(source.to_string());
    queue.push_back(source.to_string());

    while let Some(node) = queue.pop_front() {
        if node == target {
            // Reconstruct path
            let mut path = Vec::new();
            let mut current = target.to_string();
            while current != source {
                path.push(current.clone());
                if let Some(p) = prev.get(&current) {
                    current = p.clone();
                } else {
                    break;
                }
            }
            path.push(source.to_string());
            path.reverse();

            return ShortestPathResult {
                length: path.len() - 1,
                cost: (path.len() - 1) as f64,
                path,
                found: true,
            };
        }

        for (neighbor, _, _) in graph.get_outgoing_neighbors(&node) {
            if !visited.contains(&neighbor) {
                visited.insert(neighbor.clone());
                prev.insert(neighbor.clone(), node.clone());
                queue.push_back(neighbor);
            }
        }
    }

    ShortestPathResult::default()
}

/// BFS traversal from a starting node
pub fn bfs_traversal(graph: &Graph, source: &str, max_depth: Option<usize>) -> TraversalResult {
    let mut result = TraversalResult {
        visited: Vec::new(),
        depths: HashMap::new(),
        parents: HashMap::new(),
    };

    if !graph.nodes.contains_key(source) {
        return result;
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    visited.insert(source.to_string());
    queue.push_back((source.to_string(), 0));
    result.depths.insert(source.to_string(), 0);

    while let Some((node, depth)) = queue.pop_front() {
        result.visited.push(node.clone());

        // Check max depth
        if let Some(max) = max_depth {
            if depth >= max {
                continue;
            }
        }

        for (neighbor, _, _) in graph.get_outgoing_neighbors(&node) {
            if !visited.contains(&neighbor) {
                visited.insert(neighbor.clone());
                result.depths.insert(neighbor.clone(), depth + 1);
                result.parents.insert(neighbor.clone(), node.clone());
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    result
}

/// DFS traversal from a starting node
pub fn dfs_traversal(graph: &Graph, source: &str, max_depth: Option<usize>) -> TraversalResult {
    let mut result = TraversalResult {
        visited: Vec::new(),
        depths: HashMap::new(),
        parents: HashMap::new(),
    };

    if !graph.nodes.contains_key(source) {
        return result;
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut stack: Vec<(String, usize)> = vec![(source.to_string(), 0)];
    result.depths.insert(source.to_string(), 0);

    while let Some((node, depth)) = stack.pop() {
        if visited.contains(&node) {
            continue;
        }
        visited.insert(node.clone());
        result.visited.push(node.clone());

        // Check max depth
        if let Some(max) = max_depth {
            if depth >= max {
                continue;
            }
        }

        for (neighbor, _, _) in graph.get_outgoing_neighbors(&node) {
            if !visited.contains(&neighbor) {
                result.depths.insert(neighbor.clone(), depth + 1);
                result.parents.insert(neighbor.clone(), node.clone());
                stack.push((neighbor, depth + 1));
            }
        }
    }

    result
}

/// Find all shortest paths between two nodes
pub fn all_shortest_paths(graph: &Graph, source: &str, target: &str) -> AllPathsResult {
    let mut result = AllPathsResult { paths: Vec::new() };

    if !graph.nodes.contains_key(source) || !graph.nodes.contains_key(target) {
        return result;
    }

    // First, find the shortest distance using BFS
    let shortest = bfs_shortest_path(graph, source, target);
    if !shortest.found {
        return result;
    }

    let target_depth = shortest.length;

    // DFS to find all paths of exactly target_depth length
    fn find_paths(
        graph: &Graph,
        current: &str,
        target: &str,
        depth: usize,
        target_depth: usize,
        path: &mut Vec<String>,
        visited: &mut HashSet<String>,
        results: &mut Vec<Vec<String>>,
    ) {
        if depth > target_depth {
            return;
        }

        path.push(current.to_string());
        visited.insert(current.to_string());

        if current == target && depth == target_depth {
            results.push(path.clone());
        } else if depth < target_depth {
            for (neighbor, _, _) in graph.get_outgoing_neighbors(current) {
                if !visited.contains(&neighbor) {
                    find_paths(
                        graph,
                        &neighbor,
                        target,
                        depth + 1,
                        target_depth,
                        path,
                        visited,
                        results,
                    );
                }
            }
        }

        path.pop();
        visited.remove(current);
    }

    let mut path = Vec::new();
    let mut visited = HashSet::new();
    find_paths(
        graph,
        source,
        target,
        0,
        target_depth,
        &mut path,
        &mut visited,
        &mut result.paths,
    );

    result
}

/// Find K shortest paths using Yen's algorithm
pub fn k_shortest_paths(
    graph: &Graph,
    source: &str,
    target: &str,
    k: usize,
) -> KShortestPathsResult {
    let mut result = KShortestPathsResult { paths: Vec::new() };

    if k == 0 || !graph.nodes.contains_key(source) || !graph.nodes.contains_key(target) {
        return result;
    }

    // Find the first shortest path
    let first_path = dijkstra(graph, source, target);
    if !first_path.found {
        return result;
    }
    result.paths.push(first_path);

    if k == 1 {
        return result;
    }

    // Simplified: return variations by finding paths with different intermediate nodes
    // Full Yen's algorithm would be more complex
    let mut candidates: Vec<ShortestPathResult> = Vec::new();

    while result.paths.len() < k {
        let last_path = result.paths.last().unwrap();

        for i in 0..last_path.path.len() - 1 {
            let spur_node = &last_path.path[i];

            // Create a modified graph excluding edges from previous paths
            let mut modified_graph = graph.clone();

            // Remove edges used by paths in result that share the same root path
            for path_result in &result.paths {
                if path_result.path.len() > i && path_result.path[..=i] == last_path.path[..=i] {
                    if i + 1 < path_result.path.len() {
                        let next_node = &path_result.path[i + 1];
                        if let Some(neighbors) = modified_graph.adjacency.get_mut(spur_node) {
                            neighbors.retain(|(n, _, _)| n != next_node);
                        }
                    }
                }
            }

            // Find spur path from spur_node to target
            let spur_path = dijkstra(&modified_graph, spur_node, target);

            if spur_path.found {
                // Combine root path with spur path
                let mut full_path = last_path.path[..i].to_vec();
                full_path.extend(spur_path.path.clone());

                let total_cost = (i as f64) + spur_path.cost;
                candidates.push(ShortestPathResult {
                    path: full_path.clone(),
                    cost: total_cost,
                    length: full_path.len() - 1,
                    found: true,
                });
            }
        }

        if candidates.is_empty() {
            break;
        }

        // Sort candidates by cost and pick the best one
        candidates.sort_by(|a, b| a.cost.partial_cmp(&b.cost).unwrap_or(Ordering::Equal));

        // Find a candidate not already in result
        let mut found_new = false;
        for candidate in candidates.drain(..) {
            let is_duplicate = result.paths.iter().any(|p| p.path == candidate.path);
            if !is_duplicate {
                result.paths.push(candidate);
                found_new = true;
                break;
            }
        }

        if !found_new {
            break;
        }
    }

    result
}

/// Get neighbors of a node
pub fn get_neighbors(
    graph: &Graph,
    node_id: &str,
    direction: NeighborDirection,
) -> NeighborsResult {
    let mut result = NeighborsResult {
        neighbors: Vec::new(),
        edges: Vec::new(),
    };

    let neighbors = match direction {
        NeighborDirection::Outgoing => graph.get_outgoing_neighbors(node_id),
        NeighborDirection::Incoming => graph.get_incoming_neighbors(node_id),
        NeighborDirection::Both => graph.get_all_neighbors(node_id),
    };

    for (neighbor_id, weight, edge_type) in neighbors {
        result.neighbors.push(neighbor_id.clone());
        result.edges.push(GraphEdge {
            from: node_id.to_string(),
            to: neighbor_id,
            weight,
            edge_type,
            properties: HashMap::new(),
        });
    }

    result
}

/// Direction for neighbor queries
#[derive(Debug, Clone, Copy)]
pub enum NeighborDirection {
    Outgoing,
    Incoming,
    Both,
}

/// Find common neighbors between two nodes
pub fn common_neighbors(graph: &Graph, node1: &str, node2: &str) -> Vec<String> {
    let neighbors1: HashSet<String> = graph
        .get_all_neighbors(node1)
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();

    let neighbors2: HashSet<String> = graph
        .get_all_neighbors(node2)
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();

    neighbors1.intersection(&neighbors2).cloned().collect()
}

/// Calculate graph distance (shortest path length) between two nodes
pub fn graph_distance(graph: &Graph, source: &str, target: &str) -> Option<usize> {
    let result = bfs_shortest_path(graph, source, target);
    if result.found {
        Some(result.length)
    } else {
        None
    }
}

// ============================================================================
// PageRank and Centrality Algorithms
// ============================================================================

/// PageRank result
#[derive(Debug, Clone)]
pub struct PageRankResult {
    /// PageRank scores for each node
    pub scores: HashMap<String, f64>,
    /// Number of iterations performed
    pub iterations: usize,
    /// Whether the algorithm converged
    pub converged: bool,
}

/// Calculate PageRank scores using power iteration
pub fn pagerank(
    graph: &Graph,
    damping: f64,
    max_iterations: usize,
    tolerance: f64,
) -> PageRankResult {
    let nodes: Vec<&String> = graph.nodes.keys().collect();
    let n = nodes.len();

    if n == 0 {
        return PageRankResult {
            scores: HashMap::new(),
            iterations: 0,
            converged: true,
        };
    }

    // Build index mapping
    let node_to_idx: HashMap<&String, usize> =
        nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();

    // Build outgoing edge counts
    let mut outgoing_count = vec![0usize; n];
    let mut incoming_edges: Vec<Vec<usize>> = vec![Vec::new(); n];

    for (from_id, neighbors) in &graph.adjacency {
        if let Some(&from_idx) = node_to_idx.get(from_id) {
            for (to_id, _, _) in neighbors {
                if let Some(&to_idx) = node_to_idx.get(to_id) {
                    outgoing_count[from_idx] += 1;
                    incoming_edges[to_idx].push(from_idx);
                }
            }
        }
    }

    // Initialize scores
    let initial_score = 1.0 / n as f64;
    let mut scores = vec![initial_score; n];
    let mut new_scores = vec![0.0f64; n];
    let teleport = (1.0 - damping) / n as f64;

    let mut converged = false;
    let mut iterations = 0;

    for iter in 0..max_iterations {
        iterations = iter + 1;

        for i in 0..n {
            let mut sum = 0.0f64;
            for &j in &incoming_edges[i] {
                if outgoing_count[j] > 0 {
                    sum += scores[j] / outgoing_count[j] as f64;
                }
            }
            new_scores[i] = teleport + damping * sum;
        }

        // Check convergence
        let delta: f64 = scores
            .iter()
            .zip(new_scores.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();

        std::mem::swap(&mut scores, &mut new_scores);

        if delta < tolerance {
            converged = true;
            break;
        }
    }

    // Convert back to HashMap
    let result_scores: HashMap<String, f64> = nodes
        .iter()
        .enumerate()
        .map(|(i, id)| ((*id).clone(), scores[i]))
        .collect();

    PageRankResult {
        scores: result_scores,
        iterations,
        converged,
    }
}

/// Degree centrality result
#[derive(Debug, Clone)]
pub struct DegreeCentralityResult {
    pub in_degree: HashMap<String, usize>,
    pub out_degree: HashMap<String, usize>,
    pub total_degree: HashMap<String, usize>,
}

/// Calculate degree centrality for all nodes
pub fn degree_centrality(graph: &Graph) -> DegreeCentralityResult {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut out_degree: HashMap<String, usize> = HashMap::new();

    // Initialize all nodes with zero degree
    for node_id in graph.nodes.keys() {
        in_degree.insert(node_id.clone(), 0);
        out_degree.insert(node_id.clone(), 0);
    }

    // Count outgoing edges
    for (from_id, neighbors) in &graph.adjacency {
        out_degree.insert(from_id.clone(), neighbors.len());
    }

    // Count incoming edges
    for (to_id, neighbors) in &graph.reverse_adjacency {
        in_degree.insert(to_id.clone(), neighbors.len());
    }

    // Calculate total degree
    let total_degree: HashMap<String, usize> = graph
        .nodes
        .keys()
        .map(|id| {
            let in_d = in_degree.get(id).copied().unwrap_or(0);
            let out_d = out_degree.get(id).copied().unwrap_or(0);
            (id.clone(), in_d + out_d)
        })
        .collect();

    DegreeCentralityResult {
        in_degree,
        out_degree,
        total_degree,
    }
}

// ============================================================================
// Connected Components
// ============================================================================

/// Connected components result
#[derive(Debug, Clone)]
pub struct ConnectedComponentsResult {
    /// Component ID for each node
    pub component_ids: HashMap<String, usize>,
    /// Number of components found
    pub num_components: usize,
    /// Nodes in each component
    pub components: Vec<Vec<String>>,
}

/// Find connected components (treats graph as undirected)
pub fn connected_components(graph: &Graph) -> ConnectedComponentsResult {
    let mut component_ids: HashMap<String, usize> = HashMap::new();
    let mut components: Vec<Vec<String>> = Vec::new();
    let mut component_id = 0;

    for node_id in graph.nodes.keys() {
        if component_ids.contains_key(node_id) {
            continue;
        }

        // BFS to find all nodes in this component
        let mut component = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(node_id.clone());
        component_ids.insert(node_id.clone(), component_id);

        while let Some(current) = queue.pop_front() {
            component.push(current.clone());

            // Get all neighbors (both directions for undirected)
            for (neighbor, _, _) in graph.get_all_neighbors(&current) {
                if !component_ids.contains_key(&neighbor) {
                    component_ids.insert(neighbor.clone(), component_id);
                    queue.push_back(neighbor);
                }
            }
        }

        components.push(component);
        component_id += 1;
    }

    ConnectedComponentsResult {
        component_ids,
        num_components: component_id,
        components,
    }
}

/// Find strongly connected components using Tarjan's algorithm
pub fn strongly_connected_components(graph: &Graph) -> ConnectedComponentsResult {
    let nodes: Vec<&String> = graph.nodes.keys().collect();
    let n = nodes.len();
    let node_to_idx: HashMap<&String, usize> =
        nodes.iter().enumerate().map(|(i, n)| (*n, i)).collect();

    let mut index_counter = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut on_stack = vec![false; n];
    let mut indices: Vec<Option<usize>> = vec![None; n];
    let mut low_links = vec![0usize; n];
    let mut component_ids = vec![None::<usize>; n];
    let mut components: Vec<Vec<String>> = Vec::new();

    fn strongconnect(
        v: usize,
        graph: &Graph,
        nodes: &[&String],
        node_to_idx: &HashMap<&String, usize>,
        index_counter: &mut usize,
        stack: &mut Vec<usize>,
        on_stack: &mut Vec<bool>,
        indices: &mut Vec<Option<usize>>,
        low_links: &mut Vec<usize>,
        component_ids: &mut Vec<Option<usize>>,
        components: &mut Vec<Vec<String>>,
    ) {
        indices[v] = Some(*index_counter);
        low_links[v] = *index_counter;
        *index_counter += 1;
        stack.push(v);
        on_stack[v] = true;

        // Get successors
        if let Some(neighbors) = graph.adjacency.get(nodes[v]) {
            for (neighbor_id, _, _) in neighbors {
                if let Some(&w) = node_to_idx.get(neighbor_id) {
                    if indices[w].is_none() {
                        strongconnect(
                            w,
                            graph,
                            nodes,
                            node_to_idx,
                            index_counter,
                            stack,
                            on_stack,
                            indices,
                            low_links,
                            component_ids,
                            components,
                        );
                        low_links[v] = low_links[v].min(low_links[w]);
                    } else if on_stack[w] {
                        low_links[v] = low_links[v].min(indices[w].unwrap());
                    }
                }
            }
        }

        // Root of SCC
        if Some(low_links[v]) == indices[v] {
            let component_id = components.len();
            let mut component = Vec::new();

            while let Some(w) = stack.pop() {
                on_stack[w] = false;
                component_ids[w] = Some(component_id);
                component.push(nodes[w].clone());
                if w == v {
                    break;
                }
            }

            components.push(component);
        }
    }

    for i in 0..n {
        if indices[i].is_none() {
            strongconnect(
                i,
                graph,
                &nodes,
                &node_to_idx,
                &mut index_counter,
                &mut stack,
                &mut on_stack,
                &mut indices,
                &mut low_links,
                &mut component_ids,
                &mut components,
            );
        }
    }

    let component_id_map: HashMap<String, usize> = nodes
        .iter()
        .enumerate()
        .filter_map(|(i, id)| component_ids[i].map(|c| ((*id).clone(), c)))
        .collect();

    ConnectedComponentsResult {
        component_ids: component_id_map,
        num_components: components.len(),
        components,
    }
}

// ============================================================================
// Similarity Algorithms
// ============================================================================

/// Calculate Jaccard similarity between two nodes based on their neighbors
pub fn jaccard_similarity(graph: &Graph, node1: &str, node2: &str) -> f64 {
    let neighbors1: HashSet<String> = graph
        .get_all_neighbors(node1)
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();

    let neighbors2: HashSet<String> = graph
        .get_all_neighbors(node2)
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();

    let intersection = neighbors1.intersection(&neighbors2).count();
    let union = neighbors1.union(&neighbors2).count();

    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Calculate Adamic-Adar score for link prediction
pub fn adamic_adar(graph: &Graph, node1: &str, node2: &str) -> f64 {
    let common = common_neighbors(graph, node1, node2);

    common
        .iter()
        .map(|neighbor| {
            let degree = graph.get_all_neighbors(neighbor).len();
            if degree > 1 {
                1.0 / (degree as f64).ln()
            } else {
                0.0
            }
        })
        .sum()
}

/// Calculate preferential attachment score
pub fn preferential_attachment(graph: &Graph, node1: &str, node2: &str) -> usize {
    let degree1 = graph.get_all_neighbors(node1).len();
    let degree2 = graph.get_all_neighbors(node2).len();
    degree1 * degree2
}

// ============================================================================
// Triangle and Clustering
// ============================================================================

/// Count triangles involving a specific node
pub fn triangle_count_node(graph: &Graph, node_id: &str) -> usize {
    let neighbors: Vec<String> = graph
        .get_all_neighbors(node_id)
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();

    let mut count = 0;

    for i in 0..neighbors.len() {
        for j in (i + 1)..neighbors.len() {
            // Check if neighbors[i] and neighbors[j] are connected
            let neighbors_of_i: HashSet<String> = graph
                .get_all_neighbors(&neighbors[i])
                .into_iter()
                .map(|(n, _, _)| n)
                .collect();

            if neighbors_of_i.contains(&neighbors[j]) {
                count += 1;
            }
        }
    }

    count
}

/// Count total triangles in the graph
pub fn triangle_count_total(graph: &Graph) -> usize {
    let mut total = 0;

    for node_id in graph.nodes.keys() {
        total += triangle_count_node(graph, node_id);
    }

    // Each triangle is counted 3 times (once for each vertex)
    total / 3
}

/// Calculate local clustering coefficient for a node
pub fn clustering_coefficient(graph: &Graph, node_id: &str) -> f64 {
    let neighbors: Vec<String> = graph
        .get_all_neighbors(node_id)
        .into_iter()
        .map(|(n, _, _)| n)
        .collect();

    let k = neighbors.len();
    if k < 2 {
        return 0.0;
    }

    let triangles = triangle_count_node(graph, node_id);
    let possible_triangles = k * (k - 1) / 2;

    triangles as f64 / possible_triangles as f64
}

// ============================================================================
// Graph Statistics
// ============================================================================

/// Graph statistics result
#[derive(Debug, Clone)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub density: f64,
    pub avg_degree: f64,
    pub max_in_degree: usize,
    pub max_out_degree: usize,
}

/// Calculate graph statistics
pub fn graph_stats(graph: &Graph) -> GraphStats {
    let node_count = graph.nodes.len();
    let edge_count: usize = graph.adjacency.values().map(|v| v.len()).sum();

    let density = if node_count > 1 {
        edge_count as f64 / (node_count * (node_count - 1)) as f64
    } else {
        0.0
    };

    let avg_degree = if node_count > 0 {
        (2.0 * edge_count as f64) / node_count as f64
    } else {
        0.0
    };

    let max_out_degree = graph.adjacency.values().map(|v| v.len()).max().unwrap_or(0);
    let max_in_degree = graph
        .reverse_adjacency
        .values()
        .map(|v| v.len())
        .max()
        .unwrap_or(0);

    GraphStats {
        node_count,
        edge_count,
        density,
        avg_degree,
        max_in_degree,
        max_out_degree,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> Graph {
        let mut graph = Graph::new();

        // Add nodes
        for i in 1..=5 {
            graph.add_node(format!("n{}", i), HashMap::new());
        }

        // Add edges: n1 -> n2 -> n3 -> n5, n1 -> n4 -> n5
        graph.add_edge("n1".to_string(), "n2".to_string(), 1.0, None);
        graph.add_edge("n2".to_string(), "n3".to_string(), 1.0, None);
        graph.add_edge("n3".to_string(), "n5".to_string(), 1.0, None);
        graph.add_edge("n1".to_string(), "n4".to_string(), 1.0, None);
        graph.add_edge("n4".to_string(), "n5".to_string(), 1.0, None);

        graph
    }

    #[test]
    fn test_bfs_shortest_path() {
        let graph = create_test_graph();
        let result = bfs_shortest_path(&graph, "n1", "n5");

        assert!(result.found);
        assert_eq!(result.length, 2);
        assert!(result.path == vec!["n1", "n4", "n5"] || result.path == vec!["n1", "n2", "n3"]);
    }

    #[test]
    fn test_dijkstra() {
        let graph = create_test_graph();
        let result = dijkstra(&graph, "n1", "n5");

        assert!(result.found);
        assert_eq!(result.length, 2);
        assert_eq!(result.cost, 2.0);
    }

    #[test]
    fn test_bfs_traversal() {
        let graph = create_test_graph();
        let result = bfs_traversal(&graph, "n1", Some(2));

        assert!(result.visited.contains(&"n1".to_string()));
        assert!(result.visited.contains(&"n2".to_string()));
        assert!(result.visited.contains(&"n4".to_string()));
    }

    #[test]
    fn test_common_neighbors() {
        let mut graph = Graph::new();
        for i in 1..=4 {
            graph.add_node(format!("n{}", i), HashMap::new());
        }
        graph.add_edge("n1".to_string(), "n3".to_string(), 1.0, None);
        graph.add_edge("n2".to_string(), "n3".to_string(), 1.0, None);
        graph.add_edge("n1".to_string(), "n4".to_string(), 1.0, None);
        graph.add_edge("n2".to_string(), "n4".to_string(), 1.0, None);

        let common = common_neighbors(&graph, "n1", "n2");
        assert!(common.contains(&"n3".to_string()));
        assert!(common.contains(&"n4".to_string()));
    }
}
