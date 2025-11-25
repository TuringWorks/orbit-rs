//! Performance benchmarks for GPU-accelerated columnar analytics
//!
//! Measures performance of columnar aggregation operations (SUM, AVG, MIN, MAX, COUNT)
//! comparing CPU sequential, CPU parallel, and GPU-accelerated implementations.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use orbit_compute::columnar_analytics::{
    AggregateFunction, ColumnarAnalyticsConfig, GPUColumnarAnalytics,
};
use rand::Rng;

/// Generate random i32 values for testing
fn generate_random_i32(count: usize) -> Vec<i32> {
    let mut rng = rand::thread_rng();
    (0..count).map(|_| rng.gen_range(-1000..1000)).collect()
}

/// Generate random f64 values for testing
fn generate_random_f64(count: usize) -> Vec<f64> {
    let mut rng = rand::thread_rng();
    (0..count).map(|_| rng.gen_range(-1000.0..1000.0)).collect()
}

/// Generate null bitmap (some values null)
fn generate_null_bitmap(count: usize, null_ratio: f64) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut bitmap = vec![0u8; (count + 7) / 8];
    for i in 0..count {
        if rng.gen::<f64>() < null_ratio {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            bitmap[byte_idx] |= 1 << bit_idx; // Set bit to 1 for null
        }
    }
    bitmap
}

/// Benchmark i32 SUM aggregation
fn bench_aggregate_i32_sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("columnar_aggregate_i32_sum");

    let sizes = vec![1000, 10000, 100000, 1000000];

    for size in sizes {
        let values = generate_random_i32(size);
        let null_bitmap = Some(generate_null_bitmap(size, 0.1));

        // CPU sequential
        let config = ColumnarAnalyticsConfig {
            enable_gpu: false,
            use_cpu_parallel: false,
            ..Default::default()
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(GPUColumnarAnalytics::new(config)).unwrap();

        group.bench_function(format!("cpu_sequential_{}", size), |b| {
            b.to_async(&rt).iter(|| {
                engine.aggregate_i32(
                    black_box(&values),
                    black_box(null_bitmap.as_deref()),
                    black_box(AggregateFunction::Sum),
                )
            });
        });

        // CPU parallel
        let config = ColumnarAnalyticsConfig {
            enable_gpu: false,
            use_cpu_parallel: true,
            ..Default::default()
        };
        let engine = rt.block_on(GPUColumnarAnalytics::new(config)).unwrap();

        group.bench_function(format!("cpu_parallel_{}", size), |b| {
            b.to_async(&rt).iter(|| {
                engine.aggregate_i32(
                    black_box(&values),
                    black_box(null_bitmap.as_deref()),
                    black_box(AggregateFunction::Sum),
                )
            });
        });

        // GPU (if available)
        let config = ColumnarAnalyticsConfig {
            enable_gpu: true,
            gpu_min_rows: 1000, // Lower threshold for benchmarking
            use_cpu_parallel: true,
        };
        let engine = rt.block_on(GPUColumnarAnalytics::new(config)).unwrap();

        group.bench_function(format!("gpu_{}", size), |b| {
            b.to_async(&rt).iter(|| {
                engine.aggregate_i32(
                    black_box(&values),
                    black_box(null_bitmap.as_deref()),
                    black_box(AggregateFunction::Sum),
                )
            });
        });
    }

    group.finish();
}

/// Benchmark i32 AVG aggregation
fn bench_aggregate_i32_avg(c: &mut Criterion) {
    let mut group = c.benchmark_group("columnar_aggregate_i32_avg");

    let sizes = vec![1000, 10000, 100000];

    for size in sizes {
        let values = generate_random_i32(size);
        let null_bitmap = Some(generate_null_bitmap(size, 0.1));

        // CPU parallel
        let config = ColumnarAnalyticsConfig {
            enable_gpu: false,
            use_cpu_parallel: true,
            ..Default::default()
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(GPUColumnarAnalytics::new(config)).unwrap();

        group.bench_function(format!("cpu_parallel_{}", size), |b| {
            b.to_async(&rt).iter(|| {
                engine.aggregate_i32(
                    black_box(&values),
                    black_box(null_bitmap.as_deref()),
                    black_box(AggregateFunction::Avg),
                )
            });
        });

        // GPU (if available)
        let config = ColumnarAnalyticsConfig {
            enable_gpu: true,
            gpu_min_rows: 1000,
            use_cpu_parallel: true,
        };
        let engine = rt.block_on(GPUColumnarAnalytics::new(config)).unwrap();

        group.bench_function(format!("gpu_{}", size), |b| {
            b.to_async(&rt).iter(|| {
                engine.aggregate_i32(
                    black_box(&values),
                    black_box(null_bitmap.as_deref()),
                    black_box(AggregateFunction::Avg),
                )
            });
        });
    }

    group.finish();
}

/// Benchmark i32 COUNT aggregation
fn bench_aggregate_i32_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("columnar_aggregate_i32_count");

    let sizes = vec![1000, 10000, 100000];

    for size in sizes {
        let values = generate_random_i32(size);
        let null_bitmap = Some(generate_null_bitmap(size, 0.2)); // 20% nulls

        // CPU parallel
        let config = ColumnarAnalyticsConfig {
            enable_gpu: false,
            use_cpu_parallel: true,
            ..Default::default()
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(GPUColumnarAnalytics::new(config)).unwrap();

        group.bench_function(format!("cpu_parallel_{}", size), |b| {
            b.to_async(&rt).iter(|| {
                engine.aggregate_i32(
                    black_box(&values),
                    black_box(null_bitmap.as_deref()),
                    black_box(AggregateFunction::Count),
                )
            });
        });

        // GPU (if available)
        let config = ColumnarAnalyticsConfig {
            enable_gpu: true,
            gpu_min_rows: 1000,
            use_cpu_parallel: true,
        };
        let engine = rt.block_on(GPUColumnarAnalytics::new(config)).unwrap();

        group.bench_function(format!("gpu_{}", size), |b| {
            b.to_async(&rt).iter(|| {
                engine.aggregate_i32(
                    black_box(&values),
                    black_box(null_bitmap.as_deref()),
                    black_box(AggregateFunction::Count),
                )
            });
        });
    }

    group.finish();
}

/// Benchmark f64 SUM aggregation
fn bench_aggregate_f64_sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("columnar_aggregate_f64_sum");

    let sizes = vec![1000, 10000, 100000];

    for size in sizes {
        let values = generate_random_f64(size);
        let null_bitmap = Some(generate_null_bitmap(size, 0.1));

        // CPU parallel
        let config = ColumnarAnalyticsConfig {
            enable_gpu: false,
            use_cpu_parallel: true,
            ..Default::default()
        };
        let rt = tokio::runtime::Runtime::new().unwrap();
        let engine = rt.block_on(GPUColumnarAnalytics::new(config)).unwrap();

        group.bench_function(format!("cpu_parallel_{}", size), |b| {
            b.to_async(&rt).iter(|| {
                engine.aggregate_f64(
                    black_box(&values),
                    black_box(null_bitmap.as_deref()),
                    black_box(AggregateFunction::Sum),
                )
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_aggregate_i32_sum,
    bench_aggregate_i32_avg,
    bench_aggregate_i32_count,
    bench_aggregate_f64_sum
);
criterion_main!(benches);

