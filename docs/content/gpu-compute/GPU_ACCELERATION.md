---
layout: default
title: "GPU Acceleration Guide"
subtitle: "Heterogeneous compute in Orbit-RS"
category: "compute"
---

# GPU Acceleration in Orbit-RS

**Status**: PRODUCTION READY - Multi-backend support complete

---

## Overview

Orbit-RS provides comprehensive GPU acceleration for database operations across all major platforms. The system automatically detects and uses the best available backend.

### Quick Stats

| Metric | Value |
|--------|-------|
| **Backends** | Metal, CUDA, Vulkan, WindowsML |
| **Test Coverage** | 54 tests (all passing) |
| **Performance** | 2-200x speedup |
| **Fallback** | Automatic CPU SIMD |

---

## Supported Backends

### Metal (macOS/iOS)
- Apple Silicon optimized
- Unified memory architecture
- Feature flag: `gpu-metal`

### CUDA (NVIDIA)
- cudarc integration with NVRTC
- Complete kernel library
- Feature flag: `gpu-cuda`

### Vulkan (Cross-platform)
- SPIR-V shader compilation
- Windows, Linux, macOS
- Feature flag: `gpu-vulkan`

### WindowsML/DirectML
- DirectX 12 integration
- Windows 10 1903+
- Feature flag: `gpu-windowsml`

### CPU SIMD Fallback
- AVX-512 (x86_64)
- NEON (ARM64)
- SVE (ARM servers)

---

## Accelerated Operations

### Vector Similarity (50-200x speedup)

```rust
// Automatic GPU acceleration for vector search
let results = vector_index.search(
    query_vector,
    k: 10,
    distance: DistanceMetric::Cosine
).await?;
```

Supported metrics:
- Cosine similarity
- Euclidean distance (L2)
- Inner product (dot)
- Manhattan distance (L1)

### Graph Traversal (5-20x speedup)

```rust
// GPU-accelerated BFS
let components = graph.connected_components_gpu().await?;
let paths = graph.bfs_gpu(start_node).await?;
```

Supported algorithms:
- BFS/DFS traversal
- Connected components
- Shortest paths (Dijkstra)
- PageRank

### Spatial Operations (20-100x speedup)

```rust
// GPU-accelerated spatial queries
let nearby = spatial_index.within_distance(point, radius).await?;
let contained = spatial_index.points_in_polygon(polygon).await?;
```

Supported operations:
- Distance calculations (Haversine, Euclidean)
- Point-in-polygon tests
- Range queries
- K-nearest neighbors

### Columnar Analytics (20-100x speedup)

```rust
// GPU-accelerated aggregations
let stats = column.aggregate_gpu(vec![
    Aggregation::Sum,
    Aggregation::Avg,
    Aggregation::Min,
    Aggregation::Max,
]).await?;
```

Supported aggregations:
- SUM, AVG, COUNT
- MIN, MAX
- STDDEV, VARIANCE
- Histogram

### Time Series (2-32x CPU-parallel speedup)

```rust
// Parallel time series operations
let ma = series.moving_average(window: 100).await?;
let downsampled = series.downsample(interval: "1h").await?;
```

Supported operations:
- Moving averages (SMA, EMA, WMA)
- Downsampling
- Gap filling
- Anomaly detection

### Joins (5-20x CPU-parallel speedup)

```rust
// Parallel hash joins
let joined = left_table.hash_join_parallel(
    right_table,
    join_columns: &["id"]
).await?;
```

Supported join types:
- Inner join
- Left/Right outer join
- Full outer join
- Semi/Anti join

---

## Configuration

### Feature Flags

```toml
# Cargo.toml
[features]
gpu-metal = ["metal"]       # Apple Silicon
gpu-cuda = ["cudarc"]       # NVIDIA GPUs
gpu-vulkan = ["ash"]        # Cross-platform
gpu-windowsml = ["windows"] # Windows DirectML
heterogeneous-compute = []  # All backends
```

### Runtime Configuration

```toml
# orbit-server.toml
[compute]
gpu_enabled = true
preferred_backend = "auto"  # auto, metal, cuda, vulkan, cpu
min_batch_size = 1000      # Minimum for GPU offload
memory_limit = "4GB"       # GPU memory limit
```

### Backend Selection

The system automatically selects the best backend:

1. **Metal** - If Apple Silicon detected
2. **CUDA** - If NVIDIA GPU with drivers
3. **Vulkan** - If Vulkan runtime available
4. **WindowsML** - If Windows with DirectX 12
5. **CPU SIMD** - Always available fallback

---

## Performance Guidelines

### When to Use GPU

| Data Size | Recommendation |
|-----------|----------------|
| < 1,000 rows | CPU (GPU overhead exceeds benefit) |
| 1,000 - 100,000 | GPU if complex operations |
| > 100,000 rows | Always use GPU |
| > 1M rows | GPU with batching |

### Memory Considerations

- GPU memory is limited; batch large datasets
- Use streaming for datasets > GPU memory
- Monitor with `RUST_LOG=orbit_compute=debug`

### Optimal Batch Sizes

| Operation | Optimal Batch |
|-----------|---------------|
| Vector search | 10,000 - 100,000 |
| Graph traversal | 1,000,000+ nodes |
| Aggregations | 100,000+ rows |
| Joins | 100,000+ per side |

---

## Troubleshooting

### GPU Not Detected

```bash
# Check GPU availability
cargo test -p orbit-compute gpu_detection

# Enable verbose logging
RUST_LOG=orbit_compute=trace cargo run
```

### Performance Issues

1. Check batch size (too small = overhead)
2. Verify GPU memory available
3. Ensure data is GPU-resident (avoid transfers)
4. Consider CPU for small datasets

### Build Errors

```bash
# Metal (macOS)
xcode-select --install

# CUDA (requires toolkit)
export CUDA_HOME=/usr/local/cuda

# Vulkan (requires SDK)
# macOS: brew install vulkan-sdk
# Linux: apt install vulkan-tools
```

---

## Implementation Details

### Source Files

```
orbit/compute/src/
├── lib.rs              # Entry point
├── gpu_device.rs       # Device abstraction
├── gpu_metal.rs        # Metal backend
├── gpu_cuda.rs         # CUDA backend
├── gpu_vulkan.rs       # Vulkan backend
├── gpu_windowsml.rs    # WindowsML backend
├── cpu_simd.rs         # CPU fallback
└── operations/
    ├── vector.rs       # Vector similarity
    ├── graph.rs        # Graph traversal
    ├── spatial.rs      # Spatial operations
    ├── columnar.rs     # Analytics
    └── timeseries.rs   # Time series
```

### CUDA Kernels

```
orbit/compute/shaders/cuda/
└── database_kernels.cu  # All CUDA kernels
```

### Metal Shaders

```
orbit/compute/shaders/metal/
├── vector_ops.metal
├── graph_ops.metal
└── analytics_ops.metal
```

---

## Resources

- **Source**: `orbit/compute/`
- **Tests**: `cargo test -p orbit-compute`
- **Benchmarks**: `cargo bench -p orbit-compute`
- **RFC**: [Heterogeneous Compute RFC](../rfcs/RFC_INDEX.md#heterogeneous-compute-rfc)
