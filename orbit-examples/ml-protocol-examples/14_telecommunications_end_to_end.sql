-- End-to-end telecommunications example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: subscribers, towers, calls, connections
-- 2) Time-series windowed drop rate
-- 3) Drop prediction model training and inference
-- 4) Vector embeddings for issue similarity
-- 5) GraphRAG policy ingestion and Q&A
CREATE TABLE IF NOT EXISTS subscribers (
    subscriber_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    segment VARCHAR(20)
);
CREATE TABLE IF NOT EXISTS towers (
    tower_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION
);
CREATE TABLE IF NOT EXISTS calls (
    call_id SERIAL PRIMARY KEY,
    subscriber_id INTEGER REFERENCES subscribers(subscriber_id),
    ts TIMESTAMP NOT NULL DEFAULT NOW(),
    duration_sec INTEGER,
    dropped BOOLEAN DEFAULT FALSE
);
CREATE TABLE IF NOT EXISTS connections (
    subscriber_id INTEGER REFERENCES subscribers(subscriber_id),
    tower_id INTEGER REFERENCES towers(tower_id),
    ts TIMESTAMP NOT NULL,
    signal_dbm DOUBLE PRECISION
);
-- Seed core entities and sample events
INSERT INTO subscribers (name, segment) VALUES ('Eve','premium'),('Frank','standard') ON CONFLICT DO NOTHING;
INSERT INTO towers (name, latitude, longitude) VALUES ('Tower-1',37.7749,-122.4194),('Tower-2',37.7840,-122.4090) ON CONFLICT DO NOTHING;
INSERT INTO calls (subscriber_id, ts, duration_sec, dropped)
SELECT 1, NOW() - INTERVAL '40 minutes', 180, FALSE
UNION ALL SELECT 1, NOW() - INTERVAL '15 minutes', 35, TRUE
UNION ALL SELECT 2, NOW() - INTERVAL '5 minutes', 120, FALSE;
INSERT INTO connections (subscriber_id, tower_id, ts, signal_dbm)
SELECT 1,1, NOW() - (INTERVAL '1 minute' * n), -80 + (RANDOM() * 15)
FROM GENERATE_SERIES(0, 120) AS n;
INSERT INTO connections (subscriber_id, tower_id, ts, signal_dbm)
SELECT 2,2, NOW() - (INTERVAL '1 minute' * n), -85 + (RANDOM() * 20)
FROM GENERATE_SERIES(0, 120) AS n;
-- Rolling drop rate over last 60 minutes per subscriber
WITH call_flags AS (
    SELECT subscriber_id, ts,
           CASE WHEN dropped THEN 1 ELSE 0 END AS dropped_flag
    FROM calls
), drop_window AS (
    SELECT subscriber_id, ts,
           SUM(dropped_flag) OVER (
               PARTITION BY subscriber_id ORDER BY ts ROWS BETWEEN 59 PRECEDING AND CURRENT ROW
           ) AS drops_60m,
           COUNT(*) OVER (
               PARTITION BY subscriber_id ORDER BY ts ROWS BETWEEN 59 PRECEDING AND CURRENT ROW
           ) AS calls_60m
    FROM call_flags
)
SELECT subscriber_id, ts, drops_60m, calls_60m,
       CASE WHEN calls_60m > 0 THEN drops_60m::DOUBLE PRECISION / calls_60m ELSE 0 END AS drop_rate
FROM drop_window
ORDER BY ts DESC
LIMIT 20;
-- Feature engineering for dropped call prediction
WITH latest_conn AS (
    SELECT c.subscriber_id, c.ts,
           c.signal_dbm,
           EXTRACT(HOUR FROM c.ts) AS hour,
           CASE WHEN c.signal_dbm < -90 THEN 1 ELSE 0 END AS very_low_signal
    FROM connections c
), call_labels AS (
    SELECT call_id, subscriber_id, ts, CASE WHEN dropped THEN 1 ELSE 0 END AS label
    FROM calls
)
SELECT ML_TRAIN_MODEL(
    'tel_drop_rf',
    'random_forest',
    ARRAY[very_low_signal, hour, signal_dbm],
    label
) FROM (
    SELECT l.subscriber_id, l.ts, l.signal_dbm, l.hour, l.very_low_signal, cl.label
    FROM latest_conn l
    JOIN call_labels cl ON cl.subscriber_id = l.subscriber_id AND ABS(EXTRACT(EPOCH FROM (cl.ts - l.ts))) < 3600
) AS train_data;
-- Inference: predict drop likelihood on recent connections
WITH recent AS (
    SELECT subscriber_id, ts, very_low_signal, hour, signal_dbm
    FROM latest_conn
    ORDER BY ts DESC
    LIMIT 50
)
SELECT subscriber_id, ts,
       ML_PREDICT('tel_drop_rf', ARRAY[very_low_signal, hour, signal_dbm]) AS drop_likelihood
FROM recent
ORDER BY drop_likelihood DESC;
-- Vector embeddings: similarity between issue descriptions
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE IF NOT EXISTS network_issues (
    issue TEXT PRIMARY KEY,
    embedding vector(384)
);
INSERT INTO network_issues (issue, embedding)
VALUES
    ('Network congestion at peak hours', ML_EMBED_TEXT('Network congestion at peak hours', 'sentence-transformers')),
    ('Hardware fault causing signal instability', ML_EMBED_TEXT('Hardware fault causing signal instability', 'sentence-transformers'))
ON CONFLICT DO NOTHING;
SELECT 1 - (
    (SELECT embedding FROM network_issues WHERE issue = 'Network congestion at peak hours') <=>
    (SELECT embedding FROM network_issues WHERE issue = 'Hardware fault causing signal instability')
) AS issue_similarity;
-- GraphRAG: ingest telecom policies and query for reasoning
SELECT * FROM GRAPHRAG_BUILD(
    'telecom_kg',
    'policy_1',
    'Subscribers with drop rate above 3% in the last hour should receive proactive outreach.',
    '{"source":"policy","domain":"telecommunications"}'::json
);
SELECT * FROM GRAPHRAG_BUILD(
    'telecom_kg',
    'policy_2',
    'Low signal areas must be prioritized in tower optimization within 7 days.',
    '{"source":"policy","domain":"telecommunications"}'::json
);
SELECT * FROM GRAPHRAG_QUERY(
    'telecom_kg',
    'What actions are recommended for high drop rates and low signal areas?',
    2,
    2048,
    'ollama',
    true
);
