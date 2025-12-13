//! SIMD Performance Benchmarks
//!
//! Benchmarks comparing scalar vs SIMD implementations

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use orbit_server::protocols::postgres_wire::sql::execution::simd::backend::*;
use orbit_server::protocols::postgres_wire::sql::execution::NullBitmap;

fn bench_filter_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_eq");
    
    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i32> = (0..*size).collect();
        let target = size / 2;

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| {
                black_box(backend.filter_i32_eq(black_box(&values), black_box(target)))
            });
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| {
                    black_box(backend.filter_i32_eq(black_box(&values), black_box(target)))
                });
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| {
                    black_box(backend.filter_i32_eq(black_box(&values), black_box(target)))
                });
            });
        }
    }
    
    group.finish();
}

fn bench_filter_lt(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_lt");
    
    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i32> = (0..*size).collect();
        let target = size / 2;

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| {
                black_box(backend.filter_i32_lt(black_box(&values), black_box(target)))
            });
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| {
                    black_box(backend.filter_i32_lt(black_box(&values), black_box(target)))
                });
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| {
                    black_box(backend.filter_i32_lt(black_box(&values), black_box(target)))
                });
            });
        }
    }
    
    group.finish();
}

fn bench_sum(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum");
    
    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i32> = (1..=*size).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| {
                black_box(backend.sum_i32(black_box(&values), black_box(&null_bitmap)))
            });
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| {
                    black_box(backend.sum_i32(black_box(&values), black_box(&null_bitmap)))
                });
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| {
                    black_box(backend.sum_i32(black_box(&values), black_box(&null_bitmap)))
                });
            });
        }
    }
    
    group.finish();
}

fn bench_compare_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("compare_bytes");
    
    for size in [16, 64, 256, 1024, 4096].iter() {
        let a: Vec<u8> = (0..*size as u8).cycle().take(*size).collect();
        let b = a.clone();

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b_bench, _| {
            let backend = ScalarBackend;
            b_bench.iter(|| {
                black_box(backend.compare_bytes(black_box(&a), black_box(&b)))
            });
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b_bench, _| {
                let backend = Avx2Backend;
                b_bench.iter(|| {
                    black_box(backend.compare_bytes(black_box(&a), black_box(&b)))
                });
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b_bench, _| {
                let backend = NeonBackend;
                b_bench.iter(|| {
                    black_box(backend.compare_bytes(black_box(&a), black_box(&b)))
                });
            });
        }
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_filter_eq,
    bench_filter_lt,
    bench_sum,
    bench_compare_bytes
);
criterion_main!(benches);
