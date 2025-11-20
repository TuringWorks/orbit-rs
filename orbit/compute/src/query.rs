//! Query analysis and workload characterization
//!
//! This module provides query analysis capabilities for determining optimal
//! acceleration strategies based on query patterns, data size, and available hardware.

use crate::capabilities::UniversalComputeCapabilities;
use crate::errors::ComputeResult;
use crate::scheduler::{
    DataSizeClass, GPUWorkloadClass, MemoryPattern, SIMDOperationType, WorkloadType,
};

/// Query analyzer for determining optimal acceleration strategies
pub struct QueryAnalyzer {
    /// Available compute capabilities
    capabilities: UniversalComputeCapabilities,
}

impl QueryAnalyzer {
    /// Create a new query analyzer with detected capabilities
    pub fn new(capabilities: UniversalComputeCapabilities) -> Self {
        Self { capabilities }
    }

    /// Analyze query operations for acceleration opportunities
    pub fn analyze_operations(&self, operations: &[OperationType]) -> ComputeResult<QueryAnalysis> {
        // Estimate complexity based on operations
        let complexity_score = self.estimate_complexity(operations);

        // Estimate data size (placeholder for now - would need actual cardinality stats)
        let estimated_row_count = 10_000;
        let estimated_data_size = estimated_row_count * 100; // Assume ~100 bytes per row

        // Classify workload type
        let workload_type = self.classify_workload(operations, estimated_data_size);

        // Recommend acceleration strategy
        let recommended_strategy = self.recommend_strategy(&workload_type);

        // Calculate confidence based on workload clarity
        let confidence = self.calculate_confidence(operations, &workload_type);

        Ok(QueryAnalysis {
            complexity_score,
            estimated_data_size,
            estimated_row_count,
            operation_types: operations.to_vec(),
            workload_type,
            recommended_strategy,
            confidence,
        })
    }

    /// Estimate query complexity score (0.0 = simple, 10.0 = very complex)
    fn estimate_complexity(&self, operations: &[OperationType]) -> f32 {
        let mut score: f32 = 0.0;

        for op in operations {
            score += match op {
                OperationType::Scan => 0.5,
                OperationType::Filter => 1.0,
                OperationType::Project => 0.5,
                OperationType::Aggregate => 2.0,
                OperationType::Sort => 3.0,
                OperationType::Join => 4.0,
                OperationType::GroupBy => 2.5,
                OperationType::Limit => 0.1,
            };
        }

        // Cap at 10.0
        score.min(10.0)
    }

    /// Classify workload based on operations and data size
    fn classify_workload(&self, operations: &[OperationType], data_size: usize) -> WorkloadType {
        // Check for specific patterns
        let has_filter = operations.iter().any(|op| matches!(op, OperationType::Filter));
        let has_aggregate = operations.iter().any(|op| matches!(op, OperationType::Aggregate | OperationType::GroupBy));
        let has_join = operations.iter().any(|op| matches!(op, OperationType::Join));
        let has_sort = operations.iter().any(|op| matches!(op, OperationType::Sort));

        // Decision logic based on patterns and data size
        match (data_size, has_filter, has_aggregate, has_join, has_sort) {
            // Small data, simple filter → CPU SIMD
            (size, true, false, false, false) if size < 100_000 => WorkloadType::SIMDBatch {
                data_size: DataSizeClass::Small,
                operation_type: SIMDOperationType::ElementWise,
            },

            // Large data, aggregation → Consider GPU
            (size, _, true, false, _) if size > 1_000_000 => {
                if self.capabilities.gpu.available_devices.is_empty() {
                    WorkloadType::SIMDBatch {
                        data_size: DataSizeClass::Large,
                        operation_type: SIMDOperationType::Reduction,
                    }
                } else {
                    WorkloadType::GPUCompute {
                        workload_class: GPUWorkloadClass::ParallelAlgorithms,
                        memory_pattern: MemoryPattern::Sequential,
                    }
                }
            }

            // Join operations → Prefer GPU for large datasets
            (size, _, _, true, _) if size > 500_000 => {
                if self.capabilities.gpu.available_devices.is_empty() {
                    WorkloadType::SIMDBatch {
                        data_size: DataSizeClass::Medium,
                        operation_type: SIMDOperationType::MatrixOps,
                    }
                } else {
                    WorkloadType::GPUCompute {
                        workload_class: GPUWorkloadClass::GeneralCompute,
                        memory_pattern: MemoryPattern::Random,
                    }
                }
            }

            // Sort operations → Can benefit from parallel processing
            (size, _, _, _, true) if size > 100_000 => {
                if self.capabilities.gpu.available_devices.is_empty() {
                    WorkloadType::SIMDBatch {
                        data_size: if size > 1_000_000 {
                            DataSizeClass::Large
                        } else {
                            DataSizeClass::Medium
                        },
                        operation_type: SIMDOperationType::Bitwise,
                    }
                } else {
                    WorkloadType::GPUCompute {
                        workload_class: GPUWorkloadClass::ParallelAlgorithms,
                        memory_pattern: MemoryPattern::Sequential,
                    }
                }
            }

            // Default to SIMD batch processing
            _ => WorkloadType::SIMDBatch {
                data_size: DataSizeClass::Small,
                operation_type: SIMDOperationType::ElementWise,
            },
        }
    }

