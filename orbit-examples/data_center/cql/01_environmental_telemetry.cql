-- CQL: Environmental Telemetry & Sensor Data
-- Handles massive write throughput from thousands of sensors per site.

-- 1. Keyspace Setup
CREATE KEYSPACE IF NOT EXISTS dc_telemetry 
WITH REPLICATION = { 'class' : 'SimpleStrategy', 'replication_factor' : 3 };

USE dc_telemetry;

-- 2. Sensor Readings Table
-- Optimized for time-slice queries (e.g., "Give me all temps for Rack X in the last hour").
CREATE TABLE sensor_readings (
    site_id UUID,
    asset_id TEXT, -- e.g., 'rack-101' or 'sat-101'
    sensor_type TEXT, -- 'temperature', 'power_draw', 'fan_speed', 'radiation'
    bucket_time TIMESTAMP, -- Partition by hour to keep partitions manageable
    reading_time TIMESTAMP,
    value DOUBLE,
    unit TEXT,
    PRIMARY KEY ((site_id, asset_id, sensor_type, bucket_time), reading_time)
) WITH CLUSTERING ORDER BY (reading_time DESC);

-- 3. Orbital Health Check
-- Specific table for critical satellite health metrics which might be queried differently.
CREATE TABLE orbital_health (
    satellite_id TEXT,
    reading_time TIMESTAMP,
    battery_level_pct FLOAT,
    solar_input_watts FLOAT,
    attitude_fuel_kg FLOAT,
    radiation_sieverts FLOAT,
    PRIMARY KEY (satellite_id, reading_time)
) WITH CLUSTERING ORDER BY (reading_time DESC)
  AND default_time_to_live = 2592000; -- Auto-expire data after 30 days

-- Sample Data Insertion: Rack Temperature
INSERT INTO sensor_readings (site_id, asset_id, sensor_type, bucket_time, reading_time, value, unit)
VALUES (
    uuid(), 
    'rack-055', 
    'temperature_inlet', 
    '2025-10-10 10:00:00', 
    toTimestamp(now()), 
    22.5, 
    'Celsius'
);

-- Sample Data Insertion: Orbital Radiation Spike
INSERT INTO orbital_health (satellite_id, reading_time, battery_level_pct, radiation_sieverts)
VALUES ('SAT-101', toTimestamp(now()), 88.5, 0.005);

-- Query: Get last 10 readings for a specific sensor in the current hour
SELECT reading_time, value 
FROM sensor_readings 
WHERE site_id = ? 
  AND asset_id = 'rack-055' 
  AND sensor_type = 'temperature_inlet' 
  AND bucket_time = '2025-10-10 10:00:00'
LIMIT 10;
