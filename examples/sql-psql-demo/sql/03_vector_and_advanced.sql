-- =============================================================================
-- 03_vector_and_advanced.sql
-- Vector Operations and Advanced Features Demo for Orbit-RS PostgreSQL Server
-- =============================================================================

\echo '🧮 Vector Operations and Advanced Features Demo'
\echo '==============================================='
\echo ''

-- Enable vector extension
\echo '1. Enabling pgvector extension...'
CREATE EXTENSION IF NOT EXISTS vector;

\echo '✅ Vector extension enabled!'
\echo ''

-- Create tables with vector embeddings
\echo '2. Creating tables with vector embeddings...'
CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT,
    category TEXT,
    embedding VECTOR(5),  -- 5-dimensional vectors for simplicity
    metadata JSONB,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE user_preferences (
    user_id INTEGER PRIMARY KEY,
    preference_vector VECTOR(3),  -- 3-dimensional preference vector
    interests JSONB,
    updated_at TIMESTAMP DEFAULT NOW()
);

\echo '✅ Vector tables created!'
\echo ''

-- Insert sample documents with embeddings
\echo '3. Inserting documents with vector embeddings...'
INSERT INTO documents (id, title, content, category, embedding, metadata) VALUES 
    (1, 'Introduction to Machine Learning', 
     'Machine learning is a subset of artificial intelligence...', 
     'AI/ML', 
     '[0.8, 0.2, 0.9, 0.1, 0.7]',
     '{"tags": ["ML", "AI", "beginners"], "difficulty": "easy"}'::jsonb),
    (2, 'Deep Learning Neural Networks', 
     'Deep learning uses neural networks with multiple layers...', 
     'AI/ML', 
     '[0.9, 0.1, 0.8, 0.2, 0.6]',
     '{"tags": ["deep-learning", "neural-networks"], "difficulty": "advanced"}'::jsonb),
    (3, 'Vector Databases Explained', 
     'Vector databases are specialized systems for similarity search...', 
     'Database', 
     '[0.3, 0.7, 0.4, 0.8, 0.5]',
     '{"tags": ["vectors", "database", "similarity"], "difficulty": "medium"}'::jsonb),
    (4, 'PostgreSQL Query Optimization', 
     'Learn how to optimize your PostgreSQL queries for better performance...', 
     'Database', 
     '[0.2, 0.8, 0.3, 0.9, 0.4]',
     '{"tags": ["postgresql", "optimization", "performance"], "difficulty": "advanced"}'::jsonb),
    (5, 'Rust Programming Fundamentals', 
     'Rust is a systems programming language focused on safety and performance...', 
     'Programming', 
     '[0.4, 0.5, 0.6, 0.3, 0.8]',
     '{"tags": ["rust", "systems", "programming"], "difficulty": "medium"}'::jsonb);

-- Insert user preferences
INSERT INTO user_preferences (user_id, preference_vector, interests) VALUES 
    (1, '[0.9, 0.1, 0.8]', '{"topics": ["AI", "ML"], "level": "beginner"}'::jsonb),
    (2, '[0.3, 0.8, 0.5]', '{"topics": ["database", "backend"], "level": "advanced"}'::jsonb),
    (3, '[0.6, 0.4, 0.7]', '{"topics": ["programming", "rust"], "level": "intermediate"}'::jsonb);

\echo '✅ Vector data inserted!'
\echo ''

-- Create vector indices for similarity search
\echo '4. Creating vector indices for efficient similarity search...'
-- Note: These may not work with our current implementation, but demonstrate the syntax
-- CREATE INDEX documents_embedding_ivfflat_idx ON documents USING ivfflat (embedding vector_cosine_ops) WITH (lists = 10);
-- CREATE INDEX documents_embedding_hnsw_idx ON documents USING hnsw (embedding vector_cosine_ops) WITH (m = 16, ef_construction = 64);

\echo 'ℹ️  Vector indices would be created in production (syntax shown in comments)'
\echo ''

-- Vector similarity search demonstrations
\echo '5. Vector similarity search demonstrations...'
\echo ''
\echo '5.1 Find similar documents using cosine similarity:'
-- Note: These vector operators may need implementation
-- SELECT 
--     title,
--     category,
--     embedding <=> '[0.8, 0.2, 0.9, 0.1, 0.7]' as cosine_distance,
--     1 - (embedding <=> '[0.8, 0.2, 0.9, 0.1, 0.7]') as cosine_similarity
-- FROM documents
-- ORDER BY embedding <=> '[0.8, 0.2, 0.9, 0.1, 0.7]'
-- LIMIT 3;

