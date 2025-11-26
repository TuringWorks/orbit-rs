//! Performance benchmarks for GPU-accelerated graph traversal
//!
//! This benchmark suite compares:
//! - CPU sequential BFS
//! - CPU parallel BFS (Rayon)
//! - GPU-accelerated BFS (when available)
//!
//! Run with: `cargo bench --package orbit-compute --features graph-traversal`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orbit_compute::graph_traversal::{
    GPUGraphTraversal, GraphData, NodeProperties, TraversalConfig,
};
use std::collections::HashMap;
use std::time::Instant;

/// Generate a synthetic graph for benchmarking
fn generate_graph(node_count: usize, edges_per_node: usize) -> GraphData {
    let mut node_ids: Vec<u64> = (0..node_count as u64).collect();
    let mut adjacency_list: Vec<Vec<u32>> = vec![Vec::new(); node_count];
    let mut node_properties: HashMap<u64, NodeProperties> = HashMap::new();

    // Create a regular graph structure
    for i in 0..node_count {
        let node_id = i as u64;
        node_properties.insert(
            node_id,
            NodeProperties {
                importance: 1.0,
                node_type: None,
                metadata: HashMap::new(),
            },
        );

        // Connect to next nodes in a ring, plus random connections
        for j in 0..edges_per_node {
            let neighbor_idx = (i + j + 1) % node_count;
            adjacency_list[i].push(neighbor_idx as u32);
        }
    }

    let edge_count = node_count * edges_per_node;

    GraphData {
        node_ids,
        adjacency_list,
        edge_weights: None,
        node_properties,
        node_count,
        edge_count,
    }
}

/// Benchmark BFS traversal
fn benchmark_bfs(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_bfs");

    // Test different graph sizes
    let graph_sizes = vec![
        (100, 5),    // Small: 100 nodes, 5 edges/node
        (1000, 10),  // Medium: 1000 nodes, 10 edges/node
        (10000, 10), // Large: 10K nodes, 10 edges/node
        (50000, 10), // Very Large: 50K nodes, 10 edges/node
    ];

    for (node_count, edges_per_node) in graph_sizes {
        let graph = generate_graph(node_count, edges_per_node);
        let source = 0u64;
        let target = Some((node_count / 2) as u64);

        // Benchmark CPU sequential (if we had a sequential implementation)
        // For now, we'll benchmark CPU parallel as baseline

        // Benchmark CPU parallel
        group.bench_with_input(
            BenchmarkId::new("cpu_parallel", format!("{}_nodes", node_count)),
            &graph,
            |b, graph| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.to_async(&rt).iter(|| async {
                    let config = TraversalConfig {
                        use_gpu: false,
                        max_depth: 10,
                        max_paths: 100,
                        gpu_min_nodes: 10000,
                        gpu_min_edges: 50000,
                    };
                    let traversal = GPUGraphTraversal::new(config).await.unwrap();
                    black_box(traversal.bfs(graph, source, target).await.unwrap())
                });
            },
        );

        // Benchmark GPU (if available)
        #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
        {
            group.bench_with_input(
                BenchmarkId::new("gpu_metal", format!("{}_nodes", node_count)),
                &graph,
                |b, graph| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    b.to_async(&rt).iter(|| async {
                        let config = TraversalConfig {
                            use_gpu: true,
                            max_depth: 10,
                            max_paths: 100,
                            gpu_min_nodes: 1000,
                            gpu_min_edges: 5000,
                        };
                        let traversal = GPUGraphTraversal::new(config).await.unwrap();
                        black_box(traversal.bfs(graph, source, target).await.unwrap())
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark community detection
fn benchmark_community_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("community_detection");

    let graph_sizes = vec![(1000, 5), (10000, 10), (50000, 10)];

    for (node_count, edges_per_node) in graph_sizes {
        let graph = generate_graph(node_count, edges_per_node);
        let min_community_size = 10;

        // Benchmark CPU parallel
        group.bench_with_input(
            BenchmarkId::new("cpu_parallel", format!("{}_nodes", node_count)),
            &graph,
            |b, graph| {
                let rt = tokio::runtime::Runtime::new().unwrap();
                b.to_async(&rt).iter(|| async {
                    let config = TraversalConfig {
                        use_gpu: false,
                        max_depth: 10,
                        max_paths: 100,
                        gpu_min_nodes: 10000,
                        gpu_min_edges: 50000,
                    };
                    let traversal = GPUGraphTraversal::new(config).await.unwrap();
                    black_box(
                        traversal
                            .detect_communities(graph, min_community_size)
                            .await
                            .unwrap(),
                    )
                });
            },
        );

        // Benchmark GPU (if available)
        #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
        {
            group.bench_with_input(
                BenchmarkId::new("gpu_metal", format!("{}_nodes", node_count)),
                &graph,
                |b, graph| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    b.to_async(&rt).iter(|| async {
                        let config = TraversalConfig {
                            use_gpu: true,
                            max_depth: 10,
                            max_paths: 100,
                            gpu_min_nodes: 1000,
                            gpu_min_edges: 5000,
                        };
                        let traversal = GPUGraphTraversal::new(config).await.unwrap();
                        black_box(
                            traversal
                                .detect_communities(graph, min_community_size)
                                .await
                                .unwrap(),
                        )
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark graph conversion (CPU overhead)
fn benchmark_graph_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_conversion");

    let graph_sizes = vec![(1000, 10), (10000, 10), (50000, 10)];

    for (node_count, edges_per_node) in graph_sizes {
        let graph = generate_graph(node_count, edges_per_node);

        group.bench_with_input(
            BenchmarkId::new("prepare_gpu_format", format!("{}_nodes", node_count)),
            &graph,
            |b, graph| {
                let config = TraversalConfig::default();
                let rt = tokio::runtime::Runtime::new().unwrap();
                let traversal = rt.block_on(GPUGraphTraversal::new(config)).unwrap();

                b.iter(|| black_box(traversal.prepare_gpu_graph(graph).unwrap()));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_bfs,
    benchmark_community_detection,
    benchmark_graph_conversion
);
criterion_main!(benches);
