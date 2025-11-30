# GPU-Accelerated Spatial Operations

**Status**: ✅ **PRODUCTION READY** (Distance calculations)  
**Completion Date**: November 2025  
**Location**: `orbit/compute/src/spatial_operations.rs`

## Overview

GPU-accelerated spatial operations provide high-performance geometric calculations for geospatial applications. The implementation supports distance calculations, great circle distances, and point-in-polygon tests, with automatic routing between GPU and CPU execution based on dataset size.

## Supported Operations

### ✅ Implemented Operations

1. **Distance Calculation** - 2D Euclidean distance between points
2. **Distance Sphere** - Great circle distance (Haversine) between geographic points
3. **Point-in-Polygon** - Test if points are within polygons (CPU-parallel)

### 🚧 Planned Operations

1. **Spatial Joins** - Intersects, contains, within operations (deferred)
2. **R-tree Construction** - GPU-accelerated spatial index building (deferred)

## Architecture

### GPU Acceleration

The implementation uses a three-tier approach:

1. **GPU Acceleration** (Metal/Vulkan) - For large datasets (1000+ geometries)
2. **CPU-Parallel** (Rayon) - For medium datasets (100-1000 geometries)
3. **CPU-Sequential** - For small datasets (<100 geometries)

### Automatic Routing

The system automatically selects the optimal execution path:

```rust
// GPU is used when:
geometry_count >= 1000

// CPU-parallel is used when:
geometry_count > 100 && geometry_count < 1000

// CPU-sequential is used when:
geometry_count <= 100
```

## Usage

### Basic Usage

```rust
use orbit_compute::spatial_operations::{
    GPUSpatialOperations, GPUPoint, SpatialOperationsConfig,
};

// Create GPU-accelerated spatial operations engine
let config = SpatialOperationsConfig::default();
let engine = GPUSpatialOperations::new(config).await?;

// Query point
let query = GPUPoint { x: 0.0, y: 0.0 };

// Candidate points
let candidates = vec![
    GPUPoint { x: 3.0, y: 4.0 }, // Distance = 5.0
    GPUPoint { x: 0.0, y: 0.0 }, // Distance = 0.0
    GPUPoint { x: 1.0, y: 1.0 }, // Distance = sqrt(2)
];

// Calculate distances
let results = engine.batch_distance(&query, &candidates).await?;

// Results are sorted by index
for result in results {
    println!("Point {}: distance = {} meters", result.index, result.distance);
}
```

### Great Circle Distance

```rust
// Calculate distances on Earth's surface
let san_francisco = GPUPoint {
    x: -122.4194, // longitude
    y: 37.7749,   // latitude
};

let new_york = GPUPoint {
    x: -74.0060,
    y: 40.7128,
};

let results = engine
    .batch_distance_sphere(&san_francisco, &[new_york])
    .await?;

println!("Distance: {} meters", results[0].distance);
```

### Point-in-Polygon Test

```rust
use orbit_compute::spatial_operations::GPUPolygon;

// Create a polygon
let polygon = GPUPolygon {
    exterior_ring: vec![
        GPUPoint { x: 0.0, y: 0.0 },
        GPUPoint { x: 10.0, y: 0.0 },
        GPUPoint { x: 10.0, y: 10.0 },
        GPUPoint { x: 0.0, y: 10.0 },
    ],
    interior_rings: vec![], // No holes
};

// Test points
let points = vec![
    GPUPoint { x: 5.0, y: 5.0 },   // Inside
    GPUPoint { x: 15.0, y: 5.0 },  // Outside
];

let results = engine
    .batch_point_in_polygon(&points, &polygon)
    .await?;

for result in results {
    println!("Point {}: inside = {}", result.index, result.result);
}
```

### Configuration

```rust
let config = SpatialOperationsConfig {
    enable_gpu: true,              // Enable GPU acceleration
    gpu_min_geometries: 1000,     // Minimum geometries for GPU
    use_cpu_parallel: true,        // Use Rayon for CPU-parallel
};

let engine = GPUSpatialOperations::new(config).await?;
```

## GPU Kernels

### Metal (macOS)

Kernels are defined in `orbit/compute/src/shaders/database_kernels.metal`:

- `spatial_distance` - 2D Euclidean distance calculation
- `spatial_distance_sphere` - Great circle distance (Haversine) calculation

### Vulkan (Cross-platform)

Kernels are defined in `orbit/compute/src/shaders/vulkan/` (placeholder for future implementation).

## Performance

### Expected Speedups

| Dataset Size | CPU Sequential | CPU Parallel | GPU | GPU Speedup |
|--------------|----------------|--------------|-----|-------------|
| 100 points | 1.0x | 2-4x | 5-10x | 5-10x |
| 1,000 points | 1.0x | 8-16x | 20-50x | 20-50x |
| 10,000 points | 1.0x | 16-32x | 50-100x | 50-100x |
| 100,000 points | 1.0x | 16-32x | 100-200x | 100-200x |

### Break-even Points

- **GPU vs CPU-Parallel**: ~1000 geometries
- **CPU-Parallel vs CPU-Sequential**: ~100 geometries

## Benchmarks

Run benchmarks with:

```bash
cargo bench --package orbit-compute --bench spatial_operations_bench --features gpu-acceleration
```

Benchmarks compare:
- CPU sequential execution
- CPU parallel execution (Rayon)
- GPU-accelerated execution (Metal/Vulkan)

## Testing

Integration tests are in `orbit/server/tests/integration/gpu_spatial_operations_test.rs`:

```bash
cargo test --package orbit-server --test gpu_spatial_operations_test
```

Tests cover:
- CPU distance calculation
- CPU sphere distance calculation
- CPU point-in-polygon test
- Point-in-polygon with holes
- Large dataset parallel processing

## Future Enhancements

1. **Spatial Joins** - GPU-accelerated intersects, contains, within operations
2. **R-tree Construction** - GPU-accelerated spatial index building
3. **Complex Geometry Support** - LineString, MultiPolygon operations
4. **Multi-GPU Support** - Distribute large spatial queries across multiple GPUs
5. **Spatial Aggregations** - GPU-accelerated spatial clustering and aggregation

## References

- [GPU Acceleration Opportunities](./GPU_ACCELERATION_OPPORTUNITIES.md)
- [GPU Complete Documentation](./GPU_COMPLETE_DOCUMENTATION.md)
- [Spatial Operations](../orbit/shared/src/spatial/operations.rs)

---

**Last Updated**: November 2025

