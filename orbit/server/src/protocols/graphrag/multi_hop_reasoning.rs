//! Multi-Hop Reasoning Engine for GraphRAG
//!
//! This module provides algorithms for traversing graph relationships across
//! multiple hops to find complex connections and insights between entities.
//! Includes GPU acceleration support for Metal (macOS) and Vulkan (cross-platform).

use orbit_client::OrbitClient;
use orbit_shared::graphrag::{ConnectionExplanation, ReasoningPath};
use orbit_shared::{Addressable, Key, OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tracing::{debug, info, warn};

#[cfg(feature = "gpu-acceleration")]
use orbit_compute::graph_traversal::{
    GPUGraphTraversal, GraphData, NodeProperties, TraversalConfig, TraversalResult,
};

/// Multi-hop reasoning engine for graph traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHopReasoningEngine {
    /// Maximum number of hops to traverse
    pub max_hops: u32,

    /// Path scoring strategy
    pub path_scoring: PathScoringStrategy,

    /// Pruning strategy to limit search space
    pub pruning_strategy: PruningStrategy,

    /// Configuration settings
    pub config: ReasoningConfig,

    /// Statistics and performance metrics
    pub stats: ReasoningStats,

    /// Creation timestamp
    pub created_at: i64,

    /// Last activity timestamp
    pub updated_at: i64,
}

/// Configuration for reasoning engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningConfig {
    /// Maximum paths to explore per hop
    pub max_paths_per_hop: usize,

    /// Maximum total paths to return
    pub max_results: usize,

    /// Minimum path score threshold
    pub min_path_score: f32,

    /// Relationship types to include (empty = all)
    pub allowed_relationship_types: Vec<String>,

    /// Relationship types to exclude
    pub excluded_relationship_types: Vec<String>,

    /// Enable bidirectional traversal
    pub bidirectional_search: bool,

    /// Timeout for reasoning queries (ms)
    pub query_timeout_ms: u64,

    /// Enable path caching
    pub enable_caching: bool,

    /// Enable GPU acceleration for large graphs
    #[cfg(feature = "gpu-acceleration")]
    pub enable_gpu_acceleration: bool,

    /// Minimum graph size to use GPU (node count)
    #[cfg(feature = "gpu-acceleration")]
    pub gpu_min_nodes: usize,

    /// Minimum graph size to use GPU (edge count)
    #[cfg(feature = "gpu-acceleration")]
    pub gpu_min_edges: usize,
}

impl Default for ReasoningConfig {
    fn default() -> Self {
        Self {
            max_paths_per_hop: 100,
            max_results: 50,
            min_path_score: 0.1,
            allowed_relationship_types: Vec::new(),
            excluded_relationship_types: Vec::new(),
            bidirectional_search: true,
            query_timeout_ms: 30_000,
            enable_caching: true,
            #[cfg(feature = "gpu-acceleration")]
            enable_gpu_acceleration: true,
            #[cfg(feature = "gpu-acceleration")]
            gpu_min_nodes: 1000,
            #[cfg(feature = "gpu-acceleration")]
            gpu_min_edges: 5000,
        }
    }
}

/// Path scoring strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PathScoringStrategy {
    /// Score based on relationship confidence
    ConfidenceBased,

    /// Score based on path length (shorter = better)
    LengthBased,

    /// Score based on entity importance
    ImportanceBased,

    /// Combined scoring approach
    Combined {
        confidence_weight: f32,
        length_weight: f32,
        importance_weight: f32,
    },

    /// Custom scoring function
    Custom { function_name: String },
}

impl Default for PathScoringStrategy {
    fn default() -> Self {
        PathScoringStrategy::Combined {
            confidence_weight: 0.4,
            length_weight: 0.3,
            importance_weight: 0.3,
        }
    }
}

/// Pruning strategies to limit search space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PruningStrategy {
    /// No pruning
    None,

    /// Prune paths with low scores
    ScoreBased { threshold: f32 },

    /// Prune by maximum branching factor
    BranchingFactor { max_branches: usize },

    /// Prune visited nodes (avoid cycles)
    AvoidCycles,

    /// Combined pruning strategies
    Combined {
        score_threshold: f32,
        max_branches: usize,
        avoid_cycles: bool,
    },
}

