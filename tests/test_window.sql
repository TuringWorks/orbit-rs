-- Test window functions
CREATE TABLE test_window (
    id INTEGER,
    category TEXT,
    value INTEGER
);

INSERT INTO test_window VALUES
    (1, 'A', 10),
    (2, 'A', 20),
    (3, 'B', 15),
    (4, 'B', 25);

-- Test simple window function
SELECT 
    id,
    category,
    value,
    AVG(value) OVER (PARTITION BY category) AS avg_by_category
FROM test_window;

-- Test with ORDER BY
SELECT 
    id,
    category,
    value,
    AVG(value) OVER (
        PARTITION BY category 
        ORDER BY id
    ) AS running_avg
FROM test_window;

-- Test with window frame
SELECT 
    id,
    category,
    value,
    AVG(value) OVER (
        PARTITION BY category 
        ORDER BY id
        ROWS BETWEEN 1 PRECEDING AND CURRENT ROW
    ) AS moving_avg
FROM test_window;
