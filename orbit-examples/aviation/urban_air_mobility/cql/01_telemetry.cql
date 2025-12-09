-- ============================================================================
-- OrbitRS Aviation Examples - Urban Air Mobility (CQL)
-- ============================================================================
-- High-frequency eVTOL Battery & Motor Telemetry
-- ============================================================================

CREATE KEYSPACE IF NOT EXISTS uam_telemetry 
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

USE uam_telemetry;

-- eVTOLs have multiple motors/rotors (e.g., 6-12) and battery packs
CREATE TABLE motor_status (
    vehicle_id text,
    flight_id uuid,
    timestamp timestamp,
    motor_index int, -- 1 to N
    rpm int,
    temp_celsius float,
    vibration_level float,
    PRIMARY KEY ((vehicle_id, flight_id), timestamp, motor_index)
) WITH CLUSTERING ORDER BY (timestamp DESC, motor_index ASC);

CREATE TABLE battery_packs (
    vehicle_id text,
    timestamp timestamp,
    soc_percent float, -- State of Charge
    voltage float,
    current_draw float,
    cell_temp_max float,
    PRIMARY KEY (vehicle_id, timestamp)
);

-- Insert sample telemetry from an active flight
INSERT INTO motor_status (vehicle_id, flight_id, timestamp, motor_index, rpm, temp_celsius, vibration_level)
VALUES ('EVTOL-01', uuid(), toTimestamp(now()), 1, 2500, 65.5, 0.02);
