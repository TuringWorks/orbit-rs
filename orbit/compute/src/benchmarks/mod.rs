//! Benchmarking suite for heterogeneous compute acceleration
//!
//! This module provides comprehensive benchmarks for CPU SIMD, GPU compute,
//! Neural Engine operations, and system monitoring to establish performance
//! baselines and validate adaptive scheduling decisions.

use std::time::{Duration, Instant};

use crate::capabilities::UniversalComputeCapabilities;
use crate::errors::ComputeResult;
use crate::monitoring::SystemMonitor;
use crate::scheduler::{CPUSIMDType, ComputeUnit, DataSizeClass, SIMDOperationType, WorkloadType};

/// Benchmark suite for heterogeneous acceleration
#[derive(Debug)]
pub struct BenchmarkSuite {
    /// System capabilities
    capabilities: UniversalComputeCapabilities,
    /// System monitor for performance tracking
    system_monitor: SystemMonitor,
    /// Benchmark configuration
    config: BenchmarkConfig,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of iterations per benchmark
    pub iterations: usize,
    /// Warmup iterations before timing
    pub warmup_iterations: usize,
    /// Data sizes to benchmark
    pub data_sizes: Vec<usize>,
    /// Duration for sustained load tests
    pub sustained_load_duration: Duration,
    /// Enable detailed system monitoring during benchmarks
    pub monitor_system: bool,
}

/// Benchmark result for a single test
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Benchmark name/identifier
    pub name: String,
    /// Workload type tested
    pub workload_type: WorkloadType,
    /// Compute unit used
    pub compute_unit: ComputeUnit,
    /// Data size in bytes
    pub data_size: usize,
    /// Average execution time
    pub avg_time: Duration,
    /// Minimum execution time
    pub min_time: Duration,
    /// Maximum execution time
    pub max_time: Duration,
    /// Standard deviation of execution time
    pub std_dev: Duration,
    /// Throughput in operations per second
    pub throughput_ops_per_sec: f64,
    /// Memory bandwidth utilization
    pub memory_bandwidth_gbps: f64,
    /// Energy efficiency estimate (operations per joule)
    pub energy_efficiency: f64,
    /// System conditions during benchmark
    pub system_conditions: Option<crate::scheduler::SystemConditions>,
}

/// Collection of benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkReport {
    /// Individual benchmark results
    pub results: Vec<BenchmarkResult>,
    /// Overall statistics
    pub summary: BenchmarkSummary,
    /// Timestamp when benchmarks were run
    pub timestamp: std::time::SystemTime,
    /// System information
    pub system_info: String,
}

/// Summary statistics across all benchmarks
#[derive(Debug, Clone)]
pub struct BenchmarkSummary {
    /// Total benchmarks run
    pub total_benchmarks: usize,
    /// Best performing compute unit overall
    pub best_compute_unit: Option<ComputeUnit>,
    /// Average performance improvement from heterogeneous acceleration
    pub avg_acceleration_factor: f64,
    /// Performance stability score (lower is better)
    pub stability_score: f64,
    /// Energy efficiency score
    pub energy_efficiency_score: f64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            warmup_iterations: 10,
            data_sizes: vec![
                1024,      // 1KB - L1 cache
                65536,     // 64KB - L2 cache
                1048576,   // 1MB - L3 cache
                16777216,  // 16MB - Main memory
                134217728, // 128MB - Large dataset
            ],
            sustained_load_duration: Duration::from_secs(30),
            monitor_system: true,
        }
    }
}

impl BenchmarkSuite {
    /// Create a new benchmark suite
    pub async fn new(
        capabilities: UniversalComputeCapabilities,
        config: Option<BenchmarkConfig>,
    ) -> ComputeResult<Self> {
        let mut system_monitor = SystemMonitor::new().await?;
        system_monitor.start_monitoring().await?;

        Ok(Self {
            capabilities,
            system_monitor,
            config: config.unwrap_or_default(),
        })
    }

