//! Benchmarks for GPU-accelerated time-series operations

#[cfg(feature = "gpu-acceleration")]
use criterion::{black_box, criterion_group, criterion_main, Criterion};
#[cfg(feature = "gpu-acceleration")]
use orbit_compute::timeseries_operations::{
    GPUTimeSeriesOperations, TimeSeriesAggregation, TimeSeriesOperationsConfig, TimeSeriesPoint,
};

#[cfg(feature = "gpu-acceleration")]
fn create_test_points(count: usize) -> Vec<TimeSeriesPoint> {
    (0..count)
        .map(|i| TimeSeriesPoint {
            timestamp: 1000 * i as u64,      // 1 second intervals
            value: (i as f64).sin() * 100.0, // Sinusoidal pattern
        })
        .collect()
}

#[cfg(feature = "gpu-acceleration")]
fn benchmark_aggregate_by_windows(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = TimeSeriesOperationsConfig::default();
    let ops = rt.block_on(GPUTimeSeriesOperations::new(config)).unwrap();

    let mut group = c.benchmark_group("timeseries_aggregate_by_windows");

    for size in [1000, 10000, 100000].iter() {
        let points = create_test_points(*size);
        let window_size_ms = 5000; // 5 second windows

        group.bench_function(format!("avg_{}", size), |b| {
            b.iter(|| {
                ops.aggregate_by_windows(
                    black_box(&points),
                    black_box(window_size_ms),
                    black_box(TimeSeriesAggregation::Avg),
                )
                .unwrap()
            })
        });

        group.bench_function(format!("sum_{}", size), |b| {
            b.iter(|| {
                ops.aggregate_by_windows(
                    black_box(&points),
                    black_box(window_size_ms),
                    black_box(TimeSeriesAggregation::Sum),
                )
                .unwrap()
            })
        });

        group.bench_function(format!("min_max_{}", size), |b| {
            b.iter(|| {
                ops.aggregate_by_windows(
                    black_box(&points),
                    black_box(window_size_ms),
                    black_box(TimeSeriesAggregation::Range),
                )
                .unwrap()
            })
        });
    }

    group.finish();
}

#[cfg(feature = "gpu-acceleration")]
fn benchmark_moving_average(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = TimeSeriesOperationsConfig::default();
    let ops = rt.block_on(GPUTimeSeriesOperations::new(config)).unwrap();

    let mut group = c.benchmark_group("timeseries_moving_average");

    for size in [1000, 10000, 100000].iter() {
        let points = create_test_points(*size);
        let window_size_ms = 5000; // 5 second window

        group.bench_function(format!("window_{}", size), |b| {
            b.iter(|| {
                ops.moving_average(black_box(&points), black_box(window_size_ms))
                    .unwrap()
            })
        });
    }

    group.finish();
}

#[cfg(feature = "gpu-acceleration")]
fn benchmark_ewma(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = TimeSeriesOperationsConfig::default();
    let ops = rt.block_on(GPUTimeSeriesOperations::new(config)).unwrap();

    let mut group = c.benchmark_group("timeseries_ewma");

    for size in [1000, 10000, 100000].iter() {
        let points = create_test_points(*size);
        let alpha = 0.3;

        group.bench_function(format!("alpha_0.3_{}", size), |b| {
            b.iter(|| {
                ops.exponential_moving_average(black_box(&points), black_box(alpha))
                    .unwrap()
            })
        });
    }

    group.finish();
}

#[cfg(feature = "gpu-acceleration")]
criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets =
        benchmark_aggregate_by_windows,
        benchmark_moving_average,
        benchmark_ewma
}

#[cfg(feature = "gpu-acceleration")]
criterion_main!(benches);

#[cfg(not(feature = "gpu-acceleration"))]
fn main() {
    println!("GPU acceleration feature not enabled");
}
