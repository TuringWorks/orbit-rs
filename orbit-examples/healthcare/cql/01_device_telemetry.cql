-- ============================================================================
-- OrbitRS Healthcare Examples - Medical IoT & Time-Series Data (CQL/Cassandra)
-- ============================================================================
-- Wide-column storage for high-frequency medical device telemetry
-- Demonstrates OrbitRS CQL protocol for healthcare IoT use cases
-- ============================================================================

-- Create keyspace with appropriate replication for healthcare data
CREATE KEYSPACE IF NOT EXISTS medical_iot
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 3}
AND durable_writes = true;

USE medical_iot;

-- ============================================================================
-- CONTINUOUS GLUCOSE MONITOR (CGM) READINGS
-- ============================================================================
-- Partition by device and day for efficient time-range queries
-- Clustering by time for chronological ordering

CREATE TABLE cgm_readings (
    device_id uuid,
    patient_mrn text,
    reading_date date,
    reading_time timestamp,
    glucose_mgdl int,
    trend_arrow text,  -- DOUBLE_UP, SINGLE_UP, FLAT, SINGLE_DOWN, DOUBLE_DOWN
    trend_rate float,  -- mg/dL per minute
    sensor_status text,
    calibration_required boolean,
    PRIMARY KEY ((device_id, reading_date), reading_time)
) WITH CLUSTERING ORDER BY (reading_time DESC)
  AND default_time_to_live = 7776000  -- 90 days retention
  AND compaction = {'class': 'TimeWindowCompactionStrategy', 'compaction_window_size': 1, 'compaction_window_unit': 'DAYS'};

-- Create index for patient lookup
CREATE INDEX ON cgm_readings (patient_mrn);

-- Insert CGM readings (every 5 minutes)
INSERT INTO cgm_readings (device_id, patient_mrn, reading_date, reading_time, glucose_mgdl, trend_arrow, trend_rate, sensor_status, calibration_required)
VALUES (11111111-1111-1111-1111-111111111111, 'MRN-888', '2024-12-09', '2024-12-09 08:00:00', 110, 'FLAT', 0.5, 'OK', false);

INSERT INTO cgm_readings (device_id, patient_mrn, reading_date, reading_time, glucose_mgdl, trend_arrow, trend_rate, sensor_status, calibration_required)
VALUES (11111111-1111-1111-1111-111111111111, 'MRN-888', '2024-12-09', '2024-12-09 08:05:00', 115, 'SINGLE_UP', 1.0, 'OK', false);

INSERT INTO cgm_readings (device_id, patient_mrn, reading_date, reading_time, glucose_mgdl, trend_arrow, trend_rate, sensor_status, calibration_required)
VALUES (11111111-1111-1111-1111-111111111111, 'MRN-888', '2024-12-09', '2024-12-09 08:10:00', 145, 'DOUBLE_UP', 3.0, 'OK', false);

-- Query today's readings for a device
SELECT reading_time, glucose_mgdl, trend_arrow
FROM cgm_readings
WHERE device_id = 11111111-1111-1111-1111-111111111111
  AND reading_date = '2024-12-09';

-- Query last hour of readings
SELECT reading_time, glucose_mgdl, trend_arrow
FROM cgm_readings
WHERE device_id = 11111111-1111-1111-1111-111111111111
  AND reading_date = '2024-12-09'
  AND reading_time >= '2024-12-09 07:00:00';

-- ============================================================================
-- VITAL SIGNS TIME-SERIES
-- ============================================================================
-- High-frequency vitals from ICU monitors (every second for critical patients)

CREATE TABLE vital_signs_ts (
    patient_mrn text,
    monitor_id uuid,
    vital_date date,
    vital_time timestamp,
    heart_rate int,
    blood_pressure_sys int,
    blood_pressure_dia int,
    spo2 float,
    respiratory_rate int,
    temperature float,
    ecg_waveform blob,  -- Compressed ECG data
    alert_level text,   -- NORMAL, WARNING, CRITICAL
    PRIMARY KEY ((patient_mrn, vital_date), vital_time)
) WITH CLUSTERING ORDER BY (vital_time DESC)
  AND default_time_to_live = 604800  -- 7 days for high-frequency data
  AND compression = {'sstable_compression': 'LZ4Compressor'};