    /// Run comprehensive benchmark suite
    pub async fn run_full_suite(&mut self) -> ComputeResult<BenchmarkReport> {
        tracing::info!("Starting comprehensive benchmark suite");

        let mut results = Vec::new();

        // CPU SIMD benchmarks
        let cpu_results = self.benchmark_cpu_simd().await?;
        results.extend(cpu_results);

        // GPU compute benchmarks
        if !self.capabilities.gpu.available_devices.is_empty() {
            let gpu_results = self.benchmark_gpu_compute().await?;
            results.extend(gpu_results);
        }

        // Neural engine benchmarks
        let neural_results = self.benchmark_neural_engine().await?;
        results.extend(neural_results);

        // System monitoring benchmarks
        if self.config.monitor_system {
            let monitoring_results = self.benchmark_system_monitoring().await?;
            results.extend(monitoring_results);
        }

        // Generate summary
        let summary = self.generate_summary(&results);
        let system_info = format!("{:?}", self.capabilities.cpu.architecture);

        let report = BenchmarkReport {
            results,
            summary,
            timestamp: std::time::SystemTime::now(),
            system_info,
        };

        tracing::info!("Benchmark suite completed: {} tests", report.results.len());
        Ok(report)
    }

    /// Benchmark CPU SIMD operations
    async fn benchmark_cpu_simd(&self) -> ComputeResult<Vec<BenchmarkResult>> {
        let simd_types = self.detect_available_simd_types();
        let mut results = Vec::new();

        for simd_type in simd_types {
            let simd_results = self.benchmark_simd_type(simd_type).await?;
            results.extend(simd_results);
        }

        Ok(results)
    }

    /// Detect available SIMD types based on CPU features
    fn detect_available_simd_types(&self) -> Vec<CPUSIMDType> {
        if let Some(x86_features) = &self.capabilities.cpu.simd.x86_features {
            return self.collect_x86_simd_types(x86_features);
        }

        if let Some(arm_features) = &self.capabilities.cpu.simd.arm_features {
            return self.collect_arm_simd_types(arm_features);
        }

        Vec::new()
    }

    /// Collect available x86 SIMD types
    fn collect_x86_simd_types(
        &self,
        x86_features: &crate::capabilities::X86SIMDFeatures,
    ) -> Vec<CPUSIMDType> {
        let mut types = Vec::new();

        if x86_features.avx.avx2 {
            types.push(CPUSIMDType::AVX2);
        }
        if x86_features.avx.avx512f {
            types.push(CPUSIMDType::AVX512);
        }

        types
    }

    /// Collect available ARM SIMD types
    fn collect_arm_simd_types(
        &self,
        arm_features: &crate::capabilities::ARMSIMDFeatures,
    ) -> Vec<CPUSIMDType> {
        let mut types = Vec::new();

        if arm_features.neon {
            types.push(CPUSIMDType::NEON);
        }
        if let Some(sve) = &arm_features.sve {
            types.push(CPUSIMDType::SVE {
                vector_length: sve.vector_length,
            });
        }

        types
    }

    /// Benchmark all data sizes for a specific SIMD type
    async fn benchmark_simd_type(
        &self,
        simd_type: CPUSIMDType,
    ) -> ComputeResult<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for &data_size in &self.config.data_sizes {
            results.push(
                self.benchmark_elementwise_simd(&simd_type, data_size)
                    .await?,
            );

            if data_size >= 65536 {
                results.push(self.benchmark_matrix_simd(&simd_type, data_size).await?);
            }
        }

