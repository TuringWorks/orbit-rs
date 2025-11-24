//! GPU-accelerated vector similarity search
//!
//! This module provides high-performance vector similarity calculations using GPU acceleration
//! for compute-intensive operations like cosine similarity, euclidean distance, dot product,
//! and manhattan distance. Falls back to CPU-parallel execution for small datasets or when
//! GPU is unavailable.

use crate::errors::ComputeError;
use serde::{Deserialize, Serialize};

#[cfg(feature = "gpu-acceleration")]
use crate::gpu::GPUAccelerationManager;
#[cfg(feature = "gpu-acceleration")]
use std::sync::Arc;

/// Vector similarity metric types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VectorSimilarityMetric {
    /// Cosine similarity (normalized dot product)
    Cosine,
    /// Euclidean distance (L2 norm)
    Euclidean,
    /// Dot product similarity
    DotProduct,
    /// Manhattan distance (L1 norm)
    Manhattan,
}

/// Configuration for GPU-accelerated vector similarity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSimilarityConfig {
    /// Enable GPU acceleration
    pub enable_gpu: bool,
    /// Minimum number of vectors to use GPU (below this, use CPU)
    pub gpu_min_vectors: usize,
    /// Minimum vector dimension to use GPU (below this, use CPU)
    pub gpu_min_dimension: usize,
    /// Use CPU-parallel fallback (Rayon)
    pub use_cpu_parallel: bool,
}

impl Default for VectorSimilarityConfig {
    fn default() -> Self {
        Self {
            enable_gpu: true,
            gpu_min_vectors: 1000,  // Use GPU for 1000+ vectors
            gpu_min_dimension: 128, // Use GPU for 128+ dimensions
            use_cpu_parallel: true,
        }
    }
}

/// Result of vector similarity search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSimilarityResult {
    /// Vector index in the input array
    pub index: usize,
    /// Similarity score (higher = more similar)
    pub score: f32,
    /// Metric used for calculation
    pub metric: VectorSimilarityMetric,
}

/// Statistics about similarity search execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VectorSimilarityStats {
    /// Total execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of vectors processed
    pub vectors_processed: usize,
    /// Vector dimension
    pub dimension: usize,
    /// Whether GPU was used
    pub used_gpu: bool,
    /// GPU utilization percentage (if GPU used)
    pub gpu_utilization: Option<f32>,
}

/// GPU-accelerated vector similarity engine
pub struct GPUVectorSimilarity {
    #[cfg(feature = "gpu-acceleration")]
    gpu_manager: Option<Arc<GPUAccelerationManager>>,
    config: VectorSimilarityConfig,
}

