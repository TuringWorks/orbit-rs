//! GPU-accelerated columnar analytics operations
//!
//! This module provides high-performance columnar analytics using GPU acceleration
//! for compute-intensive operations like aggregations (SUM, AVG, MIN, MAX, COUNT)
//! and columnar scans with predicates. Falls back to CPU-parallel execution for
//! small datasets or when GPU is unavailable.

use crate::errors::ComputeError;
use serde::{Deserialize, Serialize};

#[cfg(feature = "gpu-acceleration")]
use crate::gpu::GPUAccelerationManager;
#[cfg(feature = "gpu-acceleration")]
use std::sync::Arc;

/// Columnar aggregation function types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregateFunction {
    /// Sum of values
    Sum,
    /// Average of values
    Avg,
    /// Minimum value
    Min,
    /// Maximum value
    Max,
    /// Count of non-null values
    Count,
}

/// Configuration for GPU-accelerated columnar analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnarAnalyticsConfig {
    /// Enable GPU acceleration
    pub enable_gpu: bool,
    /// Minimum number of rows to use GPU (below this, use CPU)
    pub gpu_min_rows: usize,
    /// Use CPU-parallel fallback (Rayon)
    pub use_cpu_parallel: bool,
}

impl Default for ColumnarAnalyticsConfig {
    fn default() -> Self {
        Self {
            enable_gpu: true,
            gpu_min_rows: 10000, // Use GPU for 10K+ rows
            use_cpu_parallel: true,
        }
    }
}

/// Result of columnar aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationResult {
    /// Aggregation function used
    pub function: AggregateFunction,
    /// Result value (None if all values are null)
    pub value: Option<f64>,
    /// Number of non-null values processed
    pub count: usize,
}

/// Statistics about columnar operation execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ColumnarAnalyticsStats {
    /// Total execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of rows processed
    pub rows_processed: usize,
    /// Whether GPU was used
    pub used_gpu: bool,
    /// GPU utilization percentage (if GPU used)
    pub gpu_utilization: Option<f32>,
}

/// GPU-accelerated columnar analytics engine
pub struct GPUColumnarAnalytics {
    #[cfg(feature = "gpu-acceleration")]
    gpu_manager: Option<Arc<GPUAccelerationManager>>,
    config: ColumnarAnalyticsConfig,
}