        Ok(results)
    }

    /// Benchmark element-wise SIMD operations
    async fn benchmark_elementwise_simd(
        &self,
        simd_type: &CPUSIMDType,
        data_size: usize,
    ) -> ComputeResult<BenchmarkResult> {
        self.benchmark_simd_operation(simd_type.clone(), SIMDOperationType::ElementWise, data_size)
            .await
    }

    /// Benchmark matrix SIMD operations
    async fn benchmark_matrix_simd(
        &self,
        simd_type: &CPUSIMDType,
        data_size: usize,
    ) -> ComputeResult<BenchmarkResult> {
        self.benchmark_simd_operation(simd_type.clone(), SIMDOperationType::MatrixOps, data_size)
            .await
    }

    /// Benchmark a specific SIMD operation
    async fn benchmark_simd_operation(
        &self,
        simd_type: CPUSIMDType,
        operation: SIMDOperationType,
        data_size: usize,
    ) -> ComputeResult<BenchmarkResult> {
        let workload_type = WorkloadType::SIMDBatch {
            data_size: Self::classify_data_size(data_size),
            operation_type: operation.clone(),
        };

        let compute_unit = ComputeUnit::CPU {
            simd_type: simd_type.clone(),
            thread_count: self.capabilities.cpu.cores.logical_cores,
        };

        let mut timings = Vec::new();

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            self.simulate_simd_workload(&simd_type, &operation, data_size)
                .await?;
        }

        // Actual benchmark
        for _ in 0..self.config.iterations {
            let start = Instant::now();
            self.simulate_simd_workload(&simd_type, &operation, data_size)
                .await?;
            let elapsed = start.elapsed();
            timings.push(elapsed);
        }

        let system_conditions = if self.config.monitor_system {
            Some(self.system_monitor.get_current_conditions().await?)
        } else {
            None
        };

        Ok(self.calculate_benchmark_result(
            format!("SIMD_{:?}_{:?}", simd_type, operation),
            workload_type,
            compute_unit,
            data_size,
            &timings,
            system_conditions,
        ))
    }

    /// Simulate SIMD workload (placeholder)
    async fn simulate_simd_workload(
        &self,
        _simd_type: &CPUSIMDType,
        _operation: &SIMDOperationType,
        data_size: usize,
    ) -> ComputeResult<()> {
        // Simulate computation time based on data size
        let computation_time = Duration::from_nanos((data_size as u64) / 1000);
        tokio::time::sleep(computation_time).await;
        Ok(())
    }

    /// Benchmark GPU compute operations
    async fn benchmark_gpu_compute(&self) -> ComputeResult<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for (device_id, _gpu_device) in self.capabilities.gpu.available_devices.iter().enumerate() {
            for data_size in self
                .config
                .data_sizes
                .iter()
                .cloned()
                .filter(|&size| size >= 65536)
            {
                // GPU compute workload
                let result = self.benchmark_gpu_operation(device_id, data_size).await?;
                results.push(result);
            }
        }

        Ok(results)
    }

    /// Benchmark a specific GPU operation
    async fn benchmark_gpu_operation(
        &self,
        device_id: usize,
        data_size: usize,
    ) -> ComputeResult<BenchmarkResult> {
        use crate::scheduler::{GPUComputeAPI, GPUWorkloadClass, MemoryPattern};

        let workload_type = WorkloadType::GPUCompute {
            workload_class: GPUWorkloadClass::GeneralCompute,
            memory_pattern: MemoryPattern::Sequential,
        };

        let compute_unit = ComputeUnit::GPU {
            device_id,
            api: GPUComputeAPI::Metal, // Would detect actual API
        };

        let mut timings = Vec::new();

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            self.simulate_gpu_workload(device_id, data_size).await?;
        }

        // Actual benchmark
        for _ in 0..self.config.iterations {
            let start = Instant::now();
            self.simulate_gpu_workload(device_id, data_size).await?;
            let elapsed = start.elapsed();
            timings.push(elapsed);
        }

        let system_conditions = if self.config.monitor_system {
            Some(self.system_monitor.get_current_conditions().await?)
        } else {
            None
        };

        Ok(self.calculate_benchmark_result(
            format!("GPU_Device_{}", device_id),
            workload_type,
            compute_unit,
            data_size,
            &timings,
            system_conditions,
        ))
    }

    /// Simulate GPU workload (placeholder)
    async fn simulate_gpu_workload(
        &self,
        _device_id: usize,
        data_size: usize,
    ) -> ComputeResult<()> {
        // Simulate GPU computation time
        let computation_time = Duration::from_nanos((data_size as u64) / 5000); // GPU is ~5x faster
        tokio::time::sleep(computation_time).await;
        Ok(())
    }

    /// Benchmark Neural Engine operations
    async fn benchmark_neural_engine(&self) -> ComputeResult<Vec<BenchmarkResult>> {
        use crate::capabilities::NeuralEngineCapabilities;
        use crate::scheduler::{InferencePrecision, ModelType, NeuralEngineType};

        let mut results = Vec::new();

        match &self.capabilities.neural {
            NeuralEngineCapabilities::AppleNeuralEngine { .. } => {
                let workload_type = WorkloadType::NeuralInference {
                    model_type: ModelType::CNN,
                    precision: InferencePrecision::FP16,
                };

                let compute_unit = ComputeUnit::NeuralEngine {
                    engine_type: NeuralEngineType::AppleNE,
                };

                let result = self
                    .benchmark_neural_operation(
                        workload_type,
                        compute_unit,
                        1024, // Standard neural model size
                    )
                    .await?;
                results.push(result);
            }
            NeuralEngineCapabilities::QualcommAI { .. } => {
                let workload_type = WorkloadType::NeuralInference {
                    model_type: ModelType::CNN,
                    precision: InferencePrecision::INT8,
                };

                let compute_unit = ComputeUnit::NeuralEngine {
                    engine_type: NeuralEngineType::QualcommAI,
                };

                let result = self
                    .benchmark_neural_operation(workload_type, compute_unit, 1024)
                    .await?;
                results.push(result);
            }
            _ => {}
        }

        Ok(results)
    }

    /// Benchmark a specific neural engine operation
    async fn benchmark_neural_operation(
        &self,
        workload_type: WorkloadType,
        compute_unit: ComputeUnit,
        data_size: usize,
    ) -> ComputeResult<BenchmarkResult> {
        let mut timings = Vec::new();

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            self.simulate_neural_workload(data_size).await?;
        }

        // Actual benchmark
        for _ in 0..self.config.iterations {
            let start = Instant::now();
            self.simulate_neural_workload(data_size).await?;
            let elapsed = start.elapsed();
            timings.push(elapsed);
        }

        let system_conditions = if self.config.monitor_system {
            Some(self.system_monitor.get_current_conditions().await?)
        } else {
            None
        };

        Ok(self.calculate_benchmark_result(
            "Neural_Engine".to_string(),
            workload_type,
            compute_unit,
            data_size,
            &timings,
            system_conditions,
        ))
    }

    /// Simulate neural engine workload (placeholder)
    async fn simulate_neural_workload(&self, data_size: usize) -> ComputeResult<()> {
        // Simulate neural inference time
        let computation_time = Duration::from_nanos((data_size as u64) / 50000); // Neural engine is very fast
        tokio::time::sleep(computation_time).await;
        Ok(())
    }

    /// Benchmark system monitoring overhead
    async fn benchmark_system_monitoring(&self) -> ComputeResult<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        // Benchmark monitoring call overhead
        let mut timings = Vec::new();

        for _ in 0..self.config.iterations {
            let start = Instant::now();
            let _ = self.system_monitor.get_current_conditions().await?;
            let elapsed = start.elapsed();
            timings.push(elapsed);
        }

        let workload_type = WorkloadType::SIMDBatch {
            data_size: DataSizeClass::Small,
            operation_type: SIMDOperationType::ElementWise,
        };

        let compute_unit = ComputeUnit::CPU {
            simd_type: CPUSIMDType::AVX2,
            thread_count: 1,
        };

        let result = self.calculate_benchmark_result(
            "System_Monitoring_Overhead".to_string(),
            workload_type,
            compute_unit,
            0, // No data processed
            &timings,
            None,
        );

        results.push(result);
        Ok(results)
    }

    /// Calculate benchmark result from timings
    fn calculate_benchmark_result(
        &self,
        name: String,
        workload_type: WorkloadType,
        compute_unit: ComputeUnit,
        data_size: usize,
        timings: &[Duration],
        system_conditions: Option<crate::scheduler::SystemConditions>,
    ) -> BenchmarkResult {
        let avg_time = Duration::from_nanos(
            timings.iter().map(|d| d.as_nanos()).sum::<u128>() as u64 / timings.len() as u64,
        );

        let min_time = timings.iter().min().cloned().unwrap_or(Duration::ZERO);
        let max_time = timings.iter().max().cloned().unwrap_or(Duration::ZERO);

        // Calculate standard deviation
        let avg_nanos = avg_time.as_nanos() as f64;
        let variance = timings
            .iter()
            .map(|d| {
                let diff = d.as_nanos() as f64 - avg_nanos;
                diff * diff
            })
            .sum::<f64>()
            / timings.len() as f64;
        let std_dev = Duration::from_nanos(variance.sqrt() as u64);

        // Calculate throughput
        let throughput_ops_per_sec = if avg_time.as_secs_f64() > 0.0 {
            1.0 / avg_time.as_secs_f64()
        } else {
            0.0
        };

        // Estimate memory bandwidth (simplified)
        let memory_bandwidth_gbps = if data_size > 0 && avg_time.as_secs_f64() > 0.0 {
            (data_size as f64) / avg_time.as_secs_f64() / 1_000_000_000.0
        } else {
            0.0
        };

        // Estimate energy efficiency (operations per joule)
        let energy_efficiency = throughput_ops_per_sec * 100.0; // Simplified estimate

        BenchmarkResult {
            name,
            workload_type,
            compute_unit,
            data_size,
            avg_time,
            min_time,
            max_time,
            std_dev,
            throughput_ops_per_sec,
            memory_bandwidth_gbps,
            energy_efficiency,
            system_conditions,
        }
    }

    /// Generate summary statistics
    fn generate_summary(&self, results: &[BenchmarkResult]) -> BenchmarkSummary {
        if results.is_empty() {
            return BenchmarkSummary {
                total_benchmarks: 0,
                best_compute_unit: None,
                avg_acceleration_factor: 1.0,
                stability_score: 0.0,
                energy_efficiency_score: 0.0,
            };
        }

        // Find best performing compute unit
        let best_compute_unit = results
            .iter()
            .max_by(|a, b| {
                a.throughput_ops_per_sec
                    .partial_cmp(&b.throughput_ops_per_sec)
                    .unwrap()
            })
            .map(|r| r.compute_unit.clone());

        // Calculate average acceleration factor (vs baseline)
        let baseline_throughput = results
            .iter()
            .map(|r| r.throughput_ops_per_sec)
            .fold(0.0, f64::min);

        let avg_acceleration_factor = if baseline_throughput > 0.0 {
            results
                .iter()
                .map(|r| r.throughput_ops_per_sec / baseline_throughput)
                .sum::<f64>()
                / results.len() as f64
        } else {
            1.0
        };

        // Calculate stability score (coefficient of variation)
        let avg_throughput = results
            .iter()
            .map(|r| r.throughput_ops_per_sec)
            .sum::<f64>()
            / results.len() as f64;

        let variance = results
            .iter()
            .map(|r| {
                let diff = r.throughput_ops_per_sec - avg_throughput;
                diff * diff
            })
            .sum::<f64>()
            / results.len() as f64;

        let stability_score = if avg_throughput > 0.0 {
            variance.sqrt() / avg_throughput
        } else {
            0.0
        };

        // Calculate average energy efficiency score
        let energy_efficiency_score =
            results.iter().map(|r| r.energy_efficiency).sum::<f64>() / results.len() as f64;

        BenchmarkSummary {
            total_benchmarks: results.len(),
            best_compute_unit,
            avg_acceleration_factor,
            stability_score,
            energy_efficiency_score,
        }
    }

    /// Classify data size into cache-friendly categories
    fn classify_data_size(size: usize) -> DataSizeClass {
        match size {
            0..=65536 => DataSizeClass::Small,                 // Up to 64KB
            65537..=1048576 => DataSizeClass::Medium,          // 64KB to 1MB
            1048577..=33554432 => DataSizeClass::Large,        // 1MB to 32MB
            33554433..=134217728 => DataSizeClass::ExtraLarge, // 32MB to 128MB
            _ => DataSizeClass::Streaming,
        }
    }
}

