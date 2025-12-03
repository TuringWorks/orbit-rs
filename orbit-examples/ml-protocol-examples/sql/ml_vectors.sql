-- ============================================================================
-- ML Vector Operations for Orbit PostgreSQL Protocol
-- ============================================================================
-- This file demonstrates vector operations combined with ML capabilities.
-- Uses pgvector-compatible syntax with Orbit's ML functions.
-- ============================================================================

-- ----------------------------------------------------------------------------
-- Setup: Create tables with vector columns
-- ----------------------------------------------------------------------------

-- Enable vector extension (pgvector compatible)
CREATE EXTENSION IF NOT EXISTS vector;

-- Documents table with embeddings
CREATE TABLE IF NOT EXISTS documents (
    id SERIAL PRIMARY KEY,
    title TEXT,
    content TEXT,
    category VARCHAR(50),
    embedding vector(384)  -- Sentence-transformers dimension
);

-- Products table with image and text embeddings
CREATE TABLE IF NOT EXISTS products (
    id SERIAL PRIMARY KEY,
    name TEXT,
    description TEXT,
    image_url TEXT,
    text_embedding vector(384),
    image_embedding vector(512)
);

-- ----------------------------------------------------------------------------
-- Example 1: Generate embeddings using ML functions
-- ----------------------------------------------------------------------------

-- Insert documents with ML-generated embeddings
INSERT INTO documents (title, content, category, embedding)
VALUES
    ('Introduction to Machine Learning',
     'Machine learning is a subset of artificial intelligence that enables systems to learn from data.',
     'ml',
     ML_EMBED_TEXT('Machine learning is a subset of artificial intelligence that enables systems to learn from data.', 'sentence-transformers')),

    ('Deep Learning Fundamentals',
     'Deep learning uses neural networks with multiple layers to model complex patterns in data.',
     'ml',
     ML_EMBED_TEXT('Deep learning uses neural networks with multiple layers to model complex patterns in data.', 'sentence-transformers')),

    ('Database Optimization Techniques',
     'Learn how to optimize database queries using indexes, query plans, and caching strategies.',
     'database',
     ML_EMBED_TEXT('Learn how to optimize database queries using indexes, query plans, and caching strategies.', 'sentence-transformers')),

    ('Vector Similarity Search',
     'Vector databases enable fast similarity search across millions of high-dimensional vectors.',
     'database',
     ML_EMBED_TEXT('Vector databases enable fast similarity search across millions of high-dimensional vectors.', 'sentence-transformers')),

    ('Natural Language Processing',
     'NLP techniques enable computers to understand, interpret, and generate human language.',
     'ml',
     ML_EMBED_TEXT('NLP techniques enable computers to understand, interpret, and generate human language.', 'sentence-transformers'));

-- ----------------------------------------------------------------------------
-- Example 2: Semantic search with L2 distance (<->)
-- ----------------------------------------------------------------------------

-- Find documents similar to a query
SELECT
    title,
    content,
    embedding <-> ML_EMBED_TEXT('how to build neural networks', 'sentence-transformers') AS distance
FROM documents
ORDER BY distance
LIMIT 5;

-- ----------------------------------------------------------------------------
-- Example 3: Semantic search with cosine distance (<=>)
-- ----------------------------------------------------------------------------

-- Cosine similarity (better for normalized vectors)
SELECT
    title,
    category,
    1 - (embedding <=> ML_EMBED_TEXT('database performance', 'sentence-transformers')) AS similarity
FROM documents
ORDER BY similarity DESC
LIMIT 3;

-- ----------------------------------------------------------------------------
-- Example 4: Inner product similarity (<#>)
-- ----------------------------------------------------------------------------

SELECT
    title,
    embedding <#> ML_EMBED_TEXT('artificial intelligence applications', 'sentence-transformers') AS score
FROM documents
ORDER BY score DESC
LIMIT 3;

-- ----------------------------------------------------------------------------
-- Example 5: Create HNSW index for fast vector search
-- ----------------------------------------------------------------------------

-- Create HNSW index with cosine distance
CREATE INDEX ON documents USING hnsw (embedding vector_cosine_ops);

-- Query will now use the index
EXPLAIN SELECT title
FROM documents
ORDER BY embedding <=> ML_EMBED_TEXT('machine learning', 'sentence-transformers')
LIMIT 10;

-- ----------------------------------------------------------------------------
-- Example 6: Filtered vector search
-- ----------------------------------------------------------------------------

-- Semantic search within a category
SELECT
    title,
    content,
    embedding <=> ML_EMBED_TEXT('learning algorithms', 'sentence-transformers') AS distance
FROM documents
WHERE category = 'ml'
ORDER BY distance
LIMIT 3;

-- ----------------------------------------------------------------------------
-- Example 7: Hybrid search (combining text and vector)
-- ----------------------------------------------------------------------------

SELECT
    title,
    content,
    embedding <=> ML_EMBED_TEXT('database queries', 'sentence-transformers') AS vector_score
FROM documents
WHERE content ILIKE '%database%'
ORDER BY vector_score
LIMIT 5;

-- ----------------------------------------------------------------------------
-- Example 8: RAG (Retrieval Augmented Generation) workflow
-- ----------------------------------------------------------------------------

-- Step 1: Create a RAG documents table
CREATE TABLE IF NOT EXISTS rag_documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    metadata JSONB,
    embedding vector(384)
);

