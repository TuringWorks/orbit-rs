-- Refinery IoT Data (CQL)
-- Storing high-frequency sensor readings from critical infrastructure.

CREATE KEYSPACE IF NOT EXISTS refinery_iot 
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

USE refinery_iot;

-- 1. Sensor Readings
-- Partitioned by sensor_id and hour to keep partitions manageable.
CREATE TABLE IF NOT EXISTS sensor_readings (
    sensor_id text,
    datebucket date,
    ts timestamp,
    temperature double,
    pressure double,
    flow_rate double,
    vibration double,
    PRIMARY KEY ((sensor_id, datebucket), ts)
) WITH CLUSTERING ORDER BY (ts DESC);

-- 2. Alarms
CREATE TABLE IF NOT EXISTS alarms (
    sensor_id text,
    ts timestamp,
    type text, -- 'HIGH_TEMP', 'LOW_FLOW'
    value double,
    threshold double,
    PRIMARY KEY (sensor_id, ts)
) WITH CLUSTERING ORDER BY (ts DESC);

-- Sample Query:
-- SELECT * FROM sensor_readings WHERE sensor_id = 'S-101' AND datebucket = '2024-05-20' ORDER BY ts DESC LIMIT 60;
