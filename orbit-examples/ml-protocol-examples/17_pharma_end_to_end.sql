-- End-to-end pharma example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: compounds, trials, subjects, dosing, adverse_events
-- 2) AE rate analytics
-- 3) AE risk classification: training and inference
-- 4) Vector embeddings for compound similarity
-- 5) GraphRAG safety policy ingestion and Q&A
CREATE TABLE IF NOT EXISTS compounds (
    compound_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    description TEXT
);
CREATE TABLE IF NOT EXISTS trials (
    trial_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    phase VARCHAR(20)
);
CREATE TABLE IF NOT EXISTS subjects (
    subject_id SERIAL PRIMARY KEY,
    age INTEGER,
    trial_id INTEGER REFERENCES trials(trial_id)
);
CREATE TABLE IF NOT EXISTS dosing (
    subject_id INTEGER REFERENCES subjects(subject_id),
    ts TIMESTAMP NOT NULL,
    dose_mg DOUBLE PRECISION
);
CREATE TABLE IF NOT EXISTS adverse_events (
    subject_id INTEGER REFERENCES subjects(subject_id),
    ts TIMESTAMP NOT NULL,
    severity INTEGER,
    serious BOOLEAN DEFAULT FALSE
);
-- Seed compounds, trial, subjects, dosing, AEs
INSERT INTO compounds (name, description) VALUES
  ('Compound-A','Selective inhibitor with anti-inflammatory profile'),
  ('Compound-B','Broad-spectrum molecule with metabolic modulation') ON CONFLICT DO NOTHING;
INSERT INTO trials (name, phase) VALUES ('Trial-Alpha','Phase II') ON CONFLICT DO NOTHING;
INSERT INTO subjects (age, trial_id) VALUES (45,1),(60,1) ON CONFLICT DO NOTHING;
INSERT INTO dosing (subject_id, ts, dose_mg)
SELECT 1, NOW() - (INTERVAL '1 hour' * n), 50 + (RANDOM() * 50)
FROM GENERATE_SERIES(0, 6) AS n;
INSERT INTO dosing (subject_id, ts, dose_mg)
SELECT 2, NOW() - (INTERVAL '1 hour' * n), 40 + (RANDOM() * 60)
FROM GENERATE_SERIES(0, 6) AS n;
INSERT INTO adverse_events (subject_id, ts, severity, serious)
SELECT 1, NOW() - INTERVAL '2 hours', 2, FALSE
UNION ALL SELECT 1, NOW() - INTERVAL '45 minutes', 3, TRUE
UNION ALL SELECT 2, NOW() - INTERVAL '30 minutes', 1, FALSE;
-- AE rate over last 3 hours per subject
WITH ae_flags AS (
    SELECT subject_id, ts, CASE WHEN serious THEN 2 ELSE 1 END AS ae_weight
    FROM adverse_events
), ae_window AS (
    SELECT subject_id, ts,
           SUM(ae_weight) OVER (PARTITION BY subject_id ORDER BY ts ROWS BETWEEN 2 PRECEDING AND CURRENT ROW) AS ae_score_3h,
           COUNT(*) OVER (PARTITION BY subject_id ORDER BY ts ROWS BETWEEN 2 PRECEDING AND CURRENT ROW) AS ae_count_3h
    FROM ae_flags
)
SELECT subject_id, ts, ae_score_3h, ae_count_3h
FROM ae_window
ORDER BY ts DESC
LIMIT 20;
-- Feature engineering for AE risk classification
WITH features AS (
    SELECT s.subject_id, d.ts,
           CASE WHEN d.dose_mg > 80 THEN 1 ELSE 0 END AS high_dose,
           CASE WHEN s.age >= 60 THEN 1 ELSE 0 END AS elderly,
           COALESCE((SELECT COUNT(*) FROM adverse_events ae WHERE ae.subject_id = s.subject_id AND ae.ts > d.ts - INTERVAL '2 hours'),0) AS recent_ae_count,
           CASE WHEN COALESCE((SELECT COUNT(*) FROM adverse_events ae WHERE ae.subject_id = s.subject_id AND ae.ts > d.ts - INTERVAL '2 hours'),0) > 0 THEN 1 ELSE 0 END AS label
    FROM subjects s
    JOIN dosing d ON d.subject_id = s.subject_id
)
SELECT ML_TRAIN_MODEL('pharma_ae_rf','random_forest', ARRAY[high_dose, elderly, recent_ae_count], label) FROM features;
-- Inference on latest dosing events
WITH recent AS (
    SELECT subject_id, ts, high_dose, elderly, recent_ae_count
    FROM features
    ORDER BY ts DESC
    LIMIT 50
)
SELECT subject_id, ts,
       ML_PREDICT('pharma_ae_rf', ARRAY[high_dose, elderly, recent_ae_count]) AS ae_risk
FROM recent
ORDER BY ae_risk DESC;
-- Vector embeddings for compound similarity
CREATE EXTENSION IF NOT EXISTS vector;
ALTER TABLE IF NOT EXISTS compounds ADD COLUMN IF NOT EXISTS embedding vector(384);
UPDATE compounds c SET embedding = ML_EMBED_TEXT(c.description,'sentence-transformers') WHERE c.embedding IS NULL;
SELECT c2.name, 1 - (c2.embedding <=> c1.embedding) AS similarity
FROM compounds c1, compounds c2
WHERE c1.name = 'Compound-A' AND c2.compound_id <> c1.compound_id
ORDER BY similarity DESC
LIMIT 5;
-- GraphRAG: ingest safety policies and ask a question
SELECT * FROM GRAPHRAG_BUILD('pharma_kg','policy_1','Serious adverse events must be reported within 24 hours to the safety board.','{"source":"policy","domain":"pharma"}'::json);
SELECT * FROM GRAPHRAG_BUILD('pharma_kg','policy_2','High dosing protocols require enhanced monitoring for elderly subjects.','{"source":"policy","domain":"pharma"}'::json);
SELECT * FROM GRAPHRAG_QUERY('pharma_kg','What actions apply to serious adverse events and high dosing in elderly?',2,2048,'ollama',true);
