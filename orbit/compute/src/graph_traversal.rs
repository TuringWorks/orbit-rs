//! GPU-Accelerated Graph Traversal Algorithms
//!
//! This module provides GPU-accelerated implementations of graph traversal algorithms
//! including BFS, DFS, shortest paths, and community detection. These operations
//! are compute-bound and benefit significantly from parallel execution on GPUs.

use crate::errors::ComputeError;
#[cfg(feature = "gpu-acceleration")]
use crate::gpu::GPUAccelerationManager;
#[cfg(feature = "gpu-acceleration")]
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{info, warn};

/// Graph representation optimized for GPU traversal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    /// Node IDs (sorted for efficient access)
    pub node_ids: Vec<u64>,
    
    /// Adjacency list: node_id -> list of neighbor node indices
    pub adjacency_list: Vec<Vec<u32>>,
    
    /// Edge weights (optional, for weighted traversals)
    pub edge_weights: Option<Vec<f32>>,
    
    /// Node properties (for filtering/scoring)
    pub node_properties: HashMap<u64, NodeProperties>,
    
    /// Total number of nodes
    pub node_count: usize,
    
    /// Total number of edges
    pub edge_count: usize,
}

/// Properties associated with graph nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeProperties {
    /// Node importance/centrality score
    pub importance: f32,
    
    /// Node type/category
    pub node_type: Option<String>,
    
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

/// Graph traversal configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalConfig {
    /// Maximum depth/hops to traverse
    pub max_depth: u32,
    
    /// Maximum number of paths to explore
    pub max_paths: usize,
    
    /// Enable GPU acceleration
    pub use_gpu: bool,
    
    /// Minimum path score threshold
    pub min_score: f32,
    
    /// Relationship types to include (empty = all)
    pub allowed_types: Vec<String>,
    
    /// Enable bidirectional search
    pub bidirectional: bool,
}

impl Default for TraversalConfig {
    fn default() -> Self {
        Self {
            max_depth: 5,
            max_paths: 1000,
            use_gpu: true,
            min_score: 0.0,
            allowed_types: Vec::new(),
            bidirectional: true,
        }
    }
}

/// Result of a graph traversal operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalResult {
    /// Found paths from source to target
    pub paths: Vec<Path>,

    /// Nodes visited during traversal
    pub visited_nodes: HashSet<u64>,

    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Whether GPU acceleration was used
    pub used_gpu: bool,

    /// Statistics about the traversal
    pub stats: TraversalStats,
}

/// PageRank result containing node scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRankResult {
    /// Map from node_id to PageRank score
    pub scores: HashMap<u64, f32>,

    /// Number of iterations performed
    pub iterations: usize,

    /// Execution time in milliseconds
    pub execution_time_ms: u64,

    /// Whether GPU acceleration was used
    pub used_gpu: bool,

    /// Convergence delta (difference from previous iteration)
    pub convergence_delta: f32,
}

/// A path through the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Path {
    /// Node IDs in the path
    pub nodes: Vec<u64>,
    
    /// Edge indices (if available)
    pub edges: Vec<usize>,
    
    /// Total path weight/cost
    pub weight: f32,
    
    /// Path score (confidence/relevance)
    pub score: f32,
    
    /// Path length (number of hops)
    pub length: usize,
}

/// Statistics about traversal execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TraversalStats {
    /// Number of nodes explored
    pub nodes_explored: usize,
    
    /// Number of edges traversed
    pub edges_traversed: usize,
    
    /// Number of paths found
    pub paths_found: usize,
    
    /// Average path length
    pub avg_path_length: f32,
    
    /// GPU utilization percentage (if GPU used)
    pub gpu_utilization: Option<f32>,
}

/// GPU-accelerated graph traversal engine
pub struct GPUGraphTraversal {
    #[cfg(feature = "gpu-acceleration")]
    gpu_manager: Arc<GPUAccelerationManager>,
    config: TraversalConfig,
}

impl GPUGraphTraversal {
    /// Create a new GPU-accelerated graph traversal engine
    pub async fn new(config: TraversalConfig) -> Result<Self, ComputeError> {
        #[cfg(feature = "gpu-acceleration")]
        let gpu_manager = Arc::new(GPUAccelerationManager::new().await?);
        
        Ok(Self {
            #[cfg(feature = "gpu-acceleration")]
            gpu_manager,
            config,
        })
    }

    /// Perform breadth-first search from source to target
    pub async fn bfs(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        let start_time = std::time::Instant::now();
        
        if self.config.use_gpu && self.should_use_gpu(graph) {
            info!("Using GPU-accelerated BFS");
            self.bfs_gpu(graph, source, target).await
        } else {
            info!("Using CPU BFS");
            self.bfs_cpu(graph, source, target).await
        }
        .map(|mut result| {
            result.execution_time_ms = start_time.elapsed().as_millis() as u64;
            result
        })
    }

