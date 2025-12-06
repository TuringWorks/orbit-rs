-- End-to-end heavy industries example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: equipment, shifts, sensor_readings, incidents
-- 2) Time-series energy/temperature analytics
-- 3) Downtime risk classification: training and inference
-- 4) Vector embeddings for incident similarity
-- 5) GraphRAG safety policy ingestion and Q&A
CREATE TABLE IF NOT EXISTS equipment (
    equipment_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    type VARCHAR(50)
);
CREATE TABLE IF NOT EXISTS shifts (
    shift_id SERIAL PRIMARY KEY,
    name VARCHAR(50),
    start_ts TIMESTAMP,
    end_ts TIMESTAMP
);
CREATE TABLE IF NOT EXISTS sensor_readings (
    equipment_id INTEGER REFERENCES equipment(equipment_id),
    ts TIMESTAMP NOT NULL,
    energy_kw DOUBLE PRECISION,
    temp_c DOUBLE PRECISION,
    load_pct DOUBLE PRECISION
);
CREATE TABLE IF NOT EXISTS incidents (
    equipment_id INTEGER REFERENCES equipment(equipment_id),
    ts TIMESTAMP NOT NULL,
    severity INTEGER,
    description TEXT
);
INSERT INTO equipment (name, type) VALUES ('Compressor-1','compressor'),('Smelter-2','smelter') ON CONFLICT DO NOTHING;
INSERT INTO shifts (name, start_ts, end_ts) VALUES ('Shift-A', NOW()-INTERVAL '8 hours', NOW()),('Shift-B', NOW()-INTERVAL '16 hours', NOW()-INTERVAL '8 hours') ON CONFLICT DO NOTHING;
INSERT INTO sensor_readings (equipment_id, ts, energy_kw, temp_c, load_pct)
SELECT 1, NOW() - (INTERVAL '5 minutes' * n), 120 + (RANDOM()*40), 70 + (RANDOM()*15), 0.6 + (RANDOM()*0.4)
FROM GENERATE_SERIES(0, 192) AS n;
INSERT INTO sensor_readings (equipment_id, ts, energy_kw, temp_c, load_pct)
SELECT 2, NOW() - (INTERVAL '5 minutes' * n), 200 + (RANDOM()*60), 800 + (RANDOM()*50), 0.7 + (RANDOM()*0.3)
FROM GENERATE_SERIES(0, 192) AS n;
INSERT INTO incidents (equipment_id, ts, severity, description)
SELECT 1, NOW() - INTERVAL '20 minutes', 2, 'Unexpected shutdown'
UNION ALL SELECT 2, NOW() - INTERVAL '50 minutes', 3, 'Overheat warning';
-- Rolling analytics: energy and temperature
SELECT s.equipment_id,
       s.ts,
       s.energy_kw,
       s.temp_c,
       AVG(s.energy_kw) OVER (PARTITION BY s.equipment_id ORDER BY s.ts ROWS BETWEEN 11 PRECEDING AND CURRENT ROW) AS energy_ma_1h,
       AVG(s.temp_c) OVER (PARTITION BY s.equipment_id ORDER BY s.ts ROWS BETWEEN 11 PRECEDING AND CURRENT ROW) AS temp_ma_1h,
       CASE WHEN s.temp_c > AVG(s.temp_c) OVER (PARTITION BY s.equipment_id ORDER BY s.ts ROWS BETWEEN 11 PRECEDING AND CURRENT ROW) + 3 * STDDEV(s.temp_c) OVER (PARTITION BY s.equipment_id ORDER BY s.ts ROWS BETWEEN 11 PRECEDING AND CURRENT ROW)
            THEN 'TEMP_ANOMALY'
            ELSE 'NORMAL' END AS status
FROM sensor_readings s
ORDER BY s.ts DESC
LIMIT 60;
-- Downtime risk features and model training
WITH features AS (
    SELECT equipment_id, ts,
           CASE WHEN temp_c > 85 THEN 1 ELSE 0 END AS high_temp,
           CASE WHEN energy_kw > 180 THEN 1 ELSE 0 END AS high_energy,
           CASE WHEN load_pct > 0.85 THEN 1 ELSE 0 END AS high_load,
           CASE WHEN EXISTS (
              SELECT 1 FROM incidents i WHERE i.equipment_id = sensor_readings.equipment_id AND ABS(EXTRACT(EPOCH FROM (i.ts - sensor_readings.ts))) < 1800 AND i.severity >= 2
           ) THEN 1 ELSE 0 END AS label
    FROM sensor_readings
    WHERE ts > NOW() - INTERVAL '8 hours'
)
SELECT ML_TRAIN_MODEL('heavy_downtime_rf','random_forest', ARRAY[high_temp, high_energy, high_load], label) FROM features;
-- Inference on recent snapshots
WITH recent AS (
    SELECT equipment_id, ts, high_temp, high_energy, high_load
    FROM features
    ORDER BY ts DESC
    LIMIT 200
)
SELECT equipment_id, ts,
       ML_PREDICT('heavy_downtime_rf', ARRAY[high_temp, high_energy, high_load]) AS downtime_risk
FROM recent
ORDER BY downtime_risk DESC;
-- Vector embeddings for incident similarity
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE IF NOT EXISTS incident_types (
    name TEXT PRIMARY KEY,
    embedding vector(384)
);
INSERT INTO incident_types (name, embedding)
VALUES
  ('Unexpected shutdown', ML_EMBED_TEXT('Unexpected shutdown','sentence-transformers')),
  ('Overheat warning', ML_EMBED_TEXT('Overheat warning','sentence-transformers'))
ON CONFLICT DO NOTHING;
SELECT 1 - (
  (SELECT embedding FROM incident_types WHERE name='Unexpected shutdown') <=>
  (SELECT embedding FROM incident_types WHERE name='Overheat warning')
) AS incident_similarity;
-- GraphRAG safety policies
SELECT * FROM GRAPHRAG_BUILD('heavy_kg','policy_1','Equipment with frequent overheat events must reduce load and schedule maintenance within 24 hours.','{"source":"policy","domain":"heavy"}'::json);
SELECT * FROM GRAPHRAG_BUILD('heavy_kg','policy_2','Unexpected shutdowns require root cause analysis before next shift.','{"source":"policy","domain":"heavy"}'::json);
SELECT * FROM GRAPHRAG_QUERY('heavy_kg','What actions apply to overheat events and unexpected shutdowns?',2,2048,'ollama',true);
