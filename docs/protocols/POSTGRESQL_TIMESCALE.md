# PostgreSQL TimescaleDB Compatibility

## Overview

TimescaleDB is a PostgreSQL extension that transforms PostgreSQL into a scalable time-series database. Orbit-RS will provide native compatibility with TimescaleDB functions and features, allowing existing applications to seamlessly migrate from TimescaleDB to Orbit's distributed architecture.

## Planned Features

### 🎯 **Phase 12: TimescaleDB Foundation**

#### Core Time Series Tables
- **Hypertables**: Distributed time-partitioned tables
- **Chunks**: Automatic time-based table partitioning
- **Time partitioning**: Automatic data partitioning by time intervals
- **Space partitioning**: Additional partitioning by space dimensions

#### Basic Functions
- **CREATE EXTENSION timescaledb**: Enable TimescaleDB compatibility
- **create_hypertable()**: Convert regular tables to hypertables
- **drop_chunks()**: Remove old data based on time intervals
- **show_chunks()**: Display chunk information
- **chunk_relation_size()**: Get chunk storage information

### 🚀 **Phase 13: Advanced TimescaleDB Features**

#### Continuous Aggregates
- **CREATE MATERIALIZED VIEW**: Time-based materialized views
- **refresh_continuous_aggregate()**: Manual refresh of aggregates
- **Automatic refresh**: Background refresh of continuous aggregates
- **Real-time aggregation**: Combine historical and real-time data

#### Advanced Analytics
- **time_bucket()**: Group data into time intervals
- **time_bucket_gapfill()**: Fill gaps in time series data
- **locf()**: Last observation carried forward
- **interpolate()**: Linear interpolation for missing values
- **Hyperfunctions**: Advanced time-series analytical functions

### 📊 **Phase 14: Enterprise TimescaleDB Features**

#### Data Retention and Compression
- **Data retention policies**: Automatic data expiration
- **Native compression**: Column-store compression for historical data
- **Compression policies**: Automatic compression based on age
- **Decompression**: Transparent decompression for queries

#### Multi-Node and Clustering
- **Distributed hypertables**: Scale across multiple Orbit nodes
- **Data nodes**: Distribute chunks across cluster
- **Query planning**: Distributed query execution
- **Replication**: Cross-node data replication

## Technical Implementation

### SQL Extensions

#### Hypertable Management
```sql
-- Create extension
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;

-- Create hypertable from existing table
SELECT create_hypertable('sensor_data', 'timestamp', 
    chunk_time_interval => INTERVAL '1 hour',
    partitioning_column => 'sensor_id',
    number_partitions => 4);

-- Create retention policy
SELECT add_retention_policy('sensor_data', INTERVAL '30 days');

-- Create compression policy
SELECT add_compression_policy('sensor_data', INTERVAL '7 days');
```

#### Time-Series Queries
```sql
-- Time bucket aggregation
SELECT 
    time_bucket('1 hour', timestamp) AS hour,
    sensor_id,
    AVG(temperature) as avg_temp,
    MAX(temperature) as max_temp,
    MIN(temperature) as min_temp
FROM sensor_data 
WHERE timestamp >= NOW() - INTERVAL '1 day'
GROUP BY hour, sensor_id
ORDER BY hour DESC;

-- Gap filling with interpolation
SELECT 
    time_bucket_gapfill('15 minutes', timestamp) AS bucket,
    sensor_id,
    AVG(temperature) as avg_temp,
    interpolate(AVG(temperature)) as interpolated_temp
FROM sensor_data 
WHERE timestamp >= NOW() - INTERVAL '4 hours'
  AND sensor_id = 'temp_001'
GROUP BY bucket, sensor_id
ORDER BY bucket;

-- Continuous aggregate (materialized view)
CREATE MATERIALIZED VIEW sensor_data_hourly
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', timestamp) AS hour,
    sensor_id,
    AVG(temperature) as avg_temp,
    COUNT(*) as sample_count
FROM sensor_data
GROUP BY hour, sensor_id;

-- Refresh continuous aggregate
CALL refresh_continuous_aggregate('sensor_data_hourly', 
    start_time => NOW() - INTERVAL '1 day',
    end_time => NOW());
```

