//! Example demonstrating the orbit-shared benchmarking capabilities
//!
//! This example shows how to use the comprehensive benchmarking suite
//! to measure performance of critical operations.

use orbit_shared::benchmarks::{BenchmarkSuite, StandardBenchmarks};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for benchmark logging
    tracing_subscriber::fmt::init();

    println!("🚀 Orbit Benchmarking Demo");
    println!("=========================");

    // Create a new benchmark suite
    let suite = BenchmarkSuite::new();

    // Run standard benchmarks included in orbit-shared
    println!("\n📊 Running standard benchmark suite...");
    StandardBenchmarks::run_all(&suite).await?;

    // Custom benchmark for string operations
    println!("\n🔧 Running custom benchmarks...");

    suite
        .benchmark_sync("string_formatting", 1000, || {
            let data = vec!["hello", "world", "benchmark", "test"];
            let _result = format!("{} {} {} {}", data[0], data[1], data[2], data[3]);
            Ok(())
        })
        .await?;

    // Async benchmark example
    suite
        .benchmark("async_sleep", 10, || async {
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            Ok(())
        })
        .await?;

    // Generate performance report
    println!("\n📈 Performance Report");
    println!("=====================");
    let report = suite.generate_report().await;
    println!("{}", report);

    // Get specific benchmark results
    let results = suite.get_results().await;
    if let Some(result) = results.get("string_formatting") {
        println!("\n🎯 String Formatting Performance:");
        println!("   Mean time: {:?}", result.mean);
        println!("   Operations/sec: {:.2}", result.ops_per_sec);
        println!("   Samples: {}", result.samples.len());
    }

    println!("\n✅ Benchmark demo completed!");
    Ok(())
}
