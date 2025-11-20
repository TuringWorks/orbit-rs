---
layout: default
title: "Hardware Acceleration Guide"
subtitle: "GPU, Neural Engine, and SIMD Acceleration"
category: "performance"
permalink: /compute-acceleration/
---

## Hardware Acceleration Guide

### Orbit-Compute: Heterogeneous Computing for Maximum Performance

**Status**: ✅ Production Ready  
**Updated**: 2025-10-09  
**Applies to**: orbit-compute v1.0+

## Overview

Orbit-RS includes a sophisticated heterogeneous compute acceleration framework (`orbit-compute`) that automatically detects and leverages diverse hardware including CPUs with SIMD, GPUs, and specialized AI accelerators to accelerate database workloads. This guide explains which operations benefit from acceleration and how to configure clients and queries to use or disable acceleration.

## Accelerated Workload Types

### 1. Vector Operations (`SIMDBatch` + `GPUCompute`)

**Workload Classification**: High Priority for GPU Acceleration

**Operations**:

- Vector similarity search (`<->`, `<=>`, `<#>` operators)
- Vector aggregations (SUM, AVG on vector columns)
- Matrix multiplications for embeddings
- Vector normalization and distance calculations
- Batch vector operations on large datasets

**Hardware Acceleration**:

- **GPU**: 8-15x speedup for large vector datasets (>1K vectors)
- **CPU SIMD**: 3-5x speedup with AVX-512/NEON
- **Neural Engine**: 10-50x speedup for ML inference on vectors

**Example Queries**:

```sql
-- Vector similarity search (GPU accelerated)
SELECT content, embedding <-> '[0.1, 0.2, 0.3]' AS distance 
FROM documents 
ORDER BY distance LIMIT 10;

-- Vector aggregations (SIMD accelerated)  
SELECT AVG(embedding) FROM documents WHERE category = 'science';

-- Batch vector operations (GPU preferred)
SELECT id, VECTOR_NORM(embedding) FROM documents;
```

### 2. Matrix Operations (`SIMDBatch::MatrixOps`)

**Workload Classification**: High Priority for GPU/Neural Engine

**Operations**:

- JOIN operations on large tables with numeric keys
- Matrix-based analytical queries
- Linear algebra operations in SQL functions
- Tensor operations for ML workloads

**Hardware Acceleration**:

- **GPU**: 8-15x speedup for matrix operations
- **Neural Engine**: 10-50x speedup for ML-specific matrix ops
- **CPU SIMD**: 3-8x speedup with optimized BLAS

**Example Queries**:

```sql
-- Large JOIN operations (GPU accelerated)
SELECT * FROM orders o JOIN customers c ON o.customer_id = c.id 
WHERE o.order_date > '2024-01-01';

-- Matrix-based analytics (Neural Engine preferred)
SELECT ML_MATRIX_MULTIPLY(features, weights) FROM ml_models;
```

### 3. Aggregation Operations (`SIMDBatch::Reduction`)

**Workload Classification**: Medium Priority for SIMD/GPU

**Operations**:

- COUNT, SUM, AVG, MIN, MAX on large datasets
- GROUP BY operations with numeric aggregations
- Window functions over large partitions
- Statistical functions (STDDEV, VARIANCE)

**Hardware Acceleration**:

- **CPU SIMD**: 3-8x speedup for numeric aggregations
- **GPU**: 5-12x speedup for massive datasets (>1M rows)
- **Specialized reductions**: Custom kernels for common patterns

**Example Queries**:

```sql
-- Aggregations on large datasets (SIMD/GPU accelerated)
SELECT category, COUNT(*), AVG(price), MAX(price) 
FROM products GROUP BY category;

-- Window functions (SIMD preferred)
SELECT *, AVG(salary) OVER (PARTITION BY department) FROM employees;
```

### 4. Time Series Operations (`GPUCompute::MemoryBound`)

**Workload Classification**: Medium Priority for GPU

**Operations**:

- Time series aggregations and rollups
- Moving averages and trend analysis
- Seasonal decomposition
- Time-based JOINs and correlations

**Hardware Acceleration**:

- **GPU**: 5-12x speedup for time series analytics
- **CPU SIMD**: 2-5x speedup for sequential processing
- **Memory optimization**: Fast NVMe for time series data

**Example Queries**:

