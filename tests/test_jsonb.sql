-- Test JSONB operators
CREATE TABLE test_jsonb (
    id INTEGER PRIMARY KEY,
    data JSONB
);

INSERT INTO test_jsonb VALUES (1, '{"name": "test", "value": 42}');

-- Test -> operator
SELECT data->'name' FROM test_jsonb;

-- Test ->> operator
SELECT data->>'name' FROM test_jsonb;

-- Test in function call (the failing case)
SELECT (data->>'value')::INTEGER FROM test_jsonb;

-- Test nested
SELECT data->'nested'->'field' FROM test_jsonb;
