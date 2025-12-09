-- ============================================================================
-- OrbitRS Protocol Examples - CQL (Cassandra)
-- ============================================================================
-- Keyspace Setup and Basic Table
-- ============================================================================

-- 1. Create Keyspace
CREATE KEYSPACE IF NOT EXISTS orbit_benchmark 
WITH replication = {
    'class': 'SimpleStrategy', 
    'replication_factor': 1
};

USE orbit_benchmark;

-- 2. Create Sensor Data Table (Time-Series)
CREATE TABLE IF NOT EXISTS sensor_readings (
    sensor_id uuid,
    bucket text,
    timestamp timestamp,
    temperature float,
    pressure float,
    status text,
    PRIMARY KEY ((sensor_id, bucket), timestamp)
) WITH CLUSTERING ORDER BY (timestamp DESC);

-- 3. Insert Sample Data
INSERT INTO sensor_readings (sensor_id, bucket, timestamp, temperature, pressure, status)
VALUES (uuid(), '2024-05', toTimestamp(now()), 72.5, 1013.2, 'OK');

-- 4. Query Data
SELECT * FROM sensor_readings WHERE sensor_id = ? AND bucket = '2024-05' LIMIT 10;