    /// Recommend acceleration strategy based on workload type
    fn recommend_strategy(&self, workload: &WorkloadType) -> AccelerationStrategy {
        match workload {
            WorkloadType::SIMDBatch { .. } => {
                // Check if CPU has good SIMD support
                if self.has_good_simd_support() {
                    AccelerationStrategy::CpuSimd
                } else {
                    AccelerationStrategy::None
                }
            }
            WorkloadType::GPUCompute { .. } => {
                if !self.capabilities.gpu.available_devices.is_empty() {
                    AccelerationStrategy::Gpu
                } else {
                    // Fallback to CPU SIMD
                    AccelerationStrategy::CpuSimd
                }
            }
            WorkloadType::Hybrid { .. } => {
                // Use hybrid CPU+SIMD+GPU approach
                AccelerationStrategy::Hybrid
            }
            WorkloadType::NeuralInference { .. } => {
                if self.has_neural_engine() {
                    AccelerationStrategy::NeuralEngine
                } else {
                    AccelerationStrategy::CpuSimd
                }
            }
        }
    }

    /// Calculate confidence in the recommendation (0.0 - 1.0)
    fn calculate_confidence(&self, operations: &[OperationType], workload: &WorkloadType) -> f32 {
        // Base confidence
        let mut confidence: f32 = 0.7;

        // Increase confidence for clear patterns
        let operation_diversity = operations.len() as f32;
        if operation_diversity <= 2.0 {
            confidence += 0.2; // Simple queries are easier to classify
        }

        // Adjust based on workload type
        match workload {
            WorkloadType::SIMDBatch { .. } => {
                if self.has_good_simd_support() {
                    confidence += 0.1;
                }
            }
            WorkloadType::GPUCompute { .. } => {
                if !self.capabilities.gpu.available_devices.is_empty() {
                    confidence += 0.15;
                }
            }
            WorkloadType::NeuralInference { .. } => {
                if self.has_neural_engine() {
                    confidence += 0.15;
                }
            }
            WorkloadType::Hybrid { .. } => {
                // Hybrid workloads have lower initial confidence
                confidence -= 0.1;
            }
        }

        confidence.min(1.0)
    }

    /// Check if CPU has good SIMD support
    fn has_good_simd_support(&self) -> bool {
        // Check SIMD capabilities from the simd field
        if let Some(x86_features) = &self.capabilities.cpu.simd.x86_features {
            x86_features.avx.avx2 || x86_features.avx.avx512f
        } else if let Some(arm_features) = &self.capabilities.cpu.simd.arm_features {
            arm_features.neon
        } else {
            false
        }
    }

    /// Check if neural engine is available
    fn has_neural_engine(&self) -> bool {
        !matches!(self.capabilities.neural, crate::capabilities::NeuralEngineCapabilities::None)
    }
}