    /// GPU-accelerated BFS implementation
    async fn bfs_gpu(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        // Convert graph to GPU-friendly format (only when GPU features are enabled)
        #[cfg(any(
            all(feature = "gpu-acceleration", target_os = "macos"),
            all(feature = "gpu-acceleration", feature = "gpu-vulkan")
        ))]
        let gpu_graph = self.prepare_gpu_graph(graph)?;

        // Use u32 indices if graph is small enough (< 4B nodes)
        #[cfg(any(
            all(feature = "gpu-acceleration", target_os = "macos"),
            all(feature = "gpu-acceleration", feature = "gpu-vulkan")
        ))]
        let use_u32_indices = graph.node_count <= u32::MAX as usize;

        // Try Metal first (macOS), then Vulkan
        #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
        {
            use crate::gpu_metal::MetalDevice;
            
            if let Ok(metal_device) = MetalDevice::new() {
                info!("Using Metal GPU for BFS traversal");
                return self.bfs_gpu_metal(
                    &metal_device,
                    graph,
                    &gpu_graph,
                    source,
                    target,
                    use_u32_indices,
                ).await;
            }
        }
        
        #[cfg(all(feature = "gpu-acceleration", feature = "gpu-vulkan"))]
        {
            use crate::gpu_vulkan::VulkanDevice;
            
            if let Ok(mut vulkan_device) = VulkanDevice::new() {
                info!("Using Vulkan GPU for BFS traversal");
                return self.bfs_gpu_vulkan(
                    &mut vulkan_device,
                    graph,
                    &gpu_graph,
                    source,
                    target,
                    use_u32_indices,
                ).await;
            }
        }
        
        // Fall back to CPU with parallelization
        warn!("GPU BFS not available, using optimized CPU");
        self.bfs_cpu_parallel(graph, source, target).await
    }
    
    /// Metal-accelerated BFS implementation
    #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
    async fn bfs_gpu_metal(
        &self,
        metal_device: &crate::gpu_metal::MetalDevice,
        graph: &GraphData,
        gpu_graph: &GPUGraphData,
        source: u64,
        target: Option<u64>,
        use_u32_indices: bool,
    ) -> Result<TraversalResult, ComputeError> {
        self.bfs_gpu_impl(
            graph,
            gpu_graph,
            source,
            target,
            use_u32_indices,
            |edge_array, edge_offset, current_level, visited, next_level, next_level_size, parent, current_level_size, max_nodes| {
                if use_u32_indices {
                    // Use optimized u32 version for smaller graphs
                    let current_level_u32: Vec<u32> = current_level.iter().map(|&x| x as u32).collect();
                    let mut next_level_u32 = vec![0u32; next_level.len()];
                    let mut next_level_size_u32 = *next_level_size;

                    metal_device.execute_bfs_level_expansion_u32(
                        edge_array,
                        edge_offset,
                        &current_level_u32,
                        visited,
                        &mut next_level_u32,
                        &mut next_level_size_u32,
                        parent,
                        current_level_size,
                        max_nodes,
                    )?;

                    // Convert back to u64
                    *next_level_size = next_level_size_u32;
                    for (i, &val) in next_level_u32.iter().enumerate().take(*next_level_size as usize) {
                        next_level[i] = val as u64;
                    }
                    Ok(())
                } else {
                    // Use u64 version (which internally converts to u32 for Metal kernel)
                    metal_device.execute_bfs_level_expansion(
                        edge_array,
                        edge_offset,
                        current_level,
                        visited,
                        next_level,
                        next_level_size,
                        parent,
                        current_level_size,
                        max_nodes,
                    )
                }
            },
        ).await
    }
    
    /// Vulkan-accelerated BFS implementation
    #[cfg(all(feature = "gpu-acceleration", feature = "gpu-vulkan"))]
    async fn bfs_gpu_vulkan(
        &self,
        vulkan_device: &mut crate::gpu_vulkan::VulkanDevice,
        graph: &GraphData,
        gpu_graph: &GPUGraphData,
        source: u64,
        target: Option<u64>,
        use_u32_indices: bool,
    ) -> Result<TraversalResult, ComputeError> {
        self.bfs_gpu_impl(
            graph,
            gpu_graph,
            source,
            target,
            use_u32_indices,
            |edge_array, edge_offset, current_level, visited, next_level, next_level_size, parent, current_level_size, max_nodes| {
                if use_u32_indices {
                    // Use optimized u32 version for smaller graphs
                    let current_level_u32: Vec<u32> = current_level.iter().map(|&x| x as u32).collect();
                    let mut next_level_u32 = vec![0u32; next_level.len()];
                    let mut next_level_size_u32 = *next_level_size;

                    vulkan_device.execute_bfs_level_expansion(
                        edge_array,
                        edge_offset,
                        &current_level_u32,
                        visited,
                        &mut next_level_u32,
                        &mut next_level_size_u32,
                        parent,
                        current_level_size,
                        max_nodes,
                    )?;

                    // Convert back to u64
                    *next_level_size = next_level_size_u32;
                    for (i, &val) in next_level_u32.iter().enumerate().take(*next_level_size as usize) {
                        next_level[i] = val as u64;
                    }
                    Ok(())
                } else {
                    // Convert u64 to u32 for Vulkan kernel
                    let current_level_u32: Vec<u32> = current_level.iter().map(|&x| x as u32).collect();
                    let mut next_level_u32 = vec![0u32; next_level.len()];
                    let mut next_level_size_u32 = *next_level_size;

                    vulkan_device.execute_bfs_level_expansion(
                        edge_array,
                        edge_offset,
                        &current_level_u32,
                        visited,
                        &mut next_level_u32,
                        &mut next_level_size_u32,
                        parent,
                        current_level_size,
                        max_nodes,
                    )?;

                    // Convert back to u64
                    *next_level_size = next_level_size_u32;
                    for (i, &val) in next_level_u32.iter().enumerate().take(*next_level_size as usize) {
                        next_level[i] = val as u64;
                    }
                    Ok(())
                }
            },
        ).await
    }
    
    /// Generic GPU BFS implementation with parent tracking
    #[allow(dead_code)]
    async fn bfs_gpu_impl<F>(
        &self,
        graph: &GraphData,
        gpu_graph: &GPUGraphData,
        source: u64,
        target: Option<u64>,
        _use_u32_indices: bool,
        mut execute_kernel: F,
    ) -> Result<TraversalResult, ComputeError>
    where
        F: FnMut(
            &[u32],          // edge_array
            &[u32],          // edge_offset
            &[u64],          // current_level
            &mut [u32],      // visited
            &mut [u64],      // next_level
            &mut u32,        // next_level_size
            &mut [u32],      // parent (NEW)
            u32,             // current_level_size
            u32,             // max_nodes
        ) -> Result<(), ComputeError>,
    {
        // Initialize BFS state
        let mut visited = vec![0u32; graph.node_count];
        
        // Convert source node ID to index
        let source_idx = graph.node_ids.iter().position(|&id| id == source)
            .ok_or_else(|| ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                kernel_name: "bfs_level_expansion".to_string(),
                error: "Source node not found in graph".to_string(),
            }))?;
        
        // Mark source as visited
        visited[source_idx] = 1;
        
        // Convert target node ID to index (if provided)
        let target_idx = target.and_then(|target_id| {
            graph.node_ids.iter().position(|&id| id == target_id)
        });
        
        // Parent tracking for path reconstruction: parent[node_idx] = parent_idx
        // GPU kernel will populate this array
        let mut parent_u32 = vec![u32::MAX; graph.node_count]; // u32::MAX = no parent
        parent_u32[source_idx] = source_idx as u32; // Source points to itself

        // Start with source index
        let mut current_level_indices = vec![source_idx as u64];
        let mut paths = Vec::new();
        let mut nodes_explored = 0;
        let mut edges_traversed = 0;

        // Iterative BFS using GPU level expansion
        for depth in 0..self.config.max_depth as usize {
            if current_level_indices.is_empty() {
                break;
            }

            let mut next_level = vec![0u64; graph.node_count];
            let mut next_level_size = 0u32;

            // Execute GPU kernel for level expansion (with GPU parent tracking!)
            if let Err(e) = execute_kernel(
                &gpu_graph.edge_array,
                &gpu_graph.edge_offset,
                &current_level_indices,
                &mut visited,
                &mut next_level,
                &mut next_level_size,
                &mut parent_u32,
                current_level_indices.len() as u32,
                graph.node_count as u32,
            ) {
                warn!("GPU BFS level expansion failed: {}, falling back to CPU", e);
                return self.bfs_cpu_parallel(graph, source, target).await;
            }

            // No CPU post-processing needed! GPU kernel has updated parent array

            nodes_explored += current_level_indices.len();

            // Count edges traversed
            for &node_idx in &current_level_indices {
                if let Some(neighbors) = graph.adjacency_list.get(node_idx as usize) {
                    edges_traversed += neighbors.len();
                }
            }

            let next_level_slice = &next_level[..next_level_size as usize];

            // Check if target found and reconstruct full path
            if let Some(target_idx_val) = target_idx {
                let target_idx_u64 = target_idx_val as u64;
                if next_level_slice.contains(&target_idx_u64) {
                    // Reconstruct full path using GPU-generated parent tracking
                    let mut path_nodes = Vec::new();
                    let mut current_idx = target_idx_val;

                    // Build path backwards from target to source
                    loop {
                        if let Some(&node_id) = graph.node_ids.get(current_idx) {
                            path_nodes.push(node_id);
                        } else {
                            break;
                        }

                        let parent_idx = parent_u32[current_idx];
                        if parent_idx == u32::MAX || parent_idx as usize == current_idx {
                            // Reached source (source points to itself)
                            break;
                        }
                        current_idx = parent_idx as usize;
                    }

                    // Reverse to get source -> target path
                    path_nodes.reverse();

                    paths.push(Path {
                        nodes: path_nodes,
                        edges: vec![], // TODO: Reconstruct edges if needed
                        weight: depth as f32 + 1.0,
                        score: 1.0,
                        length: depth + 1,
                    });

                    if paths.len() >= self.config.max_paths {
                        break;
                    }
                }
            }
            
            // Prepare next level (already in index format)
            current_level_indices = next_level_slice.iter().copied().collect();
        }
        
        let paths_count = paths.len();
        let avg_path_length = if !paths.is_empty() {
            paths.iter().map(|p| p.length as f32).sum::<f32>() / paths.len() as f32
        } else {
            0.0
        };
        
        Ok(TraversalResult {
            paths,
            visited_nodes: visited.iter()
                .enumerate()
                .filter_map(|(idx, &v)| if v == 1 { graph.node_ids.get(idx).copied() } else { None })
                .collect(),
            execution_time_ms: 0, // Set by caller
            used_gpu: true,
            stats: TraversalStats {
                nodes_explored,
                edges_traversed,
                paths_found: paths_count,
                avg_path_length,
                gpu_utilization: Some(0.0), // TODO: Measure actual GPU utilization
            },
        })
    }

    /// CPU-based BFS with parallelization
    async fn bfs_cpu_parallel(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        use rayon::prelude::*;
        
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut paths = Vec::new();
        let mut parent_map: HashMap<u64, Vec<(u64, usize)>> = HashMap::new();
        
        queue.push_back((source, 0, vec![source]));
        visited.insert(source);
        parent_map.insert(source, vec![]);
        
        let mut nodes_explored = 0;
        let mut edges_traversed = 0;
        
        while let Some((current, depth, path)) = queue.pop_front() {
            nodes_explored += 1;
            
            // Check if we reached the target
            if let Some(target_id) = target {
                if current == target_id {
                    paths.push(Path {
                        nodes: path.clone(),
                        edges: vec![],
                        weight: 0.0,
                        score: 1.0,
                        length: depth,
                    });
                    
                    if paths.len() >= self.config.max_paths {
                        break;
                    }
                }
            }
            
            if depth >= self.config.max_depth as usize {
                continue;
            }
            
            // Get neighbors (parallel processing only for large adjacency lists to avoid Rayon overhead)
            if let Some(neighbors) = graph.adjacency_list.get(current as usize) {
                let neighbor_batch: Vec<_> = if neighbors.len() > 1000 {
                    // Parallel processing for large neighbor lists
                    neighbors
                        .par_iter()
                        .filter_map(|&neighbor_idx| {
                            let neighbor_id = graph.node_ids.get(neighbor_idx as usize)?;
                            if !visited.contains(neighbor_id) {
                                Some(*neighbor_id)
                            } else {
                                None
                            }
                        })
                        .collect()
                } else {
                    // Sequential for small lists (avoid Rayon overhead)
                    neighbors
                        .iter()
                        .filter_map(|&neighbor_idx| {
                            let neighbor_id = graph.node_ids.get(neighbor_idx as usize)?;
                            if !visited.contains(neighbor_id) {
                                Some(*neighbor_id)
                            } else {
                                None
                            }
                        })
                        .collect()
                };

                edges_traversed += neighbor_batch.len();

                for neighbor_id in neighbor_batch {
                    visited.insert(neighbor_id);
                    let mut new_path = path.clone();
                    new_path.push(neighbor_id);
                    queue.push_back((neighbor_id, depth + 1, new_path));
                }
            }
        }
        
        let paths_found = paths.len();
        let avg_path_length = if !paths.is_empty() {
            paths.iter().map(|p| p.length as f32).sum::<f32>() / paths_found as f32
        } else {
            0.0
        };
        
        Ok(TraversalResult {
            paths,
            visited_nodes: visited,
            execution_time_ms: 0, // Set by caller
            used_gpu: false,
            stats: TraversalStats {
                nodes_explored,
                edges_traversed,
                paths_found,
                avg_path_length,
                gpu_utilization: None,
            },
        })
    }

    /// CPU-based BFS (fallback)
    async fn bfs_cpu(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        self.bfs_cpu_parallel(graph, source, target).await
    }

    /// Perform Dijkstra's shortest path from source to target (weighted)
    pub async fn dijkstra(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        let start_time = std::time::Instant::now();

        // Dijkstra requires edge weights
        if graph.edge_weights.is_none() {
            return Err(ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                kernel_name: "dijkstra".to_string(),
                error: "Edge weights required for Dijkstra's algorithm".to_string(),
            }));
        }

        if self.config.use_gpu && self.should_use_gpu(graph) {
            info!("Using GPU-accelerated Dijkstra");
            self.dijkstra_gpu(graph, source, target).await
        } else {
            info!("Using CPU Dijkstra");
            self.dijkstra_cpu(graph, source, target).await
        }
        .map(|mut result| {
            result.execution_time_ms = start_time.elapsed().as_millis() as u64;
            result
        })
    }

    /// GPU-accelerated Dijkstra implementation
    async fn dijkstra_gpu(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        // Convert graph to GPU-friendly format (only when GPU features are enabled)
        #[cfg(any(
            all(feature = "gpu-acceleration", target_os = "macos"),
            all(feature = "gpu-acceleration", feature = "gpu-vulkan")
        ))]
        let gpu_graph = self.prepare_gpu_graph(graph)?;

        // Try Metal first (macOS), then Vulkan
        #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
        {
            use crate::gpu_metal::MetalDevice;

            if let Ok(metal_device) = MetalDevice::new() {
                info!("Using Metal GPU for Dijkstra");
                return self.dijkstra_gpu_metal(
                    &metal_device,
                    graph,
                    &gpu_graph,
                    source,
                    target,
                ).await;
            }
        }

        #[cfg(all(feature = "gpu-acceleration", feature = "gpu-vulkan"))]
        {
            use crate::gpu_vulkan::VulkanDevice;

            if let Ok(mut vulkan_device) = VulkanDevice::new() {
                info!("Using Vulkan GPU for Dijkstra");
                return self.dijkstra_gpu_vulkan(
                    &mut vulkan_device,
                    graph,
                    &gpu_graph,
                    source,
                    target,
                ).await;
            }
        }

        // Fall back to CPU
        warn!("GPU Dijkstra not available, using optimized CPU");
        self.dijkstra_cpu(graph, source, target).await
    }

    /// Metal-accelerated Dijkstra implementation
    #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
    async fn dijkstra_gpu_metal(
        &self,
        metal_device: &crate::gpu_metal::MetalDevice,
        graph: &GraphData,
        gpu_graph: &GPUGraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        self.dijkstra_gpu_impl(
            graph,
            gpu_graph,
            source,
            target,
            |edge_array, edge_offset, edge_weights, distances, parent, active_mask, changed| {
                metal_device.execute_dijkstra_relax(
                    edge_array,
                    edge_offset,
                    edge_weights,
                    distances,
                    parent,
                    active_mask,
                    changed,
                    graph.node_count as u32,
                )
            },
        ).await
    }

    /// Vulkan-accelerated Dijkstra implementation
    #[cfg(all(feature = "gpu-acceleration", feature = "gpu-vulkan"))]
    async fn dijkstra_gpu_vulkan(
        &self,
        vulkan_device: &mut crate::gpu_vulkan::VulkanDevice,
        graph: &GraphData,
        gpu_graph: &GPUGraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        self.dijkstra_gpu_impl(
            graph,
            gpu_graph,
            source,
            target,
            |edge_array, edge_offset, edge_weights, distances, parent, active_mask, changed| {
                vulkan_device.execute_dijkstra_relax(
                    edge_array,
                    edge_offset,
                    edge_weights,
                    distances,
                    parent,
                    active_mask,
                    changed,
                    graph.node_count as u32,
                )
            },
        ).await
    }

    /// Generic GPU Dijkstra implementation
    #[allow(dead_code)]
    async fn dijkstra_gpu_impl<F>(
        &self,
        graph: &GraphData,
        gpu_graph: &GPUGraphData,
        source: u64,
        target: Option<u64>,
        mut execute_kernel: F,
    ) -> Result<TraversalResult, ComputeError>
    where
        F: FnMut(
            &[u32],          // edge_array
            &[u32],          // edge_offset
            &[f32],          // edge_weights
            &mut [f32],      // distances
            &mut [u32],      // parent
            &mut [u32],      // active_mask
            &mut u32,        // changed flag
        ) -> Result<(), ComputeError>,
    {
        // Get edge weights
        let edge_weights = graph.edge_weights.as_ref()
            .ok_or_else(|| ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                kernel_name: "dijkstra_relax".to_string(),
                error: "Edge weights required".to_string(),
            }))?;

        // Convert source node ID to index
        let source_idx = graph.node_ids.iter().position(|&id| id == source)
            .ok_or_else(|| ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                kernel_name: "dijkstra_relax".to_string(),
                error: "Source node not found in graph".to_string(),
            }))?;

        // Convert target node ID to index (if provided)
        let target_idx = target.and_then(|target_id| {
            graph.node_ids.iter().position(|&id| id == target_id)
        });

        // Initialize distances to infinity, parent to u32::MAX
        let mut distances = vec![f32::INFINITY; graph.node_count];
        let mut parent = vec![u32::MAX; graph.node_count];
        let mut active_mask = vec![1u32; graph.node_count]; // All nodes active initially

        // Set source distance to 0
        distances[source_idx] = 0.0;
        parent[source_idx] = source_idx as u32;

        let mut iteration = 0;
        let max_iterations = graph.node_count; // Prevent infinite loops

        // Iterate until no changes or max iterations
        loop {
            let mut changed = 0u32;

            // Execute GPU kernel for edge relaxation
            if let Err(e) = execute_kernel(
                &gpu_graph.edge_array,
                &gpu_graph.edge_offset,
                edge_weights,
                &mut distances,
                &mut parent,
                &mut active_mask,
                &mut changed,
            ) {
                warn!("GPU Dijkstra relaxation failed: {}, falling back to CPU", e);
                return self.dijkstra_cpu(graph, source, target).await;
            }

            iteration += 1;

            // Check if converged
            if changed == 0 || iteration >= max_iterations {
                break;
            }

            // Early termination if target found
            if let Some(target_idx_val) = target_idx {
                if distances[target_idx_val] != f32::INFINITY {
                    // Check if target's neighbors have been processed
                    break;
                }
            }
        }

        // Reconstruct path(s)
        let mut paths = Vec::new();

        if let Some(target_idx_val) = target_idx {
            // Single target path
            if distances[target_idx_val] != f32::INFINITY {
                let mut path_nodes = Vec::new();
                let mut current_idx = target_idx_val;

                // Build path backwards from target to source
                loop {
                    if let Some(&node_id) = graph.node_ids.get(current_idx) {
                        path_nodes.push(node_id);
                    } else {
                        break;
                    }

                    let parent_idx = parent[current_idx];
                    if parent_idx == u32::MAX || parent_idx as usize == current_idx {
                        break; // Reached source
                    }
                    current_idx = parent_idx as usize;
                }

                // Reverse to get source -> target path
                path_nodes.reverse();

                paths.push(Path {
                    nodes: path_nodes.clone(),
                    edges: vec![],
                    weight: distances[target_idx_val],
                    score: 1.0,
                    length: path_nodes.len() - 1,
                });
            }
        }

        // Statistics
        let nodes_explored = distances.iter().filter(|&&d| d != f32::INFINITY).count();
        let visited_nodes: HashSet<u64> = distances.iter()
            .enumerate()
            .filter_map(|(idx, &d)| {
                if d != f32::INFINITY {
                    graph.node_ids.get(idx).copied()
                } else {
                    None
                }
            })
            .collect();

        let paths_count = paths.len();
        let avg_path_length = if !paths.is_empty() {
            paths.iter().map(|p| p.length as f32).sum::<f32>() / paths.len() as f32
        } else {
            0.0
        };

        Ok(TraversalResult {
            paths,
            visited_nodes,
            execution_time_ms: 0, // Set by caller
            used_gpu: true,
            stats: TraversalStats {
                nodes_explored,
                edges_traversed: 0, // TODO: Track edge relaxations
                paths_found: paths_count,
                avg_path_length,
                gpu_utilization: Some(0.0),
            },
        })
    }

    /// CPU-based Dijkstra with priority queue
    async fn dijkstra_cpu(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        use std::collections::BinaryHeap;
        use std::cmp::Ordering;

        // Min-heap node wrapper
        #[derive(Copy, Clone, PartialEq)]
        struct State {
            cost: f32,
            node_idx: usize,
        }

        // Reverse ordering for min-heap
        impl Eq for State {}
        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                other.cost.partial_cmp(&self.cost)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| self.node_idx.cmp(&other.node_idx))
            }
        }
        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        // Get edge weights
        let edge_weights = graph.edge_weights.as_ref()
            .ok_or_else(|| ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                kernel_name: "dijkstra".to_string(),
                error: "Edge weights required".to_string(),
            }))?;

        // Find source index
        let source_idx = graph.node_ids.iter().position(|&id| id == source)
            .ok_or_else(|| ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                kernel_name: "dijkstra".to_string(),
                error: "Source node not found".to_string(),
            }))?;

        let target_idx = target.and_then(|t| graph.node_ids.iter().position(|&id| id == t));

        // Initialize
        let mut distances = vec![f32::INFINITY; graph.node_count];
        let mut parent = vec![usize::MAX; graph.node_count];
        let mut heap = BinaryHeap::new();

        distances[source_idx] = 0.0;
        parent[source_idx] = source_idx;
        heap.push(State { cost: 0.0, node_idx: source_idx });

        let mut nodes_explored = 0;
        let mut edges_traversed = 0;

        // Dijkstra's algorithm
        while let Some(State { cost, node_idx }) = heap.pop() {
            nodes_explored += 1;

            // Early termination if target found
            if let Some(target_idx_val) = target_idx {
                if node_idx == target_idx_val {
                    break;
                }
            }

            // Skip if we've already found a better path
            if cost > distances[node_idx] {
                continue;
            }

            // Process neighbors
            if let Some(neighbors) = graph.adjacency_list.get(node_idx) {
                let edge_start = graph.adjacency_list[..node_idx].iter().map(|v| v.len()).sum::<usize>();

                for (i, &neighbor_idx) in neighbors.iter().enumerate() {
                    edges_traversed += 1;

                    let edge_weight = edge_weights.get(edge_start + i).copied().unwrap_or(1.0);
                    let next_cost = cost + edge_weight;

                    if next_cost < distances[neighbor_idx as usize] {
                        distances[neighbor_idx as usize] = next_cost;
                        parent[neighbor_idx as usize] = node_idx;
                        heap.push(State { cost: next_cost, node_idx: neighbor_idx as usize });
                    }
                }
            }
        }

        // Reconstruct path
        let mut paths = Vec::new();

        if let Some(target_idx_val) = target_idx {
            if distances[target_idx_val] != f32::INFINITY {
                let mut path_nodes = Vec::new();
                let mut current_idx = target_idx_val;

                loop {
                    if let Some(&node_id) = graph.node_ids.get(current_idx) {
                        path_nodes.push(node_id);
                    } else {
                        break;
                    }

                    if parent[current_idx] == usize::MAX || parent[current_idx] == current_idx {
                        break;
                    }
                    current_idx = parent[current_idx];
                }

                path_nodes.reverse();

                paths.push(Path {
                    nodes: path_nodes.clone(),
                    edges: vec![],
                    weight: distances[target_idx_val],
                    score: 1.0,
                    length: path_nodes.len() - 1,
                });
            }
        }

        let visited_nodes: HashSet<u64> = distances.iter()
            .enumerate()
            .filter_map(|(idx, &d)| {
                if d != f32::INFINITY {
                    graph.node_ids.get(idx).copied()
                } else {
                    None
                }
            })
            .collect();

        let paths_count = paths.len();
        let avg_path_length = if !paths.is_empty() {
            paths.iter().map(|p| p.length as f32).sum::<f32>() / paths.len() as f32
        } else {
            0.0
        };

        Ok(TraversalResult {
            paths,
            visited_nodes,
            execution_time_ms: 0,
            used_gpu: false,
            stats: TraversalStats {
                nodes_explored,
                edges_traversed,
                paths_found: paths_count,
                avg_path_length,
                gpu_utilization: None,
            },
        })
    }

    /// Perform depth-first search from source to target
    pub async fn dfs(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        let start_time = std::time::Instant::now();

        // DFS is inherently sequential, so CPU is usually more efficient
        // GPU version would require stack management which is complex
        info!("Using CPU DFS (inherently sequential algorithm)");
        self.dfs_cpu(graph, source, target).await
            .map(|mut result| {
                result.execution_time_ms = start_time.elapsed().as_millis() as u64;
                result
            })
    }

    /// CPU-based DFS with stack
    async fn dfs_cpu(
        &self,
        graph: &GraphData,
        source: u64,
        target: Option<u64>,
    ) -> Result<TraversalResult, ComputeError> {
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut paths = Vec::new();
        let mut parent_map: HashMap<u64, u64> = HashMap::new();

        // Verify source exists
        let _source_idx = graph.node_ids.iter().position(|&id| id == source)
            .ok_or_else(|| ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                kernel_name: "dfs".to_string(),
                error: "Source node not found in graph".to_string(),
            }))?;

        // DFS using explicit stack: (node_id, depth)
        stack.push((source, 0));
        visited.insert(source);

        let mut nodes_explored = 0;
        let mut edges_traversed = 0;

        while let Some((current, depth)) = stack.pop() {
            nodes_explored += 1;

            // Check if we reached the target
            if let Some(target_id) = target {
                if current == target_id {
                    // Reconstruct path from source to target
                    let mut path = vec![target_id];
                    let mut current_node = target_id;

                    while current_node != source {
                        if let Some(&parent) = parent_map.get(&current_node) {
                            path.push(parent);
                            current_node = parent;
                        } else {
                            break;
                        }
                    }

                    path.reverse();

                    paths.push(Path {
                        nodes: path.clone(),
                        edges: vec![],
                        weight: 0.0,
                        score: 1.0,
                        length: path.len() - 1,
                    });

                    if paths.len() >= self.config.max_paths {
                        break;
                    }
                }
            }

            if depth >= self.config.max_depth as usize {
                continue;
            }

            // Get current node index
            let current_idx = graph.node_ids.iter().position(|&id| id == current)
                .unwrap_or(0);

            // Process neighbors (in reverse order for DFS to maintain left-to-right traversal)
            if let Some(neighbors) = graph.adjacency_list.get(current_idx) {
                edges_traversed += neighbors.len();

                for &neighbor_idx in neighbors.iter().rev() {
                    if let Some(&neighbor_id) = graph.node_ids.get(neighbor_idx as usize) {
                        if !visited.contains(&neighbor_id) {
                            visited.insert(neighbor_id);
                            parent_map.insert(neighbor_id, current);
                            stack.push((neighbor_id, depth + 1));
                        }
                    }
                }
            }
        }

        let paths_found = paths.len();
        let avg_path_length = if !paths.is_empty() {
            paths.iter().map(|p| p.length as f32).sum::<f32>() / paths_found as f32
        } else {
            0.0
        };

        Ok(TraversalResult {
            paths,
            visited_nodes: visited,
            execution_time_ms: 0, // Set by caller
            used_gpu: false,
            stats: TraversalStats {
                nodes_explored,
                edges_traversed,
                paths_found,
                avg_path_length,
                gpu_utilization: None,
            },
        })
    }

    /// Compute PageRank scores for all nodes in the graph
    /// PageRank is a link analysis algorithm that assigns importance scores to nodes
    pub async fn pagerank(
        &self,
        graph: &GraphData,
        damping_factor: f32,
        max_iterations: usize,
        convergence_threshold: f32,
    ) -> Result<PageRankResult, ComputeError> {
        let start_time = std::time::Instant::now();

        // PageRank benefits from GPU parallelization
        if self.config.use_gpu && self.should_use_gpu(graph) {
            info!("Using CPU PageRank (GPU version not yet implemented)");
            self.pagerank_cpu(graph, damping_factor, max_iterations, convergence_threshold).await
        } else {
            info!("Using CPU PageRank");
            self.pagerank_cpu(graph, damping_factor, max_iterations, convergence_threshold).await
        }
        .map(|mut result| {
            result.execution_time_ms = start_time.elapsed().as_millis() as u64;
            result
        })
    }

    /// CPU-based PageRank using power iteration method
    async fn pagerank_cpu(
        &self,
        graph: &GraphData,
        damping_factor: f32,
        max_iterations: usize,
        convergence_threshold: f32,
    ) -> Result<PageRankResult, ComputeError> {
        use rayon::prelude::*;

        let n = graph.node_count;
        let initial_rank = 1.0 / n as f32;

        // Initialize ranks
        let mut ranks = vec![initial_rank; n];
        let mut new_ranks = vec![0.0f32; n];

        // Calculate out-degrees for each node
        let out_degrees: Vec<usize> = graph.adjacency_list.iter()
            .map(|neighbors| neighbors.len().max(1)) // Avoid division by zero
            .collect();

        let mut iterations = 0;
        let mut delta = f32::MAX;

        // Power iteration
        while iterations < max_iterations && delta > convergence_threshold {
            // Reset new ranks to teleportation probability
            let teleport_prob = (1.0 - damping_factor) / n as f32;
            new_ranks.fill(teleport_prob);

            // Parallel computation of rank contributions
            if n > 1000 {
                // Use parallel iteration for large graphs
                new_ranks.par_iter_mut()
                    .enumerate()
                    .for_each(|(target_idx, new_rank)| {
                        // Sum contributions from all nodes pointing to this target
                        for (source_idx, neighbors) in graph.adjacency_list.iter().enumerate() {
                            if neighbors.contains(&(target_idx as u32)) {
                                let contribution = ranks[source_idx] * damping_factor / out_degrees[source_idx] as f32;
                                *new_rank += contribution;
                            }
                        }
                    });
            } else {
                // Sequential for small graphs
                for target_idx in 0..n {
                    for (source_idx, neighbors) in graph.adjacency_list.iter().enumerate() {
                        if neighbors.contains(&(target_idx as u32)) {
                            let contribution = ranks[source_idx] * damping_factor / out_degrees[source_idx] as f32;
                            new_ranks[target_idx] += contribution;
                        }
                    }
                }
            }

            // Calculate convergence delta (L1 norm)
            delta = ranks.iter()
                .zip(new_ranks.iter())
                .map(|(old, new)| (old - new).abs())
                .sum();

            // Swap ranks
            std::mem::swap(&mut ranks, &mut new_ranks);
            iterations += 1;
        }

        // Build result map
        let scores: HashMap<u64, f32> = graph.node_ids.iter()
            .enumerate()
            .map(|(idx, &node_id)| (node_id, ranks[idx]))
            .collect();

        Ok(PageRankResult {
            scores,
            iterations,
            execution_time_ms: 0, // Set by caller
            used_gpu: false,
            convergence_delta: delta,
        })
    }

    /// Detect communities using connected components (GPU-accelerated)
    pub async fn detect_communities(
        &self,
        graph: &GraphData,
        min_community_size: usize,
    ) -> Result<Vec<Vec<u64>>, ComputeError> {
        if self.config.use_gpu && self.should_use_gpu(graph) {
            info!("Using GPU-accelerated community detection");
            self.detect_communities_gpu(graph, min_community_size).await
        } else {
            info!("Using CPU community detection");
            self.detect_communities_cpu(graph, min_community_size).await
        }
    }

    /// GPU-accelerated community detection
    async fn detect_communities_gpu(
        &self,
        graph: &GraphData,
        min_community_size: usize,
    ) -> Result<Vec<Vec<u64>>, ComputeError> {
        // TODO: Implement GPU kernel for connected components
        // For now, prepare GPU graph format (even though we fall back to CPU)
        let __gpu_graph = self.prepare_gpu_graph(graph)?;
        warn!("GPU community detection kernel not yet implemented, using optimized CPU");
        self.detect_communities_cpu_parallel(graph, min_community_size).await
    }

    /// CPU-based community detection with parallelization
    async fn detect_communities_cpu_parallel(
        &self,
        graph: &GraphData,
        min_community_size: usize,
    ) -> Result<Vec<Vec<u64>>, ComputeError> {
        use rayon::prelude::*;
        
        let mut visited = HashSet::new();
        let mut communities = Vec::new();
        
        // Parallel BFS for each unvisited node
        for &node_id in &graph.node_ids {
            if visited.contains(&node_id) {
                continue;
            }
            
            // BFS to find connected component
            let mut community = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(node_id);
            visited.insert(node_id);
            
            while let Some(current) = queue.pop_front() {
                community.push(current);
                
                if let Some(neighbors) = graph.adjacency_list.get(current as usize) {
                    let unvisited_neighbors: Vec<_> = if neighbors.len() > 1000 {
                        // Parallel for large neighbor lists
                        neighbors
                            .par_iter()
                            .filter_map(|&neighbor_idx| {
                                let neighbor_id = graph.node_ids.get(neighbor_idx as usize)?;
                                if !visited.contains(neighbor_id) {
                                    Some(*neighbor_id)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    } else {
                        // Sequential for small lists
                        neighbors
                            .iter()
                            .filter_map(|&neighbor_idx| {
                                let neighbor_id = graph.node_ids.get(neighbor_idx as usize)?;
                                if !visited.contains(neighbor_id) {
                                    Some(*neighbor_id)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    };

                    for neighbor_id in unvisited_neighbors {
                        visited.insert(neighbor_id);
                        queue.push_back(neighbor_id);
                    }
                }
            }
            
            if community.len() >= min_community_size {
                communities.push(community);
            }
        }
        
        Ok(communities)
    }

    /// CPU-based community detection (fallback)
    async fn detect_communities_cpu(
        &self,
        graph: &GraphData,
        min_community_size: usize,
    ) -> Result<Vec<Vec<u64>>, ComputeError> {
        self.detect_communities_cpu_parallel(graph, min_community_size).await
    }

    /// Check if GPU should be used for this graph
    fn should_use_gpu(&self, graph: &GraphData) -> bool {
        // Use GPU for large graphs where parallelization benefits outweigh overhead
        graph.node_count > 1000 && graph.edge_count > 5000
    }

    /// Prepare graph data for GPU processing
    pub(crate) fn prepare_gpu_graph(&self, graph: &GraphData) -> Result<GPUGraphData, ComputeError> {
        // Convert to flat arrays for GPU processing
        let mut edge_array = Vec::with_capacity(graph.edge_count);
        let mut edge_offset = Vec::with_capacity(graph.node_count + 1);
        
        let mut offset = 0;
        for neighbors in graph.adjacency_list.iter() {
            edge_offset.push(offset);
            offset += neighbors.len() as u32;
            
            for &neighbor_idx in neighbors {
                edge_array.push(neighbor_idx);
            }
        }
        edge_offset.push(offset);

        Ok(GPUGraphData {
            _node_count: graph.node_count as u32,
            _edge_count: graph.edge_count as u32,
            edge_array,
            edge_offset,
        })
    }

    /// Estimate memory requirements for GPU processing
    fn _estimate_memory_requirements(&self, graph: &GraphData) -> u64 {
        // Rough estimate: nodes + edges + temporary buffers
        let node_memory = graph.node_count * 8; // u64 per node
        let edge_memory = graph.edge_count * 4; // u32 per edge
        let temp_memory = graph.node_count * 16; // visited flags, distances, etc.
        
        (node_memory + edge_memory + temp_memory) as u64
    }
}

/// GPU-friendly graph representation (flat arrays)
#[derive(Debug, Clone)]
pub(crate) struct GPUGraphData {
    _node_count: u32,
    _edge_count: u32,
    /// Flat array of edge destinations
    #[allow(dead_code)] // Used in Metal GPU backend (conditional compilation)
    pub(crate) edge_array: Vec<u32>,
    /// Offset array: edge_offset[i] is the start index in edge_array for node i
    #[allow(dead_code)] // Used in Metal GPU backend (conditional compilation)
    pub(crate) edge_offset: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bfs_small_graph() {
        let config = TraversalConfig {
            max_depth: 3,
            max_paths: 10,
            use_gpu: false, // Use CPU for small graph
            ..Default::default()
        };
        
        let traversal = GPUGraphTraversal::new(config).await.unwrap();
        
        // Create a simple graph: 0 -> 1 -> 2 -> 3
        let graph = GraphData {
            node_ids: vec![0, 1, 2, 3],
            adjacency_list: vec![
                vec![1],    // 0 -> 1
                vec![2],    // 1 -> 2
                vec![3],    // 2 -> 3
                vec![],     // 3 -> (none)
            ],
            edge_weights: None,
            node_properties: HashMap::new(),
            node_count: 4,
            edge_count: 3,
        };
        
        let result = traversal.bfs(&graph, 0, Some(3)).await.unwrap();
        
        assert!(!result.paths.is_empty());
        assert_eq!(result.paths[0].nodes, vec![0, 1, 2, 3]);
    }

    #[tokio::test]
    async fn test_community_detection() {
        let config = TraversalConfig::default();
        let traversal = GPUGraphTraversal::new(config).await.unwrap();

        // Create graph with two disconnected components
        let graph = GraphData {
            node_ids: vec![0, 1, 2, 3, 4, 5],
            adjacency_list: vec![
                vec![1],    // Component 1: 0-1-2
                vec![0, 2],
                vec![1],
                vec![4],    // Component 2: 3-4-5
                vec![3, 5],
                vec![4],
            ],
            edge_weights: None,
            node_properties: HashMap::new(),
            node_count: 6,
            edge_count: 6,
        };

        let communities = traversal.detect_communities(&graph, 2).await.unwrap();

        assert_eq!(communities.len(), 2);
        assert!(communities.iter().any(|c| c.len() == 3));
    }

    #[tokio::test]
    async fn test_dijkstra_weighted_graph() {
        let config = TraversalConfig {
            max_depth: 5,
            max_paths: 10,
            use_gpu: false, // Use CPU for test
            ..Default::default()
        };

        let traversal = GPUGraphTraversal::new(config).await.unwrap();

        // Create weighted graph: 0 --(1.0)-> 1 --(2.0)-> 2 --(3.0)-> 3
        //                         0 --(10.0)-> 3 (expensive direct path)
        let graph = GraphData {
            node_ids: vec![0, 1, 2, 3],
            adjacency_list: vec![
                vec![1, 3],    // 0 -> 1, 0 -> 3
                vec![2],       // 1 -> 2
                vec![3],       // 2 -> 3
                vec![],        // 3 -> (none)
            ],
            edge_weights: Some(vec![1.0, 10.0, 2.0, 3.0]),
            node_properties: HashMap::new(),
            node_count: 4,
            edge_count: 4,
        };

        let result = traversal.dijkstra(&graph, 0, Some(3)).await.unwrap();

        assert!(!result.paths.is_empty());
        assert_eq!(result.paths[0].nodes, vec![0, 1, 2, 3]);
        assert_eq!(result.paths[0].weight, 6.0); // 1.0 + 2.0 + 3.0
    }

    #[tokio::test]
    async fn test_dfs_graph() {
        let config = TraversalConfig {
            max_depth: 5,
            max_paths: 10,
            use_gpu: false,
            ..Default::default()
        };

        let traversal = GPUGraphTraversal::new(config).await.unwrap();

        // Create a simple graph: 0 -> 1 -> 2 -> 3
        let graph = GraphData {
            node_ids: vec![0, 1, 2, 3],
            adjacency_list: vec![
                vec![1],    // 0 -> 1
                vec![2],    // 1 -> 2
                vec![3],    // 2 -> 3
                vec![],     // 3 -> (none)
            ],
            edge_weights: None,
            node_properties: HashMap::new(),
            node_count: 4,
            edge_count: 3,
        };

        let result = traversal.dfs(&graph, 0, Some(3)).await.unwrap();

        assert!(!result.paths.is_empty());
        assert_eq!(result.paths[0].nodes, vec![0, 1, 2, 3]);
        assert!(!result.used_gpu); // DFS uses CPU
    }

    #[tokio::test]
    async fn test_pagerank() {
        let config = TraversalConfig::default();
        let traversal = GPUGraphTraversal::new(config).await.unwrap();

        // Create a simple graph where node 1 has highest importance
        // 0 -> 1, 2 -> 1, 3 -> 1 (node 1 has 3 incoming edges)
        let graph = GraphData {
            node_ids: vec![0, 1, 2, 3],
            adjacency_list: vec![
                vec![1],    // 0 -> 1
                vec![],     // 1 -> (none, but receives from 0, 2, 3)
                vec![1],    // 2 -> 1
                vec![1],    // 3 -> 1
            ],
            edge_weights: None,
            node_properties: HashMap::new(),
            node_count: 4,
            edge_count: 3,
        };

        let result = traversal.pagerank(&graph, 0.85, 100, 0.0001).await.unwrap();

        // Node 1 should have highest PageRank (it has most incoming edges)
        let rank_1 = result.scores.get(&1).unwrap();
        let rank_0 = result.scores.get(&0).unwrap();
        let rank_2 = result.scores.get(&2).unwrap();
        let rank_3 = result.scores.get(&3).unwrap();

        // Node 1 should have highest rank
        assert!(rank_1 > rank_0);
        assert!(rank_1 > rank_2);
        assert!(rank_1 > rank_3);

        // Verify convergence happened
        assert!(result.iterations > 0);
        assert!(result.convergence_delta < 0.01);
    }
}

