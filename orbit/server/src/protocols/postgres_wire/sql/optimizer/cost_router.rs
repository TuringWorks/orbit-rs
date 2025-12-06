//! Cost-Based Execution Routing
//!
//! This module determines the optimal execution path for SQL operations:
//! - CPU scalar execution (small datasets)
//! - CPU SIMD/vectorized execution (medium datasets)
//! - GPU accelerated execution (large datasets)
//!
//! The decision is based on:
//! - Estimated row count
//! - Operation complexity
//! - Data types involved
//! - Available hardware capabilities

use crate::protocols::postgres_wire::sql::statistics::TableStatistics;
use crate::protocols::postgres_wire::sql::vectorized_executor::VectorizedConfig;
use serde::{Deserialize, Serialize};

/// Execution backend options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionBackend {
    /// Standard CPU execution (no SIMD)
    CpuScalar,
    /// CPU with SIMD vectorization (AVX2/AVX-512/NEON)
    CpuSimd,
    /// GPU acceleration (Metal/CUDA/Vulkan)
    Gpu,
}

/// Operation type for cost estimation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// Table scan with filter
    Filter,
    /// Aggregation (SUM, COUNT, etc.)
    Aggregate,
    /// Sort operation
    Sort,
    /// Join operation
    Join,
    /// Vector similarity search
    VectorSearch,
    /// General projection
    Projection,
}

/// Cost model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRouterConfig {
    /// Minimum rows for SIMD to be beneficial
    pub simd_row_threshold: usize,
    /// Minimum rows for GPU to be beneficial
    pub gpu_row_threshold: usize,
    /// Enable GPU routing (requires hardware)
    pub gpu_enabled: bool,
    /// Enable SIMD routing
    pub simd_enabled: bool,
    /// GPU transfer overhead in "row equivalents"
    pub gpu_transfer_overhead: usize,
    /// Prefer GPU for vector operations regardless of size
    pub prefer_gpu_for_vectors: bool,
}

impl Default for CostRouterConfig {
    fn default() -> Self {
        Self {
            simd_row_threshold: 1000,   // SIMD beneficial above 1K rows
            gpu_row_threshold: 100_000, // GPU beneficial above 100K rows
            gpu_enabled: cfg!(feature = "gpu-acceleration"),
            simd_enabled: true,
            gpu_transfer_overhead: 50_000, // Equivalent to 50K rows of processing
            prefer_gpu_for_vectors: true,
        }
    }
}

/// Cost estimate for an operation
#[derive(Debug, Clone)]
pub struct CostEstimate {
    /// CPU scalar cost (baseline)
    pub cpu_scalar_cost: f64,
    /// CPU SIMD cost
    pub cpu_simd_cost: f64,
    /// GPU cost (including transfer overhead)
    pub gpu_cost: f64,
    /// Estimated rows
    pub estimated_rows: usize,
    /// Recommended backend
    pub recommended_backend: ExecutionBackend,
}

/// Cost-based execution router
pub struct CostRouter {
    config: CostRouterConfig,
}

