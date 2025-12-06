-- End-to-end scientific discovery example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: papers, experiments, measurements
-- 2) Time-series measurement analytics
-- 3) Anomaly classification: training and inference
-- 4) Vector embeddings for abstract similarity
-- 5) GraphRAG methodology policy ingestion and Q&A
CREATE TABLE IF NOT EXISTS papers (
    paper_id SERIAL PRIMARY KEY,
    title TEXT,
    abstract TEXT
);
CREATE TABLE IF NOT EXISTS experiments (
    experiment_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    paper_id INTEGER REFERENCES papers(paper_id)
);
CREATE TABLE IF NOT EXISTS measurements (
    experiment_id INTEGER REFERENCES experiments(experiment_id),
    ts TIMESTAMP NOT NULL,
    value DOUBLE PRECISION
);
INSERT INTO papers (title, abstract) VALUES
  ('Quantum Coherence in Novel Materials','We investigate coherence properties and experimental constraints.'),
  ('Catalysis Efficiency in New Compounds','Efficiency of catalysts measured across varying conditions.') ON CONFLICT DO NOTHING;
INSERT INTO experiments (name, paper_id) VALUES ('Exp-Q1',1),('Exp-C1',2) ON CONFLICT DO NOTHING;
INSERT INTO measurements (experiment_id, ts, value)
SELECT 1, NOW() - (INTERVAL '1 minute' * n), 0.8 + (RANDOM()*0.4)
FROM GENERATE_SERIES(0, 120) AS n;
INSERT INTO measurements (experiment_id, ts, value)
SELECT 2, NOW() - (INTERVAL '1 minute' * n), 1.2 + (RANDOM()*0.5)
FROM GENERATE_SERIES(0, 120) AS n;
-- Rolling mean and 3-sigma anomaly detection
SELECT m.experiment_id,
       m.ts,
       m.value,
       AVG(m.value) OVER (PARTITION BY m.experiment_id ORDER BY m.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW) AS val_ma_10m,
       CASE WHEN m.value > AVG(m.value) OVER (PARTITION BY m.experiment_id ORDER BY m.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW) + 3 * STDDEV(m.value) OVER (PARTITION BY m.experiment_id ORDER BY m.ts ROWS BETWEEN 9 PRECEDING AND CURRENT ROW)
            THEN 'ANOMALY' ELSE 'NORMAL' END AS state
FROM measurements m
ORDER BY m.ts DESC
LIMIT 40;
-- Anomaly classification features and training (label from heuristic)
WITH features AS (
    SELECT experiment_id, ts,
           CASE WHEN value > 1.3 THEN 1 ELSE 0 END AS high_value,
           CASE WHEN value < 0.7 THEN 1 ELSE 0 END AS low_value,
           CASE WHEN (value > 1.3 OR value < 0.7) THEN 1 ELSE 0 END AS label
    FROM measurements
    WHERE ts > NOW() - INTERVAL '2 hours'
)
SELECT ML_TRAIN_MODEL('science_anomaly_rf','random_forest', ARRAY[high_value, low_value], label) FROM features;
-- Inference on recent measurements
WITH recent AS (
    SELECT experiment_id, ts, high_value, low_value
    FROM features
    ORDER BY ts DESC
    LIMIT 80
)
SELECT experiment_id, ts,
       ML_PREDICT('science_anomaly_rf', ARRAY[high_value, low_value]) AS anomaly_risk
FROM recent
ORDER BY anomaly_risk DESC;
-- Vector embeddings for abstract similarity
CREATE EXTENSION IF NOT EXISTS vector;
ALTER TABLE IF NOT EXISTS papers ADD COLUMN IF NOT EXISTS embedding vector(384);
UPDATE papers p SET embedding = ML_EMBED_TEXT(p.abstract,'sentence-transformers') WHERE p.embedding IS NULL;
SELECT p2.title, 1 - (p2.embedding <=> p1.embedding) AS similarity
FROM papers p1, papers p2
WHERE p1.title = 'Quantum Coherence in Novel Materials' AND p2.paper_id <> p1.paper_id
ORDER BY similarity DESC
LIMIT 5;
-- GraphRAG methodology policies and query
SELECT * FROM GRAPHRAG_BUILD('science_kg','policy_1','Experiments showing anomalies must include replication steps and instrument calibration logs.','{"source":"policy","domain":"scientific"}'::json);
SELECT * FROM GRAPHRAG_BUILD('science_kg','policy_2','Catalysis efficiency reports require standardized measurement methodology and variance analysis.','{"source":"policy","domain":"scientific"}'::json);
SELECT * FROM GRAPHRAG_QUERY('science_kg','What methodology policies apply to anomaly reports and catalysis efficiency?',2,2048,'ollama',true);
