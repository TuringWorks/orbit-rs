//! Performance benchmarks for GPU-accelerated ML operations

#[cfg(feature = "gpu-acceleration")]
use criterion::{black_box, criterion_group, criterion_main, Criterion};
#[cfg(feature = "gpu-acceleration")]
use orbit_compute::ml_operations::{
    FeatureEngineeringOp, GPUMLOperations, MatrixOp, MLOperationsConfig,
};

#[cfg(feature = "gpu-acceleration")]
fn create_test_features(rows: usize, cols: usize) -> Vec<Vec<f64>> {
    (0..rows)
        .map(|i| {
            (0..cols)
                .map(|j| (i * cols + j) as f64)
                .collect()
        })
        .collect()
}

#[cfg(feature = "gpu-acceleration")]
fn benchmark_feature_normalization(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = MLOperationsConfig::default();
    let ops = rt.block_on(GPUMLOperations::new(config)).unwrap();

    let mut group = c.benchmark_group("ml_feature_normalization");

    for size in [100, 1000, 10000].iter() {
        let features = create_test_features(*size, 64);

        group.bench_function(format!("normalize_{}", size), |b| {
            b.iter(|| {
                ops.transform_features(
                    black_box(&features),
                    black_box(FeatureEngineeringOp::Normalize),
                )
                .unwrap()
            })
        });
    }

    group.finish();
}

#[cfg(feature = "gpu-acceleration")]
fn benchmark_matrix_multiplication(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = MLOperationsConfig::default();
    let ops = rt.block_on(GPUMLOperations::new(config)).unwrap();

    let mut group = c.benchmark_group("ml_matrix_multiplication");

    for size in [100, 500, 1000].iter() {
        let matrix_a = create_test_features(*size, *size);
        let matrix_b = create_test_features(*size, *size);

        group.bench_function(format!("matmul_{}", size), |b| {
            b.iter(|| {
                ops.matrix_operation(
                    black_box(&matrix_a),
                    black_box(Some(&matrix_b)),
                    black_box(MatrixOp::MatMul),
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
        benchmark_feature_normalization,
        benchmark_matrix_multiplication
}

#[cfg(feature = "gpu-acceleration")]
criterion_main!(benches);

#[cfg(not(feature = "gpu-acceleration"))]
fn main() {
    println!("GPU acceleration feature not enabled");
}