impl GPUVectorSimilarity {
    /// Create a new GPU-accelerated vector similarity engine
    pub async fn new(config: VectorSimilarityConfig) -> Result<Self, ComputeError> {
        #[cfg(feature = "gpu-acceleration")]
        let gpu_manager = if config.enable_gpu {
            match GPUAccelerationManager::new().await {
                Ok(manager) => {
                    tracing::info!("GPU acceleration enabled for vector similarity");
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
    fn should_use_gpu(&self, vector_count: usize, dimension: usize) -> bool {
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
            let _ = dimension; // Suppress unused variable warning
        }

        vector_count >= self.config.gpu_min_vectors
            && dimension >= self.config.gpu_min_dimension
    }

    /// Calculate similarity between a query vector and multiple candidate vectors
    pub async fn batch_similarity(
        &self,
        query_vector: &[f32],
        candidate_vectors: &[Vec<f32>],
        metric: VectorSimilarityMetric,
    ) -> Result<Vec<VectorSimilarityResult>, ComputeError> {
        let start_time = std::time::Instant::now();
        let vector_count = candidate_vectors.len();
        let dimension = query_vector.len();

        // Validate dimensions
        for (idx, candidate) in candidate_vectors.iter().enumerate() {
            if candidate.len() != dimension {
                return Err(ComputeError::execution(
                    crate::errors::ExecutionError::InvalidKernelParameters {
                        parameter: format!("candidate_vector_{}", idx),
                        value: format!("dimension={}, expected={}", candidate.len(), dimension),
                    },
                ));
            }
        }

        let results = if self.should_use_gpu(vector_count, dimension) {
            tracing::info!(
                "Using GPU-accelerated vector similarity ({} vectors, {} dimensions)",
                vector_count,
                dimension
            );
            #[cfg(feature = "gpu-acceleration")]
            {
                self.batch_similarity_gpu(query_vector, candidate_vectors, metric)
                    .await?
            }
            #[cfg(not(feature = "gpu-acceleration"))]
            {
                // Fallback to CPU if GPU not available
                self.batch_similarity_cpu_parallel(query_vector, candidate_vectors, metric)
            }
        } else if self.config.use_cpu_parallel && vector_count > 100 {
            tracing::info!(
                "Using CPU-parallel vector similarity ({} vectors, {} dimensions)",
                vector_count,
                dimension
            );
            self.batch_similarity_cpu_parallel(query_vector, candidate_vectors, metric)
        } else {
            tracing::info!(
                "Using CPU sequential vector similarity ({} vectors, {} dimensions)",
                vector_count,
                dimension
            );
            self.batch_similarity_cpu(query_vector, candidate_vectors, metric)
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        tracing::debug!(
            "Vector similarity search completed in {}ms ({} vectors)",
            execution_time_ms,
            vector_count
        );

        Ok(results)
    }

    /// GPU-accelerated batch similarity calculation
    #[cfg(feature = "gpu-acceleration")]
    async fn batch_similarity_gpu(
        &self,
        query_vector: &[f32],
        candidate_vectors: &[Vec<f32>],
        metric: VectorSimilarityMetric,
    ) -> Result<Vec<VectorSimilarityResult>, ComputeError> {
        // Try Metal first (macOS), then Vulkan
        #[cfg(all(target_os = "macos", feature = "gpu-acceleration"))]
        {
            use crate::gpu_metal::MetalDevice;
            if let Ok(metal_device) = MetalDevice::new() {
                return self
                    .batch_similarity_metal(&metal_device, query_vector, candidate_vectors, metric)
                    .await;
            }
        }

        #[cfg(all(feature = "gpu-acceleration", feature = "gpu-vulkan"))]
        {
            use crate::gpu_vulkan::VulkanDevice;
            if let Ok(mut vulkan_device) = VulkanDevice::new() {
                return self
                    .batch_similarity_vulkan(
                        &mut vulkan_device,
                        query_vector,
                        candidate_vectors,
                        metric,
                    )
                    .await;
            }
        }

        // Fallback to CPU if GPU unavailable
        tracing::warn!("GPU unavailable, falling back to CPU");
        Ok(self.batch_similarity_cpu_parallel(query_vector, candidate_vectors, metric))
    }

    /// Metal-accelerated batch similarity calculation
    #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
    async fn batch_similarity_metal(
        &self,
        device: &crate::gpu_metal::MetalDevice,
        query_vector: &[f32],
        candidate_vectors: &[Vec<f32>],
        metric: VectorSimilarityMetric,
    ) -> Result<Vec<VectorSimilarityResult>, ComputeError> {

        let kernel_name = match metric {
            VectorSimilarityMetric::Cosine => "vector_cosine_similarity",
            VectorSimilarityMetric::Euclidean => "vector_euclidean_distance",
            VectorSimilarityMetric::DotProduct => "vector_dot_product",
            VectorSimilarityMetric::Manhattan => "vector_manhattan_distance",
        };

        // Flatten candidate vectors into a single array
        let vector_count = candidate_vectors.len();
        let dimension = query_vector.len();
        let mut flat_vectors = Vec::with_capacity(vector_count * dimension);
        for vector in candidate_vectors {
            flat_vectors.extend_from_slice(vector);
        }

        // Execute GPU kernel
        let scores = device
            .execute_vector_similarity(
                query_vector,
                &flat_vectors,
                vector_count,
                dimension,
                kernel_name,
            )
            .map_err(|e| {
                ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                    kernel_name: kernel_name.to_string(),
                    error: format!("Metal execution failed: {}", e),
                })
            })?;

        // Convert scores to results
        let mut results = Vec::with_capacity(vector_count);
        for (idx, &score) in scores.iter().enumerate() {
            results.push(VectorSimilarityResult {
                index: idx,
                score,
                metric,
            });
        }

        Ok(results)
    }

    /// Vulkan-accelerated batch similarity calculation
    #[cfg(all(feature = "gpu-acceleration", feature = "gpu-vulkan"))]
    async fn batch_similarity_vulkan(
        &self,
        device: &mut crate::gpu_vulkan::VulkanDevice,
        query_vector: &[f32],
        candidate_vectors: &[Vec<f32>],
        metric: VectorSimilarityMetric,
    ) -> Result<Vec<VectorSimilarityResult>, ComputeError> {
        // Convert metric to shader constant
        let metric_value = match metric {
            VectorSimilarityMetric::Cosine => 0u32,
            VectorSimilarityMetric::Euclidean => 1u32,
            VectorSimilarityMetric::DotProduct => 2u32,
            VectorSimilarityMetric::Manhattan => 3u32,
        };

        // Flatten candidate vectors into a single array
        let vector_count = candidate_vectors.len();
        let dimension = query_vector.len();
        let mut flat_vectors = Vec::with_capacity(vector_count * dimension);
        for vector in candidate_vectors {
            flat_vectors.extend_from_slice(vector);
        }

        // Execute GPU kernel
        let scores = device
            .execute_vector_similarity(
                query_vector,
                &flat_vectors,
                vector_count,
                dimension,
                metric_value,
            )
            .map_err(|e| {
                ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                    kernel_name: "vector_similarity".to_string(),
                    error: format!("Vulkan execution failed: {}", e),
                })
            })?;

        // Convert scores to results
        let mut results = Vec::with_capacity(vector_count);
        for (idx, &score) in scores.iter().enumerate() {
            results.push(VectorSimilarityResult {
                index: idx,
                score,
                metric,
            });
        }

        Ok(results)
    }

