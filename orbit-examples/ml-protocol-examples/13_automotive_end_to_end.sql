-- End-to-end automotive example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: vehicles, drivers, trips, telemetry
-- 2) Telemetry seeding for two vehicles
-- 3) Time-series anomaly detection on engine temperature
-- 4) Maintenance risk classification: training and inference
-- 5) Vector embeddings for error code similarity
-- 6) GraphRAG knowledge ingestion and Q&A
CREATE TABLE IF NOT EXISTS vehicles (
    vehicle_id SERIAL PRIMARY KEY,
    vin VARCHAR(20),
    model VARCHAR(50),
    year INTEGER
);
CREATE TABLE IF NOT EXISTS drivers (
    driver_id SERIAL PRIMARY KEY,
    name VARCHAR(100)
);
CREATE TABLE IF NOT EXISTS trips (
    trip_id SERIAL PRIMARY KEY,
    vehicle_id INTEGER REFERENCES vehicles(vehicle_id),
    driver_id INTEGER REFERENCES drivers(driver_id),
    start_ts TIMESTAMP,
    end_ts TIMESTAMP,
    distance_km DOUBLE PRECISION
);
CREATE TABLE IF NOT EXISTS telemetry (
    vehicle_id INTEGER REFERENCES vehicles(vehicle_id),
    ts TIMESTAMP NOT NULL,
    speed_kph DOUBLE PRECISION,
    rpm INTEGER,
    engine_temp_c DOUBLE PRECISION,
    oil_pressure_psi DOUBLE PRECISION,
    tire_pressure_psi DOUBLE PRECISION
);
-- Seed vehicles, drivers, trips
INSERT INTO vehicles (vin, model, year) VALUES ('VIN12345','Sedan-X',2022),('VIN67890','SUV-Y',2021) ON CONFLICT DO NOTHING;
INSERT INTO drivers (name) VALUES ('Dana'),('Chris') ON CONFLICT DO NOTHING;
INSERT INTO trips (vehicle_id, driver_id, start_ts, end_ts, distance_km)
SELECT 1,1,NOW() - INTERVAL '2 hours', NOW() - INTERVAL '1 hours', 42.5
UNION ALL SELECT 2,2,NOW() - INTERVAL '90 minutes', NOW() - INTERVAL '20 minutes', 35.2;
-- Seed time-series telemetry for each vehicle
INSERT INTO telemetry (vehicle_id, ts, speed_kph, rpm, engine_temp_c, oil_pressure_psi, tire_pressure_psi)
SELECT 1,
       NOW() - (INTERVAL '1 minute' * n),
       30 + (RANDOM() * 70),
       1500 + (RANDOM() * 4500)::INTEGER,
       85 + (RANDOM() * 30),
       25 + (RANDOM() * 20),
       32 + (RANDOM() * 8)
FROM GENERATE_SERIES(0, 240) AS n;
INSERT INTO telemetry (vehicle_id, ts, speed_kph, rpm, engine_temp_c, oil_pressure_psi, tire_pressure_psi)
SELECT 2,
       NOW() - (INTERVAL '1 minute' * n),
       20 + (RANDOM() * 80),
       1200 + (RANDOM() * 5000)::INTEGER,
       80 + (RANDOM() * 35),
       22 + (RANDOM() * 18),
       30 + (RANDOM() * 10)
FROM GENERATE_SERIES(0, 240) AS n;
-- Rolling-window anomaly detection on engine temperature per vehicle
SELECT t.vehicle_id,
       t.ts,
       t.engine_temp_c,
       AVG(t.engine_temp_c) OVER (
           PARTITION BY t.vehicle_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW
       ) AS temp_ma_10m,
       CASE WHEN t.engine_temp_c > AVG(t.engine_temp_c) OVER (
                    PARTITION BY t.vehicle_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW
                ) + 3 * STDDEV(t.engine_temp_c) OVER (
                    PARTITION BY t.vehicle_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW
                ) THEN 'ANOMALY' ELSE 'NORMAL' END AS temp_status
FROM telemetry t
ORDER BY t.ts DESC
LIMIT 60;
-- Feature engineering for maintenance classification (binary label)
WITH maint_features AS (
    SELECT vehicle_id,
           ts,
           CASE WHEN engine_temp_c > 100 THEN 1 ELSE 0 END AS high_temp,
           CASE WHEN rpm > 5000 THEN 1 ELSE 0 END AS high_rpm,
           CASE WHEN oil_pressure_psi < 20 THEN 1 ELSE 0 END AS low_oil,
           CASE WHEN tire_pressure_psi < 30 THEN 1 ELSE 0 END AS low_tire,
           CASE WHEN (engine_temp_c > 105 OR oil_pressure_psi < 18) THEN 1 ELSE 0 END AS label
    FROM telemetry
    WHERE ts > NOW() - INTERVAL '4 hours'
)
-- Train a random forest on engineered maintenance features
SELECT ML_TRAIN_MODEL('auto_maint_rf','random_forest', ARRAY[high_temp, high_rpm, low_oil, low_tire], label) FROM maint_features;
-- Score recent telemetry snapshots with the model
WITH recent AS (
    SELECT vehicle_id,
           ts,
           high_temp,
           high_rpm,
           low_oil,
           low_tire
    FROM maint_features
    ORDER BY ts DESC
    LIMIT 100
)
SELECT vehicle_id,
       ts,
       ML_PREDICT('auto_maint_rf', ARRAY[high_temp, high_rpm, low_oil, low_tire]) AS maintenance_risk
FROM recent
ORDER BY maintenance_risk DESC;
-- Vector embeddings for automotive error codes and cosine similarity
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE IF NOT EXISTS error_codes (
    code TEXT PRIMARY KEY,
    description TEXT,
    embedding vector(384)
);
INSERT INTO error_codes (code, description, embedding)
VALUES
    ('P0217','Engine Overheating Condition', ML_EMBED_TEXT('Engine Overheating Condition','sentence-transformers')),
    ('P0521','Engine Oil Pressure Sensor Range/Performance', ML_EMBED_TEXT('Engine Oil Pressure Sensor Range/Performance','sentence-transformers'))
ON CONFLICT DO NOTHING;
SELECT 1 - (
    (SELECT embedding FROM error_codes WHERE code = 'P0217') <=>
    (SELECT embedding FROM error_codes WHERE code = 'P0521')
) AS error_similarity;
-- Ingest maintenance policies into GraphRAG KG and ask a question
SELECT * FROM GRAPHRAG_BUILD('automotive_kg','policy_1','Vehicles with engine temperature above 105C should be scheduled for inspection within 24 hours.','{"source":"policy","domain":"automotive"}'::json);
SELECT * FROM GRAPHRAG_BUILD('automotive_kg','policy_2','Persistent low oil pressure requires immediate service and diagnostic testing.','{"source":"policy","domain":"automotive"}'::json);
SELECT * FROM GRAPHRAG_QUERY('automotive_kg','What rules affect high engine temperature and low oil pressure incidents?',2,2048,'ollama',true);
