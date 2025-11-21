use super::workload::{CrashRecoveryResults, WorkloadStats};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Comprehensive benchmark results for a single persistence implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub implementation_name: String,
    pub actor_lease_workload: WorkloadStats,
    pub high_throughput_workload: WorkloadStats,
    pub query_heavy_workload: WorkloadStats,
    pub crash_recovery_results: CrashRecoveryResults,
    pub memory_efficiency_score: f64,
    pub overall_score: f64,
}

/// Comparative analysis between multiple implementations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparativeAnalysis {
    pub implementations: Vec<BenchmarkResults>,
    pub winner_by_metric: WinnerByMetric,
    pub recommendations: Vec<Recommendation>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinnerByMetric {
    pub best_write_latency: String,
    pub best_read_latency: String,
    pub best_throughput: String,
    pub best_memory_efficiency: String,
    pub best_recovery_time: String,
    pub best_overall: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub use_case: String,
    pub recommended_implementation: String,
    pub reasoning: String,
}

pub struct MetricsAnalyzer;

impl MetricsAnalyzer {
    /// Analyze and compare benchmark results
    pub fn analyze(results: Vec<BenchmarkResults>) -> ComparativeAnalysis {
        let winner_by_metric = Self::find_winners(&results);
        let recommendations = Self::generate_recommendations(&results);
        let summary = Self::generate_summary(&results, &winner_by_metric);

        ComparativeAnalysis {
            implementations: results,
            winner_by_metric,
            recommendations,
            summary,
        }
    }

    /// Calculate an overall score for an implementation (0-100)
    pub fn calculate_overall_score(stats: &WorkloadStats, recovery: &CrashRecoveryResults) -> f64 {
        // Weighted scoring system
        let write_score = Self::latency_score(stats.write_latency_p95);
        let read_score = Self::latency_score(stats.read_latency_p95);
        let throughput_score = Self::throughput_score(stats.throughput);
        let memory_score =
            Self::memory_efficiency_score(stats.peak_memory_usage, stats.total_operations);
        let recovery_score = Self::recovery_score(recovery.recovery_time);

        // Weights based on orbit-rs priorities
        let weights = [
            (write_score, 0.3),      // Write performance critical for lease updates
            (read_score, 0.2),       // Read performance important but less critical
            (throughput_score, 0.2), // Overall throughput
            (memory_score, 0.15),    // Memory efficiency
            (recovery_score, 0.15),  // Recovery time (catastrophic failure scenarios)
        ];

        weights.iter().map(|(score, weight)| score * weight).sum()
    }

    /// Score latency (lower is better): 0-100 scale
    fn latency_score(latency: Duration) -> f64 {
        let micros = latency.as_micros() as f64;
        // Excellent: <5μs = 100, Good: <50μs = 80, Average: <500μs = 60, Poor: >5ms = 0
        match micros {
            l if l < 5.0 => 100.0,
            l if l < 50.0 => 100.0 - (l - 5.0) / 45.0 * 20.0, // 100-80
            l if l < 500.0 => 80.0 - (l - 50.0) / 450.0 * 20.0, // 80-60
            l if l < 5000.0 => 60.0 - (l - 500.0) / 4500.0 * 40.0, // 60-20
            l if l < 50000.0 => 20.0 - (l - 5000.0) / 45000.0 * 20.0, // 20-0
            _ => 0.0,
        }
    }

    /// Score throughput (higher is better): 0-100 scale
    fn throughput_score(throughput: f64) -> f64 {
        // Excellent: >10k ops/s = 100, Good: >1k ops/s = 80, Average: >100 ops/s = 60
        match throughput {
            t if t > 10000.0 => 100.0,
            t if t > 1000.0 => 80.0 + (t - 1000.0) / 9000.0 * 20.0,
            t if t > 100.0 => 60.0 + (t - 100.0) / 900.0 * 20.0,
            t if t > 10.0 => 40.0 + (t - 10.0) / 90.0 * 20.0,
            t if t > 1.0 => 20.0 + (t - 1.0) / 9.0 * 20.0,
            _ => throughput * 20.0,
        }
    }