impl CostRouter {
    /// Create a new cost router
    pub fn new(config: CostRouterConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn new_default() -> Self {
        Self::new(CostRouterConfig::default())
    }

    /// Determine the best execution backend for an operation
    pub fn route(
        &self,
        operation: OperationType,
        estimated_rows: usize,
        _table_stats: Option<&TableStatistics>,
    ) -> ExecutionBackend {
        let estimate = self.estimate_costs(operation, estimated_rows);
        estimate.recommended_backend
    }

    /// Get detailed cost estimates for all backends
    pub fn estimate_costs(&self, operation: OperationType, estimated_rows: usize) -> CostEstimate {
        // Base cost per row for CPU scalar (normalized to 1.0)
        let cpu_scalar_per_row = self.operation_base_cost(operation);
        let cpu_scalar_cost = estimated_rows as f64 * cpu_scalar_per_row;

        // SIMD speedup factors (approximate)
        let simd_speedup = match operation {
            OperationType::Filter => 4.0,       // SIMD excels at filtering
            OperationType::Aggregate => 8.0,    // Very good for reductions
            OperationType::Sort => 2.0,         // Some benefit
            OperationType::Join => 3.0,         // Hash operations
            OperationType::VectorSearch => 8.0, // Dot products parallelize well
            OperationType::Projection => 2.0,
        };

        // GPU speedup factors (with overhead considered)
        let gpu_speedup = match operation {
            OperationType::Filter => 20.0,
            OperationType::Aggregate => 50.0,
            OperationType::Sort => 10.0,
            OperationType::Join => 15.0,
            OperationType::VectorSearch => 100.0, // GPU is excellent for vectors
            OperationType::Projection => 5.0,
        };

        // Calculate SIMD cost
        let cpu_simd_cost =
            if self.config.simd_enabled && estimated_rows >= self.config.simd_row_threshold {
                cpu_scalar_cost / simd_speedup
            } else {
                cpu_scalar_cost * 1.1 // Slight overhead if below threshold
            };

        // Calculate GPU cost (including transfer overhead)
        let gpu_cost = if self.config.gpu_enabled && estimated_rows >= self.config.gpu_row_threshold
        {
            let processing_cost = cpu_scalar_cost / gpu_speedup;
            let transfer_cost = self.config.gpu_transfer_overhead as f64 * cpu_scalar_per_row;
            processing_cost + transfer_cost
        } else {
            f64::INFINITY // Not beneficial
        };

        // Special case: always prefer GPU for vector operations if configured
        let gpu_cost = if operation == OperationType::VectorSearch
            && self.config.prefer_gpu_for_vectors
            && self.config.gpu_enabled
        {
            let processing_cost = cpu_scalar_cost / gpu_speedup;
            let transfer_cost =
                (self.config.gpu_transfer_overhead as f64 * cpu_scalar_per_row) * 0.5;
            processing_cost + transfer_cost
        } else {
            gpu_cost
        };

        // Determine best backend
        let recommended_backend = self.select_best_backend(
            cpu_scalar_cost,
            cpu_simd_cost,
            gpu_cost,
            operation,
            estimated_rows,
        );

        CostEstimate {
            cpu_scalar_cost,
            cpu_simd_cost,
            gpu_cost,
            estimated_rows,
            recommended_backend,
        }
    }

    /// Get base cost per row for an operation type
    fn operation_base_cost(&self, operation: OperationType) -> f64 {
        match operation {
            OperationType::Filter => 1.0,        // Simple comparison
            OperationType::Aggregate => 1.5,     // Accumulation
            OperationType::Sort => 5.0,          // O(n log n) amortized
            OperationType::Join => 3.0,          // Hash table operations
            OperationType::VectorSearch => 10.0, // High-dimensional math
            OperationType::Projection => 0.5,    // Simple copy
        }
    }

    /// Select the best backend based on costs and constraints
    fn select_best_backend(
        &self,
        cpu_scalar_cost: f64,
        cpu_simd_cost: f64,
        gpu_cost: f64,
        operation: OperationType,
        estimated_rows: usize,
    ) -> ExecutionBackend {
        // For very small datasets, always use scalar
        if estimated_rows < 100 {
            return ExecutionBackend::CpuScalar;
        }

        // Check if GPU is the winner (and available)
        if self.config.gpu_enabled && gpu_cost < cpu_simd_cost && gpu_cost < cpu_scalar_cost {
            return ExecutionBackend::Gpu;
        }

        // Check if SIMD is beneficial
        if self.config.simd_enabled
            && cpu_simd_cost < cpu_scalar_cost
            && estimated_rows >= self.config.simd_row_threshold
        {
            return ExecutionBackend::CpuSimd;
        }

        // Special handling for vector operations
        if operation == OperationType::VectorSearch && self.config.gpu_enabled {
            return ExecutionBackend::Gpu;
        }

        ExecutionBackend::CpuScalar
    }

    /// Create VectorizedConfig based on routing decision
    pub fn to_vectorized_config(&self, backend: ExecutionBackend) -> VectorizedConfig {
        match backend {
            ExecutionBackend::CpuScalar => VectorizedConfig {
                min_rows_for_vectorized: usize::MAX, // Disable vectorization
                min_rows_for_gpu: usize::MAX,
                enable_simd: false,
                enable_gpu: false,
                collect_statistics: false,
            },
            ExecutionBackend::CpuSimd => VectorizedConfig {
                min_rows_for_vectorized: 100,
                min_rows_for_gpu: usize::MAX, // Don't use GPU
                enable_simd: true,
                enable_gpu: false,
                collect_statistics: true,
            },
            ExecutionBackend::Gpu => VectorizedConfig {
                min_rows_for_vectorized: 100,
                min_rows_for_gpu: 1000,
                enable_simd: true,
                enable_gpu: true,
                collect_statistics: true,
            },
        }
    }

    /// Route a query plan and return recommended backends for each node
    pub fn route_query_plan(
        &self,
        operations: &[(OperationType, usize)], // (operation, estimated_rows)
    ) -> Vec<(OperationType, ExecutionBackend)> {
        operations
            .iter()
            .map(|(op, rows)| (*op, self.route(*op, *rows, None)))
            .collect()
    }

    /// Explain routing decision
    pub fn explain_routing(&self, operation: OperationType, estimated_rows: usize) -> String {
        let estimate = self.estimate_costs(operation, estimated_rows);

        format!(
            "Operation: {:?}\n\
             Estimated Rows: {}\n\
             \n\
             Cost Estimates:\n\
             - CPU Scalar: {:.2}\n\
             - CPU SIMD:   {:.2}{}\n\
             - GPU:        {:.2}{}\n\
             \n\
             Recommended: {:?}\n\
             Reason: {}",
            operation,
            estimated_rows,
            estimate.cpu_scalar_cost,
            estimate.cpu_simd_cost,
            if !self.config.simd_enabled {
                " (disabled)"
            } else {
                ""
            },
            if estimate.gpu_cost.is_infinite() {
                "N/A".to_string()
            } else {
                format!("{:.2}", estimate.gpu_cost)
            },
            if !self.config.gpu_enabled {
                " (disabled)"
            } else {
                ""
            },
            estimate.recommended_backend,
            self.routing_reason(&estimate, operation),
        )
    }

    fn routing_reason(&self, estimate: &CostEstimate, operation: OperationType) -> String {
        match estimate.recommended_backend {
            ExecutionBackend::CpuScalar => {
                if estimate.estimated_rows < 100 {
                    "Dataset too small for vectorization overhead".to_string()
                } else {
                    "SIMD/GPU not beneficial at this scale".to_string()
                }
            }
            ExecutionBackend::CpuSimd => {
                let speedup = estimate.cpu_scalar_cost / estimate.cpu_simd_cost;
                format!("SIMD provides {:.1}x speedup", speedup)
            }
            ExecutionBackend::Gpu => {
                if operation == OperationType::VectorSearch {
                    "GPU optimal for vector similarity operations".to_string()
                } else {
                    let speedup = estimate.cpu_scalar_cost / estimate.gpu_cost;
                    format!("GPU provides {:.1}x speedup (including transfer)", speedup)
                }
            }
        }
    }
}

impl Default for CostRouter {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_dataset_uses_scalar() {
        let router = CostRouter::new_default();

