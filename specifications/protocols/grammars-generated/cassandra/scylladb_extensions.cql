-- =============================================================================
-- ScyllaDB CQL Extensions Examples
-- Features specific to ScyllaDB that extend standard CQL
-- =============================================================================

-- =============================================================================
-- Keyspace with S3 Storage (ScyllaDB Object Storage)
-- =============================================================================

-- Create keyspace with S3 storage backend
-- Note: Requires 'keyspace-storage-options' experimental feature enabled
CREATE KEYSPACE archived_data
WITH REPLICATION = {'class': 'NetworkTopologyStrategy', 'datacenter1': 3}
AND STORAGE = {
    'type': 'S3',
    'bucket': 'my-scylla-bucket',
    'endpoint': 's3.us-east-1.amazonaws.com'
};

-- Query storage options from system table
-- SELECT * FROM system_schema.scylla_keyspaces;

USE archived_data;

-- =============================================================================
-- BYPASS CACHE Clause
-- =============================================================================

-- Create a large table for range scans
CREATE TABLE logs (
    partition_date date,
    log_id timeuuid,
    level text,
    message text,
    metadata map<text, text>,
    PRIMARY KEY ((partition_date), log_id)
) WITH CLUSTERING ORDER BY (log_id DESC);

-- Regular select (uses cache)
SELECT * FROM logs WHERE partition_date = '2024-01-15' LIMIT 100;

-- Select with BYPASS CACHE - useful for range scans that won't benefit from caching
SELECT * FROM logs 
WHERE partition_date = '2024-01-15' 
ALLOW FILTERING 
BYPASS CACHE;

-- Combined with other clauses
SELECT log_id, level, message FROM logs
WHERE partition_date >= '2024-01-01' AND partition_date <= '2024-01-31'
AND level = 'ERROR'
ALLOW FILTERING
BYPASS CACHE;

-- =============================================================================
-- USING TIMEOUT Extension
-- =============================================================================

-- Per-query timeout for SELECT
SELECT * FROM logs USING TIMEOUT 200ms;

SELECT * FROM logs 
WHERE partition_date = '2024-01-15' 
USING TIMEOUT 500ms;

-- Per-query timeout for INSERT
INSERT INTO logs (partition_date, log_id, level, message)
VALUES ('2024-01-15', now(), 'INFO', 'Application started')
USING TIMEOUT 100ms;

-- Combining TIMEOUT with other USING options
INSERT INTO logs (partition_date, log_id, level, message, metadata)
VALUES ('2024-01-15', now(), 'DEBUG', 'Processing request', {'request_id': '12345'})
USING TIMESTAMP 1234567890123 AND TIMEOUT 50ms;

INSERT INTO logs (partition_date, log_id, level, message)
VALUES ('2024-01-15', now(), 'WARNING', 'High memory usage')
USING TTL 86400 AND TIMEOUT 100ms;

INSERT INTO logs (partition_date, log_id, level, message)
VALUES ('2024-01-15', now(), 'ERROR', 'Connection failed')
USING TIMEOUT 1s AND TTL 604800;

-- Per-query timeout for UPDATE
UPDATE logs USING TIMEOUT 200ms
SET message = 'Updated message'
WHERE partition_date = '2024-01-15' AND log_id = now();

-- Per-query timeout for DELETE
DELETE FROM logs USING TIMEOUT 150ms
WHERE partition_date = '2024-01-14';

-- Per-query timeout for TRUNCATE
TRUNCATE TABLE logs USING TIMEOUT 5m;

-- Per-query timeout in BATCH
BEGIN BATCH USING TIMEOUT 500ms
    INSERT INTO logs (partition_date, log_id, level, message) VALUES ('2024-01-16', now(), 'INFO', 'Batch 1');
    INSERT INTO logs (partition_date, log_id, level, message) VALUES ('2024-01-16', now(), 'INFO', 'Batch 2');
APPLY BATCH;

-- =============================================================================
-- Paxos Grace Seconds Table Option
-- =============================================================================

-- Create table with custom paxos_grace_seconds for LWT
CREATE TABLE distributed_locks (
    lock_name text PRIMARY KEY,
    owner text,
    acquired_at timestamp,
    expires_at timestamp
) WITH paxos_grace_seconds = 3600;