    /// Score memory efficiency (lower usage per operation is better): 0-100 scale
    pub fn memory_efficiency_score(peak_memory: u64, operations: u64) -> f64 {
        if operations == 0 {
            return 0.0;
        }

        let bytes_per_op = peak_memory as f64 / operations as f64;
        // Excellent: <100 bytes/op = 100, Good: <1KB/op = 80, Average: <10KB/op = 60
        match bytes_per_op {
            b if b < 100.0 => 100.0,
            b if b < 1000.0 => 100.0 - (b - 100.0) / 900.0 * 20.0,
            b if b < 10000.0 => 80.0 - (b - 1000.0) / 9000.0 * 20.0,
            b if b < 100000.0 => 60.0 - (b - 10000.0) / 90000.0 * 40.0,
            _ => 20.0,
        }
    }

    /// Score recovery time (lower is better): 0-100 scale
    fn recovery_score(recovery_time: Duration) -> f64 {
        let seconds = recovery_time.as_secs_f64();
        // Excellent: <1s = 100, Good: <5s = 80, Average: <30s = 60, Poor: >300s = 0
        match seconds {
            s if s < 1.0 => 100.0,
            s if s < 5.0 => 100.0 - (s - 1.0) / 4.0 * 20.0,
            s if s < 30.0 => 80.0 - (s - 5.0) / 25.0 * 20.0,
            s if s < 300.0 => 60.0 - (s - 30.0) / 270.0 * 40.0,
            s if s < 3000.0 => 20.0 - (s - 300.0) / 2700.0 * 20.0,
            _ => 0.0,
        }
    }