-- Step 2: Insert documents with embeddings
INSERT INTO rag_documents (content, metadata, embedding)
SELECT
    content,
    jsonb_build_object('source', 'manual', 'created_at', NOW()),
    ML_EMBED_TEXT(content, 'sentence-transformers')
FROM (VALUES
    ('Orbit supports multiple database protocols including PostgreSQL, MySQL, Redis, and Cassandra.'),
    ('ML functions in Orbit allow you to train models directly on your data without ETL.'),
    ('Vector similarity search uses HNSW indexes for fast approximate nearest neighbor queries.'),
    ('The virtual actor model in Orbit enables distributed processing across cluster nodes.')
) AS docs(content);

-- Step 3: Retrieve relevant context for a question
SELECT
    content,
    embedding <=> ML_EMBED_TEXT('How do I perform machine learning in Orbit?', 'sentence-transformers') AS relevance
FROM rag_documents
ORDER BY relevance
LIMIT 3;

-- ----------------------------------------------------------------------------
-- Example 9: Vector clustering
-- ----------------------------------------------------------------------------

-- Cluster documents by their embeddings
SELECT
    id,
    title,
    ML_KMEANS(embedding::real[], 2) OVER () AS cluster
FROM documents;

-- Alternative: Cluster with explicit call
SELECT
    title,
    ML_VECTOR_CLUSTER(embedding, 3) AS cluster_id
FROM documents;

-- ----------------------------------------------------------------------------
-- Example 10: Dimensionality reduction
-- ----------------------------------------------------------------------------

-- Reduce embedding dimensions for visualization
SELECT
    title,
    ML_DIMENSIONALITY_REDUCTION(embedding, 'tsne', 2) AS coordinates_2d
FROM documents;

-- Using PCA
SELECT
    title,
    ML_PCA(embedding::real[], 3) AS pca_components
FROM documents;

-- ----------------------------------------------------------------------------
-- Example 11: Multi-modal search (text + image)
-- ----------------------------------------------------------------------------

-- Insert products with both text and image embeddings
INSERT INTO products (name, description, image_url, text_embedding, image_embedding)
VALUES
    ('Laptop Pro',
     'High-performance laptop for professionals',
     'https://example.com/laptop.jpg',
     ML_EMBED_TEXT('High-performance laptop for professionals', 'sentence-transformers'),
     ML_EMBED_IMAGE('https://example.com/laptop.jpg', 'clip'));

-- Search by text OR image similarity
SELECT
    name,
    description,
    LEAST(
        text_embedding <=> ML_EMBED_TEXT('powerful computer', 'sentence-transformers'),
        image_embedding <=> ML_EMBED_IMAGE('https://example.com/query.jpg', 'clip')
    ) AS combined_score
FROM products
ORDER BY combined_score
LIMIT 5;

-- ----------------------------------------------------------------------------
-- Example 12: Reranking with ML model
-- ----------------------------------------------------------------------------

-- Two-stage retrieval: vector search + ML reranking
WITH initial_results AS (
    SELECT
        id,
        title,
        content,
        embedding <=> ML_EMBED_TEXT('machine learning tutorial', 'sentence-transformers') AS vector_score
    FROM documents
    ORDER BY vector_score
    LIMIT 20
)
SELECT
    title,
    content,
    ML_RERANK('cross-encoder', content, 'machine learning tutorial') AS rerank_score
FROM initial_results
ORDER BY rerank_score DESC
LIMIT 5;

-- ----------------------------------------------------------------------------
-- Example 13: Batch embedding generation
-- ----------------------------------------------------------------------------

-- Update existing documents with embeddings
UPDATE documents
SET embedding = ML_EMBED_TEXT(content, 'sentence-transformers')
WHERE embedding IS NULL;

-- ----------------------------------------------------------------------------
-- Example 14: Vector statistics
-- ----------------------------------------------------------------------------

-- Analyze embedding distribution
SELECT
    category,
    COUNT(*) AS doc_count,
    AVG(embedding <=> (SELECT AVG(embedding) FROM documents)) AS avg_distance_from_centroid
FROM documents
GROUP BY category;

-- ----------------------------------------------------------------------------
-- Cleanup
-- ----------------------------------------------------------------------------
-- DROP INDEX IF EXISTS documents_embedding_idx;
-- DROP TABLE IF EXISTS documents;
-- DROP TABLE IF EXISTS products;
-- DROP TABLE IF EXISTS rag_documents;