-- Use LWT with the table
INSERT INTO distributed_locks (lock_name, owner, acquired_at, expires_at)
VALUES ('resource_1', 'node_a', toTimestamp(now()), toTimestamp(now()))
IF NOT EXISTS;

UPDATE distributed_locks
SET owner = 'node_b', acquired_at = toTimestamp(now())
WHERE lock_name = 'resource_1'
IF owner = 'node_a';

-- =============================================================================
-- Per-Partition Rate Limit
-- =============================================================================

-- Create table with per-partition rate limiting
CREATE TABLE rate_limited_data (
    partition_key text,
    sort_key timeuuid,
    data text,
    PRIMARY KEY ((partition_key), sort_key)
) WITH per_partition_rate_limit = {
    'max_reads_per_second': 1000,
    'max_writes_per_second': 500
};

-- Alter table to add/modify rate limits
ALTER TABLE rate_limited_data WITH per_partition_rate_limit = {
    'max_reads_per_second': 2000,
    'max_writes_per_second': 1000
};

-- Rate limit for reads only
ALTER TABLE rate_limited_data WITH per_partition_rate_limit = {
    'max_reads_per_second': 500
};

-- =============================================================================
-- Synchronous Materialized Views
-- =============================================================================

-- Create base table
CREATE TABLE orders (
    order_id uuid PRIMARY KEY,
    customer_id uuid,
    order_date timestamp,
    total decimal,
    status text
);

-- Create synchronous materialized view
-- Updates wait for view to be updated before returning
CREATE MATERIALIZED VIEW orders_by_customer AS
    SELECT order_id, customer_id, order_date, total, status
    FROM orders
    WHERE customer_id IS NOT NULL AND order_id IS NOT NULL
    PRIMARY KEY (customer_id, order_id)
WITH synchronous_updates = true;

-- Create async view (default behavior)
CREATE MATERIALIZED VIEW orders_by_date AS
    SELECT order_id, customer_id, order_date, total, status
    FROM orders
    WHERE order_date IS NOT NULL AND order_id IS NOT NULL
    PRIMARY KEY (order_date, order_id)
WITH synchronous_updates = false;

-- Alter existing view to be synchronous
ALTER MATERIALIZED VIEW orders_by_date WITH synchronous_updates = true;

-- =============================================================================
-- PRUNE MATERIALIZED VIEW
-- =============================================================================