### Actor Architecture

```rust

#[async_trait]
pub trait HypertableActor: ActorWithStringKey {
    // Hypertable management
    async fn create_hypertable(&self, config: HypertableConfig) -> OrbitResult<()>;
    async fn drop_hypertable(&self) -> OrbitResult<()>;
    async fn get_hypertable_info(&self) -> OrbitResult<HypertableInfo>;
    
    // Chunk management
    async fn show_chunks(&self, time_range: Option<TimeRange>) -> OrbitResult<Vec<ChunkInfo>>;
    async fn drop_chunks(&self, older_than: SystemTime) -> OrbitResult<Vec<String>>;
    async fn compress_chunk(&self, chunk_id: String) -> OrbitResult<()>;
    
    // Time-series operations
    async fn time_bucket_query(&self, params: TimeBucketParams) -> OrbitResult<Vec<TimeBucketResult>>;
    async fn gapfill_query(&self, params: GapfillParams) -> OrbitResult<Vec<GapfillResult>>;
    
    // Continuous aggregates
    async fn create_continuous_aggregate(&self, config: ContinuousAggregateConfig) -> OrbitResult<()>;
    async fn refresh_continuous_aggregate(&self, time_range: TimeRange) -> OrbitResult<()>;
}
```

### Data Structures

```rust

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypertableConfig {
    pub table_name: String,
    pub time_column: String,
    pub chunk_time_interval: Duration,
    pub partitioning_column: Option<String>,
    pub number_partitions: Option<u32>,
    pub replication_factor: Option<u16>,
    pub data_retention: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub chunk_id: String,
    pub chunk_name: String,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
    pub size_bytes: u64,
    pub compressed: bool,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBucketParams {
    pub bucket_size: Duration,
    pub time_column: String,
    pub group_by: Vec<String>,
    pub aggregates: Vec<AggregateFunction>,
    pub time_range: TimeRange,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuousAggregateConfig {
    pub view_name: String,
    pub source_table: String,
    pub refresh_policy: RefreshPolicy,
    pub materialization_hypertable_config: Option<HypertableConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RefreshPolicy {
    Manual,
    Automatic { 
        refresh_interval: Duration,
        max_interval_per_job: Duration,
    },
    RealTime { 
        max_interval_per_job: Duration,
        materialized_only: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregateFunction {
    Count,
    Sum(String),
    Avg(String),
    Min(String),
    Max(String),
    StdDev(String),
    Variance(String),
    Percentile { column: String, percentile: f64 },
    First { column: String, order_by: String },
    Last { column: String, order_by: String },
}
```

## TimescaleDB Functions Reference

### Hypertable Functions
- **create_hypertable(table_name, time_column, ...)** - Convert table to hypertable
- **drop_chunks(hypertable, older_than, ...)** - Remove old chunks
- **show_chunks(hypertable, ...)** - Display chunk information
- **add_dimension(hypertable, column_name, ...)** - Add space partition
- **set_chunk_time_interval(hypertable, interval)** - Change chunk interval

### Time Functions
- **time_bucket(bucket_width, ts, ...)** - Group timestamps into buckets
- **time_bucket_gapfill(bucket_width, ts, ...)** - Fill gaps in time buckets
- **locf(value)** - Last observation carried forward
- **interpolate(value)** - Linear interpolation

### Analytics Functions
- **first(value, time)** - First value in time order
- **last(value, time)** - Last value in time order
- **histogram(value, min, max, nbuckets)** - Create histogram
- **percentile_agg(percentile)** - Percentile aggregation

