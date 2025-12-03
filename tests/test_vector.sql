-- Test vector type parsing
CREATE TABLE test_vectors (
    id INTEGER PRIMARY KEY,
    embedding vector(512),
    small_vec vector(128)
);

-- Test vector literal (simplified - no type cast in INSERT)
INSERT INTO test_vectors (id, embedding, small_vec) VALUES (1, '[1.0, 2.0, 3.0]', '[0.5, 0.5]');

-- Test vector operations
SELECT embedding <=> '[1.0, 2.0, 3.0]' AS distance FROM test_vectors;

-- Test type casting separately
SELECT '[1.0, 2.0, 3.0]'::vector(3) AS vec;