```sql
-- Time series aggregations (GPU preferred)
SELECT date_trunc('hour', timestamp), AVG(value), COUNT(*)
FROM sensor_data WHERE timestamp > NOW() - INTERVAL '7 days'
GROUP BY 1 ORDER BY 1;

-- Moving averages (SIMD accelerated)
SELECT timestamp, AVG(value) OVER (
    ORDER BY timestamp ROWS BETWEEN 10 PRECEDING AND CURRENT ROW
) FROM metrics;
```

### 5. Graph Operations (`NeuralInference`)

**Workload Classification**: High Priority for Neural Engine

**Operations**:

- Graph traversals and path finding
- Community detection algorithms
- Graph neural network inference
- Knowledge graph reasoning

**Hardware Acceleration**:

- **Neural Engine**: 10-50x speedup for graph ML
- **GPU**: 4-10x speedup for parallel graph algorithms
- **Specialized DSPs**: Optimal for graph signal processing

**Example Queries**:

```sql
-- Graph traversals (Neural Engine preferred)
GRAPH MATCH (a)-[:CONNECTS*1..3]-(b) 
WHERE a.type = 'user' AND b.type = 'product'
RETURN a.id, b.id, path_length;

-- Graph analytics (GPU accelerated)
SELECT community_detection(graph_data) FROM social_network;
```

### 6. Full-Text Search (`GPUCompute::ComputeBound`)

**Workload Classification**: Medium Priority for GPU

**Operations**:

- Text similarity and fuzzy matching
- Regular expression operations on large text
- Natural language processing functions
- Search result ranking and scoring

**Hardware Acceleration**:

- **GPU**: 2-8x speedup for parallel text processing
- **Neural Engine**: 10-50x speedup for semantic search
- **CPU SIMD**: 2-4x speedup for string operations

**Example Queries**:

```sql
-- Text similarity (GPU/Neural Engine preferred)
SELECT id, content, SIMILARITY(content, 'search query') as score
FROM documents WHERE score > 0.7 ORDER BY score DESC;

-- Regex operations (GPU accelerated for large datasets)
SELECT * FROM logs WHERE content ~ 'ERROR.*[0-9]{4}-[0-9]{2}-[0-9]{2}';
```

## Hardware Selection Strategy

The orbit-compute scheduler uses the following priority order for workload assignment:

### Decision Matrix

| Workload Type | Primary Target | Fallback 1 | Fallback 2 | Fallback 3 |
|---------------|----------------|------------|------------|------------|
| **Vector Ops** | GPU (Metal/CUDA) | Neural Engine | CPU SIMD | CPU Scalar |
| **Matrix Ops** | Neural Engine | GPU | CPU SIMD | CPU Scalar |
| **Aggregations** | CPU SIMD | GPU | Specialized CPU | CPU Scalar |
| **Time Series** | GPU | CPU SIMD | Specialized Storage | CPU Scalar |
| **Graph ML** | Neural Engine | GPU | CPU Parallel | CPU Scalar |
| **Text Search** | Neural Engine | GPU | CPU SIMD | CPU Scalar |

### Performance Thresholds

Workloads are automatically routed based on data size:

- **Small** (< 1K rows): CPU SIMD preferred
- **Medium** (1K-100K rows): GPU considered
- **Large** (100K-1M rows): GPU preferred
- **Extra Large** (> 1M rows): GPU required, with CPU fallback

## Client Configuration Options

### 1. OrbitClient Configuration

```rust
use orbit_client::{OrbitClient, OrbitClientConfig};
use orbit_compute::EngineConfig;

// Enable all acceleration features (default)
let client = OrbitClient::builder()
    .with_compute_acceleration(true)           // Enable GPU/Neural acceleration
    .with_simd_acceleration(true)              // Enable CPU SIMD optimization
    .with_adaptive_scheduling(true)            // Enable smart workload routing
    .with_fallback_enabled(true)               // Enable graceful degradation
    .build()
    .await?;

// CPU-only mode (disable GPU/Neural acceleration)
let cpu_client = OrbitClient::builder()
    .with_compute_acceleration(false)          // Disable GPU/Neural acceleration
    .with_simd_acceleration(true)              // Keep CPU SIMD enabled
    .with_max_compute_threads(8)               // Limit CPU threads
    .build()
    .await?;

// Performance-focused configuration
let performance_client = OrbitClient::builder()
    .with_compute_acceleration(true)
    .with_preferred_compute_unit(ComputeUnit::GPU { device_id: 0 })
    .with_compute_timeout_ms(10000)            // 10 second timeout
    .with_memory_optimization(true)            // Enable unified memory
    .build()
    .await?;
```

