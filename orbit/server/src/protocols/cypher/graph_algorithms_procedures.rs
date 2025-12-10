//! Graph Algorithm Procedures for Cypher/Bolt Protocol
//!
//! This module provides Cypher stored procedures for graph algorithms,
//! leveraging GPU acceleration when available through orbit-compute.
//! Core algorithms delegate to the shared graph_algorithms module for consistency.

// &mut Vec parameter allows in-place modification for performance in graph traversal
#![allow(clippy::ptr_arg)]

use crate::protocols::common::graph_algorithms as graph_algo;
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
            // Advanced Graph Analytics (Phase 15)
            "orbit.graph.eigenvectorcentrality" => self.execute_eigenvector_centrality(args).await,
            "orbit.graph.jaccardSimilarity" | "orbit.graph.jaccardsimilarity" => {
                self.execute_jaccard_similarity(args).await
            }
            "orbit.graph.cosinesimilarity" => self.execute_cosine_similarity(args).await,
            "orbit.graph.overlapsimilarity" => self.execute_overlap_similarity(args).await,
            "orbit.graph.commonneighbors" => self.execute_common_neighbors(args).await,
            "orbit.graph.adamicadar" => self.execute_adamic_adar(args).await,
            "orbit.graph.preferentialattachment" => {
                self.execute_preferential_attachment(args).await
            }
            "orbit.graph.louvain" => self.execute_louvain(args).await,
            "orbit.graph.kcore" => self.execute_kcore(args).await,
            // Advanced Path Algorithms
            "orbit.graph.astar" => self.execute_astar(args).await,
            "orbit.graph.dijkstra" => self.execute_dijkstra(args).await,
            "orbit.graph.allshortestpaths" => self.execute_all_shortest_paths(args).await,
            "orbit.graph.kshortestpaths" => self.execute_k_shortest_paths(args).await,
            "orbit.graph.spanningtree" => self.execute_spanning_tree(args).await,
            "orbit.graph.singlesourceshortestpath" => {
                self.execute_single_source_shortest_path(args).await
            }
            // GDS (Graph Data Science) Algorithms
            "orbit.graph.labelpropagation" | "gds.labelpropagation" => {
                self.execute_label_propagation(args).await
            }
            "orbit.graph.randomwalk" | "gds.randomwalk" => self.execute_random_walk(args).await,
            "orbit.graph.hits" | "gds.hits" => self.execute_hits(args).await,
            "orbit.graph.articlerank" | "gds.articlerank" => self.execute_article_rank(args).await,
            "orbit.graph.nodessimilarity" | "gds.nodesimilarity" => {
                self.execute_node_similarity(args).await
            }
            "orbit.graph.graphstats" | "gds.graph.stats" => self.execute_graph_stats(args).await,
            "orbit.graph.wcc" | "gds.wcc" => self.execute_weakly_connected_components(args).await,
            "orbit.graph.scc" | "gds.scc" => self.execute_strongly_connected_components(args).await,
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
    /// Uses shared graph_algorithms module for core algorithm
    async fn execute_shortest_path_cpu(
        &self,
        from_id: &str,
        to_id: &str,
        weighted: bool,
    ) -> ProtocolResult<QueryResult> {
        // Build shared graph from storage
        let graph = self.build_shared_graph().await?;

        // Validate nodes exist
        if !graph.nodes.contains_key(from_id) || !graph.nodes.contains_key(to_id) {
            return Err(ProtocolError::CypherError(
                "Source or target node not found".to_string(),
            ));
        }

        // Use shared algorithm (Dijkstra for weighted, BFS for unweighted)
        let result = if weighted {
            graph_algo::dijkstra(&graph, from_id, to_id)
        } else {
            graph_algo::bfs_shortest_path(&graph, from_id, to_id)
        };

        // Format results
        let columns = vec!["path".to_string(), "length".to_string(), "cost".to_string()];

        if !result.found {
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

        let rows = vec![vec![
            Some(serde_json::to_string(&result.path).unwrap_or_default()),
            Some(result.length.to_string()),
            Some(format!("{:.2}", result.cost)),
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
    /// Uses shared graph_algorithms module for core algorithm
    async fn execute_bfs_cpu(
        &self,
        start_id: &str,
        max_depth: usize,
    ) -> ProtocolResult<QueryResult> {
        // Build shared graph from storage
        let graph = self.build_shared_graph().await?;

        if !graph.nodes.contains_key(start_id) {
            return Err(ProtocolError::CypherError(format!(
                "Start node '{}' not found",
                start_id
            )));
        }

        // Use shared BFS traversal algorithm
        let max_depth_opt = if max_depth == usize::MAX {
            None
        } else {
            Some(max_depth)
        };
        let result = graph_algo::bfs_traversal(&graph, start_id, max_depth_opt);

        // Format results with depth information
        let columns = vec!["node_id".to_string(), "depth".to_string()];
        let rows: Vec<Vec<Option<String>>> = result
            .visited
            .iter()
            .map(|id| {
                let depth = result.depths.get(id).copied().unwrap_or(0);
                vec![Some(id.clone()), Some(depth.to_string())]
            })
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.dfs procedure
    /// Uses shared graph_algorithms module for core algorithm
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

        // Build shared graph from storage
        let graph = self.build_shared_graph().await?;

        if !graph.nodes.contains_key(&start_id) {
            return Err(ProtocolError::CypherError(format!(
                "Start node '{}' not found",
                start_id
            )));
        }

        // Use shared DFS traversal algorithm
        let max_depth_opt = if max_depth == usize::MAX {
            None
        } else {
            Some(max_depth)
        };
        let result = graph_algo::dfs_traversal(&graph, &start_id, max_depth_opt);

        // Format results with depth information
        let columns = vec!["node_id".to_string(), "depth".to_string()];
        let rows: Vec<Vec<Option<String>>> = result
            .visited
            .iter()
            .map(|id| {
                let depth = result.depths.get(id).copied().unwrap_or(0);
                vec![Some(id.clone()), Some(depth.to_string())]
            })
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
                .or_default()
                .push(rel.end_node.to_string().clone());
            adj.entry(rel.end_node.to_string().clone())
                .or_default()
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

    // ============================================================================
    // Advanced Graph Analytics (Phase 15)
    // ============================================================================

    /// Execute orbit.graph.eigenvectorCentrality procedure
    /// Computes eigenvector centrality using power iteration method
    /// CALL orbit.graph.eigenvectorCentrality({iterations: 100, tolerance: 0.0001})
    async fn execute_eigenvector_centrality(
        &self,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let max_iterations = config
            .get("iterations")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(100);

        let tolerance = config
            .get("tolerance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0001);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec!["node_id".to_string(), "eigenvector_centrality".to_string()],
                rows: Vec::new(),
            });
        }

        // Build node index and adjacency matrix
        let node_index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let n = nodes.len();

        // Build adjacency list (undirected)
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

        // Power iteration for eigenvector centrality
        let mut centrality = vec![1.0f64 / n as f64; n];
        let mut new_centrality = vec![0.0f64; n];

        for _ in 0..max_iterations {
            // Compute new centrality values
            for i in 0..n {
                new_centrality[i] = adj[i].iter().map(|&j| centrality[j]).sum();
            }

            // Normalize
            let norm: f64 = new_centrality.iter().map(|&x| x * x).sum::<f64>().sqrt();
            if norm > 0.0 {
                for c in &mut new_centrality {
                    *c /= norm;
                }
            }

            // Check convergence
            let diff: f64 = centrality
                .iter()
                .zip(new_centrality.iter())
                .map(|(a, b)| (a - b).abs())
                .sum();

            std::mem::swap(&mut centrality, &mut new_centrality);

            if diff < tolerance {
                break;
            }
        }

        info!("Eigenvector centrality computed for {} nodes", n);

        let columns = vec!["node_id".to_string(), "eigenvector_centrality".to_string()];
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

    /// Execute orbit.graph.jaccardSimilarity procedure
    /// Computes Jaccard similarity between node pairs based on their neighborhoods
    /// CALL orbit.graph.jaccardSimilarity({node1: "id1", node2: "id2"}) or
    /// CALL orbit.graph.jaccardSimilarity({topK: 10}) for top-K similar pairs
    async fn execute_jaccard_similarity(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "similarity".to_string(),
                ],
                rows: Vec::new(),
            });
        }

        // Build neighbor sets for each node
        let mut neighbor_sets: HashMap<String, HashSet<String>> = HashMap::new();
        for rel in &relationships {
            let from = rel.start_node.to_string();
            let to = rel.end_node.to_string();
            neighbor_sets
                .entry(from.clone())
                .or_default()
                .insert(to.clone());
            neighbor_sets.entry(to).or_default().insert(from);
        }

        // If specific nodes requested
        if let (Some(node1), Some(node2)) = (
            config.get("node1").and_then(|v| v.as_str()),
            config.get("node2").and_then(|v| v.as_str()),
        ) {
            let set1 = neighbor_sets.get(node1).cloned().unwrap_or_default();
            let set2 = neighbor_sets.get(node2).cloned().unwrap_or_default();

            let intersection = set1.intersection(&set2).count();
            let union = set1.union(&set2).count();
            let similarity = if union > 0 {
                intersection as f64 / union as f64
            } else {
                0.0
            };

            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "similarity".to_string(),
                ],
                rows: vec![vec![
                    Some(node1.to_string()),
                    Some(node2.to_string()),
                    Some(format!("{:.6}", similarity)),
                ]],
            });
        }

        // Compute top-K similar pairs
        let top_k = config
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let mut similarities: Vec<(String, String, f64)> = Vec::new();
        let node_ids: Vec<&String> = neighbor_sets.keys().collect();

        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let set1 = &neighbor_sets[node_ids[i]];
                let set2 = &neighbor_sets[node_ids[j]];

                let intersection = set1.intersection(set2).count();
                let union = set1.union(set2).count();

                if union > 0 {
                    let similarity = intersection as f64 / union as f64;
                    if similarity > 0.0 {
                        similarities.push((node_ids[i].clone(), node_ids[j].clone(), similarity));
                    }
                }
            }
        }

        similarities.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(top_k);

        info!(
            "Jaccard similarity computed, returning top {} pairs",
            similarities.len()
        );

        let columns = vec![
            "node1".to_string(),
            "node2".to_string(),
            "similarity".to_string(),
        ];
        let rows: Vec<Vec<Option<String>>> = similarities
            .into_iter()
            .map(|(n1, n2, sim)| vec![Some(n1), Some(n2), Some(format!("{:.6}", sim))])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.cosineSimilarity procedure
    /// Computes cosine similarity between node feature vectors
    /// CALL orbit.graph.cosineSimilarity({property: "embedding", topK: 10})
    async fn execute_cosine_similarity(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let property = config
            .get("property")
            .and_then(|v| v.as_str())
            .unwrap_or("embedding");

        let top_k = config
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let nodes = self.get_all_nodes().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "similarity".to_string(),
                ],
                rows: Vec::new(),
            });
        }

        // Extract vectors from nodes
        let mut node_vectors: Vec<(String, Vec<f64>)> = Vec::new();
        for node in &nodes {
            if let Some(vec_val) = node.properties.get(property) {
                if let Some(arr) = vec_val.as_array() {
                    let vec: Vec<f64> = arr.iter().filter_map(|v| v.as_f64()).collect();
                    if !vec.is_empty() {
                        node_vectors.push((node.id.to_string(), vec));
                    }
                }
            }
        }

        if node_vectors.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "similarity".to_string(),
                ],
                rows: Vec::new(),
            });
        }

        // Compute cosine similarities
        let mut similarities: Vec<(String, String, f64)> = Vec::new();

        for i in 0..node_vectors.len() {
            for j in (i + 1)..node_vectors.len() {
                let (id1, vec1) = &node_vectors[i];
                let (id2, vec2) = &node_vectors[j];

                if vec1.len() == vec2.len() {
                    let dot: f64 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
                    let norm1: f64 = vec1.iter().map(|x| x * x).sum::<f64>().sqrt();
                    let norm2: f64 = vec2.iter().map(|x| x * x).sum::<f64>().sqrt();

                    if norm1 > 0.0 && norm2 > 0.0 {
                        let similarity = dot / (norm1 * norm2);
                        similarities.push((id1.clone(), id2.clone(), similarity));
                    }
                }
            }
        }

        similarities.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(top_k);

        info!(
            "Cosine similarity computed, returning top {} pairs",
            similarities.len()
        );

        let columns = vec![
            "node1".to_string(),
            "node2".to_string(),
            "similarity".to_string(),
        ];
        let rows: Vec<Vec<Option<String>>> = similarities
            .into_iter()
            .map(|(n1, n2, sim)| vec![Some(n1), Some(n2), Some(format!("{:.6}", sim))])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.overlapSimilarity procedure
    /// Computes overlap coefficient between node neighborhoods
    /// CALL orbit.graph.overlapSimilarity({topK: 10})
    async fn execute_overlap_similarity(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let top_k = config
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "similarity".to_string(),
                ],
                rows: Vec::new(),
            });
        }

        // Build neighbor sets
        let mut neighbor_sets: HashMap<String, HashSet<String>> = HashMap::new();
        for rel in &relationships {
            let from = rel.start_node.to_string();
            let to = rel.end_node.to_string();
            neighbor_sets
                .entry(from.clone())
                .or_default()
                .insert(to.clone());
            neighbor_sets.entry(to).or_default().insert(from);
        }

        // Compute overlap coefficients
        let mut similarities: Vec<(String, String, f64)> = Vec::new();
        let node_ids: Vec<&String> = neighbor_sets.keys().collect();

        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let set1 = &neighbor_sets[node_ids[i]];
                let set2 = &neighbor_sets[node_ids[j]];

                let intersection = set1.intersection(set2).count();
                let min_size = set1.len().min(set2.len());

                if min_size > 0 {
                    let similarity = intersection as f64 / min_size as f64;
                    if similarity > 0.0 {
                        similarities.push((node_ids[i].clone(), node_ids[j].clone(), similarity));
                    }
                }
            }
        }

        similarities.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(top_k);

        info!(
            "Overlap similarity computed, returning top {} pairs",
            similarities.len()
        );

        let columns = vec![
            "node1".to_string(),
            "node2".to_string(),
            "similarity".to_string(),
        ];
        let rows: Vec<Vec<Option<String>>> = similarities
            .into_iter()
            .map(|(n1, n2, sim)| vec![Some(n1), Some(n2), Some(format!("{:.6}", sim))])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.commonNeighbors procedure
    /// Link prediction using common neighbors count
    /// CALL orbit.graph.commonNeighbors({node1: "id1", node2: "id2"}) or
    /// CALL orbit.graph.commonNeighbors({topK: 10}) for top-K predictions
    async fn execute_common_neighbors(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "common_neighbors".to_string(),
                ],
                rows: Vec::new(),
            });
        }

        // Build neighbor sets and existing edges
        let mut neighbor_sets: HashMap<String, HashSet<String>> = HashMap::new();
        let mut existing_edges: HashSet<(String, String)> = HashSet::new();

        for rel in &relationships {
            let from = rel.start_node.to_string();
            let to = rel.end_node.to_string();
            neighbor_sets
                .entry(from.clone())
                .or_default()
                .insert(to.clone());
            neighbor_sets
                .entry(to.clone())
                .or_default()
                .insert(from.clone());

            existing_edges.insert((from.clone().min(to.clone()), from.max(to)));
        }

        // If specific nodes requested
        if let (Some(node1), Some(node2)) = (
            config.get("node1").and_then(|v| v.as_str()),
            config.get("node2").and_then(|v| v.as_str()),
        ) {
            let set1 = neighbor_sets.get(node1).cloned().unwrap_or_default();
            let set2 = neighbor_sets.get(node2).cloned().unwrap_or_default();
            let common = set1.intersection(&set2).count();

            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "common_neighbors".to_string(),
                ],
                rows: vec![vec![
                    Some(node1.to_string()),
                    Some(node2.to_string()),
                    Some(common.to_string()),
                ]],
            });
        }

        // Predict links for non-connected pairs
        let top_k = config
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let mut predictions: Vec<(String, String, usize)> = Vec::new();
        let node_ids: Vec<&String> = neighbor_sets.keys().collect();

        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let n1 = node_ids[i];
                let n2 = node_ids[j];
                let edge_key = (n1.clone().min(n2.clone()), n1.clone().max(n2.clone()));

                // Only predict for non-existing edges
                if !existing_edges.contains(&edge_key) {
                    let set1 = &neighbor_sets[n1];
                    let set2 = &neighbor_sets[n2];
                    let common = set1.intersection(set2).count();

                    if common > 0 {
                        predictions.push((n1.clone(), n2.clone(), common));
                    }
                }
            }
        }

        predictions.sort_by(|a, b| b.2.cmp(&a.2));
        predictions.truncate(top_k);

        info!(
            "Common neighbors link prediction: {} pairs",
            predictions.len()
        );

        let columns = vec![
            "node1".to_string(),
            "node2".to_string(),
            "common_neighbors".to_string(),
        ];
        let rows: Vec<Vec<Option<String>>> = predictions
            .into_iter()
            .map(|(n1, n2, cn)| vec![Some(n1), Some(n2), Some(cn.to_string())])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.adamicAdar procedure
    /// Link prediction using Adamic-Adar index
    /// CALL orbit.graph.adamicAdar({topK: 10})
    async fn execute_adamic_adar(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let top_k = config
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "adamic_adar_score".to_string(),
                ],
                rows: Vec::new(),
            });
        }

        // Build neighbor sets and existing edges
        let mut neighbor_sets: HashMap<String, HashSet<String>> = HashMap::new();
        let mut existing_edges: HashSet<(String, String)> = HashSet::new();

        for rel in &relationships {
            let from = rel.start_node.to_string();
            let to = rel.end_node.to_string();
            neighbor_sets
                .entry(from.clone())
                .or_default()
                .insert(to.clone());
            neighbor_sets
                .entry(to.clone())
                .or_default()
                .insert(from.clone());

            existing_edges.insert((from.clone().min(to.clone()), from.max(to)));
        }

        // Compute Adamic-Adar index for non-connected pairs
        let mut predictions: Vec<(String, String, f64)> = Vec::new();
        let node_ids: Vec<&String> = neighbor_sets.keys().collect();

        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let n1 = node_ids[i];
                let n2 = node_ids[j];
                let edge_key = (n1.clone().min(n2.clone()), n1.clone().max(n2.clone()));

                if !existing_edges.contains(&edge_key) {
                    let set1 = &neighbor_sets[n1];
                    let set2 = &neighbor_sets[n2];

                    // Adamic-Adar: sum of 1/log(degree) for common neighbors
                    let score: f64 = set1
                        .intersection(set2)
                        .map(|common| {
                            let degree = neighbor_sets.get(common).map(|s| s.len()).unwrap_or(1);
                            if degree > 1 {
                                1.0 / (degree as f64).ln()
                            } else {
                                0.0
                            }
                        })
                        .sum();

                    if score > 0.0 {
                        predictions.push((n1.clone(), n2.clone(), score));
                    }
                }
            }
        }

        predictions.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        predictions.truncate(top_k);

        info!("Adamic-Adar link prediction: {} pairs", predictions.len());

        let columns = vec![
            "node1".to_string(),
            "node2".to_string(),
            "adamic_adar_score".to_string(),
        ];
        let rows: Vec<Vec<Option<String>>> = predictions
            .into_iter()
            .map(|(n1, n2, score)| vec![Some(n1), Some(n2), Some(format!("{:.6}", score))])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.preferentialAttachment procedure
    /// Link prediction using preferential attachment score
    /// CALL orbit.graph.preferentialAttachment({topK: 10})
    async fn execute_preferential_attachment(
        &self,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let top_k = config
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "pa_score".to_string(),
                ],
                rows: Vec::new(),
            });
        }

        // Build neighbor sets and existing edges
        let mut neighbor_sets: HashMap<String, HashSet<String>> = HashMap::new();
        let mut existing_edges: HashSet<(String, String)> = HashSet::new();

        for rel in &relationships {
            let from = rel.start_node.to_string();
            let to = rel.end_node.to_string();
            neighbor_sets
                .entry(from.clone())
                .or_default()
                .insert(to.clone());
            neighbor_sets
                .entry(to.clone())
                .or_default()
                .insert(from.clone());

            existing_edges.insert((from.clone().min(to.clone()), from.max(to)));
        }

        // Preferential attachment: degree(u) * degree(v)
        let mut predictions: Vec<(String, String, usize)> = Vec::new();
        let node_ids: Vec<&String> = neighbor_sets.keys().collect();

        for i in 0..node_ids.len() {
            for j in (i + 1)..node_ids.len() {
                let n1 = node_ids[i];
                let n2 = node_ids[j];
                let edge_key = (n1.clone().min(n2.clone()), n1.clone().max(n2.clone()));

                if !existing_edges.contains(&edge_key) {
                    let deg1 = neighbor_sets[n1].len();
                    let deg2 = neighbor_sets[n2].len();
                    let score = deg1 * deg2;

                    if score > 0 {
                        predictions.push((n1.clone(), n2.clone(), score));
                    }
                }
            }
        }

        predictions.sort_by(|a, b| b.2.cmp(&a.2));
        predictions.truncate(top_k);

        info!(
            "Preferential attachment link prediction: {} pairs",
            predictions.len()
        );

        let columns = vec![
            "node1".to_string(),
            "node2".to_string(),
            "pa_score".to_string(),
        ];
        let rows: Vec<Vec<Option<String>>> = predictions
            .into_iter()
            .map(|(n1, n2, score)| vec![Some(n1), Some(n2), Some(score.to_string())])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.louvain procedure
    /// Community detection using Louvain algorithm
    /// CALL orbit.graph.louvain({resolution: 1.0, iterations: 10})
    async fn execute_louvain(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let resolution = config
            .get("resolution")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        let max_iterations = config
            .get("iterations")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec!["node_id".to_string(), "community".to_string()],
                rows: Vec::new(),
            });
        }

        // Build node index
        let node_index: HashMap<&str, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let n = nodes.len();

        // Build weighted adjacency list
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        let mut total_weight = 0.0f64;

        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                let weight = rel
                    .properties
                    .get("weight")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);

                adj[from_idx].push((to_idx, weight));
                adj[to_idx].push((from_idx, weight));
                total_weight += weight;
            }
        }

        // Initialize: each node in its own community
        let mut community: Vec<usize> = (0..n).collect();
        let mut node_weights: Vec<f64> = vec![0.0; n];

        for i in 0..n {
            for &(_, w) in &adj[i] {
                node_weights[i] += w;
            }
        }

        // Louvain phase 1: local moving
        for _ in 0..max_iterations {
            let mut improved = false;

            for i in 0..n {
                let current_community = community[i];

                // Calculate weights to neighboring communities
                let mut community_weights: HashMap<usize, f64> = HashMap::new();
                for &(neighbor, weight) in &adj[i] {
                    *community_weights.entry(community[neighbor]).or_insert(0.0) += weight;
                }

                // Calculate modularity gain for moving to each community
                let mut best_community = current_community;
                let mut best_gain = 0.0f64;

                let ki = node_weights[i];

                for (&target_community, &ki_in) in &community_weights {
                    if target_community == current_community {
                        continue;
                    }

                    // Calculate sigma_tot for target community
                    let sigma_tot: f64 = (0..n)
                        .filter(|&j| community[j] == target_community)
                        .map(|j| node_weights[j])
                        .sum();

                    // Modularity gain
                    let gain = ki_in - resolution * sigma_tot * ki / (2.0 * total_weight);

                    if gain > best_gain {
                        best_gain = gain;
                        best_community = target_community;
                    }
                }

                if best_community != current_community {
                    community[i] = best_community;
                    improved = true;
                }
            }

            if !improved {
                break;
            }
        }

        // Renumber communities to be consecutive
        let mut community_map: HashMap<usize, usize> = HashMap::new();
        let mut next_id = 0;
        for c in &mut community {
            if let Some(&new_id) = community_map.get(c) {
                *c = new_id;
            } else {
                community_map.insert(*c, next_id);
                *c = next_id;
                next_id += 1;
            }
        }

        info!("Louvain detected {} communities", next_id);

        let columns = vec!["node_id".to_string(), "community".to_string()];
        let rows: Vec<Vec<Option<String>>> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| vec![Some(node.id.to_string()), Some(community[i].to_string())])
            .collect();

        Ok(QueryResult {
            nodes: Vec::new(),
            relationships: Vec::new(),
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.kcore procedure
    /// K-core decomposition - finds the maximal subgraph where all nodes have degree >= k
    /// CALL orbit.graph.kcore({k: 3}) or CALL orbit.graph.kcore() for coreness values
    async fn execute_kcore(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let target_k = config.get("k").and_then(|v| v.as_u64()).map(|n| n as usize);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: Vec::new(),
                relationships: Vec::new(),
                columns: vec!["node_id".to_string(), "coreness".to_string()],
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

        let mut adj: Vec<HashSet<usize>> = vec![HashSet::new(); n];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(rel.start_node.to_string().as_str()),
                node_index.get(rel.end_node.to_string().as_str()),
            ) {
                adj[from_idx].insert(to_idx);
                adj[to_idx].insert(from_idx);
            }
        }

        // K-core decomposition using Batagelj-Zaversnik algorithm
        let mut degree: Vec<usize> = adj.iter().map(|s| s.len()).collect();
        let mut coreness = vec![0usize; n];
        let mut removed = vec![false; n];

        let max_degree = *degree.iter().max().unwrap_or(&0);

        // Process nodes in order of increasing degree
        for k in 0..=max_degree {
            loop {
                // Find a node with degree <= k that hasn't been removed
                let node_to_remove = (0..n).find(|&i| !removed[i] && degree[i] <= k);

                match node_to_remove {
                    Some(v) => {
                        removed[v] = true;
                        coreness[v] = k;

                        // Update degrees of neighbors
                        for &neighbor in &adj[v] {
                            if !removed[neighbor] && degree[neighbor] > 0 {
                                degree[neighbor] -= 1;
                            }
                        }
                    }
                    None => break,
                }
            }
        }

        let max_coreness = *coreness.iter().max().unwrap_or(&0);
        info!(
            "K-core decomposition complete, max coreness: {}",
            max_coreness
        );

        // Return results based on whether specific k was requested
        let columns = vec!["node_id".to_string(), "coreness".to_string()];
        let rows: Vec<Vec<Option<String>>> = if let Some(k) = target_k {
            // Return only nodes in k-core (coreness >= k)
            nodes
                .iter()
                .enumerate()
                .filter(|(i, _)| coreness[*i] >= k)
                .map(|(i, node)| vec![Some(node.id.to_string()), Some(coreness[i].to_string())])
                .collect()
        } else {
            // Return coreness for all nodes
            nodes
                .iter()
                .enumerate()
                .map(|(i, node)| vec![Some(node.id.to_string()), Some(coreness[i].to_string())])
                .collect()
        };

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

    /// Build a shared graph_algo::Graph from storage data
    /// This allows reusing the shared graph algorithms module
    async fn build_shared_graph(&self) -> ProtocolResult<graph_algo::Graph> {
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        let mut graph = graph_algo::Graph::new();

        // Add all nodes
        for node in &nodes {
            let properties: HashMap<String, serde_json::Value> = node
                .properties
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            graph.add_node(node.id.to_string(), properties);
        }

        // Add all edges
        for rel in &relationships {
            let weight = rel
                .properties
                .get("weight")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            graph.add_edge(
                rel.start_node.to_string(),
                rel.end_node.to_string(),
                weight,
                Some(rel.rel_type.clone()),
            );
        }

        Ok(graph)
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

    // ==================== Advanced Path Algorithms ====================

    /// Execute orbit.graph.astar procedure
    /// A* pathfinding with heuristic function
    /// CALL orbit.graph.astar({startNode: 'n1', endNode: 'n2', weightProperty: 'distance'})
    async fn execute_astar(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "A* requires startNode and endNode parameters".to_string(),
            ));
        } else {
            self.parse_config_arg(&args[0])?
        };

        let start_id = config
            .get("startNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ProtocolError::CypherError("startNode parameter required".to_string())
            })?;
        let end_id = config
            .get("endNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ProtocolError::CypherError("endNode parameter required".to_string()))?;
        let weight_property = config
            .get("weightProperty")
            .and_then(|v| v.as_str())
            .unwrap_or("weight");

        // Build adjacency structure with weights
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        let node_index: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        let start_idx = node_index
            .get(start_id)
            .copied()
            .ok_or_else(|| ProtocolError::CypherError("Start node not found".to_string()))?;
        let end_idx = node_index
            .get(end_id)
            .copied()
            .ok_or_else(|| ProtocolError::CypherError("End node not found".to_string()))?;

        // Build weighted adjacency list
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(&rel.start_node.to_string()),
                node_index.get(&rel.end_node.to_string()),
            ) {
                let weight = rel
                    .properties
                    .get(weight_property)
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                adj[from_idx].push((to_idx, weight));
            }
        }

        // A* algorithm using index as heuristic (simplified)
        let mut dist: Vec<f64> = vec![f64::INFINITY; nodes.len()];
        let mut prev: Vec<Option<usize>> = vec![None; nodes.len()];
        let mut open_set = std::collections::BinaryHeap::new();

        dist[start_idx] = 0.0;
        open_set.push(std::cmp::Reverse((
            ordered_float::OrderedFloat(0.0),
            start_idx,
        )));

        while let Some(std::cmp::Reverse((_, current))) = open_set.pop() {
            if current == end_idx {
                break;
            }

            for &(neighbor, weight) in &adj[current] {
                let new_dist = dist[current] + weight;
                if new_dist < dist[neighbor] {
                    dist[neighbor] = new_dist;
                    prev[neighbor] = Some(current);
                    // Heuristic: simple difference in indices (works for ordered graphs)
                    let heuristic = (neighbor as i64 - end_idx as i64).unsigned_abs() as f64 * 0.1;
                    open_set.push(std::cmp::Reverse((
                        ordered_float::OrderedFloat(new_dist + heuristic),
                        neighbor,
                    )));
                }
            }
        }

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = end_idx;
        while let Some(prev_node) = prev[current] {
            path.push(nodes[current].id.to_string());
            current = prev_node;
        }
        path.push(nodes[start_idx].id.to_string());
        path.reverse();

        let columns = vec![
            "path".to_string(),
            "cost".to_string(),
            "nodeCount".to_string(),
        ];

        let cost = if dist[end_idx].is_infinite() {
            -1.0 // No path found
        } else {
            dist[end_idx]
        };

        let rows = vec![vec![
            Some(format!("[{}]", path.join(", "))),
            Some(format!("{:.4}", cost)),
            Some(path.len().to_string()),
        ]];

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.dijkstra procedure
    /// Single source/target Dijkstra shortest path
    /// CALL orbit.graph.dijkstra({startNode: 'n1', endNode: 'n2', weightProperty: 'weight'})
    async fn execute_dijkstra(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "Dijkstra requires startNode parameter".to_string(),
            ));
        } else {
            self.parse_config_arg(&args[0])?
        };

        let start_id = config
            .get("startNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ProtocolError::CypherError("startNode parameter required".to_string())
            })?;
        let end_id = config.get("endNode").and_then(|v| v.as_str());
        let weight_property = config
            .get("weightProperty")
            .and_then(|v| v.as_str())
            .unwrap_or("weight");

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        let node_index: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        let start_idx = node_index
            .get(start_id)
            .copied()
            .ok_or_else(|| ProtocolError::CypherError("Start node not found".to_string()))?;

        // Build weighted adjacency list
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(&rel.start_node.to_string()),
                node_index.get(&rel.end_node.to_string()),
            ) {
                let weight = rel
                    .properties
                    .get(weight_property)
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                adj[from_idx].push((to_idx, weight));
            }
        }

        // Dijkstra's algorithm
        let mut dist: Vec<f64> = vec![f64::INFINITY; nodes.len()];
        let mut prev: Vec<Option<usize>> = vec![None; nodes.len()];
        let mut heap = std::collections::BinaryHeap::new();

        dist[start_idx] = 0.0;
        heap.push(std::cmp::Reverse((
            ordered_float::OrderedFloat(0.0),
            start_idx,
        )));

        while let Some(std::cmp::Reverse((d, u))) = heap.pop() {
            if d.0 > dist[u] {
                continue;
            }

            for &(v, weight) in &adj[u] {
                let new_dist = dist[u] + weight;
                if new_dist < dist[v] {
                    dist[v] = new_dist;
                    prev[v] = Some(u);
                    heap.push(std::cmp::Reverse((
                        ordered_float::OrderedFloat(new_dist),
                        v,
                    )));
                }
            }
        }

        let columns = vec![
            "node_id".to_string(),
            "distance".to_string(),
            "path".to_string(),
        ];

        // If end_id specified, return single path; otherwise return all reachable nodes
        let rows: Vec<Vec<Option<String>>> = if let Some(end) = end_id {
            if let Some(&end_idx) = node_index.get(end) {
                let mut path = Vec::new();
                let mut current = end_idx;
                while let Some(p) = prev[current] {
                    path.push(nodes[current].id.to_string());
                    current = p;
                }
                path.push(nodes[start_idx].id.to_string());
                path.reverse();

                vec![vec![
                    Some(end.to_string()),
                    Some(format!("{:.4}", dist[end_idx])),
                    Some(format!("[{}]", path.join(" -> "))),
                ]]
            } else {
                vec![]
            }
        } else {
            // Return distances to all reachable nodes
            nodes
                .iter()
                .enumerate()
                .filter(|(i, _)| dist[*i] < f64::INFINITY)
                .map(|(i, n)| {
                    vec![
                        Some(n.id.to_string()),
                        Some(format!("{:.4}", dist[i])),
                        None,
                    ]
                })
                .collect()
        };

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.allShortestPaths procedure
    /// Find all shortest paths between two nodes
    /// CALL orbit.graph.allShortestPaths({startNode: 'n1', endNode: 'n2'})
    async fn execute_all_shortest_paths(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "allShortestPaths requires startNode and endNode parameters".to_string(),
            ));
        } else {
            self.parse_config_arg(&args[0])?
        };

        let start_id = config
            .get("startNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ProtocolError::CypherError("startNode parameter required".to_string())
            })?;
        let end_id = config
            .get("endNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ProtocolError::CypherError("endNode parameter required".to_string()))?;

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        let node_index: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        let start_idx = node_index
            .get(start_id)
            .copied()
            .ok_or_else(|| ProtocolError::CypherError("Start node not found".to_string()))?;
        let end_idx = node_index
            .get(end_id)
            .copied()
            .ok_or_else(|| ProtocolError::CypherError("End node not found".to_string()))?;

        // Build adjacency list
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(&rel.start_node.to_string()),
                node_index.get(&rel.end_node.to_string()),
            ) {
                adj[from_idx].push(to_idx);
            }
        }

        // BFS to find shortest distance
        let mut dist: Vec<i32> = vec![-1; nodes.len()];
        let mut queue = VecDeque::new();
        dist[start_idx] = 0;
        queue.push_back(start_idx);

        while let Some(u) = queue.pop_front() {
            for &v in &adj[u] {
                if dist[v] == -1 {
                    dist[v] = dist[u] + 1;
                    queue.push_back(v);
                }
            }
        }

        if dist[end_idx] == -1 {
            return Ok(QueryResult {
                nodes: vec![],
                relationships: vec![],
                columns: vec!["paths".to_string(), "pathCount".to_string()],
                rows: vec![vec![Some("[]".to_string()), Some("0".to_string())]],
            });
        }

        // Find all paths of shortest length using DFS
        let target_dist = dist[end_idx];
        let mut all_paths: Vec<Vec<String>> = Vec::new();
        let mut current_path = vec![start_idx];

        fn find_paths(
            current: usize,
            end: usize,
            adj: &[Vec<usize>],
            dist: &[i32],
            current_path: &mut Vec<usize>,
            all_paths: &mut Vec<Vec<String>>,
            nodes: &[GraphNode],
        ) {
            if current == end {
                all_paths.push(
                    current_path
                        .iter()
                        .map(|&i| nodes[i].id.to_string())
                        .collect(),
                );
                return;
            }

            for &next in &adj[current] {
                if dist[next] == dist[current] + 1 {
                    current_path.push(next);
                    find_paths(next, end, adj, dist, current_path, all_paths, nodes);
                    current_path.pop();
                }
            }
        }

        find_paths(
            start_idx,
            end_idx,
            &adj,
            &dist,
            &mut current_path,
            &mut all_paths,
            &nodes,
        );

        let columns = vec![
            "paths".to_string(),
            "pathCount".to_string(),
            "pathLength".to_string(),
        ];

        let paths_str = all_paths
            .iter()
            .map(|p| format!("[{}]", p.join(" -> ")))
            .collect::<Vec<_>>()
            .join(", ");

        let rows = vec![vec![
            Some(format!("[{}]", paths_str)),
            Some(all_paths.len().to_string()),
            Some(target_dist.to_string()),
        ]];

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.kShortestPaths procedure
    /// Find k shortest paths using Yen's algorithm
    /// CALL orbit.graph.kShortestPaths({startNode: 'n1', endNode: 'n2', k: 3})
    async fn execute_k_shortest_paths(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "kShortestPaths requires startNode, endNode, and k parameters".to_string(),
            ));
        } else {
            self.parse_config_arg(&args[0])?
        };

        let start_id = config
            .get("startNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ProtocolError::CypherError("startNode parameter required".to_string())
            })?;
        let end_id = config
            .get("endNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ProtocolError::CypherError("endNode parameter required".to_string()))?;
        let k = config.get("k").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
        let weight_property = config
            .get("weightProperty")
            .and_then(|v| v.as_str())
            .unwrap_or("weight");

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        let node_index: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        let start_idx = node_index
            .get(start_id)
            .copied()
            .ok_or_else(|| ProtocolError::CypherError("Start node not found".to_string()))?;
        let end_idx = node_index
            .get(end_id)
            .copied()
            .ok_or_else(|| ProtocolError::CypherError("End node not found".to_string()))?;

        // Build weighted adjacency list
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(&rel.start_node.to_string()),
                node_index.get(&rel.end_node.to_string()),
            ) {
                let weight = rel
                    .properties
                    .get(weight_property)
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                adj[from_idx].push((to_idx, weight));
            }
        }

        // Simplified k-shortest paths using modified Dijkstra
        // (Full Yen's algorithm would be more complex)
        let mut paths: Vec<(Vec<usize>, f64)> = Vec::new();
        let mut heap = std::collections::BinaryHeap::new();

        // (negative cost, path)
        heap.push((ordered_float::OrderedFloat(0.0), vec![start_idx]));

        while let Some((cost, path)) = heap.pop() {
            let current = *path.last().unwrap();

            if current == end_idx {
                paths.push((path.clone(), -cost.0));
                if paths.len() >= k {
                    break;
                }
            }

            if paths.len() < k {
                for &(next, weight) in &adj[current] {
                    if !path.contains(&next) {
                        let mut new_path = path.clone();
                        new_path.push(next);
                        heap.push((ordered_float::OrderedFloat(cost.0 - weight), new_path));
                    }
                }
            }
        }

        let columns = vec![
            "pathIndex".to_string(),
            "path".to_string(),
            "cost".to_string(),
        ];

        let rows: Vec<Vec<Option<String>>> = paths
            .iter()
            .enumerate()
            .map(|(i, (path, cost))| {
                let path_str = path
                    .iter()
                    .map(|&idx| nodes[idx].id.to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ");
                vec![
                    Some((i + 1).to_string()),
                    Some(format!("[{}]", path_str)),
                    Some(format!("{:.4}", cost)),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.spanningTree procedure
    /// Find minimum spanning tree using Prim's or Kruskal's algorithm
    /// CALL orbit.graph.spanningTree({algorithm: 'prim', weightProperty: 'weight'})
    async fn execute_spanning_tree(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let algorithm = config
            .get("algorithm")
            .and_then(|v| v.as_str())
            .unwrap_or("prim");
        let weight_property = config
            .get("weightProperty")
            .and_then(|v| v.as_str())
            .unwrap_or("weight");

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: vec![],
                relationships: vec![],
                columns: vec![
                    "source".to_string(),
                    "target".to_string(),
                    "weight".to_string(),
                ],
                rows: vec![],
            });
        }

        let node_index: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        // Build edge list with weights
        let mut edges: Vec<(usize, usize, f64)> = Vec::new();
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(&rel.start_node.to_string()),
                node_index.get(&rel.end_node.to_string()),
            ) {
                let weight = rel
                    .properties
                    .get(weight_property)
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                edges.push((from_idx, to_idx, weight));
            }
        }

        let mst_edges: Vec<(usize, usize, f64)> = if algorithm == "kruskal" {
            // Kruskal's algorithm
            edges.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

            let mut parent: Vec<usize> = (0..nodes.len()).collect();
            let mut rank: Vec<usize> = vec![0; nodes.len()];

            fn find(parent: &mut [usize], x: usize) -> usize {
                if parent[x] != x {
                    parent[x] = find(parent, parent[x]);
                }
                parent[x]
            }

            fn union(parent: &mut [usize], rank: &mut [usize], x: usize, y: usize) -> bool {
                let px = find(parent, x);
                let py = find(parent, y);
                if px == py {
                    return false;
                }
                if rank[px] < rank[py] {
                    parent[px] = py;
                } else if rank[px] > rank[py] {
                    parent[py] = px;
                } else {
                    parent[py] = px;
                    rank[px] += 1;
                }
                true
            }

            let mut result = Vec::new();
            for (u, v, w) in edges {
                if union(&mut parent, &mut rank, u, v) {
                    result.push((u, v, w));
                }
            }
            result
        } else {
            // Prim's algorithm
            let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
            for (u, v, w) in &edges {
                adj[*u].push((*v, *w));
                adj[*v].push((*u, *w));
            }

            let mut in_mst = vec![false; nodes.len()];
            let mut result = Vec::new();
            let mut heap = std::collections::BinaryHeap::new();

            in_mst[0] = true;
            for &(v, w) in &adj[0] {
                heap.push(std::cmp::Reverse((ordered_float::OrderedFloat(w), 0, v)));
            }

            while let Some(std::cmp::Reverse((w, u, v))) = heap.pop() {
                if in_mst[v] {
                    continue;
                }
                in_mst[v] = true;
                result.push((u, v, w.0));

                for &(next, weight) in &adj[v] {
                    if !in_mst[next] {
                        heap.push(std::cmp::Reverse((
                            ordered_float::OrderedFloat(weight),
                            v,
                            next,
                        )));
                    }
                }
            }
            result
        };

        let columns = vec![
            "source".to_string(),
            "target".to_string(),
            "weight".to_string(),
        ];

        let total_weight: f64 = mst_edges.iter().map(|(_, _, w)| w).sum();

        let mut rows: Vec<Vec<Option<String>>> = mst_edges
            .iter()
            .map(|(u, v, w)| {
                vec![
                    Some(nodes[*u].id.to_string()),
                    Some(nodes[*v].id.to_string()),
                    Some(format!("{:.4}", w)),
                ]
            })
            .collect();

        // Add summary row
        rows.push(vec![
            Some("TOTAL".to_string()),
            Some(format!("{} edges", mst_edges.len())),
            Some(format!("{:.4}", total_weight)),
        ]);

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute orbit.graph.singleSourceShortestPath procedure
    /// Find shortest paths from a single source to all reachable nodes
    /// CALL orbit.graph.singleSourceShortestPath({startNode: 'n1', weightProperty: 'weight'})
    async fn execute_single_source_shortest_path(
        &self,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "singleSourceShortestPath requires startNode parameter".to_string(),
            ));
        } else {
            self.parse_config_arg(&args[0])?
        };

        let start_id = config
            .get("startNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ProtocolError::CypherError("startNode parameter required".to_string())
            })?;
        let weight_property = config
            .get("weightProperty")
            .and_then(|v| v.as_str())
            .unwrap_or("weight");
        let max_distance = config
            .get("maxDistance")
            .and_then(|v| v.as_f64())
            .unwrap_or(f64::INFINITY);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        let node_index: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        let start_idx = node_index
            .get(start_id)
            .copied()
            .ok_or_else(|| ProtocolError::CypherError("Start node not found".to_string()))?;

        // Build weighted adjacency list
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_index.get(&rel.start_node.to_string()),
                node_index.get(&rel.end_node.to_string()),
            ) {
                let weight = rel
                    .properties
                    .get(weight_property)
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1.0);
                adj[from_idx].push((to_idx, weight));
            }
        }

        // Dijkstra's algorithm
        let mut dist: Vec<f64> = vec![f64::INFINITY; nodes.len()];
        let mut heap = std::collections::BinaryHeap::new();

        dist[start_idx] = 0.0;
        heap.push(std::cmp::Reverse((
            ordered_float::OrderedFloat(0.0),
            start_idx,
        )));

        while let Some(std::cmp::Reverse((d, u))) = heap.pop() {
            if d.0 > dist[u] || d.0 > max_distance {
                continue;
            }

            for &(v, weight) in &adj[u] {
                let new_dist = dist[u] + weight;
                if new_dist < dist[v] && new_dist <= max_distance {
                    dist[v] = new_dist;
                    heap.push(std::cmp::Reverse((
                        ordered_float::OrderedFloat(new_dist),
                        v,
                    )));
                }
            }
        }

        let columns = vec![
            "targetNode".to_string(),
            "distance".to_string(),
            "reachable".to_string(),
        ];

        let mut rows: Vec<Vec<Option<String>>> = nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != start_idx)
            .map(|(i, n)| {
                let reachable = dist[i] < f64::INFINITY && dist[i] <= max_distance;
                vec![
                    Some(n.id.to_string()),
                    Some(if reachable {
                        format!("{:.4}", dist[i])
                    } else {
                        "Infinity".to_string()
                    }),
                    Some(reachable.to_string()),
                ]
            })
            .collect();

        // Sort by distance
        rows.sort_by(|a, b| {
            let da = a[1]
                .as_ref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(f64::INFINITY);
            let db = b[1]
                .as_ref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(f64::INFINITY);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    // ==================== GDS (Graph Data Science) Algorithms ====================

    /// Execute Label Propagation Algorithm for community detection
    /// CALL orbit.graph.labelpropagation({maxIterations: 10})
    async fn execute_label_propagation(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let max_iterations = config
            .get("maxIterations")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: vec![],
                relationships: vec![],
                columns: vec!["nodeId".to_string(), "communityId".to_string()],
                rows: vec![],
            });
        }

        // Build node ID to index mapping
        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        // Build adjacency list
        let mut adjacency: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_id_to_idx.get(rel.start_node.to_string().as_str()),
                node_id_to_idx.get(rel.end_node.to_string().as_str()),
            ) {
                adjacency[from_idx].push(to_idx);
                adjacency[to_idx].push(from_idx); // Undirected for LPA
            }
        }

        // Initialize labels - each node starts with its own label
        let mut labels: Vec<usize> = (0..nodes.len()).collect();

        // Label propagation iterations
        for _ in 0..max_iterations {
            let mut changed = false;
            for node_idx in 0..nodes.len() {
                if adjacency[node_idx].is_empty() {
                    continue;
                }

                // Count neighbor labels
                let mut label_counts: HashMap<usize, usize> = HashMap::new();
                for &neighbor in &adjacency[node_idx] {
                    *label_counts.entry(labels[neighbor]).or_insert(0) += 1;
                }

                // Find most frequent label
                if let Some((&best_label, _)) = label_counts.iter().max_by_key(|(_, &count)| count)
                {
                    if labels[node_idx] != best_label {
                        labels[node_idx] = best_label;
                        changed = true;
                    }
                }
            }

            if !changed {
                break; // Converged
            }
        }

        // Build results
        let columns = vec!["nodeId".to_string(), "communityId".to_string()];
        let rows: Vec<Vec<Option<String>>> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| vec![Some(node.id.to_string()), Some(labels[i].to_string())])
            .collect();

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute Random Walk algorithm
    /// CALL orbit.graph.randomwalk({startNode: 'node1', walkLength: 10, walks: 5})
    async fn execute_random_walk(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            return Err(ProtocolError::CypherError(
                "Random walk requires startNode parameter".to_string(),
            ));
        } else {
            self.parse_config_arg(&args[0])?
        };

        let start_node_id = config
            .get("startNode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ProtocolError::CypherError("startNode parameter required".to_string()))?
            .to_string();

        let walk_length = config
            .get("walkLength")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let num_walks = config
            .get("walks")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(5);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        // Build adjacency list
        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        let start_idx = node_id_to_idx.get(&start_node_id).copied().ok_or_else(|| {
            ProtocolError::CypherError(format!("Start node not found: {}", start_node_id))
        })?;

        let mut adjacency: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_id_to_idx.get(rel.start_node.to_string().as_str()),
                node_id_to_idx.get(rel.end_node.to_string().as_str()),
            ) {
                adjacency[from_idx].push(to_idx);
            }
        }

        // Perform random walks
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let mut rng_state = seed;

        let mut walks: Vec<Vec<String>> = Vec::with_capacity(num_walks);
        for _ in 0..num_walks {
            let mut walk = vec![nodes[start_idx].id.to_string()];
            let mut current = start_idx;

            for _ in 0..walk_length {
                if adjacency[current].is_empty() {
                    break;
                }
                // Simple LCG random number generator
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let next_idx = (rng_state as usize) % adjacency[current].len();
                current = adjacency[current][next_idx];
                walk.push(nodes[current].id.to_string());
            }
            walks.push(walk);
        }

        // Build results
        let columns = vec!["walkIndex".to_string(), "path".to_string()];
        let rows: Vec<Vec<Option<String>>> = walks
            .iter()
            .enumerate()
            .map(|(i, walk)| vec![Some(i.to_string()), Some(walk.join(" -> "))])
            .collect();

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute HITS (Hyperlink-Induced Topic Search) algorithm
    /// CALL orbit.graph.hits({iterations: 20, tolerance: 0.0001})
    async fn execute_hits(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let max_iterations = config
            .get("iterations")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(20);

        let tolerance = config
            .get("tolerance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0001);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: vec![],
                relationships: vec![],
                columns: vec![
                    "nodeId".to_string(),
                    "authority".to_string(),
                    "hub".to_string(),
                ],
                rows: vec![],
            });
        }

        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        // Build outgoing and incoming edge lists
        let mut outgoing: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
        let mut incoming: Vec<Vec<usize>> = vec![vec![]; nodes.len()];

        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_id_to_idx.get(rel.start_node.to_string().as_str()),
                node_id_to_idx.get(rel.end_node.to_string().as_str()),
            ) {
                outgoing[from_idx].push(to_idx);
                incoming[to_idx].push(from_idx);
            }
        }

        // Initialize authority and hub scores
        let mut authority: Vec<f64> = vec![1.0; nodes.len()];
        let mut hub: Vec<f64> = vec![1.0; nodes.len()];

        // HITS iterations
        for _ in 0..max_iterations {
            let old_authority = authority.clone();

            // Update authority scores: sum of hub scores of nodes pointing to this node
            for i in 0..nodes.len() {
                authority[i] = incoming[i].iter().map(|&j| hub[j]).sum();
            }

            // Update hub scores: sum of authority scores of nodes this node points to
            for i in 0..nodes.len() {
                hub[i] = outgoing[i].iter().map(|&j| authority[j]).sum();
            }

            // Normalize
            let auth_sum: f64 = authority.iter().map(|&x| x * x).sum::<f64>().sqrt();
            let hub_sum: f64 = hub.iter().map(|&x| x * x).sum::<f64>().sqrt();

            if auth_sum > 0.0 {
                authority.iter_mut().for_each(|x| *x /= auth_sum);
            }
            if hub_sum > 0.0 {
                hub.iter_mut().for_each(|x| *x /= hub_sum);
            }

            // Check convergence
            let diff: f64 = authority
                .iter()
                .zip(old_authority.iter())
                .map(|(&a, &b)| (a - b).abs())
                .sum();
            if diff < tolerance {
                break;
            }
        }

        // Build results
        let columns = vec![
            "nodeId".to_string(),
            "authority".to_string(),
            "hub".to_string(),
        ];
        let mut results: Vec<(f64, f64, String)> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (authority[i], hub[i], node.id.to_string()))
            .collect();
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let rows: Vec<Vec<Option<String>>> = results
            .into_iter()
            .map(|(auth, hub_score, id)| {
                vec![
                    Some(id),
                    Some(format!("{:.6}", auth)),
                    Some(format!("{:.6}", hub_score)),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute Article Rank algorithm (PageRank variant)
    /// CALL orbit.graph.articlerank({damping: 0.85, iterations: 20})
    async fn execute_article_rank(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let damping = config
            .get("damping")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.85);

        let max_iterations = config
            .get("iterations")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(20);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: vec![],
                relationships: vec![],
                columns: vec!["nodeId".to_string(), "articleRank".to_string()],
                rows: vec![],
            });
        }

        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        // Build outgoing edges and count
        let mut outgoing: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_id_to_idx.get(rel.start_node.to_string().as_str()),
                node_id_to_idx.get(rel.end_node.to_string().as_str()),
            ) {
                outgoing[from_idx].push(to_idx);
            }
        }

        let n = nodes.len() as f64;
        let avg_out_degree = relationships.len() as f64 / n;

        // Article Rank: uses average degree instead of actual out-degree for normalization
        let mut ranks: Vec<f64> = vec![1.0 / n; nodes.len()];

        for _ in 0..max_iterations {
            let old_ranks = ranks.clone();
            let mut new_ranks = vec![(1.0 - damping) / n; nodes.len()];

            for i in 0..nodes.len() {
                if !outgoing[i].is_empty() {
                    // ArticleRank: divide by average degree + node's out-degree
                    let contrib =
                        damping * old_ranks[i] / (avg_out_degree + outgoing[i].len() as f64);
                    for &j in &outgoing[i] {
                        new_ranks[j] += contrib;
                    }
                }
            }

            ranks = new_ranks;
        }

        // Build results
        let columns = vec!["nodeId".to_string(), "articleRank".to_string()];
        let mut results: Vec<(f64, String)> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (ranks[i], node.id.to_string()))
            .collect();
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let rows: Vec<Vec<Option<String>>> = results
            .into_iter()
            .map(|(rank, id)| vec![Some(id), Some(format!("{:.6}", rank))])
            .collect();

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute Node Similarity algorithm (k-nearest neighbors based on neighbors)
    /// CALL orbit.graph.nodessimilarity({topK: 10})
    async fn execute_node_similarity(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let config = if args.is_empty() {
            HashMap::new()
        } else {
            self.parse_config_arg(&args[0])?
        };

        let top_k = config
            .get("topK")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(10);

        let similarity_cutoff = config
            .get("similarityCutoff")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: vec![],
                relationships: vec![],
                columns: vec![
                    "node1".to_string(),
                    "node2".to_string(),
                    "similarity".to_string(),
                ],
                rows: vec![],
            });
        }

        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        // Build neighbor sets for each node
        let mut neighbors: Vec<HashSet<usize>> = vec![HashSet::new(); nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_id_to_idx.get(rel.start_node.to_string().as_str()),
                node_id_to_idx.get(rel.end_node.to_string().as_str()),
            ) {
                neighbors[from_idx].insert(to_idx);
                neighbors[to_idx].insert(from_idx);
            }
        }

        // Calculate pairwise Jaccard similarity
        let mut similarities: Vec<(String, String, f64)> = Vec::new();

        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                if neighbors[i].is_empty() && neighbors[j].is_empty() {
                    continue;
                }

                let intersection = neighbors[i].intersection(&neighbors[j]).count();
                let union = neighbors[i].union(&neighbors[j]).count();

                if union > 0 {
                    let similarity = intersection as f64 / union as f64;
                    if similarity >= similarity_cutoff {
                        similarities.push((
                            nodes[i].id.to_string(),
                            nodes[j].id.to_string(),
                            similarity,
                        ));
                    }
                }
            }
        }

        // Sort by similarity descending
        similarities.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        similarities.truncate(top_k);

        // Build results
        let columns = vec![
            "node1".to_string(),
            "node2".to_string(),
            "similarity".to_string(),
        ];
        let rows: Vec<Vec<Option<String>>> = similarities
            .into_iter()
            .map(|(n1, n2, sim)| vec![Some(n1), Some(n2), Some(format!("{:.6}", sim))])
            .collect();

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute Graph Statistics procedure
    /// CALL orbit.graph.graphstats()
    async fn execute_graph_stats(&self, _args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        let node_count = nodes.len();
        let relationship_count = relationships.len();

        // Calculate density: E / (V * (V-1)) for directed graph
        let density = if node_count > 1 {
            relationship_count as f64 / (node_count as f64 * (node_count as f64 - 1.0))
        } else {
            0.0
        };

        // Calculate degree statistics
        let mut in_degrees: Vec<usize> = vec![0; node_count];
        let mut out_degrees: Vec<usize> = vec![0; node_count];

        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        for rel in &relationships {
            if let Some(&from_idx) = node_id_to_idx.get(rel.start_node.to_string().as_str()) {
                out_degrees[from_idx] += 1;
            }
            if let Some(&to_idx) = node_id_to_idx.get(rel.end_node.to_string().as_str()) {
                in_degrees[to_idx] += 1;
            }
        }

        let avg_in_degree = if node_count > 0 {
            in_degrees.iter().sum::<usize>() as f64 / node_count as f64
        } else {
            0.0
        };

        let avg_out_degree = if node_count > 0 {
            out_degrees.iter().sum::<usize>() as f64 / node_count as f64
        } else {
            0.0
        };

        let max_in_degree = in_degrees.iter().max().copied().unwrap_or(0);
        let max_out_degree = out_degrees.iter().max().copied().unwrap_or(0);

        // Count isolated nodes
        let isolated_nodes = in_degrees
            .iter()
            .zip(out_degrees.iter())
            .filter(|(&in_d, &out_d)| in_d == 0 && out_d == 0)
            .count();

        // Build results
        let columns = vec!["statistic".to_string(), "value".to_string()];
        let rows: Vec<Vec<Option<String>>> = vec![
            vec![Some("nodeCount".to_string()), Some(node_count.to_string())],
            vec![
                Some("relationshipCount".to_string()),
                Some(relationship_count.to_string()),
            ],
            vec![Some("density".to_string()), Some(format!("{:.6}", density))],
            vec![
                Some("avgInDegree".to_string()),
                Some(format!("{:.2}", avg_in_degree)),
            ],
            vec![
                Some("avgOutDegree".to_string()),
                Some(format!("{:.2}", avg_out_degree)),
            ],
            vec![
                Some("maxInDegree".to_string()),
                Some(max_in_degree.to_string()),
            ],
            vec![
                Some("maxOutDegree".to_string()),
                Some(max_out_degree.to_string()),
            ],
            vec![
                Some("isolatedNodes".to_string()),
                Some(isolated_nodes.to_string()),
            ],
        ];

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute Weakly Connected Components algorithm
    /// CALL orbit.graph.wcc()
    async fn execute_weakly_connected_components(
        &self,
        _args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: vec![],
                relationships: vec![],
                columns: vec!["nodeId".to_string(), "componentId".to_string()],
                rows: vec![],
            });
        }

        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        // Build undirected adjacency list
        let mut adjacency: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_id_to_idx.get(rel.start_node.to_string().as_str()),
                node_id_to_idx.get(rel.end_node.to_string().as_str()),
            ) {
                adjacency[from_idx].push(to_idx);
                adjacency[to_idx].push(from_idx);
            }
        }

        // Union-Find for WCC
        let mut parent: Vec<usize> = (0..nodes.len()).collect();
        let mut rank: Vec<usize> = vec![0; nodes.len()];

        fn find(parent: &mut Vec<usize>, i: usize) -> usize {
            if parent[i] != i {
                parent[i] = find(parent, parent[i]);
            }
            parent[i]
        }

        fn union(parent: &mut Vec<usize>, rank: &mut Vec<usize>, x: usize, y: usize) {
            let root_x = find(parent, x);
            let root_y = find(parent, y);
            if root_x != root_y {
                if rank[root_x] < rank[root_y] {
                    parent[root_x] = root_y;
                } else if rank[root_x] > rank[root_y] {
                    parent[root_y] = root_x;
                } else {
                    parent[root_y] = root_x;
                    rank[root_x] += 1;
                }
            }
        }

        // Process edges
        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_id_to_idx.get(rel.start_node.to_string().as_str()),
                node_id_to_idx.get(rel.end_node.to_string().as_str()),
            ) {
                union(&mut parent, &mut rank, from_idx, to_idx);
            }
        }

        // Get component IDs
        let mut component_ids: Vec<usize> = Vec::with_capacity(nodes.len());
        for i in 0..nodes.len() {
            component_ids.push(find(&mut parent, i));
        }

        // Renumber components starting from 0
        let unique_components: HashSet<usize> = component_ids.iter().copied().collect();
        let component_map: HashMap<usize, usize> = unique_components
            .iter()
            .enumerate()
            .map(|(new_id, &old_id)| (old_id, new_id))
            .collect();

        let columns = vec!["nodeId".to_string(), "componentId".to_string()];
        let rows: Vec<Vec<Option<String>>> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                let comp_id = component_map.get(&component_ids[i]).copied().unwrap_or(0);
                vec![Some(node.id.to_string()), Some(comp_id.to_string())]
            })
            .collect();

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
    }

    /// Execute Strongly Connected Components algorithm (Kosaraju's)
    /// CALL orbit.graph.scc()
    async fn execute_strongly_connected_components(
        &self,
        _args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        let nodes = self.get_all_nodes().await?;
        let relationships = self.get_all_relationships().await?;

        if nodes.is_empty() {
            return Ok(QueryResult {
                nodes: vec![],
                relationships: vec![],
                columns: vec!["nodeId".to_string(), "componentId".to_string()],
                rows: vec![],
            });
        }

        let node_id_to_idx: HashMap<String, usize> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.to_string(), i))
            .collect();

        // Build directed adjacency lists
        let mut adjacency: Vec<Vec<usize>> = vec![vec![]; nodes.len()];
        let mut reverse_adj: Vec<Vec<usize>> = vec![vec![]; nodes.len()];

        for rel in &relationships {
            if let (Some(&from_idx), Some(&to_idx)) = (
                node_id_to_idx.get(rel.start_node.to_string().as_str()),
                node_id_to_idx.get(rel.end_node.to_string().as_str()),
            ) {
                adjacency[from_idx].push(to_idx);
                reverse_adj[to_idx].push(from_idx);
            }
        }

        // Kosaraju's algorithm
        // Step 1: DFS on original graph to get finish order
        let mut visited = vec![false; nodes.len()];
        let mut finish_order: Vec<usize> = Vec::new();

        fn dfs_finish(
            node: usize,
            adj: &[Vec<usize>],
            visited: &mut [bool],
            finish_order: &mut Vec<usize>,
        ) {
            visited[node] = true;
            for &neighbor in &adj[node] {
                if !visited[neighbor] {
                    dfs_finish(neighbor, adj, visited, finish_order);
                }
            }
            finish_order.push(node);
        }

        for i in 0..nodes.len() {
            if !visited[i] {
                dfs_finish(i, &adjacency, &mut visited, &mut finish_order);
            }
        }

        // Step 2: DFS on reverse graph in reverse finish order
        visited.fill(false);
        let mut component_ids = vec![0usize; nodes.len()];
        let mut current_component = 0;

        fn dfs_assign(
            node: usize,
            rev_adj: &[Vec<usize>],
            visited: &mut [bool],
            component_ids: &mut [usize],
            component_id: usize,
        ) {
            visited[node] = true;
            component_ids[node] = component_id;
            for &neighbor in &rev_adj[node] {
                if !visited[neighbor] {
                    dfs_assign(neighbor, rev_adj, visited, component_ids, component_id);
                }
            }
        }

        for &node in finish_order.iter().rev() {
            if !visited[node] {
                dfs_assign(
                    node,
                    &reverse_adj,
                    &mut visited,
                    &mut component_ids,
                    current_component,
                );
                current_component += 1;
            }
        }

        let columns = vec!["nodeId".to_string(), "componentId".to_string()];
        let rows: Vec<Vec<Option<String>>> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| {
                vec![
                    Some(node.id.to_string()),
                    Some(component_ids[i].to_string()),
                ]
            })
            .collect();

        Ok(QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        })
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

    // ============================================================================
    // Tests for Advanced Graph Analytics (Phase 15)
    // ============================================================================

    #[tokio::test]
    async fn test_eigenvector_centrality() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_eigenvector_centrality(&[])
            .await
            .unwrap();
        assert!(!result.rows.is_empty());
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0], "node_id");
        assert_eq!(result.columns[1], "eigenvector_centrality");

        // Check that values are normalized (between 0 and 1)
        for row in &result.rows {
            if let Some(ref val) = row[1] {
                let centrality: f64 = val.parse().unwrap();
                assert!((0.0..=1.0).contains(&centrality));
            }
        }
    }

    #[tokio::test]
    async fn test_jaccard_similarity() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_jaccard_similarity(&[]).await.unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.columns[0], "node1");
        assert_eq!(result.columns[1], "node2");
        assert_eq!(result.columns[2], "similarity");

        // Check similarity values are between 0 and 1
        for row in &result.rows {
            if let Some(ref val) = row[2] {
                let similarity: f64 = val.parse().unwrap();
                assert!((0.0..=1.0).contains(&similarity));
            }
        }
    }

    #[tokio::test]
    async fn test_overlap_similarity() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_overlap_similarity(&[]).await.unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.columns[0], "node1");
        assert_eq!(result.columns[1], "node2");
        assert_eq!(result.columns[2], "similarity");
    }

    #[tokio::test]
    async fn test_common_neighbors() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_common_neighbors(&[]).await.unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.columns[0], "node1");
        assert_eq!(result.columns[1], "node2");
        assert_eq!(result.columns[2], "common_neighbors");
    }

    #[tokio::test]
    async fn test_adamic_adar() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_adamic_adar(&[]).await.unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.columns[0], "node1");
        assert_eq!(result.columns[1], "node2");
        assert_eq!(result.columns[2], "adamic_adar_score");
    }

    #[tokio::test]
    async fn test_preferential_attachment() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_preferential_attachment(&[])
            .await
            .unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.columns[0], "node1");
        assert_eq!(result.columns[1], "node2");
        assert_eq!(result.columns[2], "pa_score");
    }

    #[tokio::test]
    async fn test_louvain() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_louvain(&[]).await.unwrap();
        assert!(!result.rows.is_empty());
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0], "node_id");
        assert_eq!(result.columns[1], "community");

        // All 4 nodes should have community assignments
        assert_eq!(result.rows.len(), 4);
    }

    #[tokio::test]
    async fn test_kcore() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures.execute_kcore(&[]).await.unwrap();
        assert!(!result.rows.is_empty());
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0], "node_id");
        assert_eq!(result.columns[1], "coreness");

        // All 4 nodes should have coreness values
        assert_eq!(result.rows.len(), 4);
    }

    #[tokio::test]
    async fn test_kcore_with_specific_k() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        // Request k=2 core - nodes in the triangle (n1, n2, n3) have coreness >= 2
        let args = vec![serde_json::json!({"k": 2})];
        let result = procedures.execute_kcore(&args).await.unwrap();

        // Only nodes with coreness >= 2 should be returned
        for row in &result.rows {
            if let Some(ref val) = row[1] {
                let coreness: usize = val.parse().unwrap();
                assert!(coreness >= 2);
            }
        }
    }

    #[tokio::test]
    async fn test_procedure_dispatcher() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        // Test that dispatcher routes to correct algorithms
        let result = procedures
            .execute_procedure("orbit.graph.eigenvectorcentrality", &[])
            .await;
        assert!(result.is_ok());

        let result = procedures
            .execute_procedure("orbit.graph.jaccardsimilarity", &[])
            .await;
        assert!(result.is_ok());

        let result = procedures
            .execute_procedure("orbit.graph.louvain", &[])
            .await;
        assert!(result.is_ok());

        let result = procedures.execute_procedure("orbit.graph.kcore", &[]).await;
        assert!(result.is_ok());

        // Test unknown procedure returns error
        let result = procedures
            .execute_procedure("orbit.graph.unknown", &[])
            .await;
        assert!(result.is_err());
    }

    // ============================================================================
    // Tests for GDS (Graph Data Science) Algorithms
    // ============================================================================

    #[tokio::test]
    async fn test_label_propagation() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_procedure("orbit.graph.labelpropagation", &[])
            .await
            .unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0], "nodeId");
        assert_eq!(result.columns[1], "communityId");
        assert!(!result.rows.is_empty());
    }

    #[tokio::test]
    async fn test_hits_algorithm() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_procedure("orbit.graph.hits", &[])
            .await
            .unwrap();

        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.columns[0], "nodeId");
        assert_eq!(result.columns[1], "authority");
        assert_eq!(result.columns[2], "hub");
        assert!(!result.rows.is_empty());
    }

    #[tokio::test]
    async fn test_article_rank() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_procedure("orbit.graph.articlerank", &[])
            .await
            .unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0], "nodeId");
        assert_eq!(result.columns[1], "articleRank");
        assert!(!result.rows.is_empty());
    }

    #[tokio::test]
    async fn test_node_similarity() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_procedure("orbit.graph.nodessimilarity", &[])
            .await
            .unwrap();

        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.columns[0], "node1");
        assert_eq!(result.columns[1], "node2");
        assert_eq!(result.columns[2], "similarity");
    }

    #[tokio::test]
    async fn test_graph_stats() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_procedure("orbit.graph.graphstats", &[])
            .await
            .unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0], "statistic");
        assert_eq!(result.columns[1], "value");

        // Verify key statistics are present
        let stats: HashMap<String, String> = result
            .rows
            .iter()
            .filter_map(|row| {
                if let (Some(key), Some(value)) = (&row[0], &row[1]) {
                    Some((key.clone(), value.clone()))
                } else {
                    None
                }
            })
            .collect();

        assert!(stats.contains_key("nodeCount"));
        assert!(stats.contains_key("relationshipCount"));
        assert!(stats.contains_key("density"));
        assert!(stats.contains_key("avgInDegree"));
        assert!(stats.contains_key("avgOutDegree"));
    }

    #[tokio::test]
    async fn test_weakly_connected_components() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_procedure("orbit.graph.wcc", &[])
            .await
            .unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0], "nodeId");
        assert_eq!(result.columns[1], "componentId");
        assert!(!result.rows.is_empty());
    }

    #[tokio::test]
    async fn test_strongly_connected_components() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        let result = procedures
            .execute_procedure("orbit.graph.scc", &[])
            .await
            .unwrap();

        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.columns[0], "nodeId");
        assert_eq!(result.columns[1], "componentId");
        assert!(!result.rows.is_empty());
    }

    #[tokio::test]
    async fn test_gds_aliases() {
        let storage = create_test_graph().await;
        let mut procedures = GraphAlgorithmProcedures::new(storage);
        procedures.add_known_label("Person".to_string());

        // Test GDS-style aliases work (lowercase since execute_procedure lowercases)
        let result = procedures
            .execute_procedure("gds.labelpropagation", &[])
            .await;
        assert!(result.is_ok());

        let result = procedures.execute_procedure("gds.hits", &[]).await;
        assert!(result.is_ok());

        let result = procedures.execute_procedure("gds.wcc", &[]).await;
        assert!(result.is_ok());

        let result = procedures.execute_procedure("gds.scc", &[]).await;
        assert!(result.is_ok());

        let result = procedures.execute_procedure("gds.articlerank", &[]).await;
        assert!(result.is_ok());
    }
}
