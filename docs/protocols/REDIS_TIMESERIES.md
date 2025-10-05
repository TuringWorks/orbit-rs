# Redis Time Series Compatibility

This document outlines the planned Redis Time Series (RedisTimeSeries) compatibility features for Orbit-RS, enabling time-series data storage and analysis capabilities through the Redis protocol.

## Overview

Redis Time Series is a Redis module that adds time-series data structure support to Redis. Orbit-RS will provide native compatibility with RedisTimeSeries commands, allowing existing applications to seamlessly migrate or integrate with Orbit's distributed actor system.

## Planned Features

### 🎯 **Phase 12: Time Series Foundation**

#### Core Time Series Actor
- **TimeSeriesActor**: Distributed time-series data management
- **Time-based partitioning**: Automatic data partitioning by time windows
- **Compression**: Built-in data compression for storage efficiency
- **Retention policies**: Automatic data expiration and cleanup

#### Basic Commands
- **TS.CREATE**: Create time series with metadata and retention policy
- **TS.ADD**: Add timestamp-value pairs to time series
- **TS.GET**: Get the latest or specific timestamp value
- **TS.RANGE**: Query data within time ranges
- **TS.REVRANGE**: Query data in reverse chronological order

### 🚀 **Phase 13: Advanced Time Series Operations**

#### Aggregation Commands
- **TS.CREATERULE**: Create aggregation rules between time series
- **TS.DELETERULE**: Delete aggregation rules
- **TS.MRANGE**: Multi-series range queries with filters
- **TS.MREVRANGE**: Multi-series reverse range queries
- **TS.MGET**: Get latest values from multiple time series

#### Statistical Functions
- **Built-in aggregators**: AVG, SUM, MIN, MAX, COUNT, STDDEV, VAR
- **Time-window aggregations**: Configurable time windows (1s, 1m, 1h, 1d)
- **Downsampling**: Automatic data reduction for long-term storage

### 📊 **Phase 14: Enterprise Time Series Features**

#### Advanced Analytics
- **TS.QUERYINDEX**: Query time series by labels and filters
- **TS.INFO**: Get comprehensive time series metadata
- **Labeling system**: Multi-dimensional time series organization
- **Compaction**: Automatic data compaction with configurable policies

#### Performance Optimizations
- **Chunk-based storage**: Efficient memory and disk usage
- **Indexing**: Fast time-based and label-based queries
- **Caching**: Intelligent caching for frequently accessed data
- **Parallel processing**: Multi-threaded query execution

## Technical Implementation

### Actor Architecture