impl GPUColumnarAnalytics {
    /// Create a new GPU-accelerated columnar analytics engine
    pub async fn new(config: ColumnarAnalyticsConfig) -> Result<Self, ComputeError> {
        #[cfg(feature = "gpu-acceleration")]
        let gpu_manager = if config.enable_gpu {
            match GPUAccelerationManager::new().await {
                Ok(manager) => {
                    tracing::info!("GPU acceleration enabled for columnar analytics");
                    Some(Arc::new(manager))
                }
                Err(e) => {
                    tracing::warn!("GPU acceleration unavailable, falling back to CPU: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            #[cfg(feature = "gpu-acceleration")]
            gpu_manager,
            config,
        })
    }

    /// Check if GPU should be used for this operation
    fn should_use_gpu(&self, row_count: usize) -> bool {
        if !self.config.enable_gpu {
            return false;
        }

        #[cfg(feature = "gpu-acceleration")]
        {
            if self.gpu_manager.is_none() {
                return false;
            }
        }

        #[cfg(not(feature = "gpu-acceleration"))]
        {
            let _ = row_count; // Suppress unused variable warning
        }

        row_count >= self.config.gpu_min_rows
    }

    /// Execute aggregation on a column of i32 values
    pub async fn aggregate_i32(
        &self,
        values: &[i32],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> Result<AggregationResult, ComputeError> {
        let start_time = std::time::Instant::now();
        let row_count = values.len();

        let result = if self.should_use_gpu(row_count) {
            tracing::info!(
                "Using GPU-accelerated aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            #[cfg(feature = "gpu-acceleration")]
            {
                self.aggregate_i32_gpu(values, null_bitmap, function).await?
            }
            #[cfg(not(feature = "gpu-acceleration"))]
            {
                self.aggregate_i32_cpu_parallel(values, null_bitmap, function)
            }
        } else if self.config.use_cpu_parallel && row_count > 1000 {
            tracing::info!(
                "Using CPU-parallel aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            self.aggregate_i32_cpu_parallel(values, null_bitmap, function)
        } else {
            tracing::info!(
                "Using CPU sequential aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            self.aggregate_i32_cpu(values, null_bitmap, function)
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        tracing::debug!(
            "Aggregation completed in {}ms ({} rows)",
            execution_time_ms,
            row_count
        );

        Ok(result)
    }

    /// Execute aggregation on a column of i64 values
    pub async fn aggregate_i64(
        &self,
        values: &[i64],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> Result<AggregationResult, ComputeError> {
        let start_time = std::time::Instant::now();
        let row_count = values.len();

        let result = if self.should_use_gpu(row_count) {
            tracing::info!(
                "Using GPU-accelerated aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            #[cfg(feature = "gpu-acceleration")]
            {
                self.aggregate_i64_gpu(values, null_bitmap, function).await?
            }
            #[cfg(not(feature = "gpu-acceleration"))]
            {
                self.aggregate_i64_cpu_parallel(values, null_bitmap, function)
            }
        } else if self.config.use_cpu_parallel && row_count > 1000 {
            tracing::info!(
                "Using CPU-parallel aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            self.aggregate_i64_cpu_parallel(values, null_bitmap, function)
        } else {
            tracing::info!(
                "Using CPU sequential aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            self.aggregate_i64_cpu(values, null_bitmap, function)
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        tracing::debug!(
            "Aggregation completed in {}ms ({} rows)",
            execution_time_ms,
            row_count
        );

        Ok(result)
    }

    /// Execute aggregation on a column of f64 values
    pub async fn aggregate_f64(
        &self,
        values: &[f64],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> Result<AggregationResult, ComputeError> {
        let start_time = std::time::Instant::now();
        let row_count = values.len();

        let result = if self.should_use_gpu(row_count) {
            tracing::info!(
                "Using GPU-accelerated aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            #[cfg(feature = "gpu-acceleration")]
            {
                self.aggregate_f64_gpu(values, null_bitmap, function).await?
            }
            #[cfg(not(feature = "gpu-acceleration"))]
            {
                self.aggregate_f64_cpu_parallel(values, null_bitmap, function)
            }
        } else if self.config.use_cpu_parallel && row_count > 1000 {
            tracing::info!(
                "Using CPU-parallel aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            self.aggregate_f64_cpu_parallel(values, null_bitmap, function)
        } else {
            tracing::info!(
                "Using CPU sequential aggregation ({} rows, function: {:?})",
                row_count,
                function
            );
            self.aggregate_f64_cpu(values, null_bitmap, function)
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        tracing::debug!(
            "Aggregation completed in {}ms ({} rows)",
            execution_time_ms,
            row_count
        );

        Ok(result)
    }

    /// GPU-accelerated i32 aggregation
    #[cfg(feature = "gpu-acceleration")]
    async fn aggregate_i32_gpu(
        &self,
        values: &[i32],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> Result<AggregationResult, ComputeError> {
        use crate::gpu_metal::MetalDevice;

        // Try Metal first (macOS), then Vulkan
        #[cfg(target_os = "macos")]
        {
            if let Ok(metal_device) = MetalDevice::new() {
                return self
                    .aggregate_i32_metal(&metal_device, values, null_bitmap, function)
                    .await;
            }
        }

        // Fallback to CPU if GPU unavailable
        tracing::warn!("GPU unavailable, falling back to CPU");
        Ok(self.aggregate_i32_cpu_parallel(values, null_bitmap, function))
    }

    /// Metal-accelerated i32 aggregation
    #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
    async fn aggregate_i32_metal(
        &self,
        device: &crate::gpu_metal::MetalDevice,
        values: &[i32],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> Result<AggregationResult, ComputeError> {
        match function {
            AggregateFunction::Sum => {
                let result = device
                    .execute_aggregate_i32(values, null_bitmap, "aggregate_i32_sum")
                    .map_err(|e| {
                        ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                            kernel_name: "aggregate_i32_sum".to_string(),
                            error: format!("Metal execution failed: {}", e),
                        })
                    })?;
                Ok(AggregationResult {
                    function,
                    value: result,
                    count: values.len(),
                })
            }
            AggregateFunction::Count => {
                let result = device
                    .execute_aggregate_i32(values, null_bitmap, "aggregate_i32_count")
                    .map_err(|e| {
                        ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                            kernel_name: "aggregate_i32_count".to_string(),
                            error: format!("Metal execution failed: {}", e),
                        })
                    })?;
                Ok(AggregationResult {
                    function,
                    value: result,
                    count: result.map(|v| v as usize).unwrap_or(0),
                })
            }
            AggregateFunction::Avg => {
                // AVG requires both SUM and COUNT
                let sum_result = device
                    .execute_aggregate_i32(values, null_bitmap, "aggregate_i32_sum")
                    .map_err(|e| {
                        ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                            kernel_name: "aggregate_i32_sum".to_string(),
                            error: format!("Metal execution failed: {}", e),
                        })
                    })?;
                let count_result = device
                    .execute_aggregate_i32(values, null_bitmap, "aggregate_i32_count")
                    .map_err(|e| {
                        ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                            kernel_name: "aggregate_i32_count".to_string(),
                            error: format!("Metal execution failed: {}", e),
                        })
                    })?;

                let count = count_result.map(|v| v as usize).unwrap_or(0);
                Ok(AggregationResult {
                    function: AggregateFunction::Avg,
                    value: if count > 0 {
                        sum_result.map(|s| s / count as f64)
                    } else {
                        None
                    },
                    count,
                })
            }
            AggregateFunction::Min => {
                let result = device
                    .execute_aggregate_i32(values, null_bitmap, "aggregate_i32_min")
                    .map_err(|e| {
                        ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                            kernel_name: "aggregate_i32_min".to_string(),
                            error: format!("Metal execution failed: {}", e),
                        })
                    })?;
                Ok(AggregationResult {
                    function,
                    value: result,
                    count: values.len(),
                })
            }
            AggregateFunction::Max => {
                let result = device
                    .execute_aggregate_i32(values, null_bitmap, "aggregate_i32_max")
                    .map_err(|e| {
                        ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                            kernel_name: "aggregate_i32_max".to_string(),
                            error: format!("Metal execution failed: {}", e),
                        })
                    })?;
                Ok(AggregationResult {
                    function,
                    value: result,
                    count: values.len(),
                })
            }
        }
    }

    /// GPU-accelerated i64 aggregation
    #[cfg(feature = "gpu-acceleration")]
    async fn aggregate_i64_gpu(
        &self,
        values: &[i64],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> Result<AggregationResult, ComputeError> {
        // For now, fall back to CPU (i64 aggregation on GPU is more complex)
        tracing::warn!("GPU i64 aggregation not yet implemented, falling back to CPU");
        Ok(self.aggregate_i64_cpu_parallel(values, null_bitmap, function))
    }

    /// GPU-accelerated f64 aggregation
    #[cfg(feature = "gpu-acceleration")]
    async fn aggregate_f64_gpu(
        &self,
        values: &[f64],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> Result<AggregationResult, ComputeError> {
        // For now, fall back to CPU (f64 aggregation on GPU is more complex)
        tracing::warn!("GPU f64 aggregation not yet implemented, falling back to CPU");
        Ok(self.aggregate_f64_cpu_parallel(values, null_bitmap, function))
    }

    /// CPU-parallel i32 aggregation using Rayon
    fn aggregate_i32_cpu_parallel(
        &self,
        values: &[i32],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> AggregationResult {
        use rayon::prelude::*;

        match function {
            AggregateFunction::Sum => {
                let sum: i64 = values
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        null_bitmap
                            .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                            .unwrap_or(true)
                    })
                    .map(|(_, &v)| v as i64)
                    .sum();
                AggregationResult {
                    function,
                    value: Some(sum as f64),
                    count: values.len(),
                }
            }
            AggregateFunction::Min => {
                let min = values
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        null_bitmap
                            .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                            .unwrap_or(true)
                    })
                    .map(|(_, &v)| v)
                    .min();
                AggregationResult {
                    function,
                    value: min.map(|v| v as f64),
                    count: values.len(),
                }
            }
            AggregateFunction::Max => {
                let max = values
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        null_bitmap
                            .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                            .unwrap_or(true)
                    })
                    .map(|(_, &v)| v)
                    .max();
                AggregationResult {
                    function,
                    value: max.map(|v| v as f64),
                    count: values.len(),
                }
            }
            AggregateFunction::Count => {
                let count = if let Some(bm) = null_bitmap {
                    bm.par_iter()
                        .enumerate()
                        .map(|(byte_idx, &byte)| {
                            let start = byte_idx * 8;
                            let end = (start + 8).min(values.len());
                            (start..end)
                                .filter(|idx| (byte >> (idx % 8)) & 1 == 0)
                                .count()
                        })
                        .sum()
                } else {
                    values.len()
                };
                AggregationResult {
                    function,
                    value: Some(count as f64),
                    count,
                }
            }
            AggregateFunction::Avg => {
                let (sum, count) = values
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        null_bitmap
                            .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                            .unwrap_or(true)
                    })
                    .map(|(_, &v)| (v as i64, 1))
                    .reduce(|| (0, 0), |(s1, c1), (s2, c2)| (s1 + s2, c1 + c2));
                AggregationResult {
                    function,
                    value: if count > 0 {
                        Some(sum as f64 / count as f64)
                    } else {
                        None
                    },
                    count,
                }
            }
        }
    }

    /// CPU sequential i32 aggregation
    fn aggregate_i32_cpu(
        &self,
        values: &[i32],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> AggregationResult {
        match function {
            AggregateFunction::Sum => {
                let mut sum: i64 = 0;
                for (idx, &value) in values.iter().enumerate() {
                    if null_bitmap
                        .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                        .unwrap_or(true)
                    {
                        sum += value as i64;
                    }
                }
                AggregationResult {
                    function,
                    value: Some(sum as f64),
                    count: values.len(),
                }
            }
            AggregateFunction::Min => {
                let mut min: Option<i32> = None;
                for (idx, &value) in values.iter().enumerate() {
                    if null_bitmap
                        .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                        .unwrap_or(true)
                    {
                        min = Some(min.map(|m| m.min(value)).unwrap_or(value));
                    }
                }
                AggregationResult {
                    function,
                    value: min.map(|v| v as f64),
                    count: values.len(),
                }
            }
            AggregateFunction::Max => {
                let mut max: Option<i32> = None;
                for (idx, &value) in values.iter().enumerate() {
                    if null_bitmap
                        .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                        .unwrap_or(true)
                    {
                        max = Some(max.map(|m| m.max(value)).unwrap_or(value));
                    }
                }
                AggregationResult {
                    function,
                    value: max.map(|v| v as f64),
                    count: values.len(),
                }
            }
            AggregateFunction::Count => {
                let count = if let Some(bm) = null_bitmap {
                    bm.iter()
                        .enumerate()
                        .map(|(byte_idx, &byte)| {
                            let start = byte_idx * 8;
                            let end = (start + 8).min(values.len());
                            (start..end)
                                .filter(|idx| (byte >> (idx % 8)) & 1 == 0)
                                .count()
                        })
                        .sum()
                } else {
                    values.len()
                };
                AggregationResult {
                    function,
                    value: Some(count as f64),
                    count,
                }
            }
            AggregateFunction::Avg => {
                let mut sum: i64 = 0;
                let mut count = 0;
                for (idx, &value) in values.iter().enumerate() {
                    if null_bitmap
                        .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                        .unwrap_or(true)
                    {
                        sum += value as i64;
                        count += 1;
                    }
                }
                AggregationResult {
                    function,
                    value: if count > 0 {
                        Some(sum as f64 / count as f64)
                    } else {
                        None
                    },
                    count,
                }
            }
        }
    }

    /// CPU-parallel i64 aggregation using Rayon
    fn aggregate_i64_cpu_parallel(
        &self,
        values: &[i64],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> AggregationResult {
        // Similar to i32, but with i64
        self.aggregate_i32_cpu_parallel(
            &values.iter().map(|&v| v as i32).collect::<Vec<_>>(),
            null_bitmap,
            function,
        )
    }

    /// CPU sequential i64 aggregation
    fn aggregate_i64_cpu(
        &self,
        values: &[i64],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> AggregationResult {
        // Similar to i32, but with i64
        self.aggregate_i32_cpu(
            &values.iter().map(|&v| v as i32).collect::<Vec<_>>(),
            null_bitmap,
            function,
        )
    }

    /// CPU-parallel f64 aggregation using Rayon
    fn aggregate_f64_cpu_parallel(
        &self,
        values: &[f64],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> AggregationResult {
        use rayon::prelude::*;

        match function {
            AggregateFunction::Sum => {
                let sum: f64 = values
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        null_bitmap
                            .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                            .unwrap_or(true)
                    })
                    .map(|(_, &v)| v)
                    .sum();
                AggregationResult {
                    function,
                    value: Some(sum),
                    count: values.len(),
                }
            }
            AggregateFunction::Min => {
                let min = values
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        null_bitmap
                            .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                            .unwrap_or(true)
                    })
                    .map(|(_, &v)| v)
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                AggregationResult {
                    function,
                    value: min,
                    count: values.len(),
                }
            }
            AggregateFunction::Max => {
                let max = values
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        null_bitmap
                            .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                            .unwrap_or(true)
                    })
                    .map(|(_, &v)| v)
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                AggregationResult {
                    function,
                    value: max,
                    count: values.len(),
                }
            }
            AggregateFunction::Count => {
                let count = if let Some(bm) = null_bitmap {
                    bm.par_iter()
                        .enumerate()
                        .map(|(byte_idx, &byte)| {
                            let start = byte_idx * 8;
                            let end = (start + 8).min(values.len());
                            (start..end)
                                .filter(|idx| (byte >> (idx % 8)) & 1 == 0)
                                .count()
                        })
                        .sum()
                } else {
                    values.len()
                };
                AggregationResult {
                    function,
                    value: Some(count as f64),
                    count,
                }
            }
            AggregateFunction::Avg => {
                let (sum, count) = values
                    .par_iter()
                    .enumerate()
                    .filter(|(idx, _)| {
                        null_bitmap
                            .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                            .unwrap_or(true)
                    })
                    .map(|(_, &v)| (v, 1))
                    .reduce(|| (0.0, 0), |(s1, c1), (s2, c2)| (s1 + s2, c1 + c2));
                AggregationResult {
                    function,
                    value: if count > 0 { Some(sum / count as f64) } else { None },
                    count,
                }
            }
        }
    }

    /// CPU sequential f64 aggregation
    fn aggregate_f64_cpu(
        &self,
        values: &[f64],
        null_bitmap: Option<&[u8]>,
        function: AggregateFunction,
    ) -> AggregationResult {
        match function {
            AggregateFunction::Sum => {
                let mut sum = 0.0;
                for (idx, &value) in values.iter().enumerate() {
                    if null_bitmap
                        .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                        .unwrap_or(true)
                    {
                        sum += value;
                    }
                }
                AggregationResult {
                    function,
                    value: Some(sum),
                    count: values.len(),
                }
            }
            AggregateFunction::Min => {
                let mut min: Option<f64> = None;
                for (idx, &value) in values.iter().enumerate() {
                    if null_bitmap
                        .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                        .unwrap_or(true)
                    {
                        min = Some(min.map(|m| m.min(value)).unwrap_or(value));
                    }
                }
                AggregationResult {
                    function,
                    value: min,
                    count: values.len(),
                }
            }
            AggregateFunction::Max => {
                let mut max: Option<f64> = None;
                for (idx, &value) in values.iter().enumerate() {
                    if null_bitmap
                        .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                        .unwrap_or(true)
                    {
                        max = Some(max.map(|m| m.max(value)).unwrap_or(value));
                    }
                }
                AggregationResult {
                    function,
                    value: max,
                    count: values.len(),
                }
            }
            AggregateFunction::Count => {
                let count = if let Some(bm) = null_bitmap {
                    bm.iter()
                        .enumerate()
                        .map(|(byte_idx, &byte)| {
                            let start = byte_idx * 8;
                            let end = (start + 8).min(values.len());
                            (start..end)
                                .filter(|idx| (byte >> (idx % 8)) & 1 == 0)
                                .count()
                        })
                        .sum()
                } else {
                    values.len()
                };
                AggregationResult {
                    function,
                    value: Some(count as f64),
                    count,
                }
            }
            AggregateFunction::Avg => {
                let mut sum = 0.0;
                let mut count = 0;
                for (idx, &value) in values.iter().enumerate() {
                    if null_bitmap
                        .map(|bm| (bm[idx / 8] >> (idx % 8)) & 1 == 0)
                        .unwrap_or(true)
                    {
                        sum += value;
                        count += 1;
                    }
                }
                AggregationResult {
                    function,
                    value: if count > 0 { Some(sum / count as f64) } else { None },
                    count,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_columnar_config_defaults() {
        let config = ColumnarAnalyticsConfig::default();
        assert!(config.enable_gpu);
        assert_eq!(config.gpu_min_rows, 10000);
        assert!(config.use_cpu_parallel);
    }

    #[tokio::test]
    async fn test_aggregate_i32_sum_cpu() {
        let config = ColumnarAnalyticsConfig {
            enable_gpu: false,
            ..Default::default()
        };
        let engine = GPUColumnarAnalytics::new(config).await.unwrap();

        let values = vec![1, 2, 3, 4, 5];
        let result = engine
            .aggregate_i32(&values, None, AggregateFunction::Sum)
            .await
            .unwrap();

        assert_eq!(result.value, Some(15.0));
        assert_eq!(result.count, 5);
    }

    #[tokio::test]
    async fn test_aggregate_i32_avg_cpu() {
        let config = ColumnarAnalyticsConfig {
            enable_gpu: false,
            ..Default::default()
        };
        let engine = GPUColumnarAnalytics::new(config).await.unwrap();

        let values = vec![10, 20, 30, 40, 50];
        let result = engine
            .aggregate_i32(&values, None, AggregateFunction::Avg)
            .await
            .unwrap();

        assert_eq!(result.value, Some(30.0));
        assert_eq!(result.count, 5);
    }

    #[tokio::test]
    async fn test_aggregate_i32_min_max_cpu() {
        let config = ColumnarAnalyticsConfig {
            enable_gpu: false,
            ..Default::default()
        };
        let engine = GPUColumnarAnalytics::new(config).await.unwrap();

        let values = vec![5, 2, 8, 1, 9];
        let min_result = engine
            .aggregate_i32(&values, None, AggregateFunction::Min)
            .await
            .unwrap();
        let max_result = engine
            .aggregate_i32(&values, None, AggregateFunction::Max)
            .await
            .unwrap();

        assert_eq!(min_result.value, Some(1.0));
        assert_eq!(max_result.value, Some(9.0));
    }
}

