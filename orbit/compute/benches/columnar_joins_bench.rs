//! Performance benchmarks for GPU-accelerated columnar joins

#[cfg(feature = "gpu-acceleration")]
use criterion::{black_box, criterion_group, criterion_main, Criterion};
#[cfg(feature = "gpu-acceleration")]
use orbit_compute::columnar_joins::{
    ColumnarColumn, ColumnarJoinsConfig, GPUColumnarJoins, JoinType,
};

#[cfg(feature = "gpu-acceleration")]
fn create_test_columns(size: usize, prefix: &str) -> Vec<ColumnarColumn> {
    vec![
        ColumnarColumn {
            name: format!("{}_id", prefix),
            i32_values: Some((0..size as i32).collect()),
            i64_values: None,
            f64_values: None,
            string_values: None,
            null_bitmap: None,
        },
        ColumnarColumn {
            name: format!("{}_value", prefix),
            i32_values: None,
            i64_values: None,
            f64_values: Some((0..size).map(|i| i as f64 * 1.5).collect()),
            string_values: None,
            null_bitmap: None,
        },
    ]
}

#[cfg(feature = "gpu-acceleration")]
fn benchmark_inner_join(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = ColumnarJoinsConfig::default();
    let joins = rt.block_on(GPUColumnarJoins::new(config)).unwrap();

    let mut group = c.benchmark_group("columnar_inner_join");

    for size in [1000, 10000, 100000].iter() {
        let left = create_test_columns(*size, "left");
        let right = create_test_columns(*size / 2, "right"); // Smaller right table

        group.bench_function(format!("size_{}", size), |b| {
            b.iter(|| {
                joins
                    .hash_join(
                        black_box(&left),
                        black_box(&right),
                        black_box("left_id"),
                        black_box("right_id"),
                        black_box(JoinType::Inner),
                    )
                    .unwrap()
            })
        });
    }

    group.finish();
}

#[cfg(feature = "gpu-acceleration")]
fn benchmark_left_outer_join(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = ColumnarJoinsConfig::default();
    let joins = rt.block_on(GPUColumnarJoins::new(config)).unwrap();

    let mut group = c.benchmark_group("columnar_left_outer_join");

    for size in [1000, 10000, 100000].iter() {
        let left = create_test_columns(*size, "left");
        let right = create_test_columns(*size / 2, "right");

        group.bench_function(format!("size_{}", size), |b| {
            b.iter(|| {
                joins
                    .hash_join(
                        black_box(&left),
                        black_box(&right),
                        black_box("left_id"),
                        black_box("right_id"),
                        black_box(JoinType::LeftOuter),
                    )
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
        benchmark_inner_join,
        benchmark_left_outer_join
}

#[cfg(feature = "gpu-acceleration")]
criterion_main!(benches);

#[cfg(not(feature = "gpu-acceleration"))]
fn main() {
    println!("GPU acceleration feature not enabled");
}
