-- =============================================================================
-- Apache Cassandra 5.0 Features Examples
-- Vector Search, Dynamic Data Masking, SAI, New Functions
-- =============================================================================

USE my_app;

-- =============================================================================
-- Vector Search (CEP-30)
-- =============================================================================

-- Create a table with vector column for embeddings
CREATE TABLE products (
    product_id uuid PRIMARY KEY,
    name text,
    description text,
    category text,
    price decimal,
    -- Vector column: float type with 1536 dimensions (OpenAI embedding size)
    embedding vector<float, 1536>
);

-- Create a table with smaller vector for demonstration
CREATE TABLE documents (
    doc_id uuid PRIMARY KEY,
    title text,
    content text,
    -- Smaller vector for demonstration
    content_embedding vector<float, 384>
);

-- Create Storage-Attached Index (SAI) for vector search
-- Using cosine similarity (default)
CREATE CUSTOM INDEX products_embedding_idx ON products(embedding)
USING 'StorageAttachedIndex';

-- Create SAI with explicit similarity function
CREATE CUSTOM INDEX docs_embedding_idx ON documents(content_embedding)
USING 'StorageAttachedIndex'
WITH OPTIONS = {
    'similarity_function': 'cosine'
};

-- Alternative similarity functions
-- CREATE INDEX with dot_product similarity
CREATE CUSTOM INDEX products_dot_idx ON products(embedding)
USING 'StorageAttachedIndex'
WITH OPTIONS = {
    'similarity_function': 'dot_product'
};

-- CREATE INDEX with euclidean similarity
CREATE CUSTOM INDEX products_euclidean_idx ON products(embedding)
USING 'StorageAttachedIndex'
WITH OPTIONS = {
    'similarity_function': 'euclidean'
};

-- Insert data with vector embeddings
INSERT INTO products (product_id, name, description, category, price, embedding)
VALUES (
    uuid(),
    'Laptop Pro',
    'High-performance laptop for professionals',
    'electronics',
    1299.99,
    [0.1, 0.2, 0.3, 0.4, 0.5]  -- Simplified 5-dim vector for demo
);

INSERT INTO documents (doc_id, title, content, content_embedding)
VALUES (
    550e8400-e29b-41d4-a716-446655440000,
    'Introduction to ML',
    'Machine learning is a subset of AI...',
    [0.12, -0.34, 0.56, 0.78, -0.90, 0.11]
);

-- Vector Search Queries using ANN (Approximate Nearest Neighbor)
-- Find similar products based on embedding
SELECT product_id, name, category, price
FROM products
ORDER BY embedding ANN OF [0.1, 0.2, 0.3, 0.4, 0.5]
LIMIT 10;

-- Vector search with similarity score
SELECT 
    product_id, 
    name,
    similarity(embedding, [0.1, 0.2, 0.3, 0.4, 0.5]) AS similarity_score
FROM products
ORDER BY embedding ANN OF [0.1, 0.2, 0.3, 0.4, 0.5]
LIMIT 5;

-- Combined filter with vector search
SELECT product_id, name, price
FROM products
WHERE category = 'electronics'
ORDER BY embedding ANN OF [0.1, 0.2, 0.3, 0.4, 0.5]
LIMIT 10;

-- Document similarity search
SELECT doc_id, title, 
       similarity(content_embedding, [0.12, -0.34, 0.56, 0.78, -0.90, 0.11]) AS score
FROM documents
ORDER BY content_embedding ANN OF [0.12, -0.34, 0.56, 0.78, -0.90, 0.11]
LIMIT 20;

-- =============================================================================
-- Dynamic Data Masking (CEP-20)
-- =============================================================================

-- Create table with masked columns
CREATE TABLE customers (
    customer_id uuid PRIMARY KEY,
    first_name text,
    last_name text,
    -- Mask email with default masking function
    email text MASKED WITH DEFAULT,
    -- Mask phone with default
    phone text MASKED WITH DEFAULT,
    -- Mask SSN with a custom masking function
    ssn text MASKED WITH mask_replace('***-**-****'),
    -- Mask credit card
    credit_card text MASKED WITH mask_inner(4, 4, '*'),
    date_of_birth date,
    created_at timestamp
);

-- Create table with various masking functions
CREATE TABLE sensitive_data (
    id uuid PRIMARY KEY,
    full_name text MASKED WITH DEFAULT,
    partial_mask text MASKED WITH mask_inner(2, 2, '#'),
    replace_mask text MASKED WITH mask_replace('REDACTED'),
    null_mask text MASKED WITH mask_null()
);

-- Grant UNMASK permission to trusted roles
GRANT UNMASK ON TABLE customers TO data_analyst;

-- Grant SELECT_MASKED to allow reading masked data
GRANT SELECT_MASKED ON TABLE customers TO regular_user;

-- Revoke UNMASK permission
REVOKE UNMASK ON TABLE customers FROM data_analyst;