### Compression Functions
- **add_compression_policy(hypertable, compress_after)** - Add compression policy
- **remove_compression_policy(hypertable)** - Remove compression policy
- **compress_chunk(chunk)** - Manually compress chunk
- **decompress_chunk(chunk)** - Manually decompress chunk

## Usage Examples

### IoT Sensor Data Management

```sql
-- Create sensors table
CREATE TABLE sensor_readings (
    timestamp TIMESTAMPTZ NOT NULL,
    sensor_id TEXT NOT NULL,
    location TEXT NOT NULL,
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    pressure DOUBLE PRECISION,
    battery_level INTEGER
);

-- Convert to hypertable with 1-hour chunks, partitioned by sensor_id
SELECT create_hypertable('sensor_readings', 'timestamp',
    chunk_time_interval => INTERVAL '1 hour',
    partitioning_column => 'sensor_id',
    number_partitions => 8);

-- Create index for efficient queries
CREATE INDEX ON sensor_readings (sensor_id, timestamp DESC);

-- Set up data retention (keep data for 90 days)
SELECT add_retention_policy('sensor_readings', INTERVAL '90 days');

-- Set up compression (compress data older than 1 day)
SELECT add_compression_policy('sensor_readings', INTERVAL '1 day');

-- Insert sample data
INSERT INTO sensor_readings VALUES 
('2024-01-01 10:00:00', 'temp_001', 'building_a', 23.5, 45.2, 1013.25, 87),
('2024-01-01 10:01:00', 'temp_001', 'building_a', 23.6, 45.1, 1013.30, 87),
('2024-01-01 10:02:00', 'temp_001', 'building_a', 23.4, 45.3, 1013.20, 86);

-- Query with time bucketing
SELECT 
    time_bucket('15 minutes', timestamp) AS fifteen_min_bucket,
    sensor_id,
    location,
    AVG(temperature) as avg_temp,
    MAX(temperature) - MIN(temperature) as temp_range,
    AVG(humidity) as avg_humidity
FROM sensor_readings
WHERE timestamp >= NOW() - INTERVAL '4 hours'
GROUP BY fifteen_min_bucket, sensor_id, location
ORDER BY fifteen_min_bucket DESC, sensor_id;
```

### Financial Time Series

```sql
-- Create stock prices table
CREATE TABLE stock_prices (
    timestamp TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    volume BIGINT,
    market_cap DOUBLE PRECISION
);

-- Create hypertable for stock data
SELECT create_hypertable('stock_prices', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    partitioning_column => 'symbol',
    number_partitions => 16);

-- Create continuous aggregate for OHLCV data
CREATE MATERIALIZED VIEW stock_ohlcv_1min
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 minute', timestamp) AS bucket,
    symbol,
    first(price, timestamp) AS open,
    max(price) AS high,
    min(price) AS low,
    last(price, timestamp) AS close,
    sum(volume) AS volume
FROM stock_prices
GROUP BY bucket, symbol;

-- Add automatic refresh policy
SELECT add_continuous_aggregate_policy('stock_ohlcv_1min',
    start_offset => INTERVAL '1 hour',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '1 minute');

-- Query OHLCV data with gap filling
SELECT 
    time_bucket_gapfill('1 minute', bucket, NOW() - INTERVAL '2 hours', NOW()) AS time,
    symbol,
    locf(open) AS open,
    locf(high) AS high,
    locf(low) AS low,
    locf(close) AS close,
    coalesce(volume, 0) AS volume
FROM stock_ohlcv_1min
WHERE bucket >= NOW() - INTERVAL '2 hours'
  AND symbol = 'AAPL'
GROUP BY time, symbol
ORDER BY time DESC;
```

### Application Performance Monitoring

