//! Performance benchmarks for GPU-accelerated spatial operations
//!
//! Measures performance of spatial operations (distance, sphere distance, point-in-polygon)
//! comparing CPU sequential, CPU parallel, and GPU-accelerated implementations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use orbit_compute::spatial_operations::{
    GPUPoint, GPUPolygon, GPUSpatialOperations, SpatialOperationsConfig,
};
use rand::Rng;

/// Generate random points for testing
fn generate_random_points(count: usize) -> Vec<GPUPoint> {
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| GPUPoint {
            x: rng.gen_range(-180.0..180.0),
            y: rng.gen_range(-90.0..90.0),
        })
        .collect()
}

/// Generate a test polygon (square)
fn generate_test_polygon() -> GPUPolygon {
    GPUPolygon {
        exterior_ring: vec![
            GPUPoint { x: 0.0, y: 0.0 },
            GPUPoint { x: 10.0, y: 0.0 },
            GPUPoint { x: 10.0, y: 10.0 },
            GPUPoint { x: 0.0, y: 10.0 },
        ],
        interior_rings: vec![],
    }
}

/// Benchmark distance calculation
fn bench_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial_distance");

    let sizes = vec![100, 1000, 10000, 100000];

    for size in sizes {
        let query = GPUPoint { x: 0.0, y: 0.0 };
        let candidates = generate_random_points(size);

        // CPU sequential
        let config = SpatialOperationsConfig {
            enable_gpu: false,
            use_cpu_parallel: false,
            ..Default::default()
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(GPUSpatialOperations::new(config)).unwrap();

        group.bench_function(format!("cpu_sequential_{}", size), |b| {
            b.to_async(&rt)
                .iter(|| engine.batch_distance(black_box(&query), black_box(&candidates)));
        });

        // CPU parallel
        let config = SpatialOperationsConfig {
            enable_gpu: false,
            use_cpu_parallel: true,
            ..Default::default()
        };
        let engine = rt.block_on(GPUSpatialOperations::new(config)).unwrap();

        group.bench_function(format!("cpu_parallel_{}", size), |b| {
            b.to_async(&rt)
                .iter(|| engine.batch_distance(black_box(&query), black_box(&candidates)));
        });

        // GPU (if available)
        let config = SpatialOperationsConfig {
            enable_gpu: true,
            gpu_min_geometries: 100, // Lower threshold for benchmarking
            use_cpu_parallel: true,
        };
        let engine = rt.block_on(GPUSpatialOperations::new(config)).unwrap();

        group.bench_function(format!("gpu_{}", size), |b| {
            b.to_async(&rt)
                .iter(|| engine.batch_distance(black_box(&query), black_box(&candidates)));
        });
    }

    group.finish();
}

/// Benchmark sphere distance calculation
fn bench_distance_sphere(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial_distance_sphere");

    let sizes = vec![100, 1000, 10000];

    for size in sizes {
        let query = GPUPoint {
            x: -122.4194,
            y: 37.7749,
        }; // San Francisco
        let candidates = generate_random_points(size);

        // CPU parallel
        let config = SpatialOperationsConfig {
            enable_gpu: false,
            use_cpu_parallel: true,
            ..Default::default()
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(GPUSpatialOperations::new(config)).unwrap();

        group.bench_function(format!("cpu_parallel_{}", size), |b| {
            b.to_async(&rt)
                .iter(|| engine.batch_distance_sphere(black_box(&query), black_box(&candidates)));
        });
    }

    group.finish();
}

/// Benchmark point-in-polygon test
fn bench_point_in_polygon(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial_point_in_polygon");

    let sizes = vec![100, 1000, 10000];

    for size in sizes {
        let points = generate_random_points(size);
        let polygon = generate_test_polygon();

        // CPU parallel
        let config = SpatialOperationsConfig {
            enable_gpu: false,
            use_cpu_parallel: true,
            ..Default::default()
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(GPUSpatialOperations::new(config)).unwrap();

        group.bench_function(format!("cpu_parallel_{}", size), |b| {
            b.to_async(&rt)
                .iter(|| engine.batch_point_in_polygon(black_box(&points), black_box(&polygon)));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_distance,
    bench_distance_sphere,
    bench_point_in_polygon
);
criterion_main!(benches);