-- Insert vital signs
INSERT INTO vital_signs_ts (patient_mrn, monitor_id, vital_date, vital_time, heart_rate, blood_pressure_sys, blood_pressure_dia, spo2, respiratory_rate, temperature, alert_level)
VALUES ('MRN-888', 22222222-2222-2222-2222-222222222222, '2024-12-09', '2024-12-09 08:00:00', 75, 120, 80, 98.5, 16, 98.6, 'NORMAL');

INSERT INTO vital_signs_ts (patient_mrn, monitor_id, vital_date, vital_time, heart_rate, blood_pressure_sys, blood_pressure_dia, spo2, respiratory_rate, temperature, alert_level)
VALUES ('MRN-888', 22222222-2222-2222-2222-222222222222, '2024-12-09', '2024-12-09 08:00:01', 76, 121, 81, 98.4, 16, 98.6, 'NORMAL');

-- Query last 60 seconds of vitals
SELECT vital_time, heart_rate, blood_pressure_sys, blood_pressure_dia, spo2
FROM vital_signs_ts
WHERE patient_mrn = 'MRN-888'
  AND vital_date = '2024-12-09'
  AND vital_time >= '2024-12-09 07:59:00'
LIMIT 60;

-- ============================================================================
-- INFUSION PUMP TELEMETRY
-- ============================================================================

CREATE TABLE infusion_pump_data (
    pump_id uuid,
    patient_mrn text,
    reading_date date,
    reading_time timestamp,
    drug_name text,
    concentration text,
    rate_ml_hr float,
    volume_infused_ml float,
    volume_remaining_ml float,
    battery_percent int,
    occlusion_pressure float,
    alarm_status text,
    PRIMARY KEY ((pump_id, reading_date), reading_time)
) WITH CLUSTERING ORDER BY (reading_time DESC)
  AND default_time_to_live = 2592000;  -- 30 days

-- Insert pump telemetry
INSERT INTO infusion_pump_data (pump_id, patient_mrn, reading_date, reading_time, drug_name, concentration, rate_ml_hr, volume_infused_ml, volume_remaining_ml, battery_percent, occlusion_pressure, alarm_status)
VALUES (33333333-3333-3333-3333-333333333333, 'MRN-888', '2024-12-09', '2024-12-09 08:00:00', 'Morphine', '1mg/mL', 2.0, 50.0, 200.0, 95, 150.5, 'NONE');

-- ============================================================================
-- WEARABLE DEVICE DATA (Fitbit, Apple Watch, etc.)
-- ============================================================================

CREATE TABLE wearable_data (
    patient_mrn text,
    device_type text,  -- FITBIT, APPLE_WATCH, GARMIN, etc.
    data_date date,
    data_time timestamp,
    steps int,
    heart_rate int,
    heart_rate_variability float,
    sleep_stage text,
    activity_type text,
    calories_burned int,
    stress_score int,
    blood_oxygen float,
    PRIMARY KEY ((patient_mrn, data_date), data_time, device_type)
) WITH CLUSTERING ORDER BY (data_time DESC, device_type ASC)
  AND default_time_to_live = 31536000;  -- 1 year

-- Insert wearable data
INSERT INTO wearable_data (patient_mrn, device_type, data_date, data_time, steps, heart_rate, heart_rate_variability, activity_type, calories_burned)
VALUES ('MRN-888', 'APPLE_WATCH', '2024-12-09', '2024-12-09 08:00:00', 1500, 72, 45.5, 'WALKING', 75);

-- ============================================================================
-- MEDICATION DISPENSER EVENTS
-- ============================================================================

CREATE TABLE medication_dispenser_events (
    patient_mrn text,
    dispenser_id uuid,
    event_date date,
    event_time timestamp,
    event_type text,  -- DISPENSE, MISSED, EARLY, LATE, OVERRIDE
    medication_name text,
    dosage text,
    compartment_number int,
    pills_remaining int,
    acknowledged boolean,
    caregiver_id text,
    PRIMARY KEY ((patient_mrn, event_date), event_time)
) WITH CLUSTERING ORDER BY (event_time DESC);

-- Insert dispenser event
INSERT INTO medication_dispenser_events (patient_mrn, dispenser_id, event_date, event_time, event_type, medication_name, dosage, compartment_number, pills_remaining, acknowledged)
VALUES ('MRN-888', 44444444-4444-4444-4444-444444444444, '2024-12-09', '2024-12-09 08:00:00', 'DISPENSE', 'Metformin', '500mg', 1, 28, true);

-- Query missed medications in last 7 days
SELECT event_time, medication_name, dosage
FROM medication_dispenser_events
WHERE patient_mrn = 'MRN-888'
  AND event_date >= '2024-12-02'
  AND event_type = 'MISSED'