\echo 'ℹ️  Vector similarity queries would work with full vector operator implementation'
\echo ''

-- JSON operations
\echo '6. JSON/JSONB operations...'
\echo ''
\echo '6.1 Query documents by JSON metadata:'
-- Note: JSONB operators may need implementation
SELECT 
    title,
    category,
    metadata
FROM documents
WHERE metadata->>'difficulty' = 'advanced';

\echo ''
\echo '6.2 Documents containing specific tags:'
-- SELECT 
--     title,
--     category,
--     metadata->'tags' as tags
-- FROM documents
-- WHERE metadata->'tags' @> '"ML"'::jsonb;

\echo 'ℹ️  Advanced JSONB operations would work with full JSON operator implementation'
\echo ''

-- Transaction demonstrations
\echo '7. Transaction demonstrations...'
\echo ''
\echo '7.1 Begin a transaction:'
BEGIN;

\echo '7.2 Insert data in transaction:'
INSERT INTO documents (id, title, content, category, embedding) VALUES 
    (6, 'Test Document', 'This is a test document for transaction demo', 'Test', '[0.5, 0.5, 0.5, 0.5, 0.5]');

\echo '7.3 Verify data is visible in transaction:'
SELECT COUNT(*) as total_docs FROM documents;

\echo '7.4 Rollback the transaction:'
ROLLBACK;

\echo '7.5 Verify data was rolled back:'
SELECT COUNT(*) as total_docs FROM documents;

\echo ''

-- Advanced SQL features
\echo '8. Advanced SQL features...'
\echo ''
\echo '8.1 Common Table Expressions (CTE):'
WITH category_stats AS (
    SELECT 
        category,
        COUNT(*) as doc_count,
        AVG(LENGTH(content)) as avg_content_length
    FROM documents
    GROUP BY category
)
SELECT 
    category,
    doc_count,
    avg_content_length,
    CASE 
        WHEN doc_count >= 2 THEN 'Popular'
        ELSE 'Niche'
    END as popularity
FROM category_stats
ORDER BY doc_count DESC;

\echo ''
\echo '8.2 Window functions:'
SELECT 
    title,
    category,
    LENGTH(content) as content_length,
    ROW_NUMBER() OVER (PARTITION BY category ORDER BY LENGTH(content) DESC) as length_rank_in_category,
    RANK() OVER (ORDER BY LENGTH(content) DESC) as overall_length_rank
FROM documents
ORDER BY category, length_rank_in_category;

\echo ''

-- Complex queries with multiple features
\echo '9. Complex queries combining multiple features...'
\echo ''
\echo '9.1 Document analysis with user preferences:'
SELECT 
    d.title,
    d.category,
    d.metadata->>'difficulty' as difficulty,
    u.user_id,
    u.interests->>'level' as user_level,
    CASE 
        WHEN d.metadata->>'difficulty' = u.interests->>'level' THEN 'Perfect Match'
        WHEN (d.metadata->>'difficulty' = 'easy' AND u.interests->>'level' = 'beginner') THEN 'Good Match'
        WHEN (d.metadata->>'difficulty' = 'medium' AND u.interests->>'level' = 'intermediate') THEN 'Good Match'
        WHEN (d.metadata->>'difficulty' = 'advanced' AND u.interests->>'level' = 'advanced') THEN 'Good Match'
        ELSE 'Mismatch'
    END as compatibility
FROM documents d
CROSS JOIN user_preferences u
WHERE d.category IN ('AI/ML', 'Database', 'Programming')
ORDER BY d.category, compatibility;

\echo ''

-- Clean up demo
\echo '10. Cleaning up demo data...'
DROP TABLE IF EXISTS documents;
DROP TABLE IF EXISTS user_preferences;

\echo ''
\echo '✅ Vector operations and advanced features demo completed!'
\echo ''
\echo '📝 Summary of demonstrated features:'
\echo '   ✅ Vector data types (VECTOR)'
\echo '   ✅ JSONB data storage and queries'
\echo '   ✅ Transactions (BEGIN/COMMIT/ROLLBACK)'
\echo '   ✅ Common Table Expressions (CTE)'
\echo '   ✅ Window functions'
\echo '   ✅ Complex CASE expressions'
\echo '   ✅ Cross joins with filtering'
\echo '   ⚠️  Vector similarity operators (implementation needed)'
\echo '   ⚠️  Advanced JSONB operators (implementation needed)'
\echo ''