```rust
#[async_trait]
pub trait TimeSeriesActor: ActorWithStringKey {
    // Core operations
    async fn add_sample(&self, timestamp: u64, value: f64) -> OrbitResult<u64>;
    async fn get_sample(&self, timestamp: Option<u64>) -> OrbitResult<Option<(u64, f64)>>;
    async fn range_query(&self, from: u64, to: u64, aggregation: Option<Aggregation>) -> OrbitResult<Vec<(u64, f64)>>;
    
    // Metadata operations
    async fn create_series(&self, config: TimeSeriesConfig) -> OrbitResult<()>;
    async fn add_label(&self, key: String, value: String) -> OrbitResult<()>;
    async fn get_info(&self) -> OrbitResult<TimeSeriesInfo>;
    
    // Aggregation rules
    async fn create_rule(&self, dest_key: String, aggregation: Aggregation, bucket_duration: u64) -> OrbitResult<()>;
    async fn delete_rule(&self, dest_key: String) -> OrbitResult<bool>;
}
```

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesConfig {
    pub retention_ms: Option<u64>,
    pub chunk_size: Option<usize>,
    pub duplicate_policy: DuplicatePolicy,
    pub labels: HashMap<String, String>,
    pub uncompressed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesSample {
    pub timestamp: u64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationRule {
    pub dest_key: String,
    pub aggregation: Aggregation,
    pub bucket_duration: u64,
    pub alignment_timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Aggregation {
    Avg,
    Sum,
    Min,
    Max,
    Range,
    Count,
    Std,
    Var,
    First,
    Last,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DuplicatePolicy {
    Block,    // Reject duplicate timestamps
    Last,     // Keep last value
    First,    // Keep first value
    Min,      // Keep minimum value
    Max,      // Keep maximum value
    Sum,      // Sum values
}
```

## Command Reference

### TS.CREATE
```redis
TS.CREATE key [RETENTION retentionTime] [UNCOMPRESSED] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value...]
```

### TS.ADD
```redis
TS.ADD key timestamp value [RETENTION retentionTime] [UNCOMPRESSED] [CHUNK_SIZE size] [ON_DUPLICATE policy] [LABELS label value...]
```

### TS.RANGE
```redis
TS.RANGE key fromTimestamp toTimestamp [LATEST] [FILTER_BY_TS ts...] [FILTER_BY_VALUE min max] [COUNT count] [AGGREGATION aggregator bucketDuration [alignTimestamp]] [SELECTED_LABELS label...]
```

### TS.MRANGE
```redis
TS.MRANGE fromTimestamp toTimestamp [LATEST] [FILTER_BY_TS ts...] [FILTER_BY_VALUE min max] [WITHLABELS | SELECTED_LABELS label...] [COUNT count] [AGGREGATION aggregator bucketDuration [alignTimestamp]] FILTER filter...
```

### TS.CREATERULE
```redis
TS.CREATERULE sourceKey destKey AGGREGATION aggregator bucketDuration [alignTimestamp]
```

## Usage Examples

### Basic Time Series Operations

```python
import redis
r = redis.Redis(host='localhost', port=6380)  # Orbit-RS RESP server

# Create time series with 1 hour retention
r.execute_command('TS.CREATE', 'temperature:sensor1', 
                 'RETENTION', 3600000, 
                 'LABELS', 'sensor_id', '1', 'location', 'room_a')

# Add temperature readings
import time
now = int(time.time() * 1000)
r.execute_command('TS.ADD', 'temperature:sensor1', now, 23.5)
r.execute_command('TS.ADD', 'temperature:sensor1', now + 1000, 24.1)
r.execute_command('TS.ADD', 'temperature:sensor1', now + 2000, 23.8)

# Query range with 1-minute averages
results = r.execute_command('TS.RANGE', 'temperature:sensor1', 
                           now - 300000, now,
                           'AGGREGATION', 'AVG', 60000)
print(f"1-minute averages: {results}")

# Create downsampling rule for hourly averages
r.execute_command('TS.CREATERULE', 'temperature:sensor1', 'temperature:sensor1:hourly',
                 'AGGREGATION', 'AVG', 3600000)
```

### Multi-Series Analytics

```python
# Create multiple time series for different sensors
sensors = ['sensor1', 'sensor2', 'sensor3']
for sensor_id in sensors:
    r.execute_command('TS.CREATE', f'temperature:{sensor_id}',
                     'LABELS', 'sensor_id', sensor_id, 'type', 'temperature')

# Add readings to all sensors
for i, sensor_id in enumerate(sensors):
    base_temp = 20 + i  # Different base temperatures
    for j in range(100):
        timestamp = now + j * 1000
        temp = base_temp + (j % 10)  # Simulated temperature variation
        r.execute_command('TS.ADD', f'temperature:{sensor_id}', timestamp, temp)

# Query all temperature sensors for the last 10 minutes
results = r.execute_command('TS.MRANGE', now - 600000, now,
                           'AGGREGATION', 'AVG', 60000,
                           'FILTER', 'type=temperature')

for series_data in results:
    sensor_name, labels, samples = series_data
    print(f"{sensor_name}: {len(samples)} samples")
```

### IoT Data Pipeline

```python
# Real-time IoT data ingestion
def ingest_sensor_data(device_id, sensor_type, value):
    timestamp = int(time.time() * 1000)
    key = f"{sensor_type}:{device_id}"
    
    # Create series if it doesn't exist
    try:
        r.execute_command('TS.CREATE', key,
                         'RETENTION', 86400000,  # 24 hours
                         'LABELS', 
                         'device_id', device_id,
                         'sensor_type', sensor_type,
                         'location', get_device_location(device_id))
    except:
        pass  # Series already exists
    
    # Add sample
    r.execute_command('TS.ADD', key, timestamp, value)
    
    # Create aggregation rules for analytics
    hourly_key = f"{key}:hourly"
    daily_key = f"{key}:daily"
    
    try:
        r.execute_command('TS.CREATERULE', key, hourly_key,
                         'AGGREGATION', 'AVG', 3600000)
        r.execute_command('TS.CREATERULE', hourly_key, daily_key,
                         'AGGREGATION', 'AVG', 86400000)
    except:
        pass  # Rules already exist

# Usage
ingest_sensor_data('device_001', 'temperature', 25.6)
ingest_sensor_data('device_001', 'humidity', 65.2)
ingest_sensor_data('device_002', 'pressure', 1013.25)
```

## Integration with Orbit Features

### Distributed Time Series
- **Actor partitioning**: Time series data distributed across cluster nodes
- **Consistent hashing**: Deterministic placement of time series actors
- **Replication**: Configurable replication for high availability
- **Failover**: Automatic failover for time series actors

### Transaction Support
- **ACID compliance**: Ensure data consistency across time series operations
- **Bulk operations**: Efficient batch insertion of time series data
- **Cross-series transactions**: Atomic operations across multiple time series

### Performance Optimization
- **Vectorized operations**: SIMD-optimized aggregation functions
- **Parallel queries**: Multi-threaded range and aggregation queries
- **Caching**: Intelligent caching of frequently accessed time windows
- **Compression**: Advanced compression algorithms for time series data

## Monitoring and Observability

### Metrics
- Time series operation latency and throughput
- Storage efficiency and compression ratios
- Query performance across different time ranges
- Actor distribution and load balancing

### Grafana Integration
- Pre-built dashboards for time series monitoring
- Custom query builder for Orbit time series data
- Real-time alerting based on time series patterns
- Historical trend analysis and forecasting

## Development Timeline

| Phase | Duration | Features |
|-------|----------|----------|
| **Phase 12** | 8-10 weeks | Core time series foundation, basic commands |
| **Phase 13** | 6-8 weeks | Advanced operations, aggregation rules |
| **Phase 14** | 8-10 weeks | Enterprise features, performance optimization |

**Total Estimated Effort**: 22-28 weeks

## Compatibility

### Redis Time Series Compatibility
- **API compatibility**: 100% command compatibility with RedisTimeSeries
- **Data format**: Compatible time series data structures
- **Client support**: Works with existing Redis time series clients
- **Migration**: Tools for migrating from RedisTimeSeries to Orbit-RS

### Integration Points
- **Grafana**: Native data source plugin
- **Prometheus**: Export time series data to Prometheus
- **InfluxDB**: Migration and compatibility tools
- **TimescaleDB**: Cross-platform time series analytics

This comprehensive time series implementation will position Orbit-RS as a powerful alternative to dedicated time series databases while maintaining Redis protocol compatibility.