-- End-to-end aerospace example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: aircraft, flights, flight_sensors, maint_events
-- 2) Time-series speed/altitude analytics
-- 3) Maintenance risk classification: training and inference
-- 4) Vector embeddings for fault code similarity
-- 5) GraphRAG procedures ingestion and Q&A
CREATE TABLE IF NOT EXISTS aircraft (
    aircraft_id SERIAL PRIMARY KEY,
    tail VARCHAR(20),
    model VARCHAR(50)
);
CREATE TABLE IF NOT EXISTS flights (
    flight_id SERIAL PRIMARY KEY,
    aircraft_id INTEGER REFERENCES aircraft(aircraft_id),
    route VARCHAR(50),
    status VARCHAR(20)
);
CREATE TABLE IF NOT EXISTS flight_sensors (
    aircraft_id INTEGER REFERENCES aircraft(aircraft_id),
    ts TIMESTAMP NOT NULL,
    altitude_m INTEGER,
    speed_kts INTEGER,
    engine_temp_c DOUBLE PRECISION
);
CREATE TABLE IF NOT EXISTS maint_events (
    aircraft_id INTEGER REFERENCES aircraft(aircraft_id),
    ts TIMESTAMP NOT NULL,
    code VARCHAR(20),
    severity INTEGER
);
INSERT INTO aircraft (tail, model) VALUES ('N123AB','A320'),('N987XY','B737') ON CONFLICT DO NOTHING;
INSERT INTO flights (aircraft_id, route, status) VALUES (1,'SFO-LAX','active'),(2,'SEA-SFO','active') ON CONFLICT DO NOTHING;
INSERT INTO flight_sensors (aircraft_id, ts, altitude_m, speed_kts, engine_temp_c)
SELECT 1, NOW() - (INTERVAL '1 minute' * n), 10000 + (RANDOM()*2000)::INTEGER, 350 + (RANDOM()*50)::INTEGER, 85 + (RANDOM()*25)
FROM GENERATE_SERIES(0, 240) AS n;
INSERT INTO flight_sensors (aircraft_id, ts, altitude_m, speed_kts, engine_temp_c)
SELECT 2, NOW() - (INTERVAL '1 minute' * n), 9000 + (RANDOM()*2500)::INTEGER, 340 + (RANDOM()*60)::INTEGER, 82 + (RANDOM()*28)
FROM GENERATE_SERIES(0, 240) AS n;
INSERT INTO maint_events (aircraft_id, ts, code, severity)
SELECT 1, NOW() - INTERVAL '12 minutes', 'ENG_TEMP', 2
UNION ALL SELECT 1, NOW() - INTERVAL '6 minutes', 'SPD_FLUCT', 1;
-- Rolling analytics on speed and altitude
SELECT fs.aircraft_id,
       fs.ts,
       fs.speed_kts,
       fs.altitude_m,
       AVG(fs.speed_kts) OVER (PARTITION BY fs.aircraft_id ORDER BY fs.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW) AS speed_ma_10m,
       AVG(fs.altitude_m) OVER (PARTITION BY fs.aircraft_id ORDER BY fs.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW) AS alt_ma_10m,
       CASE WHEN fs.engine_temp_c > AVG(fs.engine_temp_c) OVER (PARTITION BY fs.aircraft_id ORDER BY fs.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW) + 3 * STDDEV(fs.engine_temp_c) OVER (PARTITION BY fs.aircraft_id ORDER BY fs.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW)
            THEN 'ENGINE_TEMP_ANOMALY'
            ELSE 'NORMAL' END AS status
FROM flight_sensors fs
ORDER BY fs.ts DESC
LIMIT 60;
-- Maintenance risk classifier features and training
WITH features AS (
    SELECT aircraft_id, ts,
           CASE WHEN engine_temp_c > 100 THEN 1 ELSE 0 END AS high_temp,
           CASE WHEN speed_kts > 380 THEN 1 ELSE 0 END AS high_speed,
           CASE WHEN altitude_m < 9500 THEN 1 ELSE 0 END AS low_alt,
           CASE WHEN EXISTS (
                SELECT 1 FROM maint_events me
                WHERE me.aircraft_id = flight_sensors.aircraft_id AND ABS(EXTRACT(EPOCH FROM (me.ts - flight_sensors.ts))) < 900 AND me.severity >= 2
           ) THEN 1 ELSE 0 END AS label
    FROM flight_sensors
    WHERE ts > NOW() - INTERVAL '4 hours'
)
SELECT ML_TRAIN_MODEL('aero_maint_rf','random_forest', ARRAY[high_temp, high_speed, low_alt], label) FROM features;
-- Inference on recent snapshots
WITH recent AS (
    SELECT aircraft_id, ts, high_temp, high_speed, low_alt
    FROM features
    ORDER BY ts DESC
    LIMIT 120
)
SELECT aircraft_id, ts,
       ML_PREDICT('aero_maint_rf', ARRAY[high_temp, high_speed, low_alt]) AS maint_risk
FROM recent
ORDER BY maint_risk DESC;
-- Vector embeddings for fault code similarity
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE IF NOT EXISTS faults (
    code TEXT PRIMARY KEY,
    description TEXT,
    embedding vector(384)
);
INSERT INTO faults (code, description, embedding)
VALUES
  ('ENG_TEMP','Engine over-temperature condition', ML_EMBED_TEXT('Engine over-temperature condition','sentence-transformers')),
  ('SPD_FLUCT','Speed fluctuation beyond tolerance', ML_EMBED_TEXT('Speed fluctuation beyond tolerance','sentence-transformers'))
ON CONFLICT DO NOTHING;
SELECT 1 - (
  (SELECT embedding FROM faults WHERE code='ENG_TEMP') <=>
  (SELECT embedding FROM faults WHERE code='SPD_FLUCT')
) AS fault_similarity;
-- GraphRAG procedures ingestion and query
SELECT * FROM GRAPHRAG_BUILD('aerospace_kg','proc_1','Engine over-temperature requires thrust reduction and cooling checks upon landing.','{"source":"procedure","domain":"aerospace"}'::json);
SELECT * FROM GRAPHRAG_BUILD('aerospace_kg','proc_2','Persistent speed fluctuations require pitot-static system inspection within 24 hours.','{"source":"procedure","domain":"aerospace"}'::json);
SELECT * FROM GRAPHRAG_QUERY('aerospace_kg','What procedures apply to engine over-temperature and speed fluctuations?',2,2048,'ollama',true);
