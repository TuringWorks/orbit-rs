-- ============================================================================
-- OrbitRS Energy Examples - Smart Meter Data (CQL)
-- ============================================================================
-- Advanced Metering Infrastructure (AMI) data
-- ============================================================================

CREATE KEYSPACE IF NOT EXISTS smart_grid 
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

USE smart_grid;

CREATE TABLE meter_readings (
    meter_id text,
    reading_date date,
    reading_time timestamp,
    kwh_total double,
    voltage_avg double,
    PRIMARY KEY ((meter_id, reading_date), reading_time)
) WITH CLUSTERING ORDER BY (reading_time DESC);

INSERT INTO meter_readings (meter_id, reading_date, reading_time, kwh_total, voltage_avg)
VALUES ('MTR-1001', toDate(now()), toTimestamp(now()), 14502.5, 120.1);
