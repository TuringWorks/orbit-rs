CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS search_queries (
    query_id SERIAL PRIMARY KEY,
    query TEXT,
    embedding vector(384)
);

CREATE TABLE IF NOT EXISTS documents_index (
    doc_id SERIAL PRIMARY KEY,
    title TEXT,
    content TEXT,
    clicks INTEGER DEFAULT 0,
    embedding vector(384)
);

INSERT INTO documents_index (title, content, clicks, embedding)
VALUES
    ('Neural Networks', 'An introduction to deep learning and neural networks.', 120,
     ML_EMBED_TEXT('An introduction to deep learning and neural networks.', 'sentence-transformers')),
    ('Databases', 'Relational and NoSQL databases explained.', 80,
     ML_EMBED_TEXT('Relational and NoSQL databases explained.', 'sentence-transformers')),
    ('Vector Search', 'Approximate nearest neighbor search using HNSW.', 200,
     ML_EMBED_TEXT('Approximate nearest neighbor search using HNSW.', 'sentence-transformers'));

INSERT INTO search_queries (query, embedding)
VALUES ('deep learning basics', ML_EMBED_TEXT('deep learning basics', 'sentence-transformers'));

CREATE INDEX IF NOT EXISTS documents_index_embedding_idx 
ON documents_index USING hnsw (embedding vector_cosine_ops);

SELECT 
    di.doc_id,
    di.title,
    di.clicks,
    1 - (di.embedding <=> sq.embedding) AS similarity
FROM documents_index di
CROSS JOIN search_queries sq
ORDER BY di.embedding <=> sq.embedding
LIMIT 5;

SELECT ML_TRAIN_MODEL(
    'click_model_gbm',
    'gradient_boosting',
    ARRAY[
        1 - (di.embedding <=> sq.embedding)
    ],
    (di.clicks > 100)
) FROM documents_index di, search_queries sq;

UPDATE documents_index di
SET clicks = clicks + CASE WHEN ML_PREDICT(
    'click_model_gbm',
    ARRAY[1 - (di.embedding <=> sq.embedding)]
) > 0.5 THEN 1 ELSE 0 END
FROM search_queries sq;
