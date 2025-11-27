//! Performance benchmarks for GPU-accelerated vector similarity search
//!
//! Measures performance of vector similarity operations (cosine, euclidean, dot product, manhattan)
//! comparing CPU sequential, CPU parallel, and GPU-accelerated implementations.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orbit_compute::vector_similarity::{
    GPUVectorSimilarity, VectorSimilarityConfig, VectorSimilarityMetric,
};

/// Generate random vectors for testing
fn generate_random_vectors(count: usize, dimension: usize) -> Vec<Vec<f32>> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..count)
        .map(|_| (0..dimension).map(|_| rng.gen_range(-1.0..1.0)).collect())
        .collect()
}

/// Generate a random query vector
fn generate_query_vector(dimension: usize) -> Vec<f32> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..dimension).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

/// Benchmark CPU sequential similarity calculation
fn bench_cpu_sequential(
    c: &mut Criterion,
    vector_count: usize,
    dimension: usize,
    metric: VectorSimilarityMetric,
    metric_name: &str,
) {
    let query = generate_query_vector(dimension);
    let candidates = generate_random_vectors(vector_count, dimension);

    let config = VectorSimilarityConfig {
        enable_gpu: false,
        use_cpu_parallel: false,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let engine = rt.block_on(GPUVectorSimilarity::new(config)).unwrap();

    c.bench_function(
        &format!(
            "cpu_sequential_{}_{}v_{}d",
            metric_name, vector_count, dimension
        ),
        |b| {
            b.to_async(&rt).iter(|| {
                engine.batch_similarity(
                    black_box(&query),
                    black_box(&candidates),
                    black_box(metric),
                )
            });
        },
    );
}

/// Benchmark CPU parallel similarity calculation
fn bench_cpu_parallel(
    c: &mut Criterion,
    vector_count: usize,
    dimension: usize,
    metric: VectorSimilarityMetric,
    metric_name: &str,
) {
    let query = generate_query_vector(dimension);
    let candidates = generate_random_vectors(vector_count, dimension);

    let config = VectorSimilarityConfig {
        enable_gpu: false,
        use_cpu_parallel: true,
        ..Default::default()
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let engine = rt.block_on(GPUVectorSimilarity::new(config)).unwrap();

    c.bench_function(
        &format!(
            "cpu_parallel_{}_{}v_{}d",
            metric_name, vector_count, dimension
        ),
        |b| {
            b.to_async(&rt).iter(|| {
                engine.batch_similarity(
                    black_box(&query),
                    black_box(&candidates),
                    black_box(metric),
                )
            });
        },
    );
}

/// Benchmark GPU-accelerated similarity calculation
fn bench_gpu(
    c: &mut Criterion,
    vector_count: usize,
    dimension: usize,
    metric: VectorSimilarityMetric,
    metric_name: &str,
) {
    let query = generate_query_vector(dimension);
    let candidates = generate_random_vectors(vector_count, dimension);

    let config = VectorSimilarityConfig {
        enable_gpu: true,
        gpu_min_vectors: 100, // Lower threshold for benchmarking
        gpu_min_dimension: 64,
        use_cpu_parallel: true,
    };

    let rt = tokio::runtime::Runtime::new().unwrap();
    let engine = rt.block_on(GPUVectorSimilarity::new(config)).unwrap();

    c.bench_function(
        &format!("gpu_{}_{}v_{}d", metric_name, vector_count, dimension),
        |b| {
            b.to_async(&rt).iter(|| {
                engine.batch_similarity(
                    black_box(&query),
                    black_box(&candidates),
                    black_box(metric),
                )
            });
        },
    );
}

/// Benchmark suite for cosine similarity
fn bench_cosine_similarity(c: &mut Criterion) {
    let sizes = vec![
        (100, 128),
        (1000, 128),
        (10000, 128),
        (1000, 384),
        (1000, 768),
    ];

    for (vector_count, dimension) in sizes {
        bench_cpu_sequential(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::Cosine,
            "cosine",
        );
        bench_cpu_parallel(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::Cosine,
            "cosine",
        );
        bench_gpu(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::Cosine,
            "cosine",
        );
    }
}

/// Benchmark suite for euclidean distance
fn bench_euclidean_distance(c: &mut Criterion) {
    let sizes = vec![(100, 128), (1000, 128), (10000, 128), (1000, 384)];

    for (vector_count, dimension) in sizes {
        bench_cpu_sequential(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::Euclidean,
            "euclidean",
        );
        bench_cpu_parallel(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::Euclidean,
            "euclidean",
        );
        bench_gpu(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::Euclidean,
            "euclidean",
        );
    }
}

/// Benchmark suite for dot product
fn bench_dot_product(c: &mut Criterion) {
    let sizes = vec![(100, 128), (1000, 128), (10000, 128)];

    for (vector_count, dimension) in sizes {
        bench_cpu_sequential(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::DotProduct,
            "dot_product",
        );
        bench_cpu_parallel(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::DotProduct,
            "dot_product",
        );
        bench_gpu(
            c,
            vector_count,
            dimension,
            VectorSimilarityMetric::DotProduct,
            "dot_product",
        );
    }
}

criterion_group!(
    benches,
    bench_cosine_similarity,
    bench_euclidean_distance,
    bench_dot_product
);
criterion_main!(benches);
