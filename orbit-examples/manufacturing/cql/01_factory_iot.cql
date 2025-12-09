-- ============================================================================
-- OrbitRS Manufacturing Examples - Factory IoT (CQL)
-- ============================================================================
-- High-speed data ingestion from assembly line sensors
-- ============================================================================

CREATE KEYSPACE IF NOT EXISTS smart_factory 
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

USE smart_factory;

CREATE TABLE production_line_sensors (
    line_id text,
    station_id text,
    window_time timestamp, -- 1 minute window
    reading_time timestamp,
    sensor_type text,
    value_double double,
    value_text text,
    PRIMARY KEY ((line_id, station_id, window_time), reading_time)
) WITH CLUSTERING ORDER BY (reading_time DESC);

-- Insert vibration sensor reading
INSERT INTO production_line_sensors (
    line_id, station_id, window_time, reading_time, sensor_type, value_double
) VALUES (
    'ASSY-LINE-1', 'ROBOT-3', '2024-06-15 08:00:00', toTimestamp(now()), 'VIBRATION_RMS', 0.45
);
