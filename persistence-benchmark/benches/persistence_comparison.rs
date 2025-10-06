use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use persistence_benchmark::*;
use persistence_benchmark::cow_btree::CowBTreePersistence;
use persistence_benchmark::rocksdb_impl::RocksDBPersistence;
use persistence_benchmark::workload::{WorkloadSimulator, CrashSimulator};
use persistence_benchmark::metrics::{BenchmarkResults, MetricsAnalyzer, ComparativeAnalysis};
use std::time::Duration;

async fn run_comprehensive_benchmark<P: PersistenceProvider>(
    provider: &P,
    implementation_name: &str,
) -> BenchmarkResults {
    // Test different workload patterns
    println!("Running actor lease workload for {}...", implementation_name);
    let actor_workload = WorkloadSimulator::new_actor_lease_workload();
    let actor_results = actor_workload
        .run(provider, Duration::from_secs(30))
        .await
        .expect("Actor workload failed");
    let actor_stats = actor_results.calculate_stats();
    
    println!("Running high throughput workload for {}...", implementation_name);
    let throughput_workload = WorkloadSimulator::new_high_throughput_workload();
    let throughput_results = throughput_workload
        .run(provider, Duration::from_secs(10))
        .await
        .expect("Throughput workload failed");
    let throughput_stats = throughput_results.calculate_stats();
    
    println!("Running query heavy workload for {}...", implementation_name);
    let query_workload = WorkloadSimulator::new_query_heavy_workload();
    let query_results = query_workload
        .run(provider, Duration::from_secs(20))
        .await
        .expect("Query workload failed");
    let query_stats = query_results.calculate_stats();
    
    println!("Running crash recovery test for {}...", implementation_name);
    let crash_results = CrashSimulator::simulate_crash_recovery(provider, 1000)
        .await
        .expect("Crash recovery failed");
    
    // Calculate scores
    let memory_efficiency_score = MetricsAnalyzer::memory_efficiency_score(
        actor_stats.peak_memory_usage,
        actor_stats.total_operations,
    );
    
    let overall_score = MetricsAnalyzer::calculate_overall_score(&actor_stats, &crash_results);
    
    BenchmarkResults {
        implementation_name: implementation_name.to_string(),
        actor_lease_workload: actor_stats,
        high_throughput_workload: throughput_stats,
        query_heavy_workload: query_stats,
        crash_recovery_results: crash_results,
        memory_efficiency_score,
        overall_score,
    }
}

fn benchmark_cow_btree_vs_rocksdb(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // Set up temporary directories
    let cow_temp_dir = tempfile::tempdir().unwrap();
    let rocksdb_temp_dir = tempfile::tempdir().unwrap();
    
    let mut group = c.benchmark_group("persistence_comparison");
    group.sample_size(10); // Fewer samples for comprehensive benchmarks
    group.measurement_time(Duration::from_secs(60)); // Longer measurement time
    
    // Individual operation benchmarks
    group.bench_function("cow_btree_single_write", |b| {
        b.to_async(&rt).iter(|| async {
            let persistence = CowBTreePersistence::new(cow_temp_dir.path())
                .await
                .expect("Failed to create COW B+ Tree");
            
            let lease = ActorLease::new(
                uuid::Uuid::new_v4(),
                "bench_actor".to_string(),
                "node1".to_string(),
                Duration::from_secs(300),
            );
            
            persistence.store_lease(&lease).await.unwrap()
        });
    });
    
    group.bench_function("rocksdb_single_write", |b| {
        b.to_async(&rt).iter(|| async {
            let persistence = RocksDBPersistence::new(rocksdb_temp_dir.path())
                .await
                .expect("Failed to create RocksDB");
            
            let lease = ActorLease::new(
                uuid::Uuid::new_v4(),
                "bench_actor".to_string(),
                "node1".to_string(),
                Duration::from_secs(300),
            );
            
            persistence.store_lease(&lease).await.unwrap()
        });
    });
    
    // Read benchmarks with pre-populated data
    group.bench_function("cow_btree_single_read", |b| {
        b.to_async(&rt).iter(|| async {
            let persistence = CowBTreePersistence::new(cow_temp_dir.path())
                .await
                .expect("Failed to create COW B+ Tree");
            
            let lease = ActorLease::new(
                uuid::Uuid::new_v4(),
                "bench_actor".to_string(),
                "node1".to_string(),
                Duration::from_secs(300),
            );
            
            persistence.store_lease(&lease).await.unwrap();
            persistence.get_lease(&lease.key).await.unwrap()
        });
    });
    
    group.bench_function("rocksdb_single_read", |b| {
        b.to_async(&rt).iter(|| async {
            let persistence = RocksDBPersistence::new(rocksdb_temp_dir.path())
                .await
                .expect("Failed to create RocksDB");
            
            let lease = ActorLease::new(
                uuid::Uuid::new_v4(),
                "bench_actor".to_string(),
                "node1".to_string(),
                Duration::from_secs(300),
            );
            
            persistence.store_lease(&lease).await.unwrap();
            persistence.get_lease(&lease.key).await.unwrap()
        });
    });
    
    group.finish();
}