        let backend = router.route(OperationType::Filter, 50, None);
        assert_eq!(backend, ExecutionBackend::CpuScalar);
    }

    #[test]
    fn test_medium_dataset_uses_simd() {
        let router = CostRouter::new_default();

        let backend = router.route(OperationType::Filter, 10_000, None);
        assert_eq!(backend, ExecutionBackend::CpuSimd);
    }

    #[test]
    fn test_large_dataset_considers_gpu() {
        let config = CostRouterConfig {
            gpu_enabled: true,
            gpu_row_threshold: 100_000,
            ..Default::default()
        };
        let router = CostRouter::new(config);

        let backend = router.route(OperationType::Aggregate, 500_000, None);
        // GPU should be selected for large aggregations
        assert_eq!(backend, ExecutionBackend::Gpu);
    }

    #[test]
    fn test_vector_search_prefers_gpu() {
        let config = CostRouterConfig {
            gpu_enabled: true,
            prefer_gpu_for_vectors: true,
            gpu_row_threshold: 1000,    // Lower threshold for this test
            gpu_transfer_overhead: 100, // Low transfer overhead to make GPU cost competitive
            ..Default::default()
        };
        let router = CostRouter::new(config);

        // For vector search above GPU threshold with low transfer overhead, GPU is preferred
        let backend = router.route(OperationType::VectorSearch, 10_000, None);
        assert_eq!(backend, ExecutionBackend::Gpu);
    }

    #[test]
    fn test_cost_estimates() {
        let router = CostRouter::new_default();

        let estimate = router.estimate_costs(OperationType::Filter, 100_000);

        // SIMD should be cheaper than scalar
        assert!(estimate.cpu_simd_cost < estimate.cpu_scalar_cost);
    }

    #[test]
    fn test_vectorized_config_generation() {
        let router = CostRouter::new_default();

        let scalar_config = router.to_vectorized_config(ExecutionBackend::CpuScalar);
        assert!(!scalar_config.enable_simd);
        assert!(!scalar_config.enable_gpu);

        let simd_config = router.to_vectorized_config(ExecutionBackend::CpuSimd);
        assert!(simd_config.enable_simd);
        assert!(!simd_config.enable_gpu);

        let gpu_config = router.to_vectorized_config(ExecutionBackend::Gpu);
        assert!(gpu_config.enable_simd);
        assert!(gpu_config.enable_gpu);
    }

    #[test]
    fn test_explain_routing() {
        let router = CostRouter::new_default();

        let explanation = router.explain_routing(OperationType::Aggregate, 50_000);

        assert!(explanation.contains("Aggregate"));
        assert!(explanation.contains("50000"));
        assert!(explanation.contains("Recommended:"));
    }

    #[test]
    fn test_route_query_plan() {
        let router = CostRouter::new_default();

        let operations = vec![
            (OperationType::Filter, 10_000),
            (OperationType::Sort, 5_000),
            (OperationType::Aggregate, 5_000),
        ];

        let routing = router.route_query_plan(&operations);

        assert_eq!(routing.len(), 3);
        assert_eq!(routing[0].0, OperationType::Filter);
    }

    #[test]
    fn test_disabled_gpu() {
        let config = CostRouterConfig {
            gpu_enabled: false,
            ..Default::default()
        };
        let router = CostRouter::new(config);

        // Even for huge datasets, should not use GPU when disabled
        let backend = router.route(OperationType::Aggregate, 10_000_000, None);
        assert_ne!(backend, ExecutionBackend::Gpu);
    }
}