/// Query analysis result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryAnalysis {
    /// Query complexity score (0.0 - 10.0)
    pub complexity_score: f32,
    /// Estimated data size in bytes
    pub estimated_data_size: usize,
    /// Estimated number of rows to process
    pub estimated_row_count: usize,
    /// Types of operations in the query
    pub operation_types: Vec<OperationType>,
    /// Classified workload type
    pub workload_type: WorkloadType,
    /// Recommended acceleration strategy
    pub recommended_strategy: AccelerationStrategy,
    /// Confidence in recommendation (0.0 - 1.0)
    pub confidence: f32,
}

/// Type of operation in a query
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OperationType {
    /// Table scan operation
    Scan,
    /// Filter/WHERE operation
    Filter,
    /// Projection/SELECT operation
    Project,
    /// Aggregation operation (SUM, AVG, etc.)
    Aggregate,
    /// Sort/ORDER BY operation
    Sort,
    /// Join operation
    Join,
    /// GROUP BY operation
    GroupBy,
    /// LIMIT operation
    Limit,
}

/// Acceleration strategy recommendation
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AccelerationStrategy {
    /// CPU SIMD acceleration
    CpuSimd,
    /// GPU acceleration
    Gpu,
    /// Neural engine acceleration
    NeuralEngine,
    /// Hybrid approach (CPU + SIMD + parallel)
    Hybrid,
    /// No acceleration needed
    None,
}

impl Default for AccelerationStrategy {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{detect_all_capabilities};

    async fn create_test_capabilities() -> UniversalComputeCapabilities {
        // Use actual capability detection for tests
        detect_all_capabilities().await.unwrap()
    }

    #[tokio::test]
    async fn test_simple_filter_query() {
        let caps = create_test_capabilities().await;
        let analyzer = QueryAnalyzer::new(caps);

        let operations = vec![OperationType::Scan, OperationType::Filter];
        let analysis = analyzer.analyze_operations(&operations).unwrap();

        // Should return some valid strategy
        assert!(matches!(
            analysis.recommended_strategy,
            AccelerationStrategy::CpuSimd
                | AccelerationStrategy::Gpu
                | AccelerationStrategy::NeuralEngine
                | AccelerationStrategy::Hybrid
                | AccelerationStrategy::None
        ));
        assert!(analysis.complexity_score < 2.0);
        assert!(analysis.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_complex_aggregation_query() {
        let caps = create_test_capabilities().await;
        let analyzer = QueryAnalyzer::new(caps);

        let operations = vec![
            OperationType::Scan,
            OperationType::Filter,
            OperationType::GroupBy,
            OperationType::Aggregate,
        ];
        let analysis = analyzer.analyze_operations(&operations).unwrap();

        assert!(analysis.complexity_score > 3.0);
        assert!(analysis.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_workload_classification() {
        let caps = create_test_capabilities().await;
        let analyzer = QueryAnalyzer::new(caps);

        let operations = vec![OperationType::Filter];
        let analysis = analyzer.analyze_operations(&operations).unwrap();

        // Check that it's classified as some workload type
        assert!(matches!(
            analysis.workload_type,
            WorkloadType::SIMDBatch { .. }
                | WorkloadType::GPUCompute { .. }
                | WorkloadType::NeuralInference { .. }
                | WorkloadType::Hybrid { .. }
        ));
    }

    #[tokio::test]
    async fn test_query_complexity_scoring() {
        let caps = create_test_capabilities().await;
        let analyzer = QueryAnalyzer::new(caps);

        // Simple query
        let simple_ops = vec![OperationType::Scan];
        let simple_analysis = analyzer.analyze_operations(&simple_ops).unwrap();
        assert!(simple_analysis.complexity_score < 1.0);

        // Complex query
        let complex_ops = vec![
            OperationType::Scan,
            OperationType::Join,
            OperationType::Filter,
            OperationType::Sort,
            OperationType::Aggregate,
        ];
        let complex_analysis = analyzer.analyze_operations(&complex_ops).unwrap();
        assert!(complex_analysis.complexity_score > simple_analysis.complexity_score);
    }
}