    fn find_winners(results: &[BenchmarkResults]) -> WinnerByMetric {
        let best_write = results
            .iter()
            .min_by(|a, b| {
                a.actor_lease_workload
                    .write_latency_p95
                    .cmp(&b.actor_lease_workload.write_latency_p95)
            })
            .map(|r| r.implementation_name.clone())
            .unwrap_or_default();

        let best_read = results
            .iter()
            .min_by(|a, b| {
                a.actor_lease_workload
                    .read_latency_p95
                    .cmp(&b.actor_lease_workload.read_latency_p95)
            })
            .map(|r| r.implementation_name.clone())
            .unwrap_or_default();

        let best_throughput = results
            .iter()
            .max_by(|a, b| {
                a.high_throughput_workload
                    .throughput
                    .partial_cmp(&b.high_throughput_workload.throughput)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.implementation_name.clone())
            .unwrap_or_default();

        let best_memory = results
            .iter()
            .max_by(|a, b| {
                a.memory_efficiency_score
                    .partial_cmp(&b.memory_efficiency_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.implementation_name.clone())
            .unwrap_or_default();

        let best_recovery = results
            .iter()
            .min_by(|a, b| {
                a.crash_recovery_results
                    .recovery_time
                    .cmp(&b.crash_recovery_results.recovery_time)
            })
            .map(|r| r.implementation_name.clone())
            .unwrap_or_default();

        let best_overall = results
            .iter()
            .max_by(|a, b| {
                a.overall_score
                    .partial_cmp(&b.overall_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|r| r.implementation_name.clone())
            .unwrap_or_default();

        WinnerByMetric {
            best_write_latency: best_write,
            best_read_latency: best_read,
            best_throughput,
            best_memory_efficiency: best_memory,
            best_recovery_time: best_recovery,
            best_overall,
        }
    }

    fn generate_recommendations(results: &[BenchmarkResults]) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        // Find best for specific use cases
        let write_heavy_best = results.iter().min_by(|a, b| {
            a.actor_lease_workload
                .write_latency_p95
                .cmp(&b.actor_lease_workload.write_latency_p95)
        });

        let read_heavy_best = results.iter().min_by(|a, b| {
            a.query_heavy_workload
                .range_query_latency_p95
                .cmp(&b.query_heavy_workload.range_query_latency_p95)
        });

        let balanced_best = results.iter().max_by(|a, b| {
            a.overall_score
                .partial_cmp(&b.overall_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if let Some(impl_) = write_heavy_best {
            recommendations.push(Recommendation {
                use_case: "High-Frequency Actor Lease Updates".to_string(),
                recommended_implementation: impl_.implementation_name.clone(),
                reasoning: format!(
                    "Best write latency P95: {:.1}μs. Optimal for frequent lease renewals in busy actor systems.",
                    impl_.actor_lease_workload.write_latency_p95.as_micros()
                ),
            });
        }

        if let Some(impl_) = read_heavy_best {
            recommendations.push(Recommendation {
                use_case: "Cluster Coordination (Range Query Heavy)".to_string(),
                recommended_implementation: impl_.implementation_name.clone(),
                reasoning: format!(
                    "Best range query latency P95: {:.1}μs. Ideal for cluster coordination queries.",
                    impl_.query_heavy_workload.range_query_latency_p95.as_micros()
                ),
            });
        }

        if let Some(impl_) = balanced_best {
            recommendations.push(Recommendation {
                use_case: "Production Orbit-rs Deployment".to_string(),
                recommended_implementation: impl_.implementation_name.clone(),
                reasoning: format!(
                    "Best overall score: {:.1}/100. Balanced performance across all metrics with {:.1}s recovery time.",
                    impl_.overall_score,
                    impl_.crash_recovery_results.recovery_time.as_secs_f64()
                ),
            });
        }

        recommendations
    }

    fn generate_summary(results: &[BenchmarkResults], winners: &WinnerByMetric) -> String {
        let mut summary = String::new();

        summary.push_str("# Persistence Implementation Benchmark Results\n\n");

        if results.len() == 2 {
            let cow_results = results
                .iter()
                .find(|r| r.implementation_name.contains("COW"));
            let rocks_results = results
                .iter()
                .find(|r| r.implementation_name.contains("RocksDB"));

            if let (Some(cow), Some(rocks)) = (cow_results, rocks_results) {
                let write_improvement = if cow.actor_lease_workload.write_latency_p95
                    < rocks.actor_lease_workload.write_latency_p95
                {
                    let ratio = rocks.actor_lease_workload.write_latency_p95.as_micros() as f64
                        / cow.actor_lease_workload.write_latency_p95.as_micros() as f64;
                    format!("COW B+ Tree is {:.1}x faster for writes", ratio)
                } else {
                    let ratio = cow.actor_lease_workload.write_latency_p95.as_micros() as f64
                        / rocks.actor_lease_workload.write_latency_p95.as_micros() as f64;
                    format!("RocksDB is {:.1}x faster for writes", ratio)
                };

                let memory_improvement =
                    if cow.memory_efficiency_score > rocks.memory_efficiency_score {
                        format!(
                            "COW B+ Tree is more memory efficient ({:.1} vs {:.1} score)",
                            cow.memory_efficiency_score, rocks.memory_efficiency_score
                        )
                    } else {
                        format!(
                            "RocksDB is more memory efficient ({:.1} vs {:.1} score)",
                            rocks.memory_efficiency_score, cow.memory_efficiency_score
                        )
                    };

                summary.push_str(&format!(
                    "## Key Findings\n\n\
                    - **Write Performance**: {}\n\
                    - **Memory Efficiency**: {}\n\
                    - **Recovery Time**: COW: {:.2}s vs RocksDB: {:.2}s\n\
                    - **Overall Winner**: {} ({:.1}/100 vs {:.1}/100)\n\n",
                    write_improvement,
                    memory_improvement,
                    cow.crash_recovery_results.recovery_time.as_secs_f64(),
                    rocks.crash_recovery_results.recovery_time.as_secs_f64(),
                    winners.best_overall,
                    if winners.best_overall == cow.implementation_name {
                        cow.overall_score
                    } else {
                        rocks.overall_score
                    },
                    if winners.best_overall == cow.implementation_name {
                        rocks.overall_score
                    } else {
                        cow.overall_score
                    }
                ));
            }
        }

        summary.push_str(&format!(
            "## Performance Leaders\n\n\
            - **Fastest Writes**: {}\n\
            - **Fastest Reads**: {}\n\
            - **Highest Throughput**: {}\n\
            - **Most Memory Efficient**: {}\n\
            - **Fastest Recovery**: {}\n\n",
            winners.best_write_latency,
            winners.best_read_latency,
            winners.best_throughput,
            winners.best_memory_efficiency,
            winners.best_recovery_time
        ));

        summary
    }

    /// Export results to JSON file
    pub fn export_to_json(
        analysis: &ComparativeAnalysis,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(analysis)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Export results to CSV for further analysis
    pub fn export_to_csv(
        results: &[BenchmarkResults],
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut csv_content = String::new();

        // Header
        csv_content.push_str("implementation,write_p50_us,write_p95_us,write_p99_us,read_p50_us,read_p95_us,throughput_ops_s,memory_mb,recovery_time_s,overall_score\n");

        // Data rows
        for result in results {
            csv_content.push_str(&format!(
                "{},{},{},{},{},{},{:.2},{:.2},{:.2},{:.2}\n",
                result.implementation_name,
                result.actor_lease_workload.write_latency_p50.as_micros(),
                result.actor_lease_workload.write_latency_p95.as_micros(),
                result.actor_lease_workload.write_latency_p99.as_micros(),
                result.actor_lease_workload.read_latency_p50.as_micros(),
                result.actor_lease_workload.read_latency_p95.as_micros(),
                result.actor_lease_workload.throughput,
                result.actor_lease_workload.peak_memory_usage as f64 / 1024.0 / 1024.0,
                result.crash_recovery_results.recovery_time.as_secs_f64(),
                result.overall_score
            ));
        }

        std::fs::write(path, csv_content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scoring_functions() {
        // Test latency scoring
        assert!((MetricsAnalyzer::latency_score(Duration::from_micros(1)) - 100.0).abs() < 0.1);
        assert!((MetricsAnalyzer::latency_score(Duration::from_micros(25)) - 91.1).abs() < 1.0);
        // 10ms = 10,000μs should score low but not 0 (around 15-20)
        assert!(MetricsAnalyzer::latency_score(Duration::from_millis(10)) < 25.0);

        // Test throughput scoring
        assert!((MetricsAnalyzer::throughput_score(15000.0) - 100.0).abs() < 0.1);
        assert!((MetricsAnalyzer::throughput_score(5000.0) - 88.9).abs() < 1.0);
        // Throughput score for 50 ops/s (around 49)
        let score_50 = MetricsAnalyzer::throughput_score(50.0);
        assert!(score_50 > 48.0 && score_50 < 52.0);

        // Test memory efficiency scoring - 1000 bytes for 100 ops = 10 bytes/op
        let mem_score = MetricsAnalyzer::memory_efficiency_score(1000, 100);
        assert!(mem_score > 95.0); // Should be very high (good efficiency)

        let mem_score_bad = MetricsAnalyzer::memory_efficiency_score(100000, 100);
        assert!(mem_score_bad < 85.0); // Should be lower than good efficiency

        // Test recovery scoring
        assert!((MetricsAnalyzer::recovery_score(Duration::from_millis(500)) - 100.0).abs() < 0.1);
        let recovery_10s = MetricsAnalyzer::recovery_score(Duration::from_secs(10));
        assert!(recovery_10s > 70.0 && recovery_10s < 80.0);
    }
}