impl Default for PruningStrategy {
    fn default() -> Self {
        PruningStrategy::Combined {
            score_threshold: 0.2,
            max_branches: 50,
            avoid_cycles: true,
        }
    }
}

/// Statistics for reasoning operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReasoningStats {
    /// Total reasoning queries executed
    pub queries_executed: u64,

    /// Total paths explored
    pub paths_explored: u64,

    /// Total paths found
    pub paths_found: u64,

    /// Average query time (ms)
    pub avg_query_time_ms: f64,

    /// Cache hit ratio
    pub cache_hit_ratio: f32,

    /// Last statistics update
    pub last_updated: i64,
}

/// Reasoning query request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningQuery {
    /// Source entity/node ID
    pub from_entity: String,

    /// Target entity/node ID
    pub to_entity: String,

    /// Maximum hops to traverse
    pub max_hops: Option<u32>,

    /// Relationship types to consider
    pub relationship_types: Option<Vec<String>>,

    /// Include explanation in results
    pub include_explanation: bool,

    /// Maximum results to return
    pub max_results: Option<usize>,
}

/// Internal path representation during traversal
#[derive(Debug, Clone)]
struct PathState {
    /// Current node ID
    current_node: String,

    /// Path taken so far
    path: Vec<String>,

    /// Relationships traversed
    relationships: Vec<String>,

    /// Current path score
    score: f32,

    /// Visited nodes (for cycle detection)
    visited: HashSet<String>,

    /// Current hop count
    hop_count: u32,
}

/// Search parameters extracted from query
struct SearchParameters {
    max_hops: u32,
    max_results: usize,
}

/// Manages the search state for multi-hop reasoning
struct PathSearchManager {
    queue: VecDeque<PathState>,
    found_paths: Vec<ReasoningPath>,
    explored_count: usize,
    max_results: usize,
    max_hops: u32,
}

impl PathSearchManager {
    fn new(initial_state: PathState, max_results: usize, max_hops: u32) -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(initial_state);

        Self {
            queue,
            found_paths: Vec::new(),
            explored_count: 0,
            max_results,
            max_hops,
        }
    }

    fn next_state(&mut self) -> Option<PathState> {
        if let Some(state) = self.queue.pop_front() {
            self.explored_count += 1;
            Some(state)
        } else {
            None
        }
    }

    fn add_found_path(&mut self, path: ReasoningPath) -> bool {
        self.found_paths.push(path);
        self.found_paths.len() >= self.max_results
    }

    fn should_expand(&self, state: &PathState) -> bool {
        state.hop_count < self.max_hops
    }

    fn add_candidate_state(&mut self, state: PathState) {
        self.queue.push_back(state);
    }

    fn get_results(mut self) -> (Vec<ReasoningPath>, usize) {
        // Sort paths by score (descending)
        self.found_paths.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        (self.found_paths, self.explored_count)
    }
}

