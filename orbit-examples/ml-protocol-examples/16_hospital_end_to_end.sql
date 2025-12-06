-- End-to-end hospital systems example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: patients, vitals, notes
-- 2) Time-series vitals analytics
-- 3) Sepsis risk classification: training and inference
-- 4) Vector embeddings for clinical note similarity
-- 5) GraphRAG care policy ingestion and Q&A
CREATE TABLE IF NOT EXISTS patients (
    patient_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    age INTEGER
);
CREATE TABLE IF NOT EXISTS vitals (
    patient_id INTEGER REFERENCES patients(patient_id),
    ts TIMESTAMP NOT NULL,
    heart_rate INTEGER,
    bp_sys INTEGER,
    bp_dia INTEGER,
    temp_c DOUBLE PRECISION
);
CREATE TABLE IF NOT EXISTS notes (
    note_id SERIAL PRIMARY KEY,
    patient_id INTEGER REFERENCES patients(patient_id),
    ts TIMESTAMP NOT NULL,
    content TEXT
);
-- Seed patients and vitals
INSERT INTO patients (name, age) VALUES ('Alice P','65'),('Bob Q','54') ON CONFLICT DO NOTHING;
INSERT INTO vitals (patient_id, ts, heart_rate, bp_sys, bp_dia, temp_c)
SELECT 1, NOW() - (INTERVAL '5 minutes' * n), 70 + (RANDOM() * 50)::INTEGER, 120 - (RANDOM() * 20)::INTEGER, 80 - (RANDOM() * 10)::INTEGER, 36.5 + (RANDOM() * 2.5)
FROM GENERATE_SERIES(0, 72) AS n;
INSERT INTO vitals (patient_id, ts, heart_rate, bp_sys, bp_dia, temp_c)
SELECT 2, NOW() - (INTERVAL '5 minutes' * n), 65 + (RANDOM() * 45)::INTEGER, 118 - (RANDOM() * 18)::INTEGER, 78 - (RANDOM() * 9)::INTEGER, 36.6 + (RANDOM() * 2.2)
FROM GENERATE_SERIES(0, 72) AS n;
INSERT INTO notes (patient_id, ts, content) VALUES
    (1, NOW() - INTERVAL '30 minutes', 'Patient reports chills and fatigue.'),
    (1, NOW() - INTERVAL '10 minutes', 'Nurse observed elevated temperature.'),
    (2, NOW() - INTERVAL '20 minutes', 'Patient stable, no acute complaints.');
-- Time-series rolling mean for heart rate and temperature; basic alerting
SELECT v.patient_id,
       v.ts,
       v.heart_rate,
       v.temp_c,
       AVG(v.heart_rate) OVER (PARTITION BY v.patient_id ORDER BY v.ts ROWS BETWEEN 5 PRECEDING AND CURRENT ROW) AS hr_ma,
       AVG(v.temp_c) OVER (PARTITION BY v.patient_id ORDER BY v.ts ROWS BETWEEN 5 PRECEDING AND CURRENT ROW) AS temp_ma,
       CASE WHEN v.temp_c > 38.0 AND v.heart_rate > 110 THEN 'FEVER_TACHYCARDIA_ALERT' ELSE 'NORMAL' END AS alert
FROM vitals v
ORDER BY v.ts DESC
LIMIT 40;
-- Feature engineering for sepsis risk (heuristic label for demo)
WITH features AS (
    SELECT patient_id, ts,
           CASE WHEN temp_c > 38.3 THEN 1 ELSE 0 END AS high_temp,
           CASE WHEN heart_rate > 100 THEN 1 ELSE 0 END AS tachy,
           CASE WHEN bp_sys < 100 THEN 1 ELSE 0 END AS hypotension,
           CASE WHEN (temp_c > 38.3 AND heart_rate > 100 AND bp_sys < 100) THEN 1 ELSE 0 END AS label
    FROM vitals
    WHERE ts > NOW() - INTERVAL '6 hours'
)
SELECT ML_TRAIN_MODEL('sepsis_rf','random_forest', ARRAY[high_temp, tachy, hypotension], label) FROM features;
-- Inference on latest vitals snapshots
WITH recent AS (
    SELECT patient_id, ts, high_temp, tachy, hypotension
    FROM features
    ORDER BY ts DESC
    LIMIT 80
)
SELECT patient_id, ts,
       ML_PREDICT('sepsis_rf', ARRAY[high_temp, tachy, hypotension]) AS sepsis_risk
FROM recent
ORDER BY sepsis_risk DESC;
-- Vector embeddings for clinical notes and similarity search
CREATE EXTENSION IF NOT EXISTS vector;
ALTER TABLE IF NOT EXISTS notes ADD COLUMN IF NOT EXISTS embedding vector(384);
UPDATE notes n SET embedding = ML_EMBED_TEXT(n.content,'sentence-transformers') WHERE n.embedding IS NULL;
SELECT n2.content,
       1 - (n2.embedding <=> n1.embedding) AS similarity
FROM notes n1, notes n2
WHERE n1.patient_id = 1 AND n2.note_id <> n1.note_id
ORDER BY similarity DESC
LIMIT 3;
-- GraphRAG: ingest care pathway policies and ask a question
SELECT * FROM GRAPHRAG_BUILD('hospital_kg','policy_1','Patients with signs of sepsis require immediate fluid resuscitation and antibiotics.','{"source":"policy","domain":"hospital"}'::json);
SELECT * FROM GRAPHRAG_BUILD('hospital_kg','policy_2','Elevated temperature with tachycardia should trigger clinician review within 30 minutes.','{"source":"policy","domain":"hospital"}'::json);
SELECT * FROM GRAPHRAG_QUERY('hospital_kg','What care steps apply to sepsis risk alerts?',2,2048,'ollama',true);
