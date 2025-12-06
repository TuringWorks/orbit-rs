-- End-to-end defense example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: assets, missions, telemetry, alerts
-- 2) Rolling anomaly detection
-- 3) Threat/risk classification: training and inference
-- 4) Vector embeddings for threat similarity
-- 5) GraphRAG response policy ingestion and Q&A
CREATE TABLE IF NOT EXISTS assets (
    asset_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    asset_type VARCHAR(50)
);
CREATE TABLE IF NOT EXISTS missions (
    mission_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    status VARCHAR(20)
);
CREATE TABLE IF NOT EXISTS telemetry (
    asset_id INTEGER REFERENCES assets(asset_id),
    ts TIMESTAMP NOT NULL,
    temp_c DOUBLE PRECISION,
    vibration DOUBLE PRECISION,
    signal_quality DOUBLE PRECISION
);
CREATE TABLE IF NOT EXISTS alerts (
    asset_id INTEGER REFERENCES assets(asset_id),
    ts TIMESTAMP NOT NULL,
    severity INTEGER,
    description TEXT
);
INSERT INTO assets (name, asset_type) VALUES ('Drone-Alpha','UAV'),('Radar-1','Sensor') ON CONFLICT DO NOTHING;
INSERT INTO missions (name, status) VALUES ('Recon-Op','active') ON CONFLICT DO NOTHING;
INSERT INTO telemetry (asset_id, ts, temp_c, vibration, signal_quality)
SELECT 1, NOW() - (INTERVAL '1 minute' * n), 60 + (RANDOM()*20), 0.5 + (RANDOM()*1.0), 0.6 + (RANDOM()*0.4)
FROM GENERATE_SERIES(0, 240) AS n;
INSERT INTO telemetry (asset_id, ts, temp_c, vibration, signal_quality)
SELECT 2, NOW() - (INTERVAL '1 minute' * n), 45 + (RANDOM()*15), 0.3 + (RANDOM()*0.7), 0.7 + (RANDOM()*0.3)
FROM GENERATE_SERIES(0, 240) AS n;
INSERT INTO alerts (asset_id, ts, severity, description)
SELECT 1, NOW() - INTERVAL '12 minutes', 2, 'High vibration detected'
UNION ALL SELECT 1, NOW() - INTERVAL '5 minutes', 3, 'Signal degradation observed';
-- Rolling anomaly detection on vibration and temperature
SELECT t.asset_id,
       t.ts,
       t.vibration,
       t.temp_c,
       AVG(t.vibration) OVER (PARTITION BY t.asset_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW) AS vib_ma_10m,
       AVG(t.temp_c) OVER (PARTITION BY t.asset_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW) AS temp_ma_10m,
       CASE WHEN t.vibration > AVG(t.vibration) OVER (PARTITION BY t.asset_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW)
                         + 3 * STDDEV(t.vibration) OVER (PARTITION BY t.asset_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW)
            THEN 'VIB_ANOMALY'
            WHEN t.temp_c   > AVG(t.temp_c)   OVER (PARTITION BY t.asset_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW)
                         + 3 * STDDEV(t.temp_c)   OVER (PARTITION BY t.asset_id ORDER BY t.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW)
            THEN 'TEMP_ANOMALY'
            ELSE 'NORMAL' END AS status
FROM telemetry t
ORDER BY t.ts DESC
LIMIT 60;
-- Feature engineering for threat/risk classification
WITH features AS (
    SELECT asset_id, ts,
           CASE WHEN temp_c > 75 THEN 1 ELSE 0 END AS high_temp,
           CASE WHEN vibration > 1.2 THEN 1 ELSE 0 END AS high_vibration,
           CASE WHEN signal_quality < 0.5 THEN 1 ELSE 0 END AS low_signal,
           CASE WHEN EXISTS (
                    SELECT 1 FROM alerts a
                    WHERE a.asset_id = telemetry.asset_id AND ABS(EXTRACT(EPOCH FROM (a.ts - telemetry.ts))) < 900 AND a.severity >= 2
                ) THEN 1 ELSE 0 END AS label
    FROM telemetry
    WHERE ts > NOW() - INTERVAL '4 hours'
)
SELECT ML_TRAIN_MODEL('defense_risk_rf','random_forest', ARRAY[high_temp, high_vibration, low_signal], label) FROM features;
-- Inference on recent telemetry
WITH recent AS (
    SELECT asset_id, ts, high_temp, high_vibration, low_signal
    FROM features
    ORDER BY ts DESC
    LIMIT 120
)
SELECT asset_id, ts,
       ML_PREDICT('defense_risk_rf', ARRAY[high_temp, high_vibration, low_signal]) AS risk_score
FROM recent
ORDER BY risk_score DESC;
-- Vector embeddings for threat descriptions
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE IF NOT EXISTS threats (
    code TEXT PRIMARY KEY,
    description TEXT,
    embedding vector(384)
);
INSERT INTO threats (code, description, embedding)
VALUES
  ('THR_VIB','High vibration pattern indicative of mechanical stress', ML_EMBED_TEXT('High vibration pattern indicative of mechanical stress','sentence-transformers')),
  ('THR_SIG','Signal degradation likely due to interference', ML_EMBED_TEXT('Signal degradation likely due to interference','sentence-transformers'))
ON CONFLICT DO NOTHING;
SELECT 1 - (
  (SELECT embedding FROM threats WHERE code='THR_VIB') <=>
  (SELECT embedding FROM threats WHERE code='THR_SIG')
) AS threat_similarity;
-- GraphRAG response policies
SELECT * FROM GRAPHRAG_BUILD('defense_kg','policy_1','Assets with sustained high vibration must be grounded for inspection within 2 hours.','{"source":"policy","domain":"defense"}'::json);
SELECT * FROM GRAPHRAG_BUILD('defense_kg','policy_2','Signal interference incidents require spectrum analysis and mission reassessment.','{"source":"policy","domain":"defense"}'::json);
SELECT * FROM GRAPHRAG_QUERY('defense_kg','What response policies apply to vibration anomalies and signal degradation?',2,2048,'ollama',true);