impl MultiHopReasoningEngine {
    /// Create a new reasoning engine
    pub fn new(max_hops: u32) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            max_hops,
            path_scoring: PathScoringStrategy::default(),
            pruning_strategy: PruningStrategy::default(),
            config: ReasoningConfig::default(),
            stats: ReasoningStats::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create reasoning engine with custom configuration
    pub fn with_config(
        max_hops: u32,
        scoring: PathScoringStrategy,
        pruning: PruningStrategy,
        config: ReasoningConfig,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            max_hops,
            path_scoring: scoring,
            pruning_strategy: pruning,
            config,
            stats: ReasoningStats::default(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Find paths between two entities (with automatic GPU/CPU routing)
    pub async fn find_paths(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        query: ReasoningQuery,
    ) -> OrbitResult<Vec<ReasoningPath>> {
        #[cfg(feature = "gpu-acceleration")]
        {
            if self.should_use_gpu(&query) {
                return self.find_paths_gpu(orbit_client, kg_name, query).await;
            }
        }

        // Fall back to CPU BFS
        self.find_paths_cpu(orbit_client, kg_name, query).await
    }

    /// CPU-based path finding (original implementation)
    async fn find_paths_cpu(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        query: ReasoningQuery,
    ) -> OrbitResult<Vec<ReasoningPath>> {
        let start_time = std::time::Instant::now();

        self.log_search_start(&query);
        let search_params = self.extract_search_parameters(&query);
        let initial_state = self.create_initial_state(&query);
        let mut search_manager = PathSearchManager::new(
            initial_state,
            search_params.max_results,
            search_params.max_hops,
        );

        // Execute breadth-first search
        self.execute_search(&mut search_manager, orbit_client, kg_name, &query)
            .await?;

        let (found_paths, explored_count) = search_manager.get_results();
        let query_time = start_time.elapsed();

        self.finalize_search_results(&query, &found_paths, explored_count, query_time);

        Ok(found_paths)
    }

    /// GPU-accelerated path finding
    #[cfg(feature = "gpu-acceleration")]
    async fn find_paths_gpu(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        query: ReasoningQuery,
    ) -> OrbitResult<Vec<ReasoningPath>> {
        let start_time = std::time::Instant::now();

        info!(
            from_entity = %query.from_entity,
            to_entity = %query.to_entity,
            "Using GPU-accelerated path finding"
        );

        // Convert graph to GPU format
        let (graph_data, entity_to_id) = self
            .convert_graph_to_gpu_format_with_mapping(
                orbit_client.clone(),
                kg_name,
                &query.from_entity,
                &query.to_entity,
                query.max_hops.unwrap_or(self.max_hops),
            )
            .await?;

        // Create GPU traversal engine
        let config = TraversalConfig {
            max_depth: query.max_hops.unwrap_or(self.max_hops),
            max_paths: query.max_results.unwrap_or(self.config.max_results),
            use_gpu: true,
            min_score: self.config.min_path_score,
            allowed_types: query.relationship_types.clone().unwrap_or_default(),
            bidirectional: self.config.bidirectional_search,
        };

        let traversal = GPUGraphTraversal::new(config)
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to create GPU traversal: {e}")))?;

        // Convert entity IDs to numeric IDs
        let from_id = entity_to_id
            .get(&query.from_entity)
            .copied()
            .ok_or_else(|| {
                OrbitError::internal(format!("Entity not found: {}", query.from_entity))
            })?;
        let to_id = entity_to_id.get(&query.to_entity).copied().ok_or_else(|| {
            OrbitError::internal(format!("Entity not found: {}", query.to_entity))
        })?;

        // Execute GPU traversal
        let result = traversal
            .bfs(&graph_data, from_id, Some(to_id))
            .await
            .map_err(|e| OrbitError::internal(format!("GPU traversal failed: {e}")))?;

        // Convert results back to ReasoningPath
        let reasoning_paths = self.convert_gpu_results_to_reasoning_paths(
            result,
            &graph_data,
            &entity_to_id,
            &query,
        )?;

        let query_time = start_time.elapsed();
        self.finalize_search_results(&query, &reasoning_paths, 0, query_time);

        Ok(reasoning_paths)
    }

    /// Check if GPU should be used for this query
    #[cfg(feature = "gpu-acceleration")]
    pub fn should_use_gpu(&self, _query: &ReasoningQuery) -> bool {
        if !self.config.enable_gpu_acceleration {
            return false;
        }

        // For now, always try GPU if enabled
        // In the future, we could estimate graph size before deciding
        true
    }

    /// Convert graph to GPU-friendly format with entity ID mapping
    #[cfg(feature = "gpu-acceleration")]
    async fn convert_graph_to_gpu_format_with_mapping(
        &self,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        from_entity: &str,
        to_entity: &str,
        max_hops: u32,
    ) -> OrbitResult<(GraphData, HashMap<String, u64>)> {
        info!(
            from_entity = %from_entity,
            to_entity = %to_entity,
            max_hops = max_hops,
            "Converting graph to GPU format"
        );

        // Use BFS to discover nodes
        let mut node_set = HashSet::new();
        let mut adjacency_map: HashMap<String, Vec<(String, f32)>> = HashMap::new();
        let mut entity_to_id: HashMap<String, u64> = HashMap::new();
        let mut next_id: u64 = 0;

        // Helper to get or assign numeric ID for an entity
        let mut get_or_assign_id = |entity: &str| -> u64 {
            if let Some(&id) = entity_to_id.get(entity) {
                id
            } else {
                let id = next_id;
                next_id += 1;
                entity_to_id.insert(entity.to_string(), id);
                id
            }
        };

        // BFS to discover graph
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();

        queue.push_back((from_entity.to_string(), 0u32));
        visited.insert(from_entity.to_string());
        node_set.insert(from_entity.to_string());
        get_or_assign_id(from_entity);

        if !node_set.contains(to_entity) {
            node_set.insert(to_entity.to_string());
            get_or_assign_id(to_entity);
        }

        while let Some((current_node, hop_count)) = queue.pop_front() {
            if hop_count >= max_hops {
                continue;
            }

            match self
                .get_neighbors(orbit_client.clone(), kg_name, &current_node, &None)
                .await
            {
                Ok(neighbors) => {
                    let neighbor_list = adjacency_map
                        .entry(current_node.clone())
                        .or_insert_with(Vec::new);

                    for (neighbor_id, _rel_id, _rel_type, confidence) in neighbors {
                        neighbor_list.push((neighbor_id.clone(), confidence));

                        if !node_set.contains(&neighbor_id) {
                            node_set.insert(neighbor_id.clone());
                            get_or_assign_id(&neighbor_id);
                        }

                        if hop_count + 1 < max_hops && !visited.contains(&neighbor_id) {
                            visited.insert(neighbor_id.clone());
                            queue.push_back((neighbor_id, hop_count + 1));
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        node = %current_node,
                        error = %e,
                        "Failed to get neighbors, continuing"
                    );
                }
            }
        }

        // Build adjacency list with numeric IDs
        let node_count = node_set.len();
        let node_ids: Vec<u64> = (0..next_id).collect();
        let mut adjacency_list: Vec<Vec<u32>> = vec![Vec::new(); node_count];
        let mut edge_weights: Vec<f32> = Vec::new();
        let mut edge_count = 0;

        for (entity, neighbors) in adjacency_map.iter() {
            if let Some(&node_idx) = entity_to_id.get(entity) {
                let idx = node_idx as usize;
                if idx < adjacency_list.len() {
                    for (neighbor_entity, confidence) in neighbors {
                        if let Some(&neighbor_idx) = entity_to_id.get(neighbor_entity) {
                            adjacency_list[idx].push(neighbor_idx as u32);
                            edge_weights.push(*confidence);
                            edge_count += 1;
                        }
                    }
                }
            }
        }

        let mut node_properties: HashMap<u64, NodeProperties> = HashMap::new();
        for (_entity, &id) in entity_to_id.iter() {
            node_properties.insert(
                id,
                NodeProperties {
                    importance: 1.0,
                    node_type: None,
                    metadata: HashMap::new(),
                },
            );
        }

        info!(
            nodes = node_count,
            edges = edge_count,
            "Graph conversion complete"
        );

        Ok((
            GraphData {
                node_ids,
                adjacency_list,
                edge_weights: Some(edge_weights),
                node_properties,
                node_count,
                edge_count,
            },
            entity_to_id,
        ))
    }

    /// Convert graph incrementally for very large graphs
    /// This method loads the graph in chunks to avoid memory issues
    #[cfg(feature = "gpu-acceleration")]
    async fn _convert_graph_to_gpu_format_incremental(
        &self,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        from_entity: &str,
        to_entity: &str,
        max_hops: u32,
        chunk_size: usize,
    ) -> OrbitResult<(GraphData, HashMap<String, u64>)> {
        info!(
            from_entity = %from_entity,
            to_entity = %to_entity,
            max_hops = max_hops,
            chunk_size = chunk_size,
            "Converting large graph incrementally"
        );

        // Use BFS to discover nodes in chunks
        let mut node_set = HashSet::new();
        let mut adjacency_map: HashMap<String, Vec<(String, f32)>> = HashMap::new();
        let mut entity_to_id: HashMap<String, u64> = HashMap::new();
        let mut next_id: u64 = 0;

        // Helper to get or assign numeric ID for an entity
        let mut get_or_assign_id = |entity: &str| -> u64 {
            if let Some(&id) = entity_to_id.get(entity) {
                id
            } else {
                let id = next_id;
                next_id += 1;
                entity_to_id.insert(entity.to_string(), id);
                id
            }
        };

        // BFS with chunked processing
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut processed_in_chunk = 0;

        queue.push_back((from_entity.to_string(), 0u32));
        visited.insert(from_entity.to_string());
        node_set.insert(from_entity.to_string());
        get_or_assign_id(from_entity);

        if !node_set.contains(to_entity) {
            node_set.insert(to_entity.to_string());
            get_or_assign_id(to_entity);
        }

        // Process in chunks to avoid memory issues
        while let Some((current_node, hop_count)) = queue.pop_front() {
            if hop_count >= max_hops {
                continue;
            }

            // Process chunk, then yield if needed
            if processed_in_chunk >= chunk_size {
                // Yield control to allow other tasks
                tokio::task::yield_now().await;
                processed_in_chunk = 0;
            }

            match self
                .get_neighbors(orbit_client.clone(), kg_name, &current_node, &None)
                .await
            {
                Ok(neighbors) => {
                    let neighbor_list = adjacency_map
                        .entry(current_node.clone())
                        .or_insert_with(Vec::new);

                    for (neighbor_id, _rel_id, _rel_type, confidence) in neighbors {
                        neighbor_list.push((neighbor_id.clone(), confidence));

                        if !node_set.contains(&neighbor_id) {
                            node_set.insert(neighbor_id.clone());
                            get_or_assign_id(&neighbor_id);
                        }

                        if hop_count + 1 < max_hops && !visited.contains(&neighbor_id) {
                            visited.insert(neighbor_id.clone());
                            queue.push_back((neighbor_id, hop_count + 1));
                        }
                    }
                    processed_in_chunk += 1;
                }
                Err(e) => {
                    warn!(
                        node = %current_node,
                        error = %e,
                        "Failed to get neighbors, continuing"
                    );
                }
            }
        }

        // Build adjacency list with numeric IDs (same as before)
        let node_count = node_set.len();
        let node_ids: Vec<u64> = (0..next_id).collect();
        let mut adjacency_list: Vec<Vec<u32>> = vec![Vec::new(); node_count];
        let mut edge_weights: Vec<f32> = Vec::new();
        let mut edge_count = 0;

        for (entity, neighbors) in adjacency_map.iter() {
            if let Some(&node_idx) = entity_to_id.get(entity) {
                let idx = node_idx as usize;
                if idx < adjacency_list.len() {
                    for (neighbor_entity, confidence) in neighbors {
                        if let Some(&neighbor_idx) = entity_to_id.get(neighbor_entity) {
                            adjacency_list[idx].push(neighbor_idx as u32);
                            edge_weights.push(*confidence);
                            edge_count += 1;
                        }
                    }
                }
            }
        }

        let mut node_properties: HashMap<u64, NodeProperties> = HashMap::new();
        for (_entity, &id) in entity_to_id.iter() {
            node_properties.insert(
                id,
                NodeProperties {
                    importance: 1.0,
                    node_type: None,
                    metadata: HashMap::new(),
                },
            );
        }

        info!(
            nodes = node_count,
            edges = edge_count,
            "Incremental graph conversion complete"
        );

        Ok((
            GraphData {
                node_ids,
                adjacency_list,
                edge_weights: Some(edge_weights),
                node_properties,
                node_count,
                edge_count,
            },
            entity_to_id,
        ))
    }

    /// Convert GPU traversal results to ReasoningPath format
    #[cfg(feature = "gpu-acceleration")]
    fn convert_gpu_results_to_reasoning_paths(
        &self,
        result: TraversalResult,
        _graph_data: &GraphData,
        entity_to_id: &HashMap<String, u64>,
        query: &ReasoningQuery,
    ) -> OrbitResult<Vec<ReasoningPath>> {
        let mut reasoning_paths = Vec::new();

        // Create reverse mapping (u64 -> String)
        let mut id_to_entity: HashMap<u64, String> = HashMap::new();
        for (entity, &id) in entity_to_id.iter() {
            id_to_entity.insert(id, entity.clone());
        }

        for path in result.paths {
            // Convert numeric node IDs back to entity strings
            let nodes: Vec<String> = path
                .nodes
                .iter()
                .filter_map(|&id| id_to_entity.get(&id).cloned())
                .collect();

            if nodes.len() < 2 {
                continue; // Skip invalid paths
            }

            // Reconstruct relationships (simplified - would need to query actual relationships)
            let relationships: Vec<String> = (0..nodes.len().saturating_sub(1))
                .map(|i| format!("rel_{}_{}", nodes[i], nodes[i + 1]))
                .collect();

            // Create ReasoningPath
            reasoning_paths.push(ReasoningPath {
                nodes,
                relationships,
                score: path.score,
                length: path.length,
                explanation: if query.include_explanation {
                    format!(
                        "GPU-accelerated path with {} hops (score: {:.3})",
                        path.length, path.score
                    )
                } else {
                    String::new()
                },
            });
        }

        Ok(reasoning_paths)
    }

    /// Extract search parameters from query
    fn extract_search_parameters(&self, query: &ReasoningQuery) -> SearchParameters {
        SearchParameters {
            max_hops: query.max_hops.unwrap_or(self.max_hops),
            max_results: query.max_results.unwrap_or(self.config.max_results),
        }
    }

    /// Create initial search state
    fn create_initial_state(&self, query: &ReasoningQuery) -> PathState {
        let mut visited = HashSet::new();
        visited.insert(query.from_entity.clone());

        PathState {
            current_node: query.from_entity.clone(),
            path: vec![query.from_entity.clone()],
            relationships: Vec::new(),
            score: 1.0,
            visited,
            hop_count: 0,
        }
    }

    /// Execute the breadth-first search
    async fn execute_search(
        &self,
        search_manager: &mut PathSearchManager,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        query: &ReasoningQuery,
    ) -> OrbitResult<()> {
        while let Some(current_state) = search_manager.next_state() {
            // Check if we've reached the target
            if current_state.current_node == query.to_entity {
                let reasoning_path = self.create_reasoning_path(&current_state);
                let should_stop = search_manager.add_found_path(reasoning_path);
                if should_stop {
                    break;
                }
                continue;
            }

            // Expand current state if within hop limit
            if search_manager.should_expand(&current_state) {
                self.expand_current_state(
                    search_manager,
                    &current_state,
                    orbit_client.clone(),
                    kg_name,
                    &query.relationship_types,
                )
                .await;
            }
        }

        Ok(())
    }

    /// Expand current state by exploring neighbors
    async fn expand_current_state(
        &self,
        search_manager: &mut PathSearchManager,
        current_state: &PathState,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        relationship_types: &Option<Vec<String>>,
    ) {
        match self
            .get_neighbors(
                orbit_client,
                kg_name,
                &current_state.current_node,
                relationship_types,
            )
            .await
        {
            Ok(neighbors) => {
                for (neighbor_id, relationship_id, relationship_type, confidence) in neighbors {
                    if self.should_prune(current_state, &neighbor_id, confidence) {
                        continue;
                    }

                    let new_state = self.create_neighbor_state(
                        current_state,
                        neighbor_id,
                        relationship_id,
                        &relationship_type,
                        confidence,
                    );

                    search_manager.add_candidate_state(new_state);
                }
            }
            Err(e) => {
                warn!(
                    node_id = %current_state.current_node,
                    error = %e,
                    "Failed to get neighbors for node"
                );
            }
        }
    }

    /// Create new path state for neighbor
    fn create_neighbor_state(
        &self,
        current_state: &PathState,
        neighbor_id: String,
        relationship_id: String,
        relationship_type: &str,
        confidence: f32,
    ) -> PathState {
        let mut new_visited = current_state.visited.clone();
        new_visited.insert(neighbor_id.clone());

        let mut new_path = current_state.path.clone();
        new_path.push(neighbor_id.clone());

        let mut new_relationships = current_state.relationships.clone();
        new_relationships.push(relationship_id);

        let new_score = self.calculate_path_score(current_state, confidence, relationship_type);

        PathState {
            current_node: neighbor_id,
            path: new_path,
            relationships: new_relationships,
            score: new_score,
            visited: new_visited,
            hop_count: current_state.hop_count + 1,
        }
    }

    /// Create reasoning path from path state
    fn create_reasoning_path(&self, state: &PathState) -> ReasoningPath {
        ReasoningPath {
            nodes: state.path.clone(),
            relationships: state.relationships.clone(),
            score: state.score,
            length: state.hop_count as usize,
            explanation: self.generate_path_explanation(state),
        }
    }

    /// Log search start information
    fn log_search_start(&self, query: &ReasoningQuery) {
        info!(
            from_entity = %query.from_entity,
            to_entity = %query.to_entity,
            max_hops = query.max_hops.unwrap_or(self.max_hops),
            "Starting multi-hop reasoning query"
        );
    }

    /// Finalize search results and update statistics
    fn finalize_search_results(
        &mut self,
        query: &ReasoningQuery,
        found_paths: &[ReasoningPath],
        explored_count: usize,
        query_time: std::time::Duration,
    ) {
        self.update_stats(
            explored_count,
            found_paths.len(),
            query_time.as_millis() as u64,
        );

        info!(
            from_entity = %query.from_entity,
            to_entity = %query.to_entity,
            paths_found = found_paths.len(),
            paths_explored = explored_count,
            query_time_ms = query_time.as_millis(),
            "Multi-hop reasoning completed"
        );
    }

    /// Explain connection between two entities
    pub async fn explain_connection(
        &mut self,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        entity_a: &str,
        entity_b: &str,
    ) -> OrbitResult<ConnectionExplanation> {
        let query = ReasoningQuery {
            from_entity: entity_a.to_string(),
            to_entity: entity_b.to_string(),
            max_hops: Some(self.max_hops),
            relationship_types: None,
            include_explanation: true,
            max_results: Some(self.config.max_results),
        };

        let paths = self.find_paths(orbit_client, kg_name, query).await?;

        let best_path = paths.first().cloned();
        let connection_strength = best_path.as_ref().map(|p| p.score).unwrap_or(0.0);

        let explanation = if paths.is_empty() {
            format!(
                "No connection found between '{}' and '{}' within {} hops.",
                entity_a, entity_b, self.max_hops
            )
        } else {
            format!(
                "Found {} connection paths between '{}' and '{}'. Best path has {} hops with confidence score {:.3}.",
                paths.len(),
                entity_a,
                entity_b,
                best_path.as_ref().map(|p| p.length).unwrap_or(0),
                connection_strength
            )
        };

        Ok(ConnectionExplanation {
            from_entity: entity_a.to_string(),
            to_entity: entity_b.to_string(),
            paths,
            best_path,
            connection_strength,
            explanation,
        })
    }

    /// Get neighboring nodes from graph database
    async fn get_neighbors(
        &self,
        orbit_client: Arc<OrbitClient>,
        kg_name: &str,
        node_id: &str,
        relationship_types: &Option<Vec<String>>,
    ) -> OrbitResult<Vec<(String, String, String, f32)>> {
        let relationship_filter = if let Some(types) = relationship_types {
            if types.is_empty() {
                String::new()
            } else {
                format!(":{}", types.join("|"))
            }
        } else {
            String::new()
        };

        let cypher_query = format!(
            "MATCH (a {{id: '{node_id}'}})-[r{relationship_filter}]->(b) RETURN b.id, r.id, type(r), COALESCE(r.confidence, 1.0) AS confidence"
        );

        debug!(
            node_id = %node_id,
            query = %cypher_query,
            "Getting neighbors from graph"
        );

        let graph_actor_ref = orbit_client
            .actor_reference::<crate::protocols::graph_database::GraphActor>(Key::StringKey {
                key: kg_name.to_string(),
            })
            .await
            .map_err(|e| {
                OrbitError::internal(format!("Failed to get graph actor reference: {e}"))
            })?;

        let _result: serde_json::Value = graph_actor_ref
            .invoke("execute_query", vec![serde_json::json!(cypher_query)])
            .await
            .map_err(|e| OrbitError::internal(format!("Failed to execute neighbor query: {e}")))?;

        // Parse query results (simplified - in reality would parse from QueryResult)
        // For now, return empty vector - this would be implemented based on actual QueryResult structure
        Ok(Vec::new())
    }

    /// Apply pruning strategies to decide if a path should be explored
    fn should_prune(&self, current_state: &PathState, neighbor_id: &str, confidence: f32) -> bool {
        match &self.pruning_strategy {
            PruningStrategy::None => false,

            PruningStrategy::ScoreBased { threshold } => confidence < *threshold,

            PruningStrategy::BranchingFactor { max_branches: _ } => {
                // Would track branching factor per node
                false
            }

            PruningStrategy::AvoidCycles => current_state.visited.contains(neighbor_id),

            PruningStrategy::Combined {
                score_threshold,
                max_branches: _,
                avoid_cycles,
            } => {
                if confidence < *score_threshold {
                    return true;
                }

                if *avoid_cycles && current_state.visited.contains(neighbor_id) {
                    return true;
                }

                false
            }
        }
    }

    /// Calculate score for a path extension
    fn calculate_path_score(
        &self,
        current_state: &PathState,
        relationship_confidence: f32,
        _relationship_type: &str,
    ) -> f32 {
        match &self.path_scoring {
            PathScoringStrategy::ConfidenceBased => current_state.score * relationship_confidence,

            PathScoringStrategy::LengthBased => {
                current_state.score * (1.0 / (current_state.hop_count + 1) as f32)
            }

            PathScoringStrategy::ImportanceBased => {
                // Would use entity importance scores
                current_state.score * 0.9 // Simplified
            }

            PathScoringStrategy::Combined {
                confidence_weight,
                length_weight,
                importance_weight,
            } => {
                let confidence_component = relationship_confidence * confidence_weight;
                let length_component = (1.0 / (current_state.hop_count + 1) as f32) * length_weight;
                let importance_component = 0.5 * importance_weight; // Simplified

                current_state.score
                    * (confidence_component + length_component + importance_component)
            }

            PathScoringStrategy::Custom { function_name: _ } => {
                // Would call custom scoring function
                current_state.score * 0.8 // Simplified
            }
        }
    }

    /// Generate human-readable explanation for a path
    fn generate_path_explanation(&self, path_state: &PathState) -> String {
        if path_state.path.len() <= 1 {
            return "Direct connection".to_string();
        }

        format!(
            "Path with {} hops through {} entities (score: {:.3})",
            path_state.hop_count,
            path_state.path.len(),
            path_state.score
        )
    }

    /// Update reasoning statistics
    fn update_stats(&mut self, explored: usize, found: usize, query_time_ms: u64) {
        self.stats.queries_executed += 1;
        self.stats.paths_explored += explored as u64;
        self.stats.paths_found += found as u64;

        // Update average query time
        let total_time = (self.stats.avg_query_time_ms * (self.stats.queries_executed - 1) as f64)
            + query_time_ms as f64;
        self.stats.avg_query_time_ms = total_time / self.stats.queries_executed as f64;

        self.stats.last_updated = chrono::Utc::now().timestamp_millis();
        self.updated_at = self.stats.last_updated;
    }

    /// Get reasoning statistics
    pub fn get_stats(&self) -> &ReasoningStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = ReasoningStats::default();
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Update configuration
    pub fn update_config(&mut self, config: ReasoningConfig) {
        self.config = config;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Update path scoring strategy
    pub fn update_scoring_strategy(&mut self, strategy: PathScoringStrategy) {
        self.path_scoring = strategy;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }

    /// Update pruning strategy
    pub fn update_pruning_strategy(&mut self, strategy: PruningStrategy) {
        self.pruning_strategy = strategy;
        self.updated_at = chrono::Utc::now().timestamp_millis();
    }
}

impl Default for MultiHopReasoningEngine {
    fn default() -> Self {
        Self::new(3)
    }
}

impl Addressable for MultiHopReasoningEngine {
    fn addressable_type() -> &'static str {
        "MultiHopReasoningEngine"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_engine_creation() {
        let engine = MultiHopReasoningEngine::new(3);
        assert_eq!(engine.max_hops, 3);
        assert_eq!(engine.stats.queries_executed, 0);
    }

    #[test]
    fn test_reasoning_config() {
        let config = ReasoningConfig::default();
        assert_eq!(config.max_paths_per_hop, 100);
        assert_eq!(config.max_results, 50);
        assert!(config.bidirectional_search);
    }

    #[test]
    fn test_path_scoring_strategies() {
        let engine = MultiHopReasoningEngine::new(2);

        let path_state = PathState {
            current_node: "node1".to_string(),
            path: vec!["start".to_string(), "node1".to_string()],
            relationships: vec!["rel1".to_string()],
            score: 1.0,
            visited: HashSet::new(),
            hop_count: 1,
        };

        // Test confidence-based scoring
        let score = engine.calculate_path_score(&path_state, 0.8, "RELATED");
        assert!(score > 0.0);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_pruning_strategies() {
        let mut engine = MultiHopReasoningEngine::new(3);
        engine.pruning_strategy = PruningStrategy::AvoidCycles;

        let mut visited = HashSet::new();
        visited.insert("visited_node".to_string());

        let path_state = PathState {
            current_node: "current".to_string(),
            path: vec!["start".to_string(), "current".to_string()],
            relationships: vec!["rel1".to_string()],
            score: 1.0,
            visited,
            hop_count: 1,
        };

        // Should prune if visiting a node that's already been visited
        assert!(engine.should_prune(&path_state, "visited_node", 0.9));

        // Should not prune for new nodes
        assert!(!engine.should_prune(&path_state, "new_node", 0.9));
    }
}
