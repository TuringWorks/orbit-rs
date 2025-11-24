//! GPU-accelerated spatial operations
//!
//! This module provides high-performance spatial calculations using GPU acceleration
//! for compute-intensive operations like distance calculations, spatial joins, and
//! point-in-polygon tests. Falls back to CPU-parallel execution for small datasets
//! or when GPU is unavailable.

use crate::errors::ComputeError;
use serde::{Deserialize, Serialize};

#[cfg(feature = "gpu-acceleration")]
use crate::gpu::GPUAccelerationManager;
#[cfg(feature = "gpu-acceleration")]
use std::sync::Arc;

/// Spatial operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpatialOperationType {
    /// Calculate distance between points
    Distance,
    /// Calculate great circle distance (Haversine) between points
    DistanceSphere,
    /// Test if point is within polygon
    PointInPolygon,
    /// Test if geometries intersect
    Intersects,
    /// Test if geometry1 contains geometry2
    Contains,
    /// Test if geometry1 is within geometry2
    Within,
}

/// Configuration for GPU-accelerated spatial operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialOperationsConfig {
    /// Enable GPU acceleration
    pub enable_gpu: bool,
    /// Minimum number of geometries to use GPU (below this, use CPU)
    pub gpu_min_geometries: usize,
    /// Use CPU-parallel fallback (Rayon)
    pub use_cpu_parallel: bool,
}

impl Default for SpatialOperationsConfig {
    fn default() -> Self {
        Self {
            enable_gpu: true,
            gpu_min_geometries: 1000, // Use GPU for 1000+ geometries
            use_cpu_parallel: true,
        }
    }
}

/// Point geometry for GPU processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUPoint {
    pub x: f64,
    pub y: f64,
}

/// Polygon geometry for GPU processing (simplified representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GPUPolygon {
    /// Exterior ring points
    pub exterior_ring: Vec<GPUPoint>,
    /// Interior rings (holes)
    pub interior_rings: Vec<Vec<GPUPoint>>,
}

/// Result of spatial distance calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialDistanceResult {
    /// Index of the geometry pair
    pub index: usize,
    /// Distance in meters (for 2D) or great circle distance (for sphere)
    pub distance: f64,
}

/// Result of spatial predicate evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpatialPredicateResult {
    /// Index of the geometry pair
    pub index: usize,
    /// True if predicate is satisfied
    pub result: bool,
}

/// Statistics about spatial operation execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpatialOperationsStats {
    /// Total execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of geometries processed
    pub geometries_processed: usize,
    /// Whether GPU was used
    pub used_gpu: bool,
    /// GPU utilization percentage (if GPU used)
    pub gpu_utilization: Option<f32>,
}

/// GPU-accelerated spatial operations engine
pub struct GPUSpatialOperations {
    #[cfg(feature = "gpu-acceleration")]
    gpu_manager: Option<Arc<GPUAccelerationManager>>,
    config: SpatialOperationsConfig,
}

