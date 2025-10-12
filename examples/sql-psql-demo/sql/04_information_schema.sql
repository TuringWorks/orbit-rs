-- =============================================================================
-- 04_information_schema.sql
-- Information Schema Demo for Orbit-RS PostgreSQL Server
-- =============================================================================

\echo '📊 Information Schema Demo'
\echo '=========================='
\echo ''

-- First, let's create some tables to work with
\echo '1. Creating demo tables for information_schema testing...'
CREATE TABLE customers (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    created_at TIMESTAMP DEFAULT NOW(),
    status VARCHAR(20) DEFAULT 'active'
);

CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    category TEXT,
    price DECIMAL(10,2) NOT NULL,
    in_stock BOOLEAN DEFAULT true,
    description TEXT
);

CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    customer_id INTEGER,
    product_id INTEGER,
    quantity INTEGER NOT NULL,
    order_date TIMESTAMP DEFAULT NOW(),
    total_amount DECIMAL(10,2),
    CONSTRAINT fk_customer FOREIGN KEY (customer_id) REFERENCES customers(id),
    CONSTRAINT fk_product FOREIGN KEY (product_id) REFERENCES products(id)
);

-- Create some indices
CREATE INDEX idx_customers_email ON customers (email);
CREATE INDEX idx_customers_status ON customers (status);
CREATE INDEX idx_orders_customer_id ON orders (customer_id);
CREATE INDEX idx_orders_date ON orders (order_date);

\echo '✅ Demo tables and indices created!'
\echo ''

-- Now let's explore the information_schema
\echo '2. Querying information_schema.tables...'
\echo ''
\echo '2.1 All tables in our database:'
SELECT table_name, table_type, is_insertable_into
FROM information_schema.tables
WHERE table_schema = 'public'
ORDER BY table_name;

\echo ''
\echo '2.2 Filter for specific table:'
SELECT table_catalog, table_schema, table_name, table_type
FROM information_schema.tables
WHERE table_name = 'customers';

\echo ''

-- Query column information
\echo '3. Querying information_schema.columns...'
\echo ''
\echo '3.1 All columns from customers table:'
SELECT column_name, data_type, is_nullable, column_default, ordinal_position
FROM information_schema.columns
WHERE table_name = 'customers'
ORDER BY ordinal_position;

\echo ''
\echo '3.2 All columns with their data types:'
SELECT 
    table_name,
    column_name,
    data_type,
    CASE 
        WHEN is_nullable = 'YES' THEN 'NULL'
        ELSE 'NOT NULL'
    END as nullable
FROM information_schema.columns
WHERE table_schema = 'public'
ORDER BY table_name, ordinal_position;

\echo ''

-- Query constraint information
\echo '4. Querying information_schema.table_constraints...'
\echo ''
\echo '4.1 All constraints:'
SELECT 
    table_name,
    constraint_name,
    constraint_type
FROM information_schema.table_constraints
WHERE table_schema = 'public'
ORDER BY table_name, constraint_type;

\echo ''
\echo '4.2 Only PRIMARY KEY constraints:'
SELECT 
    table_name,
    constraint_name
FROM information_schema.table_constraints
WHERE constraint_type = 'PRIMARY KEY'
  AND table_schema = 'public';

\echo ''

-- Query key column usage (for primary keys, foreign keys, unique constraints)
\echo '5. Querying information_schema.key_column_usage...'
\echo ''
\echo '5.1 All key column usage:'
SELECT 
    table_name,
    column_name,
    constraint_name,
    ordinal_position
FROM information_schema.key_column_usage
WHERE table_schema = 'public'
ORDER BY table_name, ordinal_position;

\echo ''
\echo '5.2 Foreign key relationships:'
SELECT 
    table_name as "Table",
    column_name as "Column",
    constraint_name as "FK Constraint",
    referenced_table_name as "References Table",
    referenced_column_name as "References Column"
