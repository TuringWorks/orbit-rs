-- ML SQL Functions: consolidated examples
-- Demonstrates training, prediction, evaluation, feature engineering,
-- vector ops, and time series ML functions.

-- Setup small demo tables
CREATE TABLE IF NOT EXISTS demo_points (
    id SERIAL PRIMARY KEY,
    x DOUBLE PRECISION,
    y DOUBLE PRECISION,
    label INTEGER
);
INSERT INTO demo_points (x, y, label)
SELECT RANDOM(), RANDOM(), (RANDOM()*1)::INTEGER FROM GENERATE_SERIES(1,50);

CREATE TABLE IF NOT EXISTS demo_texts (
    id SERIAL PRIMARY KEY,
    content TEXT,
    category TEXT
);
INSERT INTO demo_texts (content, category)
VALUES
    ('Machine learning enables systems to learn from data.','ml'),
    ('Vector databases enable fast similarity search.','database'),
    ('Normalization rescales features to a common range.','ml')
ON CONFLICT DO NOTHING;

-- 1) Train/Predict/Evaluate
WITH feats AS (
    SELECT x, y, label FROM demo_points
)
SELECT ML_TRAIN_MODEL('demo_rf','random_forest', ARRAY[x,y], label) FROM feats;

WITH recent AS (
    SELECT x, y FROM demo_points ORDER BY id DESC LIMIT 5
)
SELECT id,
       ML_PREDICT('demo_rf', ARRAY[x,y]) AS score
FROM demo_points
ORDER BY id DESC
LIMIT 5;

WITH test AS (
    SELECT x, y, label FROM demo_points WHERE id <= 25
)
SELECT ML_EVALUATE_MODEL('demo_rf', ARRAY[x,y], label) FROM test;

-- 2) Feature Engineering: normalize, PCA, encode categorical
WITH raw AS (
    SELECT id, ARRAY[x,y] AS features FROM demo_points
)
SELECT id, ML_NORMALIZE(features, 'minmax') AS norm_features FROM raw;

WITH emb AS (
    SELECT id, ML_EMBED_TEXT(content, 'sentence-transformers') AS emb FROM demo_texts
)
SELECT id, ML_PCA(emb, 2) AS pca_2d FROM emb;

SELECT id, ML_ENCODE_CATEGORICAL(category, 'onehot') AS cat_onehot FROM demo_texts;

-- 3) Unsupervised: k-means, vector cluster, dimensionality reduction
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE IF NOT EXISTS demo_vectors (
    id SERIAL PRIMARY KEY,
    emb vector(384)
);
INSERT INTO demo_vectors (emb)
SELECT ML_EMBED_TEXT(content,'sentence-transformers') FROM demo_texts;

SELECT id, ML_KMEANS(emb::real[], 2) OVER () AS cluster FROM demo_vectors;
SELECT id, ML_VECTOR_CLUSTER(emb, 3) AS cluster_id FROM demo_vectors;
SELECT id, ML_DIMENSIONALITY_REDUCTION(emb, 'tsne', 2) AS coords_2d FROM demo_vectors;

-- 4) Time Series: forecast and anomaly detect
CREATE TABLE IF NOT EXISTS demo_series (
    ts TIMESTAMP NOT NULL,
    value DOUBLE PRECISION
);
INSERT INTO demo_series (ts, value)
SELECT NOW() - (INTERVAL '1 minute' * n), 10 + (RANDOM()*5)
FROM GENERATE_SERIES(0, 120) AS n;

SELECT ts,
       value,
       ML_FORECAST(value OVER (ORDER BY ts ROWS 30 PRECEDING), 10) AS forecast
FROM demo_series
ORDER BY ts DESC
LIMIT 20;

SELECT ts,
       value,
       ML_ANOMALY_DETECT(value OVER (ORDER BY ts ROWS 50 PRECEDING)) AS is_anomaly
FROM demo_series
ORDER BY ts DESC
LIMIT 20;