```sql
-- Create metrics table for APM data
CREATE TABLE app_metrics (
    timestamp TIMESTAMPTZ NOT NULL,
    service_name TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    tags JSONB
);

-- Create hypertable
SELECT create_hypertable('app_metrics', 'timestamp',
    chunk_time_interval => INTERVAL '6 hours',
    partitioning_column => 'service_name',
    number_partitions => 4);

-- Create index for efficient tag queries
CREATE INDEX ON app_metrics USING GIN (tags);

-- Continuous aggregate for service health dashboard
CREATE MATERIALIZED VIEW service_health_5min
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('5 minutes', timestamp) AS bucket,
    service_name,
    metric_name,
    AVG(metric_value) as avg_value,
    percentile_agg(0.95) as p95_value,
    percentile_agg(0.99) as p99_value,
    COUNT(*) as sample_count
FROM app_metrics
WHERE metric_name IN ('response_time', 'cpu_usage', 'memory_usage', 'error_rate')
GROUP BY bucket, service_name, metric_name;

-- Query service performance trends
SELECT 
    bucket,
    service_name,
    metric_name,
    avg_value,
    percentile(p95_value, 0.95) OVER (
        PARTITION BY service_name, metric_name 
        ORDER BY bucket 
        ROWS BETWEEN 11 PRECEDING AND CURRENT ROW
    ) as rolling_p95
FROM service_health_5min
WHERE bucket >= NOW() - INTERVAL '1 hour'
  AND service_name = 'user-service'
ORDER BY bucket DESC, metric_name;
```

## Integration with Orbit Features

### Distributed Architecture
- **Actor-based hypertables**: Each hypertable managed by dedicated actors
- **Chunk distribution**: Intelligent chunk placement across cluster nodes
- **Query coordination**: Distributed query execution and result aggregation
- **Fault tolerance**: Automatic failover and chunk replication

### Performance Optimization
- **Vectorized execution**: SIMD-optimized time-series functions
- **Parallel chunk processing**: Multi-threaded chunk operations
- **Smart caching**: Cache frequently accessed chunks and aggregates
- **Query optimization**: Advanced query planning for time-series workloads

### Advanced Features
- **Real-time analytics**: Stream processing integration
- **Machine learning**: Built-in time-series ML functions
- **Alerting**: Pattern detection and anomaly alerting
- **Data lineage**: Track data transformations and aggregations

## Monitoring and Observability

### TimescaleDB-specific Metrics
- Hypertable and chunk statistics
- Compression ratios and storage efficiency
- Continuous aggregate refresh performance
- Query performance across time ranges

### Grafana Integration
- TimescaleDB data source compatibility
- Pre-built dashboards for monitoring hypertables
- Custom time-series visualization components
- Real-time alerting based on continuous aggregates

## Development Timeline

| Phase | Duration | Features |
|-------|----------|----------|
| **Phase 12** | 10-12 weeks | Core hypertable functionality, basic time functions |
| **Phase 13** | 8-10 weeks | Continuous aggregates, advanced analytics functions |
| **Phase 14** | 10-12 weeks | Compression, multi-node features, enterprise analytics |

**Total Estimated Effort**: 28-34 weeks

## Compatibility and Migration

### TimescaleDB Compatibility
- **SQL compatibility**: 100% compatibility with TimescaleDB SQL extensions
- **Function compatibility**: All TimescaleDB functions supported
- **Client compatibility**: Works with existing PostgreSQL clients and ORMs
- **Schema compatibility**: Compatible with TimescaleDB table structures

### Migration Tools
- **pg_dump/pg_restore**: Standard PostgreSQL migration tools
- **Parallel migration**: Multi-threaded data migration from TimescaleDB
- **Schema conversion**: Automated conversion of TimescaleDB schemas
- **Validation tools**: Verify data integrity after migration

### Integration Ecosystem
- **Grafana**: Native TimescaleDB data source support
- **Prometheus**: TimescaleDB-compatible metrics ingestion
- **Apache Kafka**: Stream processing integration
- **Machine Learning**: Integration with popular ML frameworks

This comprehensive TimescaleDB implementation will establish Orbit-RS as a powerful alternative to TimescaleDB while providing enhanced distributed capabilities and maintaining full PostgreSQL compatibility.