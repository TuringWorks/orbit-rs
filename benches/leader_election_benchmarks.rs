use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use orbit_shared::consensus::{RaftConsensus, RaftConfig};
use orbit_shared::cluster_manager::{EnhancedClusterManager, QuorumConfig};
use orbit_shared::election_state::{ElectionStateManager, ElectionRecord};
use orbit_shared::election_metrics::{ElectionMetrics, ElectionOutcome};
use orbit_shared::{NodeId, OrbitResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tokio::runtime::Runtime;

/// Benchmark configuration for different scenarios
#[derive(Debug, Clone)]
struct BenchConfig {
    node_count: usize,
    election_timeout_min: Duration,
    election_timeout_max: Duration,
    heartbeat_interval: Duration,
}

impl BenchConfig {
    fn fast() -> Self {
        Self {
            node_count: 3,
            election_timeout_min: Duration::from_millis(50),
            election_timeout_max: Duration::from_millis(100),
            heartbeat_interval: Duration::from_millis(25),
        }
    }
    
    fn standard() -> Self {
        Self {
            node_count: 5,
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
        }
    }
    
    fn large_cluster() -> Self {
        Self {
            node_count: 10,
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
        }
    }
}

/// Setup test cluster for benchmarking
async fn setup_test_cluster(config: &BenchConfig) -> OrbitResult<Vec<Arc<RaftConsensus>>> {
    let mut node_ids = Vec::new();
    for i in 0..config.node_count {
        node_ids.push(NodeId::new(format!("bench-node-{}", i), "benchmark".to_string()));
    }
    
    let raft_config = RaftConfig {
        election_timeout_min: config.election_timeout_min,
        election_timeout_max: config.election_timeout_max,
        heartbeat_interval: config.heartbeat_interval,
        ..Default::default()
    };
    
    let mut consensus_nodes = Vec::new();
    for node_id in &node_ids {
        let consensus = Arc::new(RaftConsensus::new(
            node_id.clone(),
            node_ids.clone(),
            raft_config.clone(),
        ));
        consensus_nodes.push(consensus);
    }
    
    Ok(consensus_nodes)
}

/// Benchmark basic election creation and initialization
fn bench_election_initialization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("election_initialization");
    
    for &node_count in &[3, 5, 7, 10, 15] {
        let config = BenchConfig {
            node_count,
            ..BenchConfig::standard()
        };
        
        group.bench_with_input(
            BenchmarkId::new("raft_consensus_creation", node_count),
            &config,
            |b, config| {
                b.iter(|| {
                    rt.block_on(async {
                        let cluster = setup_test_cluster(config).await.unwrap();
                        black_box(cluster);
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark state persistence operations
fn bench_state_persistence(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("state_persistence");
    group.throughput(Throughput::Elements(1));
    
    // Benchmark state saving
    group.bench_function("save_election_state", |b| {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("bench_state.json");
        let node_id = NodeId::new("bench-node".to_string(), "bench".to_string());
        
        b.iter(|| {
            rt.block_on(async {
                let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
                state_manager.update_term(fastrand::u64(1..1000)).await.unwrap();
                state_manager.vote_for(node_id.clone(), 1).await.unwrap();
                black_box(state_manager);
            });
        });
    });
    
    // Benchmark state loading
    group.bench_function("load_election_state", |b| {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("bench_state.json");
        let node_id = NodeId::new("bench-node".to_string(), "bench".to_string());
        
        // Create initial state
        rt.block_on(async {
            let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
            state_manager.update_term(42).await.unwrap();
        });
        
        b.iter(|| {
            rt.block_on(async {
                let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
                state_manager.load().await.unwrap();
                let term = state_manager.get_current_term().await;
                black_box(term);
            });
        });
    });
    
    // Benchmark election record insertion
    group.bench_function("record_election", |b| {
        let temp_dir = tempdir().unwrap();
        let state_path = temp_dir.path().join("bench_state.json");
        let node_id = NodeId::new("bench-node".to_string(), "bench".to_string());
        
        b.iter(|| {
            rt.block_on(async {
                let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
                
                let record = ElectionRecord {
                    term: fastrand::u64(1..1000),
                    candidate: node_id.clone(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    successful: fastrand::bool(),
                    vote_count: fastrand::u32(1..10),
                    total_nodes: fastrand::u32(5..20),
                };
                
                state_manager.record_election(record).await.unwrap();
                black_box(state_manager);
            });
        });
    });
    
    group.finish();
}

/// Benchmark metrics collection performance
fn bench_metrics_collection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("metrics_collection");
    group.throughput(Throughput::Elements(1));
    
    // Benchmark election tracking
    group.bench_function("election_tracking", |b| {
        b.iter(|| {
            rt.block_on(async {
                let metrics = ElectionMetrics::new();
                let tracker = metrics.start_election(fastrand::u64(1..1000)).await;
                
                // Simulate some metrics collection
                let node_id = NodeId::new("bench-node".to_string(), "bench".to_string());
                metrics.record_vote_latency(&node_id, Duration::from_millis(fastrand::u64(1..100))).await;
                metrics.record_message_loss(&node_id, 10, 9).await;
                metrics.record_performance(fastrand::f64() * 100.0, fastrand::u64(1024..1024*1024)).await;
                
                tracker.complete(
                    if fastrand::bool() { ElectionOutcome::Won } else { ElectionOutcome::Lost },
                    fastrand::u32(1..10),
                    fastrand::u32(5..20)
                ).await;
                
                black_box(metrics);
            });
        });
    });
    
    // Benchmark metrics summary generation
    group.bench_function("metrics_summary", |b| {
        let metrics = rt.block_on(async {
            let metrics = ElectionMetrics::new();
            let node_id = NodeId::new("bench-node".to_string(), "bench".to_string());
            
            // Pre-populate with some data
            for _ in 0..100 {
                let tracker = metrics.start_election(fastrand::u64(1..1000)).await;
                metrics.record_vote_latency(&node_id, Duration::from_millis(fastrand::u64(1..100))).await;
                metrics.record_message_loss(&node_id, 10, fastrand::u64(8..10)).await;
                tracker.complete(ElectionOutcome::Won, 3, 5).await;
            }
            
            metrics
        });
        
        b.iter(|| {
            rt.block_on(async {
                let summary = metrics.get_summary().await;
                black_box(summary);
            });
        });
    });
    
    // Benchmark Prometheus export
    group.bench_function("prometheus_export", |b| {
        let metrics = rt.block_on(async {
            let metrics = ElectionMetrics::new();
            let node_id = NodeId::new("bench-node".to_string(), "bench".to_string());
            
            // Pre-populate with data
            for _ in 0..50 {
                let tracker = metrics.start_election(fastrand::u64(1..1000)).await;
                metrics.record_vote_latency(&node_id, Duration::from_millis(fastrand::u64(1..100))).await;
                tracker.complete(ElectionOutcome::Won, 3, 5).await;
            }
            
            metrics
        });
        
        b.iter(|| {
            rt.block_on(async {
                let prometheus_output = metrics.export_prometheus().await;
                black_box(prometheus_output);
            });
        });
    });
    
    group.finish();
}

/// Benchmark cluster manager operations
fn bench_cluster_management(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("cluster_management");
    
    for &node_count in &[3, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("cluster_health_check", node_count),
            &node_count,
            |b, &node_count| {
                let cluster_manager = rt.block_on(async {
                    let mut node_ids = Vec::new();
                    for i in 0..node_count {
                        node_ids.push(NodeId::new(format!("node-{}", i), "bench".to_string()));
                    }
                    
                    let quorum_config = QuorumConfig {
                        min_quorum_size: node_count / 2 + 1,
                        max_failures: node_count / 2,
                        quorum_timeout: Duration::from_secs(10),
                        dynamic_quorum: false,
                    };
                    
                    let raft_config = RaftConfig::default();
                    
                    Arc::new(EnhancedClusterManager::new(
                        node_ids[0].clone(),
                        node_ids,
                        quorum_config,
                        raft_config,
                    ))
                });
                
                b.iter(|| {
                    rt.block_on(async {
                        let has_quorum = cluster_manager.has_quorum().await.unwrap_or(false);
                        let nodes = cluster_manager.get_partition_nodes().await.unwrap_or_default();
                        black_box((has_quorum, nodes));
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_operations");
    group.throughput(Throughput::Elements(10));
    
    // Benchmark concurrent state operations
    group.bench_function("concurrent_state_operations", |b| {
        b.iter(|| {
            rt.block_on(async {
                let temp_dir = tempdir().unwrap();
                let state_path = temp_dir.path().join("bench_concurrent.json");
                let node_id = NodeId::new("concurrent-node".to_string(), "bench".to_string());
                let state_manager = Arc::new(ElectionStateManager::new(&state_path, node_id.clone()));
                
                let mut tasks = Vec::new();
                
                for i in 0..10 {
                    let state_manager = Arc::clone(&state_manager);
                    let node_id = node_id.clone();
                    
                    let task = tokio::spawn(async move {
                        let term = i as u64 + 1;
                        state_manager.update_term(term).await.unwrap();
                        
                        let record = ElectionRecord {
                            term,
                            candidate: node_id,
                            timestamp: chrono::Utc::now().timestamp_millis(),
                            successful: i % 2 == 0,
                            vote_count: 3,
                            total_nodes: 5,
                        };
                        
                        state_manager.record_election(record).await.unwrap();
                    });
                    
                    tasks.push(task);
                }
                
                for task in tasks {
                    task.await.unwrap();
                }
                
                black_box(state_manager);
            });
        });
    });
    
    // Benchmark concurrent metrics collection
    group.bench_function("concurrent_metrics_collection", |b| {
        b.iter(|| {
            rt.block_on(async {
                let metrics = Arc::new(ElectionMetrics::new());
                let mut tasks = Vec::new();
                
                for i in 0..10 {
                    let metrics = Arc::clone(&metrics);
                    
                    let task = tokio::spawn(async move {
                        let node_id = NodeId::new(format!("node-{}", i), "bench".to_string());
                        let tracker = metrics.start_election(i as u64).await;
                        
                        metrics.record_vote_latency(&node_id, Duration::from_millis(50)).await;
                        metrics.record_message_loss(&node_id, 10, 9).await;
                        
                        tracker.complete(ElectionOutcome::Won, 3, 5).await;
                    });
                    
                    tasks.push(task);
                }
                
                for task in tasks {
                    task.await.unwrap();
                }
                
                black_box(metrics);
            });
        });
    });
    
    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("memory_usage");
    
    // Benchmark memory usage with growing election history
    for &election_count in &[10, 100, 1000, 5000] {
        group.bench_with_input(
            BenchmarkId::new("election_history_memory", election_count),
            &election_count,
            |b, &election_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let temp_dir = tempdir().unwrap();
                        let state_path = temp_dir.path().join("memory_bench.json");
                        let node_id = NodeId::new("memory-test".to_string(), "bench".to_string());
                        let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
                        
                        for i in 0..election_count {
                            let record = ElectionRecord {
                                term: i as u64,
                                candidate: node_id.clone(),
                                timestamp: chrono::Utc::now().timestamp_millis(),
                                successful: i % 3 == 0,
                                vote_count: 3,
                                total_nodes: 5,
                            };
                            
                            state_manager.record_election(record).await.unwrap();
                        }
                        
                        let stats = state_manager.get_stats().await;
                        black_box(stats);
                    });
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark network simulation operations
fn bench_network_simulation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("network_simulation");
    
    // Benchmark partition detection
    group.bench_function("partition_detection", |b| {
        let metrics = Arc::new(ElectionMetrics::new());
        
        b.iter(|| {
            rt.block_on(async {
                let nodes: Vec<NodeId> = (0..10)
                    .map(|i| NodeId::new(format!("partition-node-{}", i), "bench".to_string()))
                    .collect();
                
                // Simulate partition events
                for partition_size in [2, 3, 5] {
                    let affected_nodes = nodes.iter()
                        .take(partition_size)
                        .cloned()
                        .collect();
                    
                    metrics.record_partition(affected_nodes).await;
                }
                
                black_box(&metrics);
            });
        });
    });
    
    group.finish();
}

/// Comprehensive benchmark suite
fn bench_end_to_end_scenarios(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("end_to_end_scenarios");
    group.sample_size(10); // Reduce sample size for long-running tests
    
    // Benchmark complete election cycle
    group.bench_function("complete_election_cycle", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = BenchConfig::fast();
                let cluster = setup_test_cluster(&config).await.unwrap();
                
                let temp_dir = tempdir().unwrap();
                let mut state_managers = Vec::new();
                let mut metrics = Vec::new();
                
                for (i, consensus) in cluster.iter().enumerate() {
                    let state_path = temp_dir.path().join(format!("node_{}.json", i));
                    let node_id = NodeId::new(format!("node-{}", i), "bench".to_string());
                    
                    let state_manager = ElectionStateManager::new(&state_path, node_id.clone());
                    let metric = ElectionMetrics::new();
                    
                    state_manager.load().await.unwrap();
                    
                    state_managers.push(state_manager);
                    metrics.push(metric);
                }
                
                // Simulate election process
                let start = Instant::now();
                
                // Record some election activity
                for (i, metric) in metrics.iter().enumerate() {
                    let tracker = metric.start_election(1).await;
                    
                    let node_id = NodeId::new(format!("node-{}", i), "bench".to_string());
                    metric.record_vote_latency(&node_id, Duration::from_millis(25)).await;
                    
                    tracker.complete(
                        if i == 0 { ElectionOutcome::Won } else { ElectionOutcome::Lost },
                        if i == 0 { 3 } else { 0 },
                        config.node_count as u32
                    ).await;
                }
                
                let elapsed = start.elapsed();
                black_box((cluster, state_managers, metrics, elapsed));
            });
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_election_initialization,
    bench_state_persistence,
    bench_metrics_collection,
    bench_cluster_management,
    bench_concurrent_operations,
    bench_memory_usage,
    bench_network_simulation,
    bench_end_to_end_scenarios
);

criterion_main!(benches);