-- Remove ghost rows from materialized view
-- (rows in view that don't correspond to base table rows)

-- Prune entire view
PRUNE MATERIALIZED VIEW orders_by_customer;

-- Prune specific token range
PRUNE MATERIALIZED VIEW orders_by_customer 
WHERE token(customer_id) > 0 AND token(customer_id) < 1000000;

-- Prune specific partition
PRUNE MATERIALIZED VIEW orders_by_customer 
WHERE customer_id = 550e8400-e29b-41d4-a716-446655440000;

-- =============================================================================
-- REDUCEFUNC for User-Defined Aggregates
-- =============================================================================

-- Create state function
CREATE FUNCTION sum_state(state bigint, val int)
RETURNS NULL ON NULL INPUT
RETURNS bigint
LANGUAGE lua
AS $$
    return state + val
$$;

-- Create reduce function for distributed aggregation
CREATE FUNCTION sum_reduce(state1 bigint, state2 bigint)
RETURNS NULL ON NULL INPUT
RETURNS bigint
LANGUAGE lua
AS $$
    return state1 + state2
$$;

-- Create aggregate with REDUCEFUNC for distributed execution
CREATE AGGREGATE distributed_sum(int)
SFUNC sum_state
STYPE bigint
REDUCEFUNC sum_reduce
INITCOND 0;

-- More complex example: distributed average
CREATE FUNCTION avg_state(acc tuple<bigint, int>, val int)
RETURNS NULL ON NULL INPUT
RETURNS tuple<bigint, int>
LANGUAGE lua
AS $$
    return { acc[1] + val, acc[2] + 1 }
$$;

CREATE FUNCTION avg_reduce(acc1 tuple<bigint, int>, acc2 tuple<bigint, int>)
RETURNS NULL ON NULL INPUT
RETURNS tuple<bigint, int>
LANGUAGE lua
AS $$
    return { acc1[1] + acc2[1], acc1[2] + acc2[2] }
$$;

CREATE FUNCTION avg_final(acc tuple<bigint, int>)
RETURNS NULL ON NULL INPUT
RETURNS double
LANGUAGE lua
AS $$
    return acc[1] / acc[2]
$$;

CREATE AGGREGATE distributed_avg(int)
SFUNC avg_state
STYPE tuple<bigint, int>
REDUCEFUNC avg_reduce
FINALFUNC avg_final
INITCOND (0, 0);

-- =============================================================================
-- Service Levels (Workload Management)
-- =============================================================================

-- Create service levels with different configurations
CREATE SERVICE LEVEL gold 
WITH timeout = 50ms 
AND workload_type = 'interactive';

CREATE SERVICE LEVEL silver 
WITH timeout = 200ms 
AND workload_type = 'interactive';

CREATE SERVICE LEVEL bronze 
WITH timeout = 1s 
AND workload_type = 'batch';

CREATE SERVICE LEVEL IF NOT EXISTS background
WITH timeout = 30s
AND workload_type = 'batch';

-- Alter existing service level
ALTER SERVICE LEVEL silver WITH timeout = 150ms;

-- Attach service level to role
ATTACH SERVICE LEVEL gold TO premium_users;
ATTACH SERVICE LEVEL silver TO standard_users;
ATTACH SERVICE LEVEL bronze TO api_users;

-- Detach service level from role
DETACH SERVICE LEVEL FROM trial_users;

-- List all service levels
LIST ALL SERVICE LEVELS;
LIST SERVICE LEVELS;

-- List effective service level for a role (shows inherited values)
LIST EFFECTIVE SERVICE LEVEL OF premium_users;

-- Drop service level
DROP SERVICE LEVEL IF EXISTS background;

-- =============================================================================
-- DESCRIBE SCHEMA WITH INTERNALS
-- =============================================================================

-- Describe full schema including internal objects
DESCRIBE SCHEMA WITH INTERNALS;

-- Include hashed passwords in output (admin use)
DESCRIBE SCHEMA WITH INTERNALS AND PASSWORDS;

-- =============================================================================
-- NULL Handling Differences (ScyllaDB specific behavior)
-- =============================================================================

-- ScyllaDB allows NULL comparisons (returns FALSE, not error)
-- These work in ScyllaDB but may error in Cassandra:

CREATE TABLE null_test (
    id int PRIMARY KEY,
    value text,
    number int
);

INSERT INTO null_test (id, value, number) VALUES (1, 'test', 42);
INSERT INTO null_test (id, value, number) VALUES (2, NULL, NULL);

-- In ScyllaDB: these return FALSE (no rows), not an error
-- SELECT * FROM null_test WHERE value = NULL;
-- SELECT * FROM null_test WHERE number != NULL;

-- LWT with NULL (ScyllaDB specific behavior)
UPDATE null_test SET value = 'new' WHERE id = 2 IF value = NULL;

-- =============================================================================
-- List Subscript in WHERE Clause
-- =============================================================================

CREATE TABLE list_test (
    id int PRIMARY KEY,
    items list<text>
);

INSERT INTO list_test (id, items) VALUES (1, ['a', 'b', 'c']);

-- ScyllaDB allows list subscript in WHERE (like maps)
SELECT * FROM list_test WHERE items[0] = 'a' ALLOW FILTERING;

-- =============================================================================
-- Clean up
-- =============================================================================

DROP MATERIALIZED VIEW IF EXISTS orders_by_customer;
DROP MATERIALIZED VIEW IF EXISTS orders_by_date;
DROP TABLE IF EXISTS logs;
DROP TABLE IF EXISTS distributed_locks;
DROP TABLE IF EXISTS rate_limited_data;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS null_test;
DROP TABLE IF EXISTS list_test;

DROP AGGREGATE IF EXISTS distributed_sum;
DROP AGGREGATE IF EXISTS distributed_avg;
DROP FUNCTION IF EXISTS sum_state;
DROP FUNCTION IF EXISTS sum_reduce;
DROP FUNCTION IF EXISTS avg_state;
DROP FUNCTION IF EXISTS avg_reduce;
DROP FUNCTION IF EXISTS avg_final;

DROP SERVICE LEVEL IF EXISTS gold;
DROP SERVICE LEVEL IF EXISTS silver;
DROP SERVICE LEVEL IF EXISTS bronze;

DROP KEYSPACE IF EXISTS archived_data;