impl GPUSpatialOperations {
    /// Create a new GPU-accelerated spatial operations engine
    pub async fn new(config: SpatialOperationsConfig) -> Result<Self, ComputeError> {
        #[cfg(feature = "gpu-acceleration")]
        let gpu_manager = if config.enable_gpu {
            match GPUAccelerationManager::new().await {
                Ok(manager) => {
                    tracing::info!("GPU acceleration enabled for spatial operations");
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
    fn should_use_gpu(&self, geometry_count: usize) -> bool {
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
            let _ = geometry_count; // Suppress unused variable warning
        }

        geometry_count >= self.config.gpu_min_geometries
    }

    /// Calculate distances between a query point and multiple candidate points
    pub async fn batch_distance(
        &self,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Result<Vec<SpatialDistanceResult>, ComputeError> {
        let start_time = std::time::Instant::now();
        let geometry_count = candidate_points.len();

        let results = if self.should_use_gpu(geometry_count) {
            tracing::info!(
                "Using GPU-accelerated distance calculation ({} points)",
                geometry_count
            );
            #[cfg(feature = "gpu-acceleration")]
            {
                self.batch_distance_gpu(query_point, candidate_points).await?
            }
            #[cfg(not(feature = "gpu-acceleration"))]
            {
                self.batch_distance_cpu_parallel(query_point, candidate_points)
            }
        } else if self.config.use_cpu_parallel && geometry_count > 100 {
            tracing::info!(
                "Using CPU-parallel distance calculation ({} points)",
                geometry_count
            );
            self.batch_distance_cpu_parallel(query_point, candidate_points)
        } else {
            tracing::info!(
                "Using CPU sequential distance calculation ({} points)",
                geometry_count
            );
            self.batch_distance_cpu(query_point, candidate_points)
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        tracing::debug!(
            "Distance calculation completed in {}ms ({} points)",
            execution_time_ms,
            geometry_count
        );

        Ok(results)
    }

    /// Calculate great circle distances (Haversine) between a query point and multiple candidate points
    pub async fn batch_distance_sphere(
        &self,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Result<Vec<SpatialDistanceResult>, ComputeError> {
        let start_time = std::time::Instant::now();
        let geometry_count = candidate_points.len();

        let results = if self.should_use_gpu(geometry_count) {
            tracing::info!(
                "Using GPU-accelerated sphere distance calculation ({} points)",
                geometry_count
            );
            #[cfg(feature = "gpu-acceleration")]
            {
                self.batch_distance_sphere_gpu(query_point, candidate_points)
                    .await?
            }
            #[cfg(not(feature = "gpu-acceleration"))]
            {
                self.batch_distance_sphere_cpu_parallel(query_point, candidate_points)
            }
        } else if self.config.use_cpu_parallel && geometry_count > 100 {
            tracing::info!(
                "Using CPU-parallel sphere distance calculation ({} points)",
                geometry_count
            );
            self.batch_distance_sphere_cpu_parallel(query_point, candidate_points)
        } else {
            tracing::info!(
                "Using CPU sequential sphere distance calculation ({} points)",
                geometry_count
            );
            self.batch_distance_sphere_cpu(query_point, candidate_points)
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        tracing::debug!(
            "Sphere distance calculation completed in {}ms ({} points)",
            execution_time_ms,
            geometry_count
        );

        Ok(results)
    }

    /// Test if multiple points are within a polygon
    pub async fn batch_point_in_polygon(
        &self,
        points: &[GPUPoint],
        polygon: &GPUPolygon,
    ) -> Result<Vec<SpatialPredicateResult>, ComputeError> {
        let start_time = std::time::Instant::now();
        let geometry_count = points.len();

        let results = if self.should_use_gpu(geometry_count) {
            tracing::info!(
                "Using GPU-accelerated point-in-polygon test ({} points)",
                geometry_count
            );
            #[cfg(feature = "gpu-acceleration")]
            {
                self.batch_point_in_polygon_gpu(points, polygon).await?
            }
            #[cfg(not(feature = "gpu-acceleration"))]
            {
                self.batch_point_in_polygon_cpu_parallel(points, polygon)
            }
        } else if self.config.use_cpu_parallel && geometry_count > 100 {
            tracing::info!(
                "Using CPU-parallel point-in-polygon test ({} points)",
                geometry_count
            );
            self.batch_point_in_polygon_cpu_parallel(points, polygon)
        } else {
            tracing::info!(
                "Using CPU sequential point-in-polygon test ({} points)",
                geometry_count
            );
            self.batch_point_in_polygon_cpu(points, polygon)
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        tracing::debug!(
            "Point-in-polygon test completed in {}ms ({} points)",
            execution_time_ms,
            geometry_count
        );

        Ok(results)
    }

    /// GPU-accelerated batch distance calculation
    #[cfg(feature = "gpu-acceleration")]
    async fn batch_distance_gpu(
        &self,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Result<Vec<SpatialDistanceResult>, ComputeError> {
        use crate::gpu_metal::MetalDevice;

        // Try Metal first (macOS), then Vulkan
        #[cfg(target_os = "macos")]
        {
            if let Ok(metal_device) = MetalDevice::new() {
                return self
                    .batch_distance_metal(&metal_device, query_point, candidate_points)
                    .await;
            }
        }

        #[cfg(feature = "gpu-vulkan")]
        {
            use crate::gpu_vulkan::VulkanDevice;
            if let Ok(vulkan_device) = VulkanDevice::new() {
                return self
                    .batch_distance_vulkan(&vulkan_device, query_point, candidate_points)
                    .await;
            }
        }

        // Fallback to CPU if GPU unavailable
        tracing::warn!("GPU unavailable, falling back to CPU");
        Ok(self.batch_distance_cpu_parallel(query_point, candidate_points))
    }

    /// Metal-accelerated batch distance calculation
    #[cfg(all(feature = "gpu-acceleration", target_os = "macos"))]
    async fn batch_distance_metal(
        &self,
        device: &crate::gpu_metal::MetalDevice,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Result<Vec<SpatialDistanceResult>, ComputeError> {
        // Flatten points into arrays
        let point_count = candidate_points.len();
        let mut points_x = Vec::with_capacity(point_count);
        let mut points_y = Vec::with_capacity(point_count);

        for point in candidate_points {
            points_x.push(point.x as f32);
            points_y.push(point.y as f32);
        }

        // Execute GPU kernel
        let distances = device
            .execute_spatial_distance(
                query_point.x as f32,
                query_point.y as f32,
                &points_x,
                &points_y,
                point_count,
            )
            .map_err(|e| {
                ComputeError::gpu(crate::errors::GPUError::KernelLaunchFailed {
                    kernel_name: "spatial_distance".to_string(),
                    error: format!("Metal execution failed: {}", e),
                })
            })?;

        // Convert distances to results
        let mut results = Vec::with_capacity(point_count);
        for (idx, &distance) in distances.iter().enumerate() {
            results.push(SpatialDistanceResult {
                index: idx,
                distance: distance as f64,
            });
        }

        Ok(results)
    }

    /// Vulkan-accelerated batch distance calculation
    #[cfg(all(feature = "gpu-acceleration", feature = "gpu-vulkan"))]
    async fn batch_distance_vulkan(
        &self,
        _device: &crate::gpu_vulkan::VulkanDevice,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Result<Vec<SpatialDistanceResult>, ComputeError> {
        // TODO: Implement Vulkan spatial distance
        // For now, fall back to CPU
        tracing::warn!("Vulkan spatial distance not yet implemented, falling back to CPU");
        Ok(self.batch_distance_cpu_parallel(query_point, candidate_points))
    }

    /// GPU-accelerated batch sphere distance calculation
    #[cfg(feature = "gpu-acceleration")]
    async fn batch_distance_sphere_gpu(
        &self,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Result<Vec<SpatialDistanceResult>, ComputeError> {
        // For now, fall back to CPU (Haversine is more complex)
        tracing::warn!("GPU sphere distance not yet implemented, falling back to CPU");
        Ok(self.batch_distance_sphere_cpu_parallel(query_point, candidate_points))
    }

    /// GPU-accelerated batch point-in-polygon test
    #[cfg(feature = "gpu-acceleration")]
    async fn batch_point_in_polygon_gpu(
        &self,
        points: &[GPUPoint],
        polygon: &GPUPolygon,
    ) -> Result<Vec<SpatialPredicateResult>, ComputeError> {
        // For now, fall back to CPU (point-in-polygon is complex)
        tracing::warn!("GPU point-in-polygon not yet implemented, falling back to CPU");
        Ok(self.batch_point_in_polygon_cpu_parallel(points, polygon))
    }

    /// CPU-parallel batch distance calculation using Rayon
    fn batch_distance_cpu_parallel(
        &self,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Vec<SpatialDistanceResult> {
        use rayon::prelude::*;

        candidate_points
            .par_iter()
            .enumerate()
            .map(|(idx, point)| {
                let dx = point.x - query_point.x;
                let dy = point.y - query_point.y;
                let distance = (dx * dx + dy * dy).sqrt();

                SpatialDistanceResult {
                    index: idx,
                    distance,
                }
            })
            .collect()
    }

    /// CPU sequential batch distance calculation
    fn batch_distance_cpu(
        &self,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Vec<SpatialDistanceResult> {
        candidate_points
            .iter()
            .enumerate()
            .map(|(idx, point)| {
                let dx = point.x - query_point.x;
                let dy = point.y - query_point.y;
                let distance = (dx * dx + dy * dy).sqrt();

                SpatialDistanceResult {
                    index: idx,
                    distance,
                }
            })
            .collect()
    }

    /// CPU-parallel batch sphere distance calculation using Rayon
    fn batch_distance_sphere_cpu_parallel(
        &self,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Vec<SpatialDistanceResult> {
        use rayon::prelude::*;

        candidate_points
            .par_iter()
            .enumerate()
            .map(|(idx, point)| {
                let distance = Self::haversine_distance(query_point, point);
                SpatialDistanceResult {
                    index: idx,
                    distance,
                }
            })
            .collect()
    }

    /// CPU sequential batch sphere distance calculation
    fn batch_distance_sphere_cpu(
        &self,
        query_point: &GPUPoint,
        candidate_points: &[GPUPoint],
    ) -> Vec<SpatialDistanceResult> {
        candidate_points
            .iter()
            .enumerate()
            .map(|(idx, point)| {
                let distance = Self::haversine_distance(query_point, point);
                SpatialDistanceResult {
                    index: idx,
                    distance,
                }
            })
            .collect()
    }

    /// CPU-parallel batch point-in-polygon test using Rayon
    fn batch_point_in_polygon_cpu_parallel(
        &self,
        points: &[GPUPoint],
        polygon: &GPUPolygon,
    ) -> Vec<SpatialPredicateResult> {
        use rayon::prelude::*;

        points
            .par_iter()
            .enumerate()
            .map(|(idx, point)| {
                let result = Self::point_in_polygon_cpu(point, polygon);
                SpatialPredicateResult {
                    index: idx,
                    result,
                }
            })
            .collect()
    }

    /// CPU sequential batch point-in-polygon test
    fn batch_point_in_polygon_cpu(
        &self,
        points: &[GPUPoint],
        polygon: &GPUPolygon,
    ) -> Vec<SpatialPredicateResult> {
        points
            .iter()
            .enumerate()
            .map(|(idx, point)| {
                let result = Self::point_in_polygon_cpu(point, polygon);
                SpatialPredicateResult {
                    index: idx,
                    result,
                }
            })
            .collect()
    }

    /// Haversine distance calculation (CPU)
    fn haversine_distance(p1: &GPUPoint, p2: &GPUPoint) -> f64 {
        const EARTH_RADIUS_KM: f64 = 6371.0;
        const PI: f64 = std::f64::consts::PI;

        let lat1_rad = p1.y * PI / 180.0;
        let lat2_rad = p2.y * PI / 180.0;
        let delta_lat = (p2.y - p1.y) * PI / 180.0;
        let delta_lon = (p2.x - p1.x) * PI / 180.0;

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        EARTH_RADIUS_KM * c * 1000.0 // Return distance in meters
    }

    /// Point-in-polygon test using ray casting algorithm (CPU)
    fn point_in_polygon_cpu(point: &GPUPoint, polygon: &GPUPolygon) -> bool {
        // Test exterior ring
        let mut inside = Self::point_in_ring(point, &polygon.exterior_ring);

        // Test interior rings (holes)
        if inside {
            for hole in &polygon.interior_rings {
                if Self::point_in_ring(point, hole) {
                    inside = false;
                    break;
                }
            }
        }

        inside
    }

    /// Test if point is in a ring using ray casting
    fn point_in_ring(point: &GPUPoint, ring: &[GPUPoint]) -> bool {
        if ring.is_empty() {
            return false;
        }

        let mut inside = false;
        let mut j = ring.len() - 1;

        for i in 0..ring.len() {
            if ((ring[i].y > point.y) != (ring[j].y > point.y))
                && (point.x
                    < (ring[j].x - ring[i].x) * (point.y - ring[i].y)
                        / (ring[j].y - ring[i].y)
                        + ring[i].x)
            {
                inside = !inside;
            }
            j = i;
        }

        inside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spatial_config_defaults() {
        let config = SpatialOperationsConfig::default();
        assert!(config.enable_gpu);
        assert_eq!(config.gpu_min_geometries, 1000);
        assert!(config.use_cpu_parallel);
    }

    #[tokio::test]
    async fn test_distance_cpu() {
        let config = SpatialOperationsConfig {
            enable_gpu: false,
            ..Default::default()
        };
        let engine = GPUSpatialOperations::new(config).await.unwrap();

        let query = GPUPoint { x: 0.0, y: 0.0 };
        let candidates = vec![
            GPUPoint { x: 3.0, y: 4.0 }, // Distance = 5.0
            GPUPoint { x: 0.0, y: 0.0 }, // Distance = 0.0
            GPUPoint { x: 1.0, y: 1.0 }, // Distance = sqrt(2) ≈ 1.414
        ];

        let results = engine.batch_distance(&query, &candidates).await.unwrap();

        assert_eq!(results.len(), 3);
        assert!((results[0].distance - 5.0).abs() < 0.01);
        assert!((results[1].distance - 0.0).abs() < 0.01);
        assert!((results[2].distance - 1.414).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_point_in_polygon_cpu() {
        let config = SpatialOperationsConfig {
            enable_gpu: false,
            ..Default::default()
        };
        let engine = GPUSpatialOperations::new(config).await.unwrap();

        // Create a square polygon
        let polygon = GPUPolygon {
            exterior_ring: vec![
                GPUPoint { x: 0.0, y: 0.0 },
                GPUPoint { x: 10.0, y: 0.0 },
                GPUPoint { x: 10.0, y: 10.0 },
                GPUPoint { x: 0.0, y: 10.0 },
            ],
            interior_rings: vec![],
        };

        let points = vec![
            GPUPoint { x: 5.0, y: 5.0 },   // Inside
            GPUPoint { x: 15.0, y: 5.0 },  // Outside
            GPUPoint { x: 0.0, y: 0.0 },   // On corner (may be inside or outside depending on implementation)
        ];

        let results = engine
            .batch_point_in_polygon(&points, &polygon)
            .await
            .unwrap();

        assert_eq!(results.len(), 3);
        assert!(results[0].result); // Inside
        assert!(!results[1].result); // Outside
    }
}

