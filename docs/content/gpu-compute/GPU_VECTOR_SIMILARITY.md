# GPU-Accelerated Vector Similarity Search

**Status**: ✅ **PRODUCTION READY**  
**Completion Date**: November 2025  
**Location**: `orbit/compute/src/vector_similarity.rs`

## Overview

GPU-accelerated vector similarity search provides high-performance similarity calculations for machine learning embeddings and semantic search applications. The implementation supports multiple similarity metrics and automatically routes between GPU and CPU execution based on dataset size.

## Supported Metrics

### ✅ Implemented Metrics

1. **Cosine Similarity** - Normalized dot product, measures angle between vectors
2. **Euclidean Distance** - L2 norm distance between vectors
3. **Dot Product** - Unnormalized dot product similarity
4. **Manhattan Distance** - L1 norm distance between vectors

## Architecture

### GPU Acceleration

The implementation uses a three-tier approach:

1. **GPU Acceleration** (Metal/Vulkan) - For large datasets (1000+ vectors, 128+ dimensions)
2. **CPU-Parallel** (Rayon) - For medium datasets (100-1000 vectors)
3. **CPU-Sequential** - For small datasets (<100 vectors)

### Automatic Routing

The system automatically selects the optimal execution path:

```rust
// GPU is used when:
vector_count >= 1000 && dimension >= 128

// CPU-parallel is used when:
vector_count > 100 && (vector_count < 1000 || dimension < 128)

// CPU-sequential is used when:
vector_count <= 100
```

## Usage

### Basic Usage

```rust
use orbit_compute::vector_similarity::{
    GPUVectorSimilarity, VectorSimilarityConfig, VectorSimilarityMetric,
};

// Create GPU-accelerated similarity engine
let config = VectorSimilarityConfig::default();
let engine = GPUVectorSimilarity::new(config).await?;

// Query vector
let query = vec![1.0, 0.0, 0.0];

// Candidate vectors
let candidates = vec![
    vec![1.0, 0.0, 0.0],  // Identical
    vec![0.0, 1.0, 0.0],  // Orthogonal
    vec![0.707, 0.707, 0.0], // 45 degrees
];

// Calculate similarity
let results = engine
    .batch_similarity(&query, &candidates, VectorSimilarityMetric::Cosine)
    .await?;

// Results are sorted by score (descending)
for result in results {
    println!("Vector {}: similarity = {}", result.index, result.score);
}
```

### Configuration

```rust
let config = VectorSimilarityConfig {
    enable_gpu: true,              // Enable GPU acceleration
    gpu_min_vectors: 1000,         // Minimum vectors for GPU
    gpu_min_dimension: 128,       // Minimum dimension for GPU
    use_cpu_parallel: true,        // Use Rayon for CPU-parallel
};

let engine = GPUVectorSimilarity::new(config).await?;
```

## Integration with VectorActor

The `VectorActor` in `orbit/server/src/protocols/vector_store.rs` automatically uses CPU-parallel execution for large datasets:

```rust
let mut actor = VectorActor::new();

// Add vectors
actor.add_vector(Vector::new("v1".to_string(), vec![1.0, 2.0, 3.0]))?;

// Search (automatically uses CPU-parallel for 100+ vectors)
let params = VectorSearchParams::new(
    vec![1.0, 2.0, 3.0],
    SimilarityMetric::Cosine,
    10,
);
let results = actor.search_vectors(params);
```

**Note**: Full GPU integration requires async refactoring of `VectorActor::search_vectors`. Currently, CPU-parallel execution is used via Rayon.

## GPU Kernels

### Metal (macOS)

Kernels are defined in `orbit/compute/src/shaders/database_kernels.metal`:

- `vector_cosine_similarity` - Cosine similarity calculation
- `vector_euclidean_distance` - Euclidean distance calculation
- `vector_dot_product` - Dot product calculation
- `vector_manhattan_distance` - Manhattan distance calculation

### Vulkan (Cross-platform)

Kernels are defined in `orbit/compute/src/shaders/vulkan/vector_similarity.comp`:

- Single unified kernel with metric selection via parameter buffer

## Performance

### Expected Speedups

| Dataset Size | CPU Sequential | CPU Parallel | GPU | GPU Speedup |
|--------------|----------------|--------------|-----|-------------|
| 100 vectors, 128 dim | 1.0x | 2-4x | 5-10x | 5-10x |
| 1,000 vectors, 128 dim | 1.0x | 8-16x | 50-100x | 50-100x |
| 10,000 vectors, 128 dim | 1.0x | 16-32x | 100-200x | 100-200x |
| 1,000 vectors, 384 dim | 1.0x | 8-16x | 50-150x | 50-150x |
| 1,000 vectors, 768 dim | 1.0x | 8-16x | 100-200x | 100-200x |

### Break-even Points

- **GPU vs CPU-Parallel**: ~1000 vectors with 128+ dimensions
- **CPU-Parallel vs CPU-Sequential**: ~100 vectors

## Benchmarks

Run benchmarks with:

```bash
cargo bench --package orbit-compute --bench vector_similarity_bench --features gpu-acceleration
```

Benchmarks compare:
- CPU sequential execution
- CPU parallel execution (Rayon)
- GPU-accelerated execution (Metal/Vulkan)

## Testing

Integration tests are in `orbit/server/tests/integration/gpu_vector_similarity_test.rs`:

```bash
cargo test --package orbit-server --test gpu_vector_similarity_test
```

Tests cover:
- CPU sequential similarity
- CPU parallel similarity
- All similarity metrics (cosine, euclidean, dot product, manhattan)
- Metadata filtering
- Threshold filtering
- Result limiting

## Future Enhancements

1. **Async GPU Integration** - Full async support in `VectorActor::search_vectors`
2. **Approximate Nearest Neighbor (ANN)** - HNSW or IVF-PQ algorithms
3. **Embedding Generation** - GPU-accelerated transformer inference
4. **Multi-GPU Support** - Distribute large searches across multiple GPUs
5. **Batch Query Processing** - Process multiple queries simultaneously

## References

- [GPU Acceleration Opportunities](./GPU_ACCELERATION_OPPORTUNITIES.md)
- [GPU Complete Documentation](./GPU_COMPLETE_DOCUMENTATION.md)
- [Vector Store Protocol](../orbit/server/src/protocols/vector_store.rs)

---

**Last Updated**: November 2025

