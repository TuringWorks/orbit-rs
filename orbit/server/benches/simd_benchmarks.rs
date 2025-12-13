//! SIMD Performance Benchmarks
//!
//! Benchmarks comparing scalar vs SIMD implementations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use orbit_server::protocols::postgres_wire::sql::execution::simd::backend::*;
use orbit_server::protocols::postgres_wire::sql::execution::NullBitmap;

fn bench_filter_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_eq");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i32> = (0..*size).collect();
        let target = size / 2;

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.filter_i32_eq(black_box(&values), black_box(target))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.filter_i32_eq(black_box(&values), black_box(target))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_i32_eq(black_box(&values), black_box(target))));
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
            b.iter(|| black_box(backend.filter_i32_lt(black_box(&values), black_box(target))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.filter_i32_lt(black_box(&values), black_box(target))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_i32_lt(black_box(&values), black_box(target))));
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
            b.iter(|| black_box(backend.sum_i32(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.sum_i32(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.sum_i32(black_box(&values), black_box(&null_bitmap))));
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
            b_bench.iter(|| black_box(backend.compare_bytes(black_box(&a), black_box(&b))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b_bench, _| {
                let backend = Avx2Backend;
                b_bench.iter(|| black_box(backend.compare_bytes(black_box(&a), black_box(&b))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b_bench, _| {
                let backend = NeonBackend;
                b_bench.iter(|| black_box(backend.compare_bytes(black_box(&a), black_box(&b))));
            });
        }
    }

    group.finish();
}

fn bench_min(c: &mut Criterion) {
    let mut group = c.benchmark_group("min");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i32> = (1..=*size).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.min_i32(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.min_i32(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.min_i32(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }

    group.finish();
}

fn bench_max(c: &mut Criterion) {
    let mut group = c.benchmark_group("max");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i32> = (1..=*size).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.max_i32(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.max_i32(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.max_i32(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }

    group.finish();
}


fn bench_filter_f32_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_f32_eq");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<f32> = (0..*size).map(|x| x as f32).collect();
        let target = (*size / 2) as f32;

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.filter_f32_eq(black_box(&values), black_box(target))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.filter_f32_eq(black_box(&values), black_box(target))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_f32_eq(black_box(&values), black_box(target))));
            });
        }
    }
    group.finish();
}

fn bench_sum_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum_f32");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<f32> = (0..*size).map(|x| x as f32).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.sum_f32(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.sum_f32(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.sum_f32(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }
    group.finish();
}

fn bench_min_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("min_f32");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<f32> = (0..*size).map(|x| x as f32).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.min_f32(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.min_f32(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.min_f32(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }
    group.finish();
}

fn bench_max_f32(c: &mut Criterion) {
    let mut group = c.benchmark_group("max_f32");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<f32> = (0..*size).map(|x| x as f32).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.max_f32(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.max_f32(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.max_f32(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }
    group.finish();
}

fn bench_filter_f64_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_f64_eq");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<f64> = (0..*size).map(|x| x as f64).collect();
        let target = (*size / 2) as f64;

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.filter_f64_eq(black_box(&values), black_box(target))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.filter_f64_eq(black_box(&values), black_box(target))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_f64_eq(black_box(&values), black_box(target))));
            });
        }
    }
    group.finish();
}

fn bench_sum_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum_f64");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<f64> = (0..*size).map(|x| x as f64).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.sum_f64(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.sum_f64(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.sum_f64(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }
    group.finish();
}

fn bench_min_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("min_f64");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<f64> = (0..*size).map(|x| x as f64).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.min_f64(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.min_f64(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.min_f64(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }
    group.finish();
}

fn bench_max_f64(c: &mut Criterion) {
    let mut group = c.benchmark_group("max_f64");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<f64> = (0..*size).map(|x| x as f64).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.max_f64(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.max_f64(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.max_f64(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }
    group.finish();
}

fn bench_filter_i64_eq(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_i64_eq");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i64> = (0..*size).map(|x| x as i64).collect();
        let target = (*size / 2) as i64;

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.filter_i64_eq(black_box(&values), black_box(target))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.filter_i64_eq(black_box(&values), black_box(target))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_i64_eq(black_box(&values), black_box(target))));
            });
        }
    }
    group.finish();
}

