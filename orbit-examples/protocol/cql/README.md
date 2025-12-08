# CQL (Cassandra) Protocol Examples for Orbit-RS

This directory contains comprehensive examples demonstrating CQL (Cassandra Query Language) protocol support in Orbit-RS.

## Overview

Orbit-RS implements the CQL protocol, allowing you to use standard Cassandra clients and tools to interact with the Orbit database using wide-column data modeling patterns.

## Quick Start

### Prerequisites

1. **Start Orbit Server** with CQL protocol enabled (default port 9042):
   ```bash
   cargo run --bin orbit-server
   ```

2. **Install CQL Client**:
   ```bash
   # Install cqlsh
   pip install cqlsh
   
   # Or use Docker
   docker run -it --rm cassandra cqlsh host.docker.internal 9042
   
   # Python driver
   pip install cassandra-driver
   ```

### Basic Connection

```bash
# Connect with cqlsh
cqlsh localhost 9042

# Or run a CQL file
cqlsh localhost 9042 -f 01_keyspace_setup.cql
```

## Examples Structure

```
cql/
├── README.md                          # This file
├── 01_keyspace_setup.cql              # Keyspace and table creation
├── 02_wide_column_modeling.cql        # Wide-column data patterns
├── 03_ml_integration.cql              # ML function integration
├── 04_time_series.cql                 # Time-series data modeling
├── 05_materialized_views.cql          # Materialized views
├── python/
│   ├── cassandra_driver.py            # Python cassandra-driver examples
│   └── cql_ml.py                      # ML integration
└── scenarios/
    ├── iot_scenario.cql               # IoT sensor data
    └── timeseries_scenario.cql        # Time-series analytics
```

## Running Examples

### CQL Examples

```bash
# Run with cqlsh
cqlsh localhost 9042 -f 01_keyspace_setup.cql

# Or run interactively
cqlsh localhost 9042
cqlsh> SOURCE '01_keyspace_setup.cql';
```

### Python Examples

```bash
cd python
python cassandra_driver.py
python cql_ml.py
```

## Key Features Demonstrated

### 1. Keyspace Management
- CREATE KEYSPACE with replication strategies
- ALTER KEYSPACE
- DROP KEYSPACE
- Replication factor configuration

### 2. Wide-Column Tables
- Partition keys
- Clustering keys
- Static columns
- Collections (lists, sets, maps)

### 3. Data Modeling Patterns
- Time-series data
- Event logging
- Sensor data
- User activity tracking

### 4. ML Integration
- ML_PREDICT() for inference
- ML_TRAIN_MODEL() for training
- ML_EMBED_TEXT() for embeddings

### 5. Performance Features
- Materialized views
- Secondary indexes
- Batch operations
- Prepared statements

## CQL vs Other Protocols

**When to use CQL protocol:**
- Wide-column data modeling
- Time-series data
- High write throughput
- Distributed data storage
- Cassandra migration

**Cross-Protocol Access:**
```cql
-- Write via CQL
INSERT INTO sensor_data (sensor_id, timestamp, temperature, humidity)
VALUES ('SENSOR-001', toTimestamp(now()), 23.5, 65.0);

-- Read via PostgreSQL
psql> SELECT * FROM sensor_data WHERE sensor_id = 'SENSOR-001';

-- Read via MongoDB
db.sensor_data.find({sensor_id: 'SENSOR-001'})

-- Read via Redis
redis-cli> HGETALL sensor:SENSOR-001:latest
```

## Data Modeling Patterns

### Time-Series Pattern
```cql
CREATE TABLE sensor_readings (
    sensor_id text,
    reading_date date,
    reading_time timestamp,
    temperature double,
    humidity double,
    PRIMARY KEY ((sensor_id, reading_date), reading_time)
) WITH CLUSTERING ORDER BY (reading_time DESC);
```

### Event Logging Pattern
```cql
CREATE TABLE user_events (
    user_id uuid,
    event_date date,
    event_time timestamp,
    event_type text,
    event_data map<text, text>,
    PRIMARY KEY ((user_id, event_date), event_time)
) WITH CLUSTERING ORDER BY (event_time DESC);
```

### Wide Row Pattern
```cql
CREATE TABLE user_preferences (
    user_id uuid PRIMARY KEY,
    preferences map<text, text>,
    settings frozen<list<text>>,
    metadata map<text, frozen<map<text, text>>>
);
```

## ML Function Examples

### Model Training
```cql
-- Train a model on sensor data
SELECT ML_TRAIN_MODEL(
    'sensor_anomaly_model',
    'isolation_forest',
    [temperature, humidity, pressure],
    anomaly_label
) FROM sensor_data;
```

### Prediction
```cql
-- Detect anomalies in real-time
SELECT 
    sensor_id,
    reading_time,
    temperature,
    ML_PREDICT('sensor_anomaly_model', [temperature, humidity, pressure]) as is_anomaly
FROM sensor_readings
WHERE sensor_id = 'SENSOR-001'
  AND reading_date = '2024-03-15';
```

## Performance Tips

1. **Choose Partition Keys Wisely**: Distribute data evenly
   ```cql
   -- Good: Distributes by sensor and date
   PRIMARY KEY ((sensor_id, reading_date), reading_time)
   
   -- Bad: All data in one partition
   PRIMARY KEY (sensor_id, reading_time)
   ```

2. **Use Clustering Keys for Sorting**: Pre-sort data
   ```cql
   CREATE TABLE events (
       user_id uuid,
       event_time timestamp,
       event_type text,
       PRIMARY KEY (user_id, event_time)
   ) WITH CLUSTERING ORDER BY (event_time DESC);
   ```

3. **Batch Inserts**: Use BATCH for related writes
   ```cql
   BEGIN BATCH
       INSERT INTO sensor_data (...) VALUES (...);
       INSERT INTO sensor_summary (...) VALUES (...);
   APPLY BATCH;
   ```

4. **Materialized Views**: Pre-compute queries
   ```cql
   CREATE MATERIALIZED VIEW sensors_by_location AS
       SELECT * FROM sensors
       WHERE location IS NOT NULL
       PRIMARY KEY (location, sensor_id);
   ```

## Troubleshooting

### Connection Issues
```bash
# Check if CQL port is listening
lsof -i :9042

# Test connection
cqlsh localhost 9042 -e "SELECT now() FROM system.local"
```

### Query Performance
```cql
-- Use TRACING to analyze queries
TRACING ON;
SELECT * FROM sensor_data WHERE sensor_id = 'SENSOR-001';
TRACING OFF;
```

### Data Consistency
```cql
-- Check consistency level
CONSISTENCY QUORUM;
SELECT * FROM sensor_data;
```

## Differences from Apache Cassandra

Orbit-RS implements CQL protocol but uses its own storage:

**Supported:**
- CQL syntax and semantics
- Partition and clustering keys
- Collections (lists, sets, maps)
- Materialized views
- ML functions (Orbit extension)

**Not Supported (yet):**
- Multi-datacenter replication
- Lightweight transactions (LWT)
- User-defined types (UDT) - in progress
- User-defined functions (UDF)

## See Also

- [CQL Protocol Implementation](../../orbit/server/src/protocols/cql/)
- [Cross-Protocol Examples](../cross-protocol/)
- [Time-Series Examples](../ml-protocol-examples/)
- [Cassandra CQL Documentation](https://cassandra.apache.org/doc/latest/cql/)

## Contributing

To add new CQL examples:
1. Follow the existing file naming convention
2. Include comprehensive comments
3. Demonstrate CQL-specific features
4. Test against running Orbit server
5. Update this README
