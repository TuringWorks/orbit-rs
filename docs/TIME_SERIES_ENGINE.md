---
layout: default
title: Orbit-RS Time Series Engine
category: documentation
---

# Orbit-RS Time Series Engine

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Storage Backends](#storage-backends)
4. [Data Compression](#data-compression)
5. [Query Engine](#query-engine)
6. [Partitioning & Retention](#partitioning--retention)
7. [Real-time Ingestion](#real-time-ingestion)
8. [Performance & Scalability](#performance--scalability)
9. [Use Cases](#use-cases)
10. [Examples](#examples)
11. [API Reference](#api-reference)

## Overview

The Orbit-RS Time Series Engine is designed for high-throughput, low-latency time series data processing with:

- **Multi-backend support**: In-memory, Redis TimeSeries, PostgreSQL/TimescaleDB
- **Advanced compression**: Delta, DoubleDelta, Gorilla, LZ4, Zstd algorithms
- **High performance**: 3000+ data points ingested in 7ms
- **Scalability**: Multi-terabyte memory limits with smart partitioning
- **Real-time analytics**: Advanced aggregations and windowing functions
- **Integration**: Seamless integration with graph database and OrbitQL

## Architecture

The time series engine follows a layered architecture:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Query Layer   │    │ Analytics Layer │    │  Ingestion      │
│  ┌───────────┐  │    │  ┌───────────┐  │    │  ┌───────────┐  │
│  │ OrbitQL   │  │    │  │Aggregators│  │    │  │  Batch    │  │
│  │Time Series│  │    │  │ Windows   │  │    │  │ Processing│  │
│  │   Query   │  │    │  │Functions  │  │    │  │Real-time  │  │
│  └───────────┘  │    │  └───────────┘  │    │  └───────────┘  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────────────────────────────────────────────────┐
│                Time Series Engine Core                      │
│  ┌─────────────────────────────────────────────────────┐    │
│  │            Query Execution & Planning               │    │
│  │  • Time Range Queries  • Aggregation Pushdown       │    │
│  │  • Series Selection    • Memory Management          │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                  Storage Abstraction                        │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐  │
│  │  Compression    │  │  Partitioning   │  │  Retention  │  │
│  │     Layer       │  │     Manager     │  │   Policies  │  │
│  │                 │  │                 │  │             │  │
│  │ • Delta         │  │ • Time-based    │  │ • TTL       │  │
│  │ • DoubleDelta   │  │ • Size-based    │  │ • Downsample│  │
│  │ • Gorilla       │  │ • Label-based   │  │ • Cleanup   │  │
│  │ • LZ4/Zstd      │  │                 │  │             │  │
│  └─────────────────┘  └─────────────────┘  └─────────────┘  │
└─────────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────────┐
│                   Storage Backends                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────-┐ │
│  │   In-Memory     │  │ Redis TimeSeries│  │PostgreSQL/   │ │
│  │    Storage      │  │                 │  │ TimescaleDB  │ │
│  │                 │  │                 │  │              │ │
│  │ • HashMap Index │  │ • Native TS     │  │ • Hypertables│ │
│  │ • Memory Limits │  │ • Redis Modules │  │ • Compression│ │
│  │ • Fast Lookups  │  │ • Clustering    │  │ • SQL Queries│ │
│  └─────────────────┘  └─────────────────┘  └─────────────-┘ │
└─────────────────────────────────────────────────────────────┘
```

## Storage Backends

### In-Memory Storage

High-performance storage optimized for real-time analytics:

```rust
use orbit_shared::timeseries::{TimeSeriesEngine, MemoryStorageConfig};

let config = MemoryStorageConfig {
    max_memory_mb: 1024,           // 1GB memory limit
    max_series: 100_000,           // Maximum series count
    retention_hours: 24 * 7,       // 7 days retention
    chunk_size_points: 1000,       // Points per chunk
    compression_algorithm: CompressionType::Gorilla,
};

let engine = TimeSeriesEngine::with_memory_storage(config).await?;
```

**Features:**
- **Memory-mapped storage**: Efficient memory usage with OS-level caching
- **Configurable limits**: Memory and series count limits
- **Fast queries**: O(log n) lookups with skip-list indexing
- **Concurrent access**: Lock-free read operations

### Redis TimeSeries Backend

Integration with Redis TimeSeries for persistence and clustering:

```rust
use orbit_shared::timeseries::{TimeSeriesEngine, RedisConfig};

let config = RedisConfig {
    url: "redis://localhost:6379",
    key_prefix: "orbit_ts:",
    pool_size: 10,
    retention_policy: Some("7d".to_string()),  // 7 days
    chunk_size: 1000,
    duplicate_policy: DuplicatePolicy::Last,
};

let engine = TimeSeriesEngine::with_redis_storage(config).await?;
```

**Features:**
- **Redis native**: Uses Redis TimeSeries module
- **High availability**: Redis clustering support
- **Persistence**: RDB and AOF persistence options
- **Memory efficiency**: Native compression in Redis

### PostgreSQL/TimescaleDB Backend

Enterprise-grade storage with SQL compatibility:

```rust
use orbit_shared::timeseries::{TimeSeriesEngine, PostgreSQLConfig};

let config = PostgreSQLConfig {
    connection_string: "postgresql://localhost/timeseries",
    schema: "orbit_ts",
    chunk_time_interval: "1 hour",
    compression_policy: Some("7 days"),
    retention_policy: Some("30 days"),
    replication_factor: 1,
};

let engine = TimeSeriesEngine::with_postgresql_storage(config).await?;
```

**Features:**
- **Hypertables**: Automatic table partitioning
- **Native compression**: TimescaleDB compression
- **SQL queries**: Full SQL query support
- **ACID transactions**: Complete transaction support

## Data Compression

The time series engine supports multiple compression algorithms optimized for different data patterns:

### Delta Compression

Ideal for slowly changing values:

```rust
use orbit_shared::timeseries::compression::DeltaCompressor;

// Configuration
let compressor = DeltaCompressor::new();

// Compression ratio: ~50-70% for typical sensor data
let compressed = compressor.compress(&data_points)?;
```

### DoubleDelta Compression

Optimized for regular interval data:

```rust
use orbit_shared::timeseries::compression::DoubleDeltaCompressor;

// Best for metrics with regular timestamps
let compressor = DoubleDeltaCompressor::new();
let compressed = compressor.compress(&data_points)?;
```

### Gorilla Compression

Facebook's Gorilla algorithm for floating-point values:

```rust
use orbit_shared::timeseries::compression::GorillaCompressor;

// Excellent for floating-point time series
let compressor = GorillaCompressor::new()
    .with_precision(0.01);  // 1% precision

let compressed = compressor.compress(&data_points)?;
```

### LZ4/Zstd Compression

General-purpose compression for mixed data:

```rust
use orbit_shared::timeseries::compression::{LZ4Compressor, ZstdCompressor};

// Fast compression (LZ4)
let lz4 = LZ4Compressor::new();

// High compression ratio (Zstd)
let zstd = ZstdCompressor::new()
    .with_level(3);  // Compression level 1-22
```

### Compression Performance

| Algorithm | Compression Ratio | Speed | Use Case |
|-----------|------------------|-------|----------|
| Delta | 2-3x | Very Fast | Slowly changing values |
| DoubleDelta | 3-5x | Fast | Regular intervals |
| Gorilla | 10-20x | Fast | Floating-point data |
| LZ4 | 2-4x | Very Fast | General purpose |
| Zstd | 3-8x | Medium | Mixed data types |

## Query Engine

### Basic Queries

```rust
use orbit_shared::timeseries::{TimeSeriesEngine, QueryBuilder};

// Point-in-time query
let point = engine.get_point("sensor_01", timestamp).await?;

// Range query
let data = engine.query_range(
    "sensor_01",
    start_time,
    end_time,
).await?;

// Multiple series
let results = engine.query_multiple(
    &["sensor_01", "sensor_02", "sensor_03"],
    start_time,
    end_time,
).await?;
```

### Advanced Queries with Aggregations

```rust
// Query builder with aggregations
let query = QueryBuilder::new()
    .series("temperature_sensors/*")  // Wildcard selection
    .time_range(start_time, end_time)
    .aggregate(AggregationType::Average)
    .window_size(Duration::minutes(5))
    .group_by(vec!["location", "sensor_type"])
    .limit(1000);

let results = engine.execute_query(query).await?;

// Advanced aggregations
let query = QueryBuilder::new()
    .series("cpu_usage")
    .time_range(last_hour, now)
    .aggregates(vec![
        AggregationType::Average,
        AggregationType::Max,
        AggregationType::Percentile(95.0),
        AggregationType::StandardDeviation,
    ])
    .window_size(Duration::seconds(30));

let stats = engine.execute_query(query).await?;
```

### Window Functions

```rust
use orbit_shared::timeseries::window::{WindowType, WindowFunction};

// Moving average
let query = QueryBuilder::new()
    .series("stock_price")
    .window(WindowType::Moving {
        size: Duration::minutes(10),
        step: Duration::minutes(1),
    })
    .function(WindowFunction::Average);

// Tumbling window aggregation
let query = QueryBuilder::new()
    .series("request_count")
    .window(WindowType::Tumbling {
        size: Duration::minutes(5),
    })
    .function(WindowFunction::Sum);
```

## Partitioning & Retention

### Time-based Partitioning

```rust
use orbit_shared::timeseries::partitioning::{PartitionStrategy, TimePartition};

let strategy = PartitionStrategy::Time {
    interval: TimePartition::Hour,  // Hourly partitions
    retention: Duration::days(30),
    compression_after: Duration::hours(6),
};

engine.configure_partitioning("sensor_data", strategy).await?;
```

### Size-based Partitioning

```rust
use orbit_shared::timeseries::partitioning::SizePartition;

let strategy = PartitionStrategy::Size {
    max_size_mb: 100,  // 100MB per partition
    max_points: 1_000_000,
    compression_threshold: 0.8,  // 80% full
};

engine.configure_partitioning("metrics", strategy).await?;
```

### Retention Policies

```rust
use orbit_shared::timeseries::retention::{RetentionPolicy, DownsampleRule};

let policy = RetentionPolicy::new()
    .add_rule(DownsampleRule {
        source_resolution: Duration::seconds(1),
        target_resolution: Duration::minutes(1),
        aggregation: AggregationType::Average,
        retention: Duration::hours(24),
    })
    .add_rule(DownsampleRule {
        source_resolution: Duration::minutes(1),
        target_resolution: Duration::hours(1),
        aggregation: AggregationType::Average,
        retention: Duration::days(30),
    })
    .add_rule(DownsampleRule {
        source_resolution: Duration::hours(1),
        target_resolution: Duration::days(1),
        aggregation: AggregationType::Average,
        retention: Duration::days(365),
    });

engine.set_retention_policy("long_term_metrics", policy).await?;
```

## Real-time Ingestion

### High-throughput Ingestion

```rust
use orbit_shared::timeseries::{TimeSeriesEngine, BatchInserter, DataPoint};

// Batch insertion for high throughput
let mut inserter = BatchInserter::new(&engine)
    .batch_size(1000)
    .flush_interval(Duration::milliseconds(100))
    .backpressure_threshold(10000);

// Insert data points
for i in 0..10000 {
    inserter.add_point(DataPoint {
        series_id: format!("sensor_{}", i % 100),
        timestamp: chrono::Utc::now() + chrono::Duration::milliseconds(i),
        value: (i as f64).sin(),
        labels: Some([("type", "temperature")].iter().cloned().collect()),
    }).await?;
}

// Flush remaining data
inserter.flush().await?;
```

### Streaming Ingestion

```rust
use tokio_stream::StreamExt;
use orbit_shared::timeseries::StreamingInserter;

let mut stream = StreamingInserter::new(&engine)
    .with_backpressure(BackpressureStrategy::Block)
    .with_error_handling(ErrorHandling::RetryWithBackoff);

// Process streaming data
while let Some(data_batch) = data_source.next().await {
    stream.ingest_batch(data_batch).await?;
}
```

## Performance & Scalability

### Memory Management

```rust
use orbit_shared::timeseries::memory::{MemoryManager, MemoryConfig};

let memory_config = MemoryConfig {
    max_memory_gb: 8,                    // 8GB total limit
    series_cache_size: 1000000,          // 1M series in cache
    chunk_cache_size_mb: 512,            // 512MB chunk cache
    compression_threshold: 0.8,          // Compress at 80% memory
    eviction_policy: EvictionPolicy::LRU,
};

engine.configure_memory(memory_config).await?;
```

### Performance Monitoring

```rust
use orbit_shared::timeseries::metrics::EngineMetrics;

// Get engine metrics
let metrics = engine.get_metrics().await?;

println!("Ingestion rate: {} points/sec", metrics.ingestion_rate);
println!("Query latency P95: {}ms", metrics.query_latency_p95.as_millis());
println!("Memory usage: {}MB", metrics.memory_usage_mb);
println!("Compression ratio: {:.2}x", metrics.compression_ratio);
println!("Active series: {}", metrics.active_series_count);
```

### Horizontal Scaling

```rust
use orbit_shared::timeseries::cluster::{ClusterConfig, ShardingStrategy};

let cluster_config = ClusterConfig {
    nodes: vec![
        "node1:8080".to_string(),
        "node2:8080".to_string(),
        "node3:8080".to_string(),
    ],
    sharding_strategy: ShardingStrategy::ConsistentHash,
    replication_factor: 2,
    consistency_level: ConsistencyLevel::Quorum,
};

engine.configure_cluster(cluster_config).await?;
```

## Use Cases

### IoT Sensor Data Collection

```rust
// IoT sensor monitoring system
let iot_engine = TimeSeriesEngine::new()
    .with_retention_policy("sensors", RetentionPolicy::new()
        .raw_data_retention(Duration::days(7))
        .hourly_aggregates_retention(Duration::days(90))
        .daily_aggregates_retention(Duration::days(365))
    )
    .with_compression(CompressionType::Gorilla)
    .build().await?;

// Ingest sensor data
iot_engine.insert_point(
    "temperature_sensor_01",
    chrono::Utc::now(),
    23.5,
    Some([("location", "warehouse_a"), ("floor", "2")].iter().cloned().collect())
).await?;
```

### Application Performance Monitoring (APM)

```rust
// APM system for application metrics
let apm_engine = TimeSeriesEngine::new()
    .with_high_cardinality_support(true)
    .with_labels_indexing(true)
    .with_real_time_alerting(true)
    .build().await?;

// Track application metrics
apm_engine.insert_batch(vec![
    DataPoint::new("http_requests_total", timestamp, 1247.0)
        .with_labels([("method", "GET"), ("status", "200"), ("endpoint", "/api/users")]),
    DataPoint::new("response_time_ms", timestamp, 45.2)
        .with_labels([("method", "GET"), ("endpoint", "/api/users")]),
    DataPoint::new("memory_usage_bytes", timestamp, 512_000_000.0)
        .with_labels([("service", "user_service"), ("instance", "i-1234")]),
]).await?;
```

### Infrastructure Metrics

```rust
// Infrastructure monitoring
let infra_engine = TimeSeriesEngine::new()
    .with_backend(Backend::PostgreSQL)  // For long-term storage
    .with_partitioning(PartitionStrategy::Time {
        interval: TimePartition::Day,
        retention: Duration::days(365),
    })
    .build().await?;

// CPU, memory, disk metrics
infra_engine.insert_infrastructure_metrics(vec![
    ("cpu_usage_percent", 67.5, [("host", "web-01"), ("core", "0")]),
    ("memory_usage_bytes", 8_589_934_592, [("host", "web-01")]),
    ("disk_io_ops", 1250, [("host", "web-01"), ("device", "sda1")]),
]).await?;
```

### Financial Time Series

```rust
// Financial data analysis
let finance_engine = TimeSeriesEngine::new()
    .with_precision(8)  // High precision for financial data
    .with_compression(CompressionType::DoubleDelta)  // Good for price data
    .with_real_time_streaming(true)
    .build().await?;

// Stock prices, trading volume
finance_engine.insert_financial_data(vec![
    ("AAPL_price", timestamp, 150.25, [("exchange", "NASDAQ")]),
    ("AAPL_volume", timestamp, 45_000_000.0, [("exchange", "NASDAQ")]),
]).await?;

// Calculate technical indicators
let sma_20 = finance_engine.calculate_moving_average(
    "AAPL_price", 
    Duration::days(20),
    timestamp
).await?;
```

## Examples

### Complete Time Series Pipeline

```rust
use orbit_shared::timeseries::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize engine with multiple backends
    let engine = TimeSeriesEngine::builder()
        .with_memory_storage(MemoryStorageConfig {
            max_memory_mb: 1024,
            retention_hours: 1, // Hot data
            chunk_size_points: 1000,
            compression_algorithm: CompressionType::Gorilla,
        })
        .with_redis_storage(RedisConfig {
            url: "redis://localhost:6379",
            retention_policy: Some("24h".to_string()),
            ..Default::default()
        })
        .with_postgresql_storage(PostgreSQLConfig {
            connection_string: "postgresql://localhost/timeseries",
            chunk_time_interval: "1 hour",
            retention_policy: Some("365 days"),
            ..Default::default()
        })
        .build().await?;

    // Configure tiered storage
    engine.configure_tiered_storage(vec![
        StorageTier {
            backend: Backend::Memory,
            duration: Duration::hours(1),
            criteria: TieringCriteria::Age,
        },
        StorageTier {
            backend: Backend::Redis,
            duration: Duration::days(1),
            criteria: TieringCriteria::Age,
        },
        StorageTier {
            backend: Backend::PostgreSQL,
            duration: Duration::days(365),
            criteria: TieringCriteria::Age,
        },
    ]).await?;

    // Set up streaming ingestion
    let mut inserter = BatchInserter::new(&engine)
        .batch_size(5000)
        .flush_interval(Duration::milliseconds(50))
        .with_compression(CompressionType::Gorilla);

    // Simulate real-time data
    let start_time = chrono::Utc::now();
    for i in 0..100_000 {
        let timestamp = start_time + chrono::Duration::milliseconds(i * 10);
        
        inserter.add_point(DataPoint {
            series_id: format!("sensor_{}", i % 100),
            timestamp,
            value: 20.0 + 10.0 * ((i as f64) / 1000.0).sin(),
            labels: Some([
                ("location", format!("building_{}", i % 10)),
                ("type", "temperature".to_string()),
            ].iter().cloned().collect()),
        }).await?;

        if i % 10000 == 0 {
            println!("Inserted {} points", i);
        }
    }

    inserter.flush().await?;

    // Query with aggregations
    let results = engine.query(
        QueryBuilder::new()
            .series("sensor_*")
            .time_range(
                start_time,
                start_time + chrono::Duration::minutes(10)
            )
            .aggregate(AggregationType::Average)
            .window_size(Duration::seconds(30))
            .group_by(vec!["location"])
            .build()
    ).await?;

    println!("Query results: {} series", results.len());

    // Get engine statistics
    let metrics = engine.get_metrics().await?;
    println!("Engine metrics: {:#?}", metrics);

    Ok(())
}
```

## API Reference

### Engine Creation and Configuration

```rust
// Create engine with default in-memory storage
let engine = TimeSeriesEngine::new();

// Create with specific backend
let engine = TimeSeriesEngine::with_redis_storage(redis_config).await?;

// Builder pattern for complex configurations
let engine = TimeSeriesEngine::builder()
    .with_memory_storage(memory_config)
    .with_compression(CompressionType::Gorilla)
    .with_retention_policy(retention_policy)
    .build().await?;
```

### Data Insertion

```rust
// Single point insertion
async fn insert_point(
    series_id: &str,
    timestamp: DateTime<Utc>,
    value: f64
) -> Result<()>

// Batch insertion
async fn insert_batch(
    points: Vec<DataPoint>
) -> Result<()>

// Streaming insertion
async fn insert_stream<S>(
    stream: S
) -> Result<()>
where
    S: Stream<Item = DataPoint>
```

### Querying

```rust
// Point queries
async fn get_point(series_id: &str, timestamp: DateTime<Utc>) -> Result<Option<DataPoint>>
async fn get_latest(series_id: &str) -> Result<Option<DataPoint>>

// Range queries
async fn query_range(
    series_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>
) -> Result<Vec<DataPoint>>

// Advanced queries
async fn execute_query(query: Query) -> Result<QueryResult>
```

### Series Management

```rust
// Create/delete series
async fn create_series(series_id: &str, config: SeriesConfig) -> Result<()>
async fn delete_series(series_id: &str) -> Result<bool>

// List and search
async fn list_series(pattern: Option<&str>) -> Result<Vec<String>>
async fn search_series(labels: HashMap<String, String>) -> Result<Vec<String>>
```

The Orbit-RS Time Series Engine provides a complete solution for time series data management, from high-frequency IoT sensor data to financial market analytics, with enterprise-grade performance and scalability.