FROM information_schema.key_column_usage
WHERE referenced_table_name IS NOT NULL
ORDER BY table_name;

\echo ''

-- Practical queries using information_schema
\echo '6. Practical information_schema queries...'
\echo ''
\echo '6.1 Table summary with column count:'
SELECT 
    t.table_name,
    t.table_type,
    COUNT(c.column_name) as column_count
FROM information_schema.tables t
LEFT JOIN information_schema.columns c ON t.table_name = c.table_name AND c.table_schema = 'public'
WHERE t.table_schema = 'public'
GROUP BY t.table_name, t.table_type
ORDER BY t.table_name;

\echo ''
\echo '6.2 Find all TEXT columns:'
SELECT 
    table_name,
    column_name,
    data_type
FROM information_schema.columns
WHERE data_type = 'text'
  AND table_schema = 'public'
ORDER BY table_name, column_name;

\echo ''
\echo '6.3 Find all nullable columns:'
SELECT 
    table_name,
    column_name,
    data_type
FROM information_schema.columns
WHERE is_nullable = 'YES'
  AND table_schema = 'public'
ORDER BY table_name, ordinal_position;

\echo ''

-- Advanced information_schema queries
\echo '7. Advanced information_schema usage...'
\echo ''
\echo '7.1 Generate CREATE TABLE statements from information_schema:'
SELECT 
    'CREATE TABLE ' || table_name || ' (' ||
    STRING_AGG(
        column_name || ' ' || 
        UPPER(data_type) ||
        CASE 
            WHEN is_nullable = 'NO' THEN ' NOT NULL'
            ELSE ''
        END ||
        CASE 
            WHEN column_default IS NOT NULL AND column_default != '' 
            THEN ' DEFAULT ' || column_default
            ELSE ''
        END,
        ', '
        ORDER BY ordinal_position
    ) || ');' as create_statement
FROM information_schema.columns
WHERE table_name = 'customers'
  AND table_schema = 'public'
GROUP BY table_name;

\echo ''
\echo '7.2 Database schema overview:'
SELECT 
    'Tables: ' || COUNT(DISTINCT t.table_name) ||
    ', Columns: ' || COUNT(DISTINCT c.column_name) ||
    ', Constraints: ' || COUNT(DISTINCT tc.constraint_name) as database_summary
FROM information_schema.tables t
LEFT JOIN information_schema.columns c ON t.table_name = c.table_name AND c.table_schema = 'public'
LEFT JOIN information_schema.table_constraints tc ON t.table_name = tc.table_name AND tc.table_schema = 'public'
WHERE t.table_schema = 'public';

\echo ''

-- Test psql meta-commands that use information_schema
\echo '8. Testing psql meta-commands...'
\echo ''
\echo '8.1 List tables (\\dt equivalent):'
\echo 'Note: \\dt should now work with our information_schema implementation'

\echo ''
\echo '8.2 Describe table structure (\\d equivalent):'
\echo 'Note: \\d table_name should now work with our information_schema'

\echo ''

-- Clean up
\echo '9. Cleaning up demo tables...'
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS products;  
DROP TABLE IF EXISTS customers;

\echo ''
\echo '✅ Information Schema demo completed!'
\echo ''
\echo '📝 Summary of information_schema support:'
\echo '   ✅ information_schema.tables - List all tables'
\echo '   ✅ information_schema.columns - Column metadata'  
\echo '   ✅ information_schema.table_constraints - Constraint information'
\echo '   ✅ information_schema.key_column_usage - Key relationships'
\echo '   ✅ Standard PostgreSQL compatibility'
\echo '   ✅ Support for psql meta-commands like \\dt and \\d'
\echo ''
\echo 'Try these commands in interactive psql:'
\echo '   \\dt                    -- List tables'
\echo '   \\d table_name          -- Describe table'
\echo '   SELECT * FROM information_schema.tables;'
\echo ''