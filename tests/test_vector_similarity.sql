-- Test vector similarity operators
CREATE TABLE test_vectors (
    id INTEGER PRIMARY KEY,
    embedding vector(3)
);

INSERT INTO test_vectors VALUES 
    (1, '[1.0, 0.0, 0.0]'),
    (2, '[0.0, 1.0, 0.0]'),
    (3, '[1.0, 1.0, 0.0]');

-- Test cosine distance (<=>)
SELECT id, embedding <=> '[1.0, 0.0, 0.0]'::vector(3) AS cosine_dist 
FROM test_vectors 
ORDER BY cosine_dist;

-- Test L2 distance (<->)
SELECT id, embedding <-> '[1.0, 0.0, 0.0]'::vector(3) AS l2_dist 
FROM test_vectors 
ORDER BY l2_dist;

-- Test inner product (<#>)
SELECT id, embedding <#> '[1.0, 0.0, 0.0]'::vector(3) AS inner_prod 
FROM test_vectors 
ORDER BY inner_prod DESC;