ALLOW FILTERING;

-- ============================================================================
-- LAB RESULT HISTORY (Longitudinal)
-- ============================================================================

CREATE TABLE lab_results_history (
    patient_mrn text,
    test_code text,  -- LOINC code
    result_date date,
    result_time timestamp,
    result_value float,
    result_unit text,
    reference_low float,
    reference_high float,
    is_abnormal boolean,
    ordering_provider text,
    lab_facility text,
    PRIMARY KEY ((patient_mrn, test_code), result_date, result_time)
) WITH CLUSTERING ORDER BY (result_date DESC, result_time DESC);

-- Insert lab results
INSERT INTO lab_results_history (patient_mrn, test_code, result_date, result_time, result_value, result_unit, reference_low, reference_high, is_abnormal, ordering_provider, lab_facility)
VALUES ('MRN-888', '2345-7', '2024-12-09', '2024-12-09 10:30:00', 95.0, 'mg/dL', 70.0, 100.0, false, 'Dr. Smith', 'Main Lab');

-- HbA1c trend over time
INSERT INTO lab_results_history (patient_mrn, test_code, result_date, result_time, result_value, result_unit, reference_low, reference_high, is_abnormal, ordering_provider, lab_facility)
VALUES ('MRN-888', '4548-4', '2024-09-09', '2024-09-09 10:30:00', 7.2, '%', 4.0, 5.6, true, 'Dr. Smith', 'Main Lab');

INSERT INTO lab_results_history (patient_mrn, test_code, result_date, result_time, result_value, result_unit, reference_low, reference_high, is_abnormal, ordering_provider, lab_facility)
VALUES ('MRN-888', '4548-4', '2024-12-09', '2024-12-09 10:30:00', 6.8, '%', 4.0, 5.6, true, 'Dr. Smith', 'Main Lab');

-- Query HbA1c trend for a patient
SELECT result_date, result_value
FROM lab_results_history
WHERE patient_mrn = 'MRN-888'
  AND test_code = '4548-4'
ORDER BY result_date DESC
LIMIT 10;

-- ============================================================================
-- AGGREGATED STATISTICS (Materialized View pattern)
-- ============================================================================

CREATE TABLE daily_vital_stats (
    patient_mrn text,
    stat_date date,
    vital_type text,  -- HR, BP_SYS, BP_DIA, SPO2, TEMP
    min_value float,
    max_value float,
    avg_value float,
    reading_count int,
    abnormal_count int,
    PRIMARY KEY ((patient_mrn, stat_date), vital_type)
);

-- Insert daily aggregates (would be computed by analytics job)
INSERT INTO daily_vital_stats (patient_mrn, stat_date, vital_type, min_value, max_value, avg_value, reading_count, abnormal_count)
VALUES ('MRN-888', '2024-12-09', 'HR', 65, 95, 78.5, 288, 5);

INSERT INTO daily_vital_stats (patient_mrn, stat_date, vital_type, min_value, max_value, avg_value, reading_count, abnormal_count)
VALUES ('MRN-888', '2024-12-09', 'SPO2', 94.5, 99.0, 97.8, 288, 2);

-- Query daily stats for dashboard
SELECT vital_type, min_value, max_value, avg_value, abnormal_count
FROM daily_vital_stats
WHERE patient_mrn = 'MRN-888'
  AND stat_date = '2024-12-09';

-- ============================================================================
-- VECTOR SEARCH FOR SIMILAR PATIENTS (OrbitRS Extension)
-- ============================================================================

-- Table for patient embeddings (for similar patient cohort analysis)
CREATE TABLE patient_embeddings (
    patient_mrn text PRIMARY KEY,
    embedding vector<float, 256>,  -- Patient feature embedding
    diagnosis_codes list<text>,
    age int,
    gender text,
    primary_diagnosis text,
    updated_at timestamp
);

-- Insert patient embedding
INSERT INTO patient_embeddings (patient_mrn, diagnosis_codes, age, gender, primary_diagnosis, updated_at)
VALUES ('MRN-888', ['E11.9', 'I10', 'E78.5'], 65, 'M', 'Type 2 Diabetes', toTimestamp(now()));

-- ANN query to find similar patients (OrbitRS extension)
-- SELECT patient_mrn, primary_diagnosis
-- FROM patient_embeddings
-- ORDER BY embedding ANN OF [0.1, 0.2, ...]
-- LIMIT 10;
