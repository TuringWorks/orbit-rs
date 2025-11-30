---
layout: default
title: Orbit Time Series Commands
category: documentation
---

## Orbit Time Series Commands

## Table of Contents

- [Overview](#overview)
- [Time Series Creation](#time-series-creation)
- [Data Ingestion](#data-ingestion)
- [Data Retrieval](#data-retrieval)
- [Data Management](#data-management)
- [Aggregation and Analytics](#aggregation-and-analytics)
- [Compaction Rules](#compaction-rules)
- [Labels and Metadata](#labels-and-metadata)
- [Error Handling](#error-handling)
- [Examples and Use Cases](#examples-and-use-cases)
- [Performance Considerations](#performance-considerations)

## Overview

Orbit's time series implementation provides:

- **High-performance ingestion**: Optimized for high-frequency data writes
- **Flexible retention policies**: Automatic data expiration based on time or size
- **Advanced aggregation**: Built-in support for statistical functions
- **Label-based filtering**: Multi-dimensional time series with metadata
- **Automatic downsampling**: Compaction rules for data reduction
- **Redis compatibility**: Drop-in replacement for Redis TimeSeries

## Time Series Creation

### TS.CREATE

Creates a new time series with optional configuration.

```redis
TS.CREATE key [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
```

**Parameters:**

- `key`: Time series identifier
- `RETENTION`: Data retention time in milliseconds (optional)
- `CHUNK_SIZE`: Maximum samples per chunk (optional, default: 4096)
- `DUPLICATE_POLICY`: How to handle duplicate timestamps (optional, default: BLOCK)
  - `BLOCK`: Reject duplicate timestamps
  - `FIRST`: Keep first value
  - `LAST`: Keep last value  
  - `MIN`: Keep minimum value
  - `MAX`: Keep maximum value
  - `SUM`: Sum values
- `LABELS`: Key-value pairs for metadata (optional)

**Examples:**

```redis

# Basic time series
TS.CREATE temperature:sensor1

# With retention (24 hours)
TS.CREATE temperature:sensor2 RETENTION 86400000

# With labels and configuration
TS.CREATE cpu:usage RETENTION 3600000 CHUNK_SIZE 2048 DUPLICATE_POLICY LAST LABELS location server1 metric cpu
```

### TS.ALTER

Modifies an existing time series configuration.

```redis
TS.ALTER key [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
```

**Example:**

```redis
TS.ALTER temperature:sensor1 RETENTION 7200000 LABELS environment production
```

## Data Ingestion

### TS.ADD

Adds a single sample to a time series.

```redis
TS.ADD key timestamp value [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
```

**Parameters:**

- `timestamp`: Unix timestamp in milliseconds, or `*` for current time
- `value`: Numeric value to store

**Examples:**

```redis

# Add sample with specific timestamp
TS.ADD temperature:sensor1 1609459200000 23.5

# Add sample with current timestamp
TS.ADD temperature:sensor1 * 24.1

# Add with configuration (creates series if doesn't exist)
TS.ADD cpu:usage * 75.2 RETENTION 3600000 LABELS server web01
```

### TS.MADD

Adds multiple samples to multiple time series in a single command.

```redis
TS.MADD key1 timestamp1 value1 [key2 timestamp2 value2 ...]
```

**Example:**

```redis
TS.MADD temperature:sensor1 1609459200000 23.5 temperature:sensor2 1609459200000 22.8 cpu:usage 1609459200000 45.2
```

### TS.INCRBY / TS.DECRBY

Increments or decrements the value at a timestamp.

```redis
TS.INCRBY key value [TIMESTAMP timestamp] [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
TS.DECRBY key value [TIMESTAMP timestamp] [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
```

**Examples:**

```redis

# Increment counter
TS.INCRBY requests:count 1

# Increment with specific timestamp  
TS.INCRBY requests:count 5 TIMESTAMP 1609459200000

# Decrement
TS.DECRBY active:connections 1
```

## Data Retrieval

### TS.GET

Retrieves the latest sample from a time series.

```redis
TS.GET key
```

**Example:**

```redis
TS.GET temperature:sensor1

# Returns: [timestamp, value]
```

### TS.MGET

Retrieves the latest samples from multiple time series.

```redis
TS.MGET [FILTER label=value ...] key1 [key2 ...]
```

**Example:**

```redis

# Get latest from specific series
TS.MGET temperature:sensor1 temperature:sensor2

# With label filtering (simplified implementation)
TS.MGET temperature:sensor1 temperature:sensor2
```

### TS.RANGE / TS.REVRANGE

Retrieves samples within a time range.

```redis
TS.RANGE key fromTimestamp toTimestamp [AGGREGATION aggregation bucketDuration]
TS.REVRANGE key fromTimestamp toTimestamp [AGGREGATION aggregation bucketDuration]
```

**Aggregation types:**

- `AVG`: Average value
- `SUM`: Sum of values
- `MIN`: Minimum value
- `MAX`: Maximum value
- `COUNT`: Count of samples
- `FIRST`: First value in bucket
- `LAST`: Last value in bucket
- `RANGE`: Max - Min
- `STD`: Standard deviation

**Examples:**

```redis

# Get all samples in range
TS.RANGE temperature:sensor1 1609459200000 1609462800000

# Get hourly averages
TS.RANGE temperature:sensor1 1609459200000 1609462800000 AGGREGATION AVG 3600000

# Reverse chronological order
TS.REVRANGE temperature:sensor1 1609459200000 1609462800000
```

### TS.MRANGE / TS.MREVRANGE

Retrieves samples from multiple time series within a time range.

```redis
TS.MRANGE fromTimestamp toTimestamp [AGGREGATION aggregation bucketDuration] [FILTER label=value ...] key1 [key2 ...]
TS.MREVRANGE fromTimestamp toTimestamp [AGGREGATION aggregation bucketDuration] [FILTER label=value ...] key1 [key2 ...]
```

**Example:**

```redis
TS.MRANGE 1609459200000 1609462800000 temperature:sensor1 temperature:sensor2
```

## Data Management

### TS.DEL

Deletes samples within a time range.

```redis
TS.DEL key fromTimestamp toTimestamp
```

**Example:**

```redis

# Delete samples from last hour
TS.DEL temperature:sensor1 1609459200000 1609462800000
```

### TS.INFO

Returns information about a time series.

```redis
TS.INFO key
```

**Returns:**

- `totalSamples`: Number of samples
- `memoryUsage`: Memory consumption in bytes
- `firstTimestamp`: Timestamp of first sample
- `lastTimestamp`: Timestamp of last sample
- `retentionTime`: Retention policy
- `chunkSize`: Chunk size configuration
- `duplicatePolicy`: Duplicate handling policy
- `labels`: Associated labels

**Example:**

```redis
TS.INFO temperature:sensor1
```

## Aggregation and Analytics

### Built-in Aggregation Functions

Orbit supports comprehensive aggregation functions:

1. **AVG** - Arithmetic mean
2. **SUM** - Sum of all values
3. **MIN** - Minimum value
4. **MAX** - Maximum value  
5. **COUNT** - Number of samples
6. **FIRST** - First value chronologically
7. **LAST** - Last value chronologically
8. **RANGE** - Difference between max and min
9. **STD** - Standard deviation

### Time-based Bucketing

Aggregations are performed over time buckets specified in milliseconds:

```redis

# 5-minute averages
TS.RANGE metrics:cpu 1609459200000 1609462800000 AGGREGATION AVG 300000

# Hourly maximums
TS.RANGE metrics:temperature 1609459200000 1609545600000 AGGREGATION MAX 3600000
```

## Compaction Rules

Compaction rules automatically downsample high-resolution data to lower-resolution aggregated series.

### TS.CREATERULE

Creates an automatic compaction rule.

```redis
TS.CREATERULE sourceKey destKey AGGREGATION aggregation bucketDuration [retention]
```

**Example:**

```redis

# Create hourly averages from minute data
TS.CREATERULE metrics:cpu:1min metrics:cpu:1hour AGGREGATION AVG 3600000

# With retention for destination
TS.CREATERULE temperature:raw temperature:hourly AGGREGATION AVG 3600000 2592000000
```

### TS.DELETERULE

Removes a compaction rule.

```redis
TS.DELETERULE sourceKey destKey
```

**Example:**

```redis
TS.DELETERULE metrics:cpu:1min metrics:cpu:1hour
```

## Labels and Metadata

Time series can be tagged with labels for organization and filtering:

```redis
# Create series with labels
TS.CREATE metrics:cpu LABELS server web01 datacenter us-east region production

# Query by labels (using MGET/MRANGE)
TS.MGET FILTER server=web01
```

Labels are key-value pairs that help:

- Organize related time series
- Enable multi-dimensional queries
- Support monitoring and alerting systems
- Facilitate data discovery

## Error Handling

### Common Error Scenarios

1. **Non-existent series**: Operations on undefined time series return errors
2. **Invalid timestamps**: Non-numeric or negative timestamps are rejected
3. **Duplicate timestamps**: Handled according to the duplicate policy
4. **Invalid values**: Non-numeric values are rejected
5. **Range errors**: Invalid time ranges (end before start) are rejected

### Error Messages

```redis
ERR wrong number of arguments for 'TS.ADD' command
ERR invalid timestamp format  
ERR actor error: [specific error]
ERR failed to add sample: [reason]
```

## Examples and Use Cases

### IoT Sensor Monitoring

```redis

# Create temperature sensors with metadata
TS.CREATE temp:living_room LABELS room living location home sensor ds18b20
TS.CREATE temp:kitchen LABELS room kitchen location home sensor ds18b20

# Ingest sensor data
TS.ADD temp:living_room * 22.5
TS.ADD temp:kitchen * 24.1

# Query recent temperatures
TS.MGET temp:living_room temp:kitchen

# Get daily averages
TS.RANGE temp:living_room 1609459200000 1609545600000 AGGREGATION AVG 86400000
```

### Application Performance Monitoring

```redis

# Create performance metrics
TS.CREATE app:response_time LABELS service api version v2
TS.CREATE app:request_count LABELS service api version v2
TS.CREATE app:error_rate LABELS service api version v2

# Track request metrics
TS.ADD app:response_time * 120.5
TS.INCRBY app:request_count 1
TS.ADD app:error_rate * 0.02

# Create compaction rules for long-term storage
TS.CREATERULE app:response_time app:response_time:hourly AGGREGATION AVG 3600000
TS.CREATERULE app:request_count app:request_count:hourly AGGREGATION SUM 3600000
```

### System Metrics Collection

```redis

# CPU usage tracking
TS.CREATE system:cpu RETENTION 604800000 LABELS host server01 metric cpu
TS.CREATE system:memory RETENTION 604800000 LABELS host server01 metric memory
TS.CREATE system:disk RETENTION 604800000 LABELS host server01 metric disk

# Batch insert multiple metrics
TS.MADD system:cpu 1609459200000 75.2 system:memory 1609459200000 68.5 system:disk 1609459200000 42.1

# Get system overview
TS.MRANGE 1609459200000 1609462800000 system:cpu system:memory system:disk
```

### Financial Time Series

```redis

# Stock price tracking
TS.CREATE stock:AAPL:price LABELS symbol AAPL exchange NASDAQ type price
TS.CREATE stock:AAPL:volume LABELS symbol AAPL exchange NASDAQ type volume

# Add price and volume data
TS.ADD stock:AAPL:price 1609459200000 150.25
TS.ADD stock:AAPL:volume 1609459200000 1250000

# Calculate daily statistics
TS.RANGE stock:AAPL:price 1609459200000 1609545600000 AGGREGATION MAX 86400000
TS.RANGE stock:AAPL:price 1609459200000 1609545600000 AGGREGATION MIN 86400000
```

## Performance Considerations

### Write Performance

- Batch operations with `TS.MADD` for better throughput
- Use appropriate chunk sizes (default 4096 is usually optimal)
- Consider retention policies to limit memory usage

### Read Performance  

- Use aggregation for large time ranges
- Leverage compaction rules for pre-computed aggregates
- Index series with meaningful labels

### Memory Management

- Set appropriate retention periods
- Use compaction to reduce raw data storage
- Monitor memory usage with `TS.INFO`

### Scaling

- Distribute time series across multiple Orbit nodes
- Use consistent labeling for cross-node queries
- Consider data locality for related time series

## Integration Examples

### Python Client

```python
import redis
import time

# Connect to Orbit
r = redis.Redis(host='localhost', port=6379)

# Create time series
r.execute_command('TS.CREATE', 'temperature:sensor1', 'LABELS', 'location', 'office')

# Add data
timestamp = int(time.time() * 1000)
r.execute_command('TS.ADD', 'temperature:sensor1', timestamp, 23.5)

# Query data  
result = r.execute_command('TS.GET', 'temperature:sensor1')
print(f"Latest temperature: {result}")
```

### Node.js Client

```javascript
const redis = require('redis');
const client = redis.createClient();

// Create and populate time series
async function example() {
  await client.sendCommand(['TS.CREATE', 'metrics:cpu']);
  
  const timestamp = Date.now();
  await client.sendCommand(['TS.ADD', 'metrics:cpu', timestamp.toString(), '75.5']);
  
  const result = await client.sendCommand(['TS.GET', 'metrics:cpu']);
  console.log('Latest CPU usage:', result);
}
```

### Go Client

```go
package main

import (
    "time"
    "github.com/go-redis/redis/v8"
)

func main() {
    rdb := redis.NewClient(&redis.Options{
        Addr: "localhost:6379",
    })
    
    // Create time series
    rdb.Do(ctx, "TS.CREATE", "temperature:sensor1").Result()
    
    // Add sample
    timestamp := time.Now().UnixNano() / 1e6
    rdb.Do(ctx, "TS.ADD", "temperature:sensor1", timestamp, 23.5).Result()
    
    // Get latest
    result := rdb.Do(ctx, "TS.GET", "temperature:sensor1").Result()
    fmt.Println("Latest temperature:", result)
}
```

## Migration from Redis TimeSeries

Orbit's time series implementation is designed to be a drop-in replacement for Redis TimeSeries. Most existing applications using Redis TimeSeries commands will work without modification.

### Command Compatibility

-  All core TS.* commands supported
-  Same parameter syntax and semantics  
-  Compatible data formats and return values
-  Same aggregation functions and behavior

### Differences

- Built on Orbit's distributed actor system
- Enhanced scalability and fault tolerance
- Consistent performance under high load
- Native integration with Orbit's other data types

---

For more information about Orbit's time series capabilities, see the [Orbit documentation](./README.md).