    /// CPU-parallel batch similarity calculation using Rayon
    fn batch_similarity_cpu_parallel(
        &self,
        query_vector: &[f32],
        candidate_vectors: &[Vec<f32>],
        metric: VectorSimilarityMetric,
    ) -> Vec<VectorSimilarityResult> {
        use rayon::prelude::*;

        candidate_vectors
            .par_iter()
            .enumerate()
            .map(|(idx, candidate)| {
                let score = match metric {
                    VectorSimilarityMetric::Cosine => {
                        Self::cosine_similarity_cpu(query_vector, candidate)
                    }
                    VectorSimilarityMetric::Euclidean => {
                        let distance = Self::euclidean_distance_cpu(query_vector, candidate);
                        // Convert distance to similarity
                        if distance == 0.0 {
                            1.0
                        } else {
                            1.0 / (1.0 + distance)
                        }
                    }
                    VectorSimilarityMetric::DotProduct => {
                        Self::dot_product_cpu(query_vector, candidate)
                    }
                    VectorSimilarityMetric::Manhattan => {
                        let distance = Self::manhattan_distance_cpu(query_vector, candidate);
                        // Convert distance to similarity
                        if distance == 0.0 {
                            1.0
                        } else {
                            1.0 / (1.0 + distance)
                        }
                    }
                };

                VectorSimilarityResult {
                    index: idx,
                    score,
                    metric,
                }
            })
            .collect()
    }

    /// CPU sequential batch similarity calculation
    fn batch_similarity_cpu(
        &self,
        query_vector: &[f32],
        candidate_vectors: &[Vec<f32>],
        metric: VectorSimilarityMetric,
    ) -> Vec<VectorSimilarityResult> {
        candidate_vectors
            .iter()
            .enumerate()
            .map(|(idx, candidate)| {
                let score = match metric {
                    VectorSimilarityMetric::Cosine => {
                        Self::cosine_similarity_cpu(query_vector, candidate)
                    }
                    VectorSimilarityMetric::Euclidean => {
                        let distance = Self::euclidean_distance_cpu(query_vector, candidate);
                        if distance == 0.0 {
                            1.0
                        } else {
                            1.0 / (1.0 + distance)
                        }
                    }
                    VectorSimilarityMetric::DotProduct => {
                        Self::dot_product_cpu(query_vector, candidate)
                    }
                    VectorSimilarityMetric::Manhattan => {
                        let distance = Self::manhattan_distance_cpu(query_vector, candidate);
                        if distance == 0.0 {
                            1.0
                        } else {
                            1.0 / (1.0 + distance)
                        }
                    }
                };

                VectorSimilarityResult {
                    index: idx,
                    score,
                    metric,
                }
            })
            .collect()
    }

    /// CPU implementation of cosine similarity
    fn cosine_similarity_cpu(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if magnitude_a == 0.0 || magnitude_b == 0.0 {
            return 0.0;
        }

        dot_product / (magnitude_a * magnitude_b)
    }

    /// CPU implementation of euclidean distance
    fn euclidean_distance_cpu(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::INFINITY;
        }

        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y) * (x - y))
            .sum::<f32>()
            .sqrt()
    }

    /// CPU implementation of dot product
    fn dot_product_cpu(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// CPU implementation of manhattan distance
    fn manhattan_distance_cpu(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return f32::INFINITY;
        }

        a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vector_similarity_config_defaults() {
        let config = VectorSimilarityConfig::default();
        assert!(config.enable_gpu);
        assert_eq!(config.gpu_min_vectors, 1000);
        assert_eq!(config.gpu_min_dimension, 128);
        assert!(config.use_cpu_parallel);
    }

    #[tokio::test]
    async fn test_cosine_similarity_cpu() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let similarity = GPUVectorSimilarity::cosine_similarity_cpu(&a, &b);
        assert!((similarity - 1.0).abs() < f32::EPSILON);

        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let similarity = GPUVectorSimilarity::cosine_similarity_cpu(&a, &b);
        assert!((similarity - 0.0).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_euclidean_distance_cpu() {
        let a = vec![0.0, 0.0];
        let b = vec![3.0, 4.0];
        let distance = GPUVectorSimilarity::euclidean_distance_cpu(&a, &b);
        assert!((distance - 5.0).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_batch_similarity_cpu() {
        let config = VectorSimilarityConfig {
            enable_gpu: false,
            ..Default::default()
        };
        let engine = GPUVectorSimilarity::new(config).await.unwrap();

        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            vec![1.0, 0.0, 0.0], // Should be identical (score = 1.0)
            vec![0.0, 1.0, 0.0], // Should be orthogonal (score = 0.0)
            vec![0.707, 0.707, 0.0], // Should be ~0.707
        ];

        let results = engine
            .batch_similarity(&query, &candidates, VectorSimilarityMetric::Cosine)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        assert!((results[0].score - 1.0).abs() < 0.01); // Exact match
        assert!((results[1].score - 0.0).abs() < 0.01); // Orthogonal
        assert!((results[2].score - 0.707).abs() < 0.1); // 45 degrees
    }
}

