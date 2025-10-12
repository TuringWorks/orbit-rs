-- =============================================================================
-- 01_basic_operations.sql
-- Basic SQL Operations Demo for Orbit-RS PostgreSQL Server
-- =============================================================================

\echo '🚀 Basic SQL Operations Demo'
\echo '============================'
\echo ''

-- Create basic tables
\echo '1. Creating basic tables...'
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE,
    age INTEGER,
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    price DECIMAL(10,2),
    category TEXT,
    in_stock BOOLEAN DEFAULT true
);

CREATE TABLE orders (
    id INTEGER PRIMARY KEY,
    user_id INTEGER,
    product_id INTEGER,
    quantity INTEGER,
    order_date TIMESTAMP DEFAULT NOW(),
    total_amount DECIMAL(10,2)
);

\echo '✅ Tables created successfully!'
\echo ''

-- Insert sample data
\echo '2. Inserting sample data...'
INSERT INTO users (id, name, email, age) VALUES 
    (1, 'Alice Johnson', 'alice@example.com', 28),
    (2, 'Bob Smith', 'bob@example.com', 34),
    (3, 'Carol Davis', 'carol@example.com', 22),
    (4, 'David Wilson', 'david@example.com', 41),
    (5, 'Eve Brown', 'eve@example.com', 29);

INSERT INTO products (id, name, price, category) VALUES 
    (101, 'Laptop Pro', 1299.99, 'Electronics'),
    (102, 'Wireless Mouse', 29.99, 'Electronics'),
    (103, 'Coffee Maker', 89.99, 'Kitchen'),
    (104, 'Running Shoes', 129.99, 'Sports'),
    (105, 'Desk Chair', 199.99, 'Furniture');

INSERT INTO orders (id, user_id, product_id, quantity, total_amount) VALUES 
    (1001, 1, 101, 1, 1299.99),
    (1002, 2, 102, 2, 59.98),
    (1003, 3, 103, 1, 89.99),
    (1004, 1, 104, 1, 129.99),
    (1005, 4, 105, 1, 199.99),
    (1006, 2, 101, 1, 1299.99),
    (1007, 5, 102, 3, 89.97);

\echo '✅ Sample data inserted!'
\echo ''

-- Basic SELECT operations
\echo '3. Basic SELECT operations...'
\echo ''
\echo '3.1 Select all users:'
SELECT * FROM users;

\echo ''
\echo '3.2 Select specific columns:'
SELECT name, email FROM users WHERE age > 30;

\echo ''
\echo '3.3 Select with ORDER BY:'
SELECT name, age FROM users ORDER BY age DESC;

\echo ''
\echo '3.4 Select products by category:'
SELECT name, price FROM products WHERE category = 'Electronics';

\echo ''

-- Basic UPDATE operations
\echo '4. UPDATE operations...'
\echo ''
\echo '4.1 Update a user age:'
UPDATE users SET age = 35 WHERE name = 'Bob Smith';

\echo '4.2 Verify the update:'
SELECT name, age FROM users WHERE name = 'Bob Smith';

\echo ''

-- Basic DELETE operations
\echo '5. DELETE operations...'
\echo ''
\echo '5.1 Count orders before deletion:'
SELECT COUNT(*) as total_orders FROM orders;

\echo '5.2 Delete an order:'
DELETE FROM orders WHERE id = 1007;

\echo '5.3 Count orders after deletion:'
SELECT COUNT(*) as total_orders FROM orders;

\echo ''
\echo '✅ Basic operations demo completed!'
\echo ''