### 2. Connection String Configuration

You can control acceleration through connection parameters:

```bash

# Enable all acceleration (default)
postgres://user:pass@localhost:5432/db?compute_acceleration=true&simd_acceleration=true

# CPU-only mode
postgres://user:pass@localhost:5432/db?compute_acceleration=false&simd_acceleration=true

# Specific GPU device
postgres://user:pass@localhost:5432/db?preferred_gpu=0&gpu_memory_limit=4GB

# Performance tuning
postgres://user:pass@localhost:5432/db?compute_timeout=5000&adaptive_scheduling=true
```

### 3. Environment Variables

```bash

# Global acceleration settings
export ORBIT_COMPUTE_ACCELERATION=true        # Enable GPU/Neural acceleration
export ORBIT_SIMD_ACCELERATION=true           # Enable CPU SIMD
export ORBIT_ADAPTIVE_SCHEDULING=true         # Enable smart scheduling
export ORBIT_COMPUTE_TIMEOUT=10000            # Timeout in milliseconds

# Hardware preferences
export ORBIT_PREFERRED_GPU=0                  # Select specific GPU
export ORBIT_GPU_MEMORY_LIMIT=8GB            # Limit GPU memory usage
export ORBIT_CPU_THREADS=16                  # Maximum CPU threads
export ORBIT_ENABLE_NEURAL_ENGINE=true       # Enable Neural Engine (Apple/Qualcomm)

# Performance tuning
export ORBIT_UNIFIED_MEMORY=true             # Use unified memory (Apple Silicon)
export ORBIT_MEMORY_ALIGNMENT=64             # SIMD alignment (bytes)
export ORBIT_WORKLOAD_PROFILING=true         # Enable performance learning
```

### 4. Per-Query Configuration

You can control acceleration on a per-query basis using SQL comments:

```sql
-- Force GPU acceleration
/*+ GPU_COMPUTE */ 
SELECT embedding <-> '[0.1, 0.2, 0.3]' AS distance FROM documents;

-- Force CPU-only execution
/*+ CPU_ONLY */
SELECT COUNT(*) FROM large_table GROUP BY category;

-- Specify compute preferences
/*+ PREFERRED_COMPUTE=NEURAL_ENGINE */
SELECT ML_INFERENCE(model, features) FROM data;

-- Disable acceleration for debugging
/*+ NO_ACCELERATION */
SELECT * FROM debug_table WHERE complex_condition = true;

-- Set resource limits
/*+ MAX_MEMORY=2GB, TIMEOUT=5000 */
SELECT * FROM huge_table JOIN another_huge_table;
```

### 5. Programmatic Configuration

```rust
use orbit_client::query::{QueryBuilder, ComputeHint};

// Query with compute hints
let query = QueryBuilder::new("SELECT * FROM vectors")
    .with_compute_hint(ComputeHint::PreferGPU)
    .with_memory_limit_gb(4.0)
    .with_timeout_ms(10000)
    .build();

let results = client.execute_query(query).await?;

// Disable acceleration for specific query
let cpu_query = QueryBuilder::new("SELECT COUNT(*) FROM small_table")
    .with_compute_hint(ComputeHint::CPUOnly)
    .build();
```

## Monitoring Acceleration Usage

### 1. Query Performance Metrics

```sql
-- Check query execution statistics
SELECT 
    query_hash,
    compute_unit_used,
    execution_time_ms,
    acceleration_speedup,
    fallback_occurred
FROM orbit_query_stats 
WHERE timestamp > NOW() - INTERVAL '1 hour';

-- Monitor compute unit utilization
SELECT 
    compute_unit,
    utilization_percent,
    active_queries,
    queue_depth
FROM orbit_compute_status;
```

### 2. Client Status API

```rust
// Get acceleration status
let status = client.get_compute_status().await?;
println!("Available GPUs: {}", status.available_gpus);
println!("Neural Engine: {}", status.neural_engine_available);
println!("SIMD Support: {}", status.simd_capabilities);

// Get performance statistics
let stats = client.get_performance_stats().await?;
println!("GPU queries: {} ({:.1}x avg speedup)", 
         stats.gpu_query_count, stats.gpu_avg_speedup);
```

### 3. Kubernetes Monitoring

```bash

# Check GPU utilization in pods
kubectl top pod -l app=orbit-compute --containers

# Monitor acceleration metrics
kubectl logs -f deployment/orbit-server | grep "ACCELERATION"

# Check compute resource allocation
kubectl describe pod -l app=orbit-server | grep -A 10 "Requests\|Limits"
```

## Performance Tuning Guidelines

### 1. Data Size Thresholds

- **Enable GPU acceleration** for datasets > 100K rows
- **Use SIMD acceleration** for all numeric operations
- **Neural Engine** for ML inference and graph operations
- **CPU fallback** for small datasets (< 1K rows)

### 2. Memory Considerations

- **Unified Memory** (Apple Silicon): Optimal for GPU-CPU data sharing
- **GPU Memory Limit**: Set to 70-80% of available GPU memory
- **Memory Alignment**: Use 64-byte alignment for optimal SIMD performance

### 3. Query Optimization

- **Vector Operations**: Ensure indexes on vector columns
- **Batch Operations**: Group similar operations together
- **Avoid Frequent Fallbacks**: Profile queries to understand acceleration patterns

### 4. Hardware-Specific Tuning

#### Apple Silicon (M1/M2/M3/M4)

```bash
export ORBIT_UNIFIED_MEMORY=true
export ORBIT_ENABLE_NEURAL_ENGINE=true
export ORBIT_METAL_OPTIMIZATION=true
```

#### NVIDIA GPUs

```bash
export ORBIT_CUDA_OPTIMIZATION=true
export ORBIT_TENSOR_CORES=true
export ORBIT_GPU_MEMORY_POOL=true
```

#### Intel/AMD CPUs

```bash
export ORBIT_AVX512_OPTIMIZATION=true
export ORBIT_NUMA_AWARENESS=true
export ORBIT_HYPER_THREADING=true
```

## Troubleshooting

### Common Issues

1. **GPU Not Detected**

   ```bash
   # Check GPU availability
   orbit-compute --check-gpu
   
   # Verify drivers
   nvidia-smi  # NVIDIA
   system_profiler SPDisplaysDataType  # macOS
   ```

2. **Acceleration Not Working**

   ```sql
   -- Check if acceleration is enabled
   SHOW orbit_compute_acceleration;
   
   -- Verify query uses acceleration
   EXPLAIN (ANALYZE, BUFFERS) SELECT * FROM vectors;
   ```

3. **Performance Regression**

   ```bash
   # Enable profiling
   export ORBIT_WORKLOAD_PROFILING=true
   
   # Check query plans
   SET orbit_log_query_plans = 'on';
   ```

### Debug Configuration

```bash

# Enable detailed logging
export RUST_LOG=orbit_compute=debug,orbit_scheduler=debug

# Disable acceleration for debugging
export ORBIT_COMPUTE_ACCELERATION=false

# Force CPU execution
export ORBIT_FORCE_CPU_FALLBACK=true
```

## Best Practices

### 1. Development Environment

- Start with acceleration **enabled** but with conservative timeouts
- Use **profiling** to understand query patterns
- Test both **accelerated and CPU-only** execution paths

### 2. Production Deployment

- Enable **all acceleration features** by default
- Set appropriate **GPU memory limits** (70-80% of available)
- Monitor **fallback rates** and **performance metrics**
- Use **Kubernetes resource limits** to prevent resource contention

### 3. Query Development

- Use **query hints** for fine-tuning critical queries
- **Batch similar operations** to maximize GPU utilization
- **Profile vector operations** to ensure index usage
- Test **large dataset performance** with GPU acceleration

### 4. Monitoring and Alerting

- Alert on high **fallback rates** (>10%)
- Monitor **GPU memory usage** and **temperature**
- Track **query performance trends** over time
- Set alerts for **compute unit failures**

## Conclusion

The orbit-compute acceleration framework provides transparent performance improvements for database workloads while maintaining compatibility and reliability through graceful degradation. By understanding the workload types that benefit from acceleration and properly configuring clients and queries, you can achieve 5-50x performance improvements for compute-intensive database operations.

For workloads involving large datasets, vector operations, matrix computations, or AI/ML inference, GPU and Neural Engine acceleration can provide substantial performance benefits. The system automatically handles hardware detection, workload scheduling, and fallback to ensure optimal performance across diverse deployment environments.

---

**See Also**:

- [README-K8S-DEPLOYMENT.md](../README-K8S-DEPLOYMENT.md) - Kubernetes deployment with GPU support
- [RFC Heterogeneous Compute](rfcs/rfc_heterogeneous_compute.md) - Technical architecture details
- [Kubernetes Deployment Sizing Guide](k8s-deployment-sizing-guide.md) - Hardware sizing recommendations