/// Helper function to run quick benchmark
pub async fn quick_benchmark() -> ComputeResult<BenchmarkReport> {
    let capabilities = crate::capabilities::detect_all_capabilities().await?;
    let config = BenchmarkConfig {
        iterations: 10,
        warmup_iterations: 2,
        data_sizes: vec![1024, 65536, 1048576],
        sustained_load_duration: Duration::from_secs(5),
        monitor_system: true,
    };

    let mut suite = BenchmarkSuite::new(capabilities, Some(config)).await?;
    suite.run_full_suite().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quick_benchmark() {
        let result = quick_benchmark().await;
        assert!(result.is_ok());

        let report = result.unwrap();
        assert!(!report.results.is_empty());
        assert!(report.summary.total_benchmarks > 0);
    }

    #[test]
    fn test_data_size_classification() {
        assert_eq!(
            BenchmarkSuite::classify_data_size(1024),
            DataSizeClass::Small
        );
        assert_eq!(
            BenchmarkSuite::classify_data_size(65536),
            DataSizeClass::Small
        );
        assert_eq!(
            BenchmarkSuite::classify_data_size(1048576),
            DataSizeClass::Medium
        );
        assert_eq!(
            BenchmarkSuite::classify_data_size(16777216),
            DataSizeClass::Large
        );
        assert_eq!(
            BenchmarkSuite::classify_data_size(134217728),
            DataSizeClass::ExtraLarge
        );
    }
}