-- Insert data into masked table
INSERT INTO customers (customer_id, first_name, last_name, email, phone, ssn, credit_card, date_of_birth)
VALUES (
    uuid(),
    'John',
    'Doe',
    'john.doe@example.com',
    '+1-555-123-4567',
    '123-45-6789',
    '4111111111111111',
    '1985-03-15'
);

-- Query masked data (will show masked values for users without UNMASK permission)
SELECT customer_id, first_name, last_name, email, phone, ssn
FROM customers;

-- =============================================================================
-- New Mathematical CQL Functions
-- =============================================================================

-- Create table for mathematical operations
CREATE TABLE measurements (
    id uuid PRIMARY KEY,
    value double,
    raw_reading double,
    log_value double
);

INSERT INTO measurements (id, value, raw_reading, log_value)
VALUES (uuid(), -15.7, 2.71828, 100.0);

-- Using abs() function
SELECT id, abs(value) AS absolute_value
FROM measurements;

-- Using exp() function (e^x)
SELECT id, exp(raw_reading) AS exponential
FROM measurements;

-- Using log() function (natural logarithm)
SELECT id, log(log_value) AS natural_log
FROM measurements;

-- Using log10() function (base-10 logarithm)
SELECT id, log10(log_value) AS log_base_10
FROM measurements;

-- Using round() function
SELECT id, round(value) AS rounded_value
FROM measurements;

-- Combining math functions
SELECT id,
       abs(value) AS abs_val,
       round(abs(value)) AS rounded_abs,
       exp(1) AS e_value,
       log(exp(1)) AS log_of_e
FROM measurements;

-- =============================================================================
-- Storage-Attached Indexes (SAI) - General Usage
-- =============================================================================

-- Create table for SAI demonstration
CREATE TABLE orders (
    order_id uuid,
    customer_id uuid,
    order_date timestamp,
    status text,
    total_amount decimal,
    items list<text>,
    metadata map<text, text>,
    PRIMARY KEY (order_id)
);

-- Create SAI on regular columns
CREATE CUSTOM INDEX orders_customer_idx ON orders(customer_id)
USING 'StorageAttachedIndex';

CREATE CUSTOM INDEX orders_status_idx ON orders(status)
USING 'StorageAttachedIndex';

CREATE CUSTOM INDEX orders_date_idx ON orders(order_date)
USING 'StorageAttachedIndex';

-- Create SAI on collection columns
-- Index on list values
CREATE CUSTOM INDEX orders_items_idx ON orders(values(items))
USING 'StorageAttachedIndex';

-- Index on map keys
CREATE CUSTOM INDEX orders_meta_keys_idx ON orders(keys(metadata))
USING 'StorageAttachedIndex';

-- Index on map values
CREATE CUSTOM INDEX orders_meta_values_idx ON orders(values(metadata))
USING 'StorageAttachedIndex';

-- Index on map entries
CREATE CUSTOM INDEX orders_meta_entries_idx ON orders(entries(metadata))
USING 'StorageAttachedIndex';

-- Query using SAI indexes
SELECT * FROM orders WHERE customer_id = 550e8400-e29b-41d4-a716-446655440000;

SELECT * FROM orders WHERE status = 'pending' AND order_date > '2024-01-01';

SELECT * FROM orders WHERE items CONTAINS 'laptop';

SELECT * FROM orders WHERE metadata CONTAINS KEY 'priority';

SELECT * FROM orders WHERE metadata['priority'] = 'high';

-- =============================================================================
-- Other Cassandra 5.0 Features
-- =============================================================================

-- TTL and WRITETIME on collections and UDTs
CREATE TABLE user_preferences (
    user_id uuid PRIMARY KEY,
    settings map<text, text>,
    tags set<text>
);

INSERT INTO user_preferences (user_id, settings, tags)
VALUES (uuid(), {'theme': 'dark', 'language': 'en'}, {'vip', 'beta'})
USING TTL 86400;

-- Query writetime and TTL on collection
SELECT user_id, writetime(settings), ttl(settings)
FROM user_preferences;

-- Unified Compaction Strategy (UCS)
CREATE TABLE time_series (
    device_id uuid,
    event_time timestamp,
    value double,
    PRIMARY KEY ((device_id), event_time)
) WITH compaction = {
    'class': 'UnifiedCompactionStrategy',
    'base_shard_count': 4,
    'target_sstable_size': '1GB'
};

-- CIDR Authorizer (network-based access control)
-- Note: Configured at server level, not CQL

-- Extended TTL (beyond 20 years)
INSERT INTO measurements (id, value, raw_reading, log_value)
VALUES (uuid(), 42.0, 1.0, 10.0)
USING TTL 946080000;  -- ~30 years

-- Clean up
DROP TABLE IF EXISTS products;
DROP TABLE IF EXISTS documents;
DROP TABLE IF EXISTS customers;
DROP TABLE IF EXISTS sensitive_data;
DROP TABLE IF EXISTS measurements;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS user_preferences;
DROP TABLE IF EXISTS time_series;