fn bench_sum_i64(c: &mut Criterion) {
    let mut group = c.benchmark_group("sum_i64");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i64> = (0..*size).map(|x| x as i64).collect();
        let null_bitmap = NullBitmap::new_all_valid(*size as usize);

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.sum_i64(black_box(&values), black_box(&null_bitmap))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.sum_i64(black_box(&values), black_box(&null_bitmap))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.sum_i64(black_box(&values), black_box(&null_bitmap))));
            });
        }
    }
    group.finish();
}

fn bench_filter_i64_lt(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_i64_lt");

    for size in [100, 1_000, 10_000, 100_000].iter() {
        let values: Vec<i64> = (0..*size).map(|x| x as i64).collect();
        let target = (*size / 2) as i64;

        group.bench_with_input(BenchmarkId::new("scalar", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.filter_i64_lt(black_box(&values), black_box(target))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.filter_i64_lt(black_box(&values), black_box(target))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_i64_lt(black_box(&values), black_box(target))));
            });
        }
    }
    group.finish();
}

fn bench_float_filters(c: &mut Criterion) {
    let mut group = c.benchmark_group("float_filters_lt");

    for size in [1000, 10_000].iter() {
        let f32_vals: Vec<f32> = (0..*size).map(|x| x as f32).collect();
        let f32_target = (*size / 2) as f32;
        
        // f32 lt
        group.bench_with_input(BenchmarkId::new("scalar_f32_lt", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.filter_f32_lt(black_box(&f32_vals), black_box(f32_target))));
        });
        
        #[cfg(target_arch = "aarch64")]
        {
             group.bench_with_input(BenchmarkId::new("neon_f32_lt", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_f32_lt(black_box(&f32_vals), black_box(f32_target))));
            });
        }

        let f64_vals: Vec<f64> = (0..*size).map(|x| x as f64).collect();
        let f64_target = (*size / 2) as f64;

        // f64 lt
        group.bench_with_input(BenchmarkId::new("scalar_f64_lt", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.filter_f64_lt(black_box(&f64_vals), black_box(f64_target))));
        });

        #[cfg(target_arch = "aarch64")]
        {
             group.bench_with_input(BenchmarkId::new("neon_f64_lt", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_f64_lt(black_box(&f64_vals), black_box(f64_target))));
            });
        }
    }
    group.finish();
}

fn bench_operators(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_operators");

    // We can't use get_simd_backend() comfortably here because benchmarks test specific backends usually.
    // But since the trait exposes the methods, we can just test the active one or specific ones.
    // The previous benchmarks tested Scalar/AVX2/NEON explicitly. 
    // I should follow that pattern for consistency and isolation.

    for size in [1000, 100_000].iter() {
        let values: Vec<i32> = (0..*size).map(|i| (i % 100) as i32).collect();
        let target = 50;

        // Scalar
        group.bench_with_input(BenchmarkId::new("scalar/filter_i32_le", size), size, |b, _| {
            let backend = ScalarBackend;
            b.iter(|| black_box(backend.filter_i32_le(black_box(&values), black_box(target))));
        });

        #[cfg(target_arch = "x86_64")]
        if is_x86_feature_detected!("avx2") {
            group.bench_with_input(BenchmarkId::new("avx2/filter_i32_le", size), size, |b, _| {
                let backend = Avx2Backend;
                b.iter(|| black_box(backend.filter_i32_le(black_box(&values), black_box(target))));
            });
        }

        #[cfg(target_arch = "aarch64")]
        {
            group.bench_with_input(BenchmarkId::new("neon/filter_i32_le", size), size, |b, _| {
                let backend = NeonBackend;
                b.iter(|| black_box(backend.filter_i32_le(black_box(&values), black_box(target))));
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
    bench_min,
    bench_max,
    bench_compare_bytes,
    bench_filter_f32_eq,
    bench_sum_f32,
    bench_min_f32,
    bench_max_f32,
    bench_filter_f64_eq,
    bench_sum_f64,
    bench_min_f64,
    bench_max_f64,
    bench_filter_i64_eq,
    bench_sum_i64,
    bench_operators,
    bench_filter_i64_lt,
    bench_float_filters
);
criterion_main!(benches);
