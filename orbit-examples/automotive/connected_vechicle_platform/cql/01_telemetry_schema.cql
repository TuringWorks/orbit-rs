-- Automotive Telemetry (CQL / Cassandra)
-- High-velocity write workload for sensor data.

CREATE KEYSPACE IF NOT EXISTS automotive_telemetry 
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

USE automotive_telemetry;

-- 1. Vehicle Telemetry
-- Partitioned by vehicle_id and day (bucket) to manage partition size.
-- Clustered by timestamp for fast range queries (e.g., "Give me last hour of data").
CREATE TABLE IF NOT EXISTS telemetry_raw (
    vehicle_id text,
    datebucket date,
    ts timestamp,
    speed double,
    engine_rpm int,
    fuel_level float,
    latitude double,
    longitude double,
    odometer double,
    PRIMARY KEY ((vehicle_id, datebucket), ts)
) WITH CLUSTERING ORDER BY (ts DESC);

-- 2. Diagnostic Trouble Codes (DTC)
-- Partitioned by vehicle_id to quickly find all issues for a car.
CREATE TABLE IF NOT EXISTS diagnostic_codes (
    vehicle_id text,
    ts timestamp,
    code text, -- e.g., P0300
    description text,
    severity text,
    PRIMARY KEY (vehicle_id, ts)
) WITH CLUSTERING ORDER BY (ts DESC);

-- Sample Queries (Comments):
-- INSERT INTO telemetry_raw (vehicle_id, datebucket, ts, speed, engine_rpm) VALUES ('VIN123', '2024-01-01', '2024-01-01 10:00:00', 65.0, 2500);
-- SELECT * FROM telemetry_raw WHERE vehicle_id = 'VIN123' AND datebucket = '2024-01-01' LIMIT 10;
