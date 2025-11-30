//! Graph Algorithm Procedures for Cypher/Bolt Protocol
//!
//! This module provides Cypher stored procedures for graph algorithms,
//! leveraging GPU acceleration when available through orbit-compute.

use crate::protocols::cypher::graph_engine::QueryResult;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_shared::graph::{Direction, GraphNode, GraphRelationship, GraphStorage};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tracing::info;

#[cfg(feature = "gpu-graph-traversal")]
use orbit_compute::graph_traversal::{
    GPUGraphTraversal, GraphData, NodeProperties, TraversalConfig,
};
#[cfg(feature = "gpu-graph-traversal")]
use tokio::sync::RwLock;

/// Graph algorithm procedure handler for Bolt/Cypher protocol
pub struct GraphAlgorithmProcedures<S: GraphStorage> {
    storage: Arc<S>,
    /// Known labels to scan when getting all nodes
    known_labels: Vec<String>,
    /// GPU-accelerated graph traversal engine (when feature enabled)
    #[cfg(feature = "gpu-graph-traversal")]
    gpu_traversal: Option<Arc<RwLock<GPUGraphTraversal>>>,
}

impl<S: GraphStorage + Send + Sync + 'static> GraphAlgorithmProcedures<S> {
    /// Create new graph algorithm procedures handler
    pub fn new(storage: Arc<S>) -> Self {
        Self {
            storage,
            // Default known labels - in production this should be configurable
            known_labels: vec![
                "Person".to_string(),
                "Node".to_string(),
                "Entity".to_string(),
            ],
            #[cfg(feature = "gpu-graph-traversal")]
            gpu_traversal: None,
        }
    }

    /// Create with custom known labels
    pub fn with_labels(storage: Arc<S>, labels: Vec<String>) -> Self {
        Self {
            storage,
            known_labels: labels,
            #[cfg(feature = "gpu-graph-traversal")]
            gpu_traversal: None,
        }
    }

    /// Initialize GPU acceleration for graph algorithms
    #[cfg(feature = "gpu-graph-traversal")]
    pub async fn init_gpu_acceleration(&mut self) -> ProtocolResult<()> {
        let config = TraversalConfig::default();
        match GPUGraphTraversal::new(config).await {
            Ok(traversal) => {
                info!("GPU graph traversal engine initialized successfully");
                self.gpu_traversal = Some(Arc::new(RwLock::new(traversal)));
                Ok(())
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize GPU graph traversal: {}, using CPU fallback",
                    e
                );
                Ok(()) // Non-fatal, will use CPU fallback
            }
        }
    }

    /// Check if GPU acceleration is available
    #[cfg(feature = "gpu-graph-traversal")]
    pub fn has_gpu_acceleration(&self) -> bool {
        self.gpu_traversal.is_some()
    }

    /// Add a label to the known labels list
    pub fn add_known_label(&mut self, label: String) {
        if !self.known_labels.contains(&label) {
            self.known_labels.push(label);
        }
    }

    /// Execute a graph algorithm procedure call
    pub async fn execute_procedure(
        &self,
        procedure_name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        match procedure_name.to_lowercase().as_str() {
            "orbit.graph.pagerank" => self.execute_pagerank(args).await,
            "orbit.graph.shortestpath" => self.execute_shortest_path(args).await,
            "orbit.graph.bfs" => self.execute_bfs(args).await,
            "orbit.graph.dfs" => self.execute_dfs(args).await,
            "orbit.graph.communitydetection" => self.execute_community_detection(args).await,
            "orbit.graph.connectedcomponents" => self.execute_connected_components(args).await,
            "orbit.graph.betweennesscentrality" => self.execute_betweenness_centrality(args).await,
            "orbit.graph.closenesscentrality" => self.execute_closeness_centrality(args).await,
            "orbit.graph.degreecentrality" => self.execute_degree_centrality(args).await,
            "orbit.graph.trianglecount" => self.execute_triangle_count(args).await,
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown graph algorithm procedure: {procedure_name}"
            ))),
        }
    }

    /// Execute orbit.graph.pagerank procedure
    /// CALL orbit.graph.pagerank({damping: 0.85, iterations: 20, tolerance: 0.0001})
    async fn execute_pagerank(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let damping = config
            .get("damping")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(0.85);

        let max_iterations = config
            .get("iterations")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(20);

        let tolerance = config
            .get("tolerance")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32)
            .unwrap_or(0.0001);

        // Try GPU-accelerated PageRank if available
        #[cfg(feature = "gpu-graph-traversal")]
        if let Some(ref gpu_traversal) = self.gpu_traversal {
            let (graph_data, _node_index) = self.build_graph_data().await?;

            if graph_data.node_count > 0 {
                let traversal = gpu_traversal.read().await;
                match traversal
                    .pagerank(&graph_data, damping, max_iterations, tolerance)
                    .await
                {
                    Ok(result) => {
                        info!(
                            "GPU PageRank completed: {} iterations in {}ms (GPU: {}, delta: {:.6})",
                            result.iterations,
                            result.execution_time_ms,
                            result.used_gpu,
                            result.convergence_delta
                        );

                        // Get all nodes for ID reverse lookup
                        let nodes = self.get_all_nodes().await?;
                        let idx_to_id: HashMap<u64, String> = nodes
                            .iter()
                            .enumerate()
                            .map(|(i, n)| (i as u64, n.id.to_string()))
                            .collect();

                        // Build sorted results
                        let columns = vec!["node_id".to_string(), "pagerank".to_string()];
                        let mut results: Vec<(f32, String)> = result
                            .scores
                            .iter()
                            .filter_map(|(&node_idx, &score)| {
                                idx_to_id.get(&node_idx).map(|id| (score, id.clone()))
                            })
                            .collect();
                        results.sort_by(|a, b| {
                            b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal)
                        });

                        let rows: Vec<Vec<Option<String>>> = results
                            .into_iter()
                            .map(|(score, id)| vec![Some(id), Some(format!("{:.6}", score))])
                            .collect();

                        return Ok(QueryResult {
                            nodes: Vec::new(),
                            relationships: Vec::new(),
                            columns,
                            rows,
                        });
                    }
                    Err(e) => {
                        tracing::warn!("GPU PageRank failed, falling back to CPU: {}", e);
                        // Fall through to CPU implementation
                    }
                }
            }
        }

        // CPU fallback implementation
        self.execute_pagerank_cpu(damping, max_iterations, tolerance)
            .await
    }

    /// CPU-based PageRank implementation (fallback)
    async fn execute_pagerank_cpu(
        &self,
        damping: f32,
        max_iterations: usize,
        tolerance: f32,
    ) -> ProtocolResult<QueryResult> {
        // Get all nodes and relationships
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec!["node_id".to_string(), "pagerank".to_string()],
                rows: Vec::new(),
            });
        }

        // Build adjacency information
        let node_ids: Vec<String> = nodes.iter().map(|n| n.id.to_string()).collect();
        let node_index: HashMap<&str, usize> = node_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let n = nodes.len();

        // Build outgoing edge counts and incoming edges
        let mut outgoing_count = vec![0usize; n];
        let mut incoming_edges: Vec<Vec<usize>> = vec![Vec::new(); n];

        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                outgoing_count[from_idx] += 1;
                incoming_edges[to_idx].push(from_idx);
            }
        }

        // Initialize PageRank scores
        let initial_score = 1.0 / n as f32;
        let mut scores = vec![initial_score; n];
        let mut new_scores = vec![0.0f32; n];

        // PageRank iteration
        for iteration in 0..max_iterations {
            let teleport = (1.0 - damping) / n as f32;

            for i in 0..n {
                let mut sum = 0.0f32;
                for &j in &incoming_edges[i] {
                    if outgoing_count[j] > 0 {
                        sum += scores[j] / outgoing_count[j] as f32;
                    }
                }
                new_scores[i] = teleport + damping * sum;
            }

            // Check convergence
            let delta: f32 = scores
                .iter()
                .zip(new_scores.iter())
                .map(|(a, b)| (a - b).abs())
                .sum();

            std::mem::swap(&mut scores, &mut new_scores);

            if delta < tolerance {
                info!(
                    "PageRank converged after {} iterations with delta {}",
                    iteration + 1,
                    delta
                );
                break;
            }
        }

        // Format results
        let columns = vec!["node_id".to_string(), "pagerank".to_string()];
        let mut results: Vec<(f32, String)> = scores
            .iter()
            .zip(node_ids.iter())
            .map(|(&score, id)| (score, id.clone()))
            .collect();
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let rows: Vec<Vec<Option<String>>> = results
            .into_iter()
            .map(|(score, id)| vec![Some(id), Some(format!("{:.6}", score))])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.shortestPath procedure
    /// CALL orbit.graph.shortestPath(from_node_id, to_node_id, {weighted: false})
    async fn execute_shortest_path(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.len() < 2 {
            return Err(ProtocolError::CypherError(
                "orbit.graph.shortestPath requires 2 arguments: (from_node_id, to_node_id)"
                    .to_string(),
            ));
        }

        let from_id = self.extract_string_arg(&args[0], "from_node_id")?;
        let to_id = self.extract_string_arg(&args[1], "to_node_id")?;

        let config = if args.len() > 2 {
            self.parse_config_arg(&args[2])?
        } else {
            HashMap::new()
        };

        let weighted = config
            .get("weighted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Try GPU-accelerated shortest path if available
        #[cfg(feature = "gpu-graph-traversal")]
        if let Some(ref gpu_traversal) = self.gpu_traversal {
            let (graph_data, node_index) = if weighted {
                self.build_weighted_graph_data().await?
            } else {
                // For unweighted, use BFS which is faster
                self.build_graph_data().await?
            };

            if graph_data.node_count > 0 {
                if let (Some(&source_idx), Some(&target_idx)) =
                    (node_index.get(&from_id), node_index.get(&to_id))
                {
                    let traversal = gpu_traversal.read().await;
                    let result = if weighted && graph_data.edge_weights.is_some() {
                        // Use Dijkstra for weighted graphs
                        traversal
                            .dijkstra(&graph_data, source_idx as u64, Some(target_idx as u64))
                            .await
                    } else {
                        // Use BFS for unweighted graphs
                        traversal
                            .bfs(&graph_data, source_idx as u64, Some(target_idx as u64))
                            .await
                    };

                    match result {
                        Ok(result) => {
                            info!(
                                "GPU shortest path completed: {} nodes explored in {}ms (GPU: {})",
                                result.stats.nodes_explored,
                                result.execution_time_ms,
                                result.used_gpu
                            );

                            // Get all nodes for ID reverse lookup
                            let nodes = self.get_all_nodes().await?;
                            let idx_to_id: HashMap<u64, String> = nodes
                                .iter()
                                .enumerate()
                                .map(|(i, n)| (i as u64, n.id.to_string()))
                                .collect();

                            let columns =
                                vec!["path".to_string(), "length".to_string(), "cost".to_string()];

                            if result.paths.is_empty() {
                                return Ok(QueryResult {
                                    nodes: Vec::new(),
                                    relationships: Vec::new(),
                                    columns,
                                    rows: vec![vec![
                                        Some("[]".to_string()),
                                        Some("0".to_string()),
                                        Some("infinity".to_string()),
                                    ]],
                                });
                            }

                            // Convert path node indices to IDs
                            let path = &result.paths[0];
                            let path_ids: Vec<String> = path
                                .nodes
                                .iter()
                                .filter_map(|&idx| idx_to_id.get(&idx).cloned())
                                .collect();

                            let rows = vec![vec![
                                Some(serde_json::to_string(&path_ids).unwrap_or_default()),
                                Some(path.length.to_string()),
                                Some(format!("{:.2}", path.weight)),
                            ]];

                            return Ok(QueryResult {
                                nodes: Vec::new(),
                                relationships: Vec::new(),
                                columns,
                                rows,
                            });
                        }
                        Err(e) => {
                            tracing::warn!("GPU shortest path failed, falling back to CPU: {}", e);
                            // Fall through to CPU implementation
                        }
                    }
                }
            }
        }

        // CPU fallback implementation
        self.execute_shortest_path_cpu(&from_id, &to_id, weighted)
            .await
    }

    /// CPU-based shortest path implementation (fallback)
    async fn execute_shortest_path_cpu(
        &self,
        from_id: &str,
        to_id: &str,
        weighted: bool,
    ) -> ProtocolResult<QueryResult> {
        // Get all nodes and relationships
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        // Build node index
        let node_index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let n = nodes.len();

        let source = node_index.get(from_id).copied();
        let target = node_index.get(to_id).copied();

        if source.is_none() || target.is_none() {
            return Err(ProtocolError::CypherError(
                "Source or target node not found".to_string(),
            ));
        }

        let source = source.unwrap();
        let target = target.unwrap();

        // Build adjacency list
        let mut adj: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                let weight = if weighted {
                    rel.properties
                        .get("weight")
                        .and_then(|v| v.as_f64())
                        .map(|f| f as f32)
                        .unwrap_or(1.0)
                } else {
                    1.0
                };
                adj[from_idx].push((to_idx, weight));
                // Add reverse edge for undirected graphs (comment out for directed)
                adj[to_idx].push((from_idx, weight));
            }
        }

        // Dijkstra's algorithm
        let mut dist = vec![f32::INFINITY; n];
        let mut prev: Vec<Option<usize>> = vec![None; n];
        let mut visited = vec![false; n];

        dist[source] = 0.0;

        for _ in 0..n {
            // Find minimum distance node
            let mut min_dist = f32::INFINITY;
            let mut min_idx = None;
            for (i, &d) in dist.iter().enumerate() {
                if !visited[i] && d < min_dist {
                    min_dist = d;
                    min_idx = Some(i);
                }
            }

            let u = match min_idx {
                Some(idx) => idx,
                None => break,
            };

            if u == target {
                break;
            }

            visited[u] = true;

            for &(v, weight) in &adj[u] {
                let alt = dist[u] + weight;
                if alt < dist[v] {
                    dist[v] = alt;
                    prev[v] = Some(u);
                }
            }
        }

        // Reconstruct path
        let columns = vec!["path".to_string(), "length".to_string(), "cost".to_string()];

        if dist[target].is_infinite() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns,
                rows: vec![vec![
                    Some("[]".to_string()),
                    Some("0".to_string()),
                    Some("infinity".to_string()),
                ]],
            });
        }

        let mut path = Vec::new();
        let mut current = Some(target);
        while let Some(idx) = current {
            path.push(nodes[idx].id.clone());
            current = prev[idx];
        }
        path.reverse();

        let rows = vec![vec![
            Some(serde_json::to_string(&path).unwrap_or_default()),
            Some((path.len() - 1).to_string()),
            Some(format!("{:.2}", dist[target])),
        ]];

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.bfs procedure
    /// CALL orbit.graph.bfs(start_node_id, {maxDepth: 5})
    async fn execute_bfs(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "orbit.graph.bfs requires 1 argument: (start_node_id)".to_string(),
            ));
        }

        let start_id = self.extract_string_arg(&args[0], "start_node_id")?;

        let config = if args.len() > 1 {
            self.parse_config_arg(&args[1])?
        } else {
            HashMap::new()
        };

        let max_depth = config
            .get("maxDepth")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(usize::MAX);

        // Try GPU-accelerated BFS if available
        #[cfg(feature = "gpu-graph-traversal")]
        if let Some(ref gpu_traversal) = self.gpu_traversal {
            let (graph_data, node_index) = self.build_graph_data().await?;

            if graph_data.node_count > 0 {
                if let Some(&start_idx) = node_index.get(&start_id) {
                    let traversal = gpu_traversal.read().await;
                    match traversal.bfs(&graph_data, start_idx as u64, None).await {
                        Ok(result) => {
                            info!(
                                "GPU BFS completed: {} nodes visited in {}ms (GPU: {})",
                                result.stats.nodes_explored,
                                result.execution_time_ms,
                                result.used_gpu
                            );

                            // Get all nodes for ID reverse lookup
                            let nodes = self.get_all_nodes().await?;
                            let idx_to_id: HashMap<u64, String> = nodes
                                .iter()
                                .enumerate()
                                .map(|(i, n)| (i as u64, n.id.to_string()))
                                .collect();

                            // Build result from visited nodes
                            let columns = vec!["node_id".to_string(), "depth".to_string()];
                            let rows: Vec<Vec<Option<String>>> = result
                                .visited_nodes
                                .iter()
                                .filter_map(|&node_idx| {
                                    idx_to_id.get(&node_idx).map(|id| {
                                        // Estimate depth from path if available
                                        let depth = result
                                            .paths
                                            .iter()
                                            .find(|p| p.nodes.contains(&node_idx))
                                            .map(|p| p.length)
                                            .unwrap_or(0);
                                        vec![Some(id.clone()), Some(depth.to_string())]
                                    })
                                })
                                .collect();

                            return Ok(QueryResult {
                                nodes: Vec::new(),
                                relationships: Vec::new(),
                                columns,
                                rows,
                            });
                        }
                        Err(e) => {
                            tracing::warn!("GPU BFS failed, falling back to CPU: {}", e);
                            // Fall through to CPU implementation
                        }
                    }
                }
            }
        }

        // CPU fallback implementation
        self.execute_bfs_cpu(&start_id, max_depth).await
    }

    /// CPU-based BFS implementation (fallback)
    async fn execute_bfs_cpu(
        &self,
        start_id: &str,
        max_depth: usize,
    ) -> ProtocolResult<QueryResult> {
        // Get all nodes and relationships
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        // Build node index and adjacency
        let node_index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let n = nodes.len();

        let start = node_index.get(start_id).copied();
        if start.is_none() {
            return Err(ProtocolError::CypherError(format!(
                "Start node '{}' not found",
                start_id
            )));
        }
        let start = start.unwrap();

        // Build adjacency list
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                adj[from_idx].push(to_idx);
                adj[to_idx].push(from_idx);
            }
        }

        // BFS
        let mut visited = vec![false; n];
        let mut depth = vec![0usize; n];
        let mut queue = VecDeque::new();
        let mut result_nodes = Vec::new();

        visited[start] = true;
        depth[start] = 0;
        queue.push_back(start);

        while let Some(u) = queue.pop_front() {
            if depth[u] <= max_depth {
                result_nodes.push((nodes[u].id.clone(), depth[u]));
            }

            if depth[u] < max_depth {
                for &v in &adj[u] {
                    if !visited[v] {
                        visited[v] = true;
                        depth[v] = depth[u] + 1;
                        queue.push_back(v);
                    }
                }
            }
        }

        let columns = vec!["node_id".to_string(), "depth".to_string()];
        let rows: Vec<Vec<Option<String>>> = result_nodes
            .into_iter()
            .map(|(id, d)| vec![Some(id.to_string()), Some(d.to_string())])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.dfs procedure
    /// CALL orbit.graph.dfs(start_node_id, {maxDepth: 10})
    async fn execute_dfs(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "orbit.graph.dfs requires 1 argument: (start_node_id)".to_string(),
            ));
        }

        let start_id = self.extract_string_arg(&args[0], "start_node_id")?;

        let config = if args.len() > 1 {
            self.parse_config_arg(&args[1])?
        } else {
            HashMap::new()
        };

        let max_depth = config
            .get("maxDepth")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(usize::MAX);

        // Get all nodes and relationships
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        // Build node index and adjacency
        let node_index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let n = nodes.len();

        let start = node_index.get(start_id.as_str()).copied();
        if start.is_none() {
            return Err(ProtocolError::CypherError(format!(
                "Start node '{}' not found",
                start_id
            )));
        }
        let start = start.unwrap();

        // Build adjacency list
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                adj[from_idx].push(to_idx);
                adj[to_idx].push(from_idx);
            }
        }

        // DFS (iterative)
        let mut visited = vec![false; n];
        let mut result_nodes = Vec::new();
        let mut stack = vec![(start, 0usize)];

        while let Some((u, depth)) = stack.pop() {
            if visited[u] {
                continue;
            }
            visited[u] = true;

            if depth <= max_depth {
                result_nodes.push((nodes[u].id.clone(), depth));
            }

            if depth < max_depth {
                for &v in &adj[u] {
                    if !visited[v] {
                        stack.push((v, depth + 1));
                    }
                }
            }
        }

        let columns = vec!["node_id".to_string(), "depth".to_string()];
        let rows: Vec<Vec<Option<String>>> = result_nodes
            .into_iter()
            .map(|(id, d)| vec![Some(id.to_string()), Some(d.to_string())])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.communityDetection procedure
    /// CALL orbit.graph.communityDetection({minSize: 3, algorithm: 'louvain'})
    async fn execute_community_detection(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let min_size = config
            .get("minSize")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(2);

        // Try GPU-accelerated community detection if available
        #[cfg(feature = "gpu-graph-traversal")]
        if let Some(ref gpu_traversal) = self.gpu_traversal {
            let (graph_data, _node_index) = self.build_graph_data().await?;

            if graph_data.node_count > 0 {
                let traversal = gpu_traversal.read().await;
                match traversal.detect_communities(&graph_data, min_size).await {
                    Ok(communities) => {
                        info!(
                            "GPU community detection completed: {} communities found",
                            communities.len()
                        );

                        // Get all nodes for ID reverse lookup
                        let nodes = self.get_all_nodes().await?;
                        let idx_to_id: HashMap<u64, String> = nodes
                            .iter()
                            .enumerate()
                            .map(|(i, n)| (i as u64, n.id.to_string()))
                            .collect();

                        let columns = vec![
                            "community_id".to_string(),
                            "size".to_string(),
                            "members".to_string(),
                        ];

                        let rows: Vec<Vec<Option<String>>> = communities
                            .into_iter()
                            .enumerate()
                            .map(|(idx, member_indices)| {
                                let members: Vec<String> = member_indices
                                    .iter()
                                    .filter_map(|&idx| idx_to_id.get(&idx).cloned())
                                    .collect();
                                vec![
                                    Some(idx.to_string()),
                                    Some(members.len().to_string()),
                                    Some(serde_json::to_string(&members).unwrap_or_default()),
                                ]
                            })
                            .collect();

                        return Ok(QueryResult {
                            nodes: Vec::new(),
                            relationships: Vec::new(),
                            columns,
                            rows,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            "GPU community detection failed, falling back to CPU: {}",
                            e
                        );
                        // Fall through to CPU implementation
                    }
                }
            }
        }

        // CPU fallback implementation
        self.execute_community_detection_cpu(min_size).await
    }

    /// CPU-based community detection implementation (fallback)
    async fn execute_community_detection_cpu(
        &self,
        min_size: usize,
    ) -> ProtocolResult<QueryResult> {
        // Get all nodes and relationships
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        // Build adjacency
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();
        for rel in &relationships {
            adj.entry(rel.start_node.to_string().clone())
                .or_insert_with(Vec::new)
                .push(rel.end_node.to_string().clone());
            adj.entry(rel.end_node.to_string().clone())
                .or_insert_with(Vec::new)
                .push(rel.start_node.to_string().clone());
        }

        // Connected components (simple community detection)
        let mut visited: HashSet<String> = HashSet::new();
        let mut communities = Vec::new();

        for node in &nodes {
            let node_id_str = node.id.to_string();
            if visited.contains(&node_id_str) {
                continue;
            }

            let mut community = Vec::new();
            let mut queue: VecDeque<String> = VecDeque::new();
            queue.push_back(node_id_str.clone());
            visited.insert(node_id_str.clone());

            while let Some(current) = queue.pop_front() {
                community.push(current.clone());

                if let Some(neighbors) = adj.get(&current) {
                    for neighbor in neighbors {
                        if !visited.contains(neighbor) {
                            visited.insert(neighbor.clone());
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }

            if community.len() >= min_size {
                communities.push(community);
            }
        }

        let columns = vec![
            "community_id".to_string(),
            "size".to_string(),
            "members".to_string(),
        ];

        let rows: Vec<Vec<Option<String>>> = communities
            .into_iter()
            .enumerate()
            .map(|(idx, members)| {
                vec![
                    Some(idx.to_string()),
                    Some(members.len().to_string()),
                    Some(serde_json::to_string(&members).unwrap_or_default()),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.connectedComponents procedure
    async fn execute_connected_components(
        &self,
        _args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        self.execute_community_detection(&[]).await
    }

    /// Execute orbit.graph.betweennessCentrality procedure
    async fn execute_betweenness_centrality(
        &self,
        _args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        // Get all nodes and relationships
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec!["node_id".to_string(), "betweenness".to_string()],
                rows: Vec::new(),
            });
        }

        // Build node index and adjacency
        let node_index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let n = nodes.len();

        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                adj[from_idx].push(to_idx);
                adj[to_idx].push(from_idx);
            }
        }

        // Brandes' algorithm for betweenness centrality
        let mut centrality = vec![0.0f64; n];

        for s in 0..n {
            let mut stack = Vec::new();
            let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut sigma = vec![0.0f64; n];
            sigma[s] = 1.0;
            let mut dist: Vec<i64> = vec![-1; n];
            dist[s] = 0;

            let mut queue = VecDeque::new();
            queue.push_back(s);

            while let Some(v) = queue.pop_front() {
                stack.push(v);
                for &w in &adj[v] {
                    if dist[w] < 0 {
                        dist[w] = dist[v] + 1;
                        queue.push_back(w);
                    }
                    if dist[w] == dist[v] + 1 {
                        sigma[w] += sigma[v];
                        predecessors[w].push(v);
                    }
                }
            }

            let mut delta = vec![0.0f64; n];
            while let Some(w) = stack.pop() {
                for &v in &predecessors[w] {
                    delta[v] += (sigma[v] / sigma[w]) * (1.0 + delta[w]);
                }
                if w != s {
                    centrality[w] += delta[w];
                }
            }
        }

        // Normalize for undirected graph
        for c in &mut centrality {
            *c /= 2.0;
        }

        let columns = vec!["node_id".to_string(), "betweenness".to_string()];
        let mut results: Vec<(f64, String)> = centrality
            .iter()
            .zip(nodes.iter())
            .map(|(&c, n)| (c, n.id.to_string()))
            .collect();
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let rows: Vec<Vec<Option<String>>> = results
            .into_iter()
            .map(|(c, id)| vec![Some(id), Some(format!("{:.6}", c))])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.closenessCentrality procedure
    async fn execute_closeness_centrality(
        &self,
        _args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        // Get all nodes and relationships
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec!["node_id".to_string(), "closeness".to_string()],
                rows: Vec::new(),
            });
        }

        let node_index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let n = nodes.len();

        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                adj[from_idx].push(to_idx);
                adj[to_idx].push(from_idx);
            }
        }

        let mut closeness = vec![0.0f64; n];

        for s in 0..n {
            // BFS to find shortest distances
            let mut dist = vec![usize::MAX; n];
            dist[s] = 0;
            let mut queue = VecDeque::new();
            queue.push_back(s);

            while let Some(u) = queue.pop_front() {
                for &v in &adj[u] {
                    if dist[v] == usize::MAX {
                        dist[v] = dist[u] + 1;
                        queue.push_back(v);
                    }
                }
            }

            // Sum of distances
            let sum: usize = dist.iter().filter(|&&d| d < usize::MAX && d > 0).sum();
            if sum > 0 {
                closeness[s] = (n - 1) as f64 / sum as f64;
            }
        }

        let columns = vec!["node_id".to_string(), "closeness".to_string()];
        let mut results: Vec<(f64, String)> = closeness
            .iter()
            .zip(nodes.iter())
            .map(|(&c, n)| (c, n.id.to_string()))
            .collect();
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let rows: Vec<Vec<Option<String>>> = results
            .into_iter()
            .map(|(c, id)| vec![Some(id), Some(format!("{:.6}", c))])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.degreeCentrality procedure
    async fn execute_degree_centrality(&self, _args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node_id".to_string(),
                    "in_degree".to_string(),
                    "out_degree".to_string(),
                    "total_degree".to_string(),
                ],
                rows: Vec::new(),
            });
        }

        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut out_degree: HashMap<String, usize> = HashMap::new();

        for rel in &relationships {
            *out_degree
                .entry(rel.start_node.to_string().clone())
                .or_insert(0) += 1;
            *in_degree
                .entry(rel.end_node.to_string().clone())
                .or_insert(0) += 1;
        }

        let columns = vec![
            "node_id".to_string(),
            "in_degree".to_string(),
            "out_degree".to_string(),
            "total_degree".to_string(),
        ];

        let mut results: Vec<(usize, &GraphNode)> = nodes
            .iter()
            .map(|n| {
                let node_id_str = n.id.to_string();
                let in_d = in_degree.get(&node_id_str).copied().unwrap_or(0);
                let out_d = out_degree.get(&node_id_str).copied().unwrap_or(0);
                (in_d + out_d, n)
            })
            .collect();
        results.sort_by(|a, b| b.0.cmp(&a.0));

        let rows: Vec<Vec<Option<String>>> = results
            .into_iter()
            .map(|(_, n)| {
                let node_id_str = n.id.to_string();
                let in_d = in_degree.get(&node_id_str).copied().unwrap_or(0);
                let out_d = out_degree.get(&node_id_str).copied().unwrap_or(0);
                vec![
                    Some(n.id.to_string()),
                    Some(in_d.to_string()),
                    Some(out_d.to_string()),
                    Some((in_d + out_d).to_string()),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.triangleCount procedure
    async fn execute_triangle_count(&self, _args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec!["node_id".to_string(), "triangles".to_string()],
                rows: Vec::new(),
            });
        }

        let node_index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let n = nodes.len();

        // Build adjacency set for O(1) lookup
        let mut adj_set: Vec<HashSet<usize>> = vec![HashSet::new(); n];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                adj_set[from_idx].insert(to_idx);
                adj_set[to_idx].insert(from_idx);
            }
        }

        // Count triangles for each node
        let mut triangles = vec![0usize; n];

        for u in 0..n {
            let neighbors: Vec<_> = adj_set[u].iter().copied().collect();
            for i in 0..neighbors.len() {
                for j in (i + 1)..neighbors.len() {
                    let v = neighbors[i];
                    let w = neighbors[j];
                    if adj_set[v].contains(&w) {
                        triangles[u] += 1;
                    }
                }
            }
        }

        let columns = vec!["node_id".to_string(), "triangles".to_string()];
        let mut results: Vec<(usize, String)> = triangles
            .iter()
            .zip(nodes.iter())
            .map(|(&t, n)| (t, n.id.to_string()))
            .collect();
        results.sort_by(|a, b| b.0.cmp(&a.0));

        let rows: Vec<Vec<Option<String>>> = results
            .into_iter()
            .map(|(t, id)| vec![Some(id), Some(t.to_string())])
            .collect();

        let total_triangles: usize = triangles.iter().sum::<usize>() / 3; // Each triangle counted 3 times
        info!("Total triangles in graph: {}", total_triangles);

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    // Helper methods

    /// Convert GraphStorage data to orbit-compute GraphData format for GPU processing
    #[cfg(feature = "gpu-graph-traversal")]
    async fn build_graph_data(&self) -> ProtocolResult<(GraphData, HashMap<String, usize>)> {
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok((
                GraphData {
                    node_ids: Vec::new(),
                    adjacency_list: Vec::new(),
                    edge_weights: None,
                    node_properties: HashMap::new(),
                    node_count: 0,
                    edge_count: 0,
                },
                HashMap::new(),
            ));
        }

        // Create node ID mapping (string ID -> index)
        let node_index: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        // Build node_ids as u64 (use index as node ID for GPU)
        let node_ids: Vec<u64> = (0..nodes.len() as u64).collect();

        // Build adjacency list
        let mut adjacency_list: Vec<Vec<u32>> = vec![Vec::new(); nodes.len()];
        let mut edge_weights: Vec<f32> = Vec::new();

        for rel in &relationships {
            let from_str = rel.start_node.to_string();
            let to_str = rel.end_node.to_string();
            if let (Some(&from_idx), Some(&to_idx)) =
                (node_index.get(&from_str), node_index.get(&to_str))
            {
                adjacency_list[from_idx].push(to_idx as u32);
                // Extract edge weight if available
                let weight = rel
                    .properties
                    .get("weight")
                    .and_then(|v| v.as_f64())
                    .map(|f| f as f32)
                    .unwrap_or(1.0);
                edge_weights.push(weight);
            }
        }

        // Build node properties
        let node_properties: HashMap<u64, NodeProperties> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let props = NodeProperties {
                    importance: n
                        .properties
                        .get("importance")
                        .and_then(|v| v.as_f64())
                        .map(|f| f as f32)
                        .unwrap_or(1.0),
                    node_type: n.labels.first().cloned(),
                    metadata: n
                        .properties
                        .iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect(),
                };
                (i as u64, props)
            })
            .collect();

        let edge_count = relationships.len();

        Ok((
            GraphData {
                node_ids,
                adjacency_list,
                edge_weights: if edge_weights.is_empty() {
                    None
                } else {
                    Some(edge_weights)
                },
                node_properties,
                node_count: nodes.len(),
                edge_count,
            },
            node_index,
        ))
    }

    /// Build weighted graph data for algorithms like Dijkstra
    #[cfg(feature = "gpu-graph-traversal")]
    async fn build_weighted_graph_data(
        &self,
    ) -> ProtocolResult<(GraphData, HashMap<String, usize>)> {
        let (mut graph_data, node_index) = self.build_graph_data().await?;

        // Ensure edge weights are present (default to 1.0 if not)
        if graph_data.edge_weights.is_none() {
            graph_data.edge_weights = Some(vec![1.0f32; graph_data.edge_count]);
        }

        Ok((graph_data, node_index))
    }

    async fn get_all_nodes(&self) -> ProtocolResult<Vec<GraphNode>> {
        let mut all_nodes = Vec::new();

        // Scan all known labels
        for label in &self.known_labels {
            let nodes = self
                .storage
                .find_nodes_by_label(label, None, None)
                .await
                .map_err(|e| ProtocolError::ActorError(e.to_string()))?;
            all_nodes.extend(nodes);
        }

        // Deduplicate by ID
        let mut seen = HashSet::new();
        all_nodes.retain(|n| seen.insert(n.id.to_string()));

        Ok(all_nodes)
    }

    async fn get_all_relationships(&self) -> ProtocolResult<Vec<GraphRelationship>> {
        // We need to iterate through nodes and get their relationships
        let nodes = self.get_all_nodes().await?;
        let mut all_rels = Vec::new();
        let mut seen = HashSet::new();

        for node in &nodes {
            let rels = self
                .storage
                .get_relationships(&node.id, Direction::Both, None)
                .await
                .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

            for rel in rels {
                if seen.insert(rel.id.to_string()) {
                    all_rels.push(rel);
                }
            }
        }

        Ok(all_rels)
    }

    fn extract_string_arg(&self, arg: &JsonValue, arg_name: &str) -> ProtocolResult<String> {
        match arg {
            JsonValue::String(s) => Ok(s.clone()),
            _ => Err(ProtocolError::CypherError(format!(
                "{arg_name} argument must be a string"
            ))),
        }
    }

    fn parse_config_arg(&self, arg: &JsonValue) -> ProtocolResult<HashMap<String, JsonValue>> {
        match arg {
            JsonValue::Object(obj) => Ok(obj.clone().into_iter().collect()),
            JsonValue::Null => Ok(HashMap::new()),
            _ => Err(ProtocolError::CypherError(
                "Config argument must be an object or null".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_shared::graph::InMemoryGraphStorage;
    use std::collections::HashMap;

    async fn create_test_graph() -> Arc<InMemoryGraphStorage> {
        let storage = Arc::new(InMemoryGraphStorage::new());

        // Create nodes using the trait method signature: create_node(labels, properties) -> GraphNode
        let props: HashMap<String, serde_json::Value> = HashMap::new();
        let n1 = storage
            .create_node(vec!["Person".to_string()], props.clone())
            .await
            .unwrap();
        let n2 = storage
            .create_node(vec!["Person".to_string()], props.clone())
            .await
            .unwrap();
        let n3 = storage
            .create_node(vec!["Person".to_string()], props.clone())
            .await
            .unwrap();
        let n4 = storage
            .create_node(vec!["Person".to_string()], props.clone())
            .await
            .unwrap();

        // Create relationships: n1-n2, n1-n3, n2-n3, n3-n4
        // Signature: create_relationship(&start_node, &end_node, rel_type, properties)
        storage
            .create_relationship(&n1.id, &n2.id, "KNOWS".to_string(), props.clone())
            .await
            .unwrap();
        storage
            .create_relationship(&n1.id, &n3.id, "KNOWS".to_string(), props.clone())
            .await
            .unwrap();
        storage
            .create_relationship(&n2.id, &n3.id, "KNOWS".to_string(), props.clone())
            .await
            .unwrap();
        storage
            .create_relationship(&n3.id, &n4.id, "KNOWS".to_string(), props)
            .await
            .unwrap();

        storage
    }

    #[tokio::test]
    async fn test_pagerank() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_pagerank(&[]).await.unwrap();
        assert!(!result.rows.is_empty());
        assert_eq!(result.columns.len(), 2);
    }

    #[tokio::test]
    async fn test_degree_centrality() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_degree_centrality(&[]).await.unwrap();
        assert!(!result.rows.is_empty());
    }

    #[tokio::test]
    async fn test_triangle_count() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_triangle_count(&[]).await.unwrap();
        assert!(!result.rows.is_empty());
        // Nodes n1, n2, n3 form a triangle, so at least some should have triangles > 0
    }

    #[tokio::test]
    async fn test_community_detection() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_community_detection(&[]).await.unwrap();
        // Should find at least one community with all 4 connected nodes
        assert!(!result.rows.is_empty());
    }
}