fn benchmark_comprehensive_workloads(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("comprehensive_workloads");
    group.sample_size(3); // Very few samples for comprehensive tests
    group.measurement_time(Duration::from_secs(120)); // Long measurement time
    
    // Run comprehensive benchmarks and collect results
    let results = rt.block_on(async {
        let mut all_results = Vec::new();
        
        // COW B+ Tree comprehensive benchmark
        let cow_temp_dir = tempfile::tempdir().unwrap();
        let cow_persistence = CowBTreePersistence::new(cow_temp_dir.path())
            .await
            .expect("Failed to create COW B+ Tree");
        
        let cow_results = run_comprehensive_benchmark(&cow_persistence, "COW B+ Tree").await;
        all_results.push(cow_results);
        
        // RocksDB comprehensive benchmark
        let rocksdb_temp_dir = tempfile::tempdir().unwrap();
        let rocksdb_persistence = RocksDBPersistence::new(rocksdb_temp_dir.path())
            .await
            .expect("Failed to create RocksDB");
        
        let rocksdb_results = run_comprehensive_benchmark(&rocksdb_persistence, "RocksDB").await;
        all_results.push(rocksdb_results);
        
        all_results
    });
    
    // Analyze results
    let analysis = MetricsAnalyzer::analyze(results);
    
    // Print comprehensive analysis
    println!("\n{}", "=".repeat(80));
    println!("COMPREHENSIVE PERSISTENCE BENCHMARK RESULTS");
    println!("{}", "=".repeat(80));
    println!("{}", analysis.summary);
    
    println!("## Detailed Results\n");
    for impl_result in &analysis.implementations {
        println!("### {}", impl_result.implementation_name);
        println!("- **Overall Score**: {:.1}/100", impl_result.overall_score);
        println!("- **Write Latency P95**: {:.1}μs", impl_result.actor_lease_workload.write_latency_p95.as_micros());
        println!("- **Read Latency P95**: {:.1}μs", impl_result.actor_lease_workload.read_latency_p95.as_micros());
        println!("- **Throughput**: {:.1} ops/s", impl_result.actor_lease_workload.throughput);
        println!("- **Peak Memory**: {:.2} MB", impl_result.actor_lease_workload.peak_memory_usage as f64 / 1024.0 / 1024.0);
        println!("- **Recovery Time**: {:.2}s", impl_result.crash_recovery_results.recovery_time.as_secs_f64());
        println!();
    }
    
    println!("## Recommendations\n");
    for recommendation in &analysis.recommendations {
        println!("**{}**: {} - {}", 
                recommendation.use_case,
                recommendation.recommended_implementation,
                recommendation.reasoning);
    }
    
    // Export results
    let export_dir = std::path::Path::new("benchmark_results");
    std::fs::create_dir_all(export_dir).unwrap();
    
    MetricsAnalyzer::export_to_json(&analysis, &export_dir.join("results.json"))
        .expect("Failed to export JSON");
    
    MetricsAnalyzer::export_to_csv(&analysis.implementations, &export_dir.join("results.csv"))
        .expect("Failed to export CSV");
    
    println!("\n📊 Results exported to:");
    println!("  - JSON: {}", export_dir.join("results.json").display());
    println!("  - CSV: {}", export_dir.join("results.csv").display());
    
    // Add benchmarks that use the analysis results
    group.bench_function("analysis_winner", |b| {
        b.iter(|| {
            // Benchmark the winner's performance characteristics
            let winner = &analysis.winner_by_metric.best_overall;
            winner.len() // Simple operation to benchmark
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_cow_btree_vs_rocksdb,
    benchmark_comprehensive_workloads
);
criterion_main!(benches);