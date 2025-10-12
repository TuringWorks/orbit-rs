-- =============================================================================
-- 02_joins_and_indices.sql
-- JOINs and Indices Demo for Orbit-RS PostgreSQL Server
-- =============================================================================

\echo '🔗 JOINs and Indices Demo'
\echo '========================='
\echo ''

-- Assume tables from 01_basic_operations.sql are already created
-- We'll create indices first for better performance

\echo '1. Creating indices for better performance...'
CREATE INDEX idx_users_age ON users (age);
CREATE INDEX idx_users_email ON users (email);
CREATE INDEX idx_orders_user_id ON orders (user_id);
CREATE INDEX idx_orders_product_id ON orders (product_id);
CREATE INDEX idx_products_category ON products (category);

\echo '✅ Indices created!'
\echo ''

-- INNER JOIN demonstrations
\echo '2. INNER JOIN demonstrations...'
\echo ''
\echo '2.1 Users with their orders (INNER JOIN):'
SELECT 
    u.name as user_name,
    u.email,
    o.id as order_id,
    o.quantity,
    o.total_amount
FROM users u 
INNER JOIN orders o ON u.id = o.user_id
ORDER BY u.name;

\echo ''
\echo '2.2 Orders with product details (INNER JOIN):'
SELECT 
    o.id as order_id,
    u.name as customer,
    p.name as product_name,
    p.price as unit_price,
    o.quantity,
    o.total_amount
FROM orders o
INNER JOIN users u ON o.user_id = u.id
INNER JOIN products p ON o.product_id = p.id
ORDER BY o.id;

\echo ''

-- LEFT JOIN demonstrations  
\echo '3. LEFT JOIN demonstrations...'
\echo ''
\echo '3.1 All users and their orders (including users with no orders):'
SELECT 
    u.name as user_name,
    u.email,
    COALESCE(o.id, 0) as order_id,
    COALESCE(o.total_amount, 0) as total_amount
FROM users u 
LEFT JOIN orders o ON u.id = o.user_id
ORDER BY u.name;

\echo ''
\echo '3.2 Users with order count:'
SELECT 
    u.name as user_name,
    u.email,
    COUNT(o.id) as order_count,
    COALESCE(SUM(o.total_amount), 0) as total_spent
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name, u.email
ORDER BY total_spent DESC;

\echo ''

-- RIGHT JOIN demonstration
\echo '4. RIGHT JOIN demonstration...'
\echo ''
\echo '4.1 All products and their orders (including products never ordered):'
SELECT 
    p.name as product_name,
    p.category,
    p.price,
    COALESCE(o.id, 0) as order_id,
    COALESCE(u.name, 'No orders') as customer
FROM orders o
RIGHT JOIN products p ON o.product_id = p.id
LEFT JOIN users u ON o.user_id = u.id
ORDER BY p.name;

\echo ''

-- FULL OUTER JOIN demonstration
\echo '5. FULL OUTER JOIN demonstration...'
\echo ''
-- First, let's add a user with no orders and a product with no orders
INSERT INTO users (id, name, email, age) VALUES (6, 'Frank Miller', 'frank@example.com', 33);
INSERT INTO products (id, name, price, category) VALUES (106, 'Tablet', 599.99, 'Electronics');

\echo '5.1 All users and all products relationship:'
SELECT 
    COALESCE(u.name, 'No user') as user_name,
    COALESCE(p.name, 'No product') as product_name,
    COALESCE(o.total_amount, 0) as order_amount
FROM users u
FULL OUTER JOIN orders o ON u.id = o.user_id
FULL OUTER JOIN products p ON o.product_id = p.id
ORDER BY user_name, product_name;

\echo ''

-- CROSS JOIN demonstration
\echo '6. CROSS JOIN demonstration...'
\echo ''
\echo '6.1 All possible user-category combinations:'
SELECT 
    u.name as user_name,
    p.category
FROM users u
CROSS JOIN (SELECT DISTINCT category FROM products) p
WHERE u.age < 30
ORDER BY u.name, p.category;

\echo ''

-- Complex JOIN with aggregations
\echo '7. Complex JOINs with aggregations...'
\echo ''
\echo '7.1 Category sales summary:'
SELECT 
    p.category,
    COUNT(o.id) as orders_count,
    SUM(o.quantity) as total_quantity,
    SUM(o.total_amount) as total_revenue,
    AVG(o.total_amount) as avg_order_value
FROM products p
INNER JOIN orders o ON p.id = o.product_id
GROUP BY p.category
ORDER BY total_revenue DESC;

\echo ''
\echo '7.2 Top customers by spending:'
SELECT 
    u.name as customer_name,
    u.email,
    COUNT(o.id) as orders_count,
    SUM(o.total_amount) as total_spent,
    AVG(o.total_amount) as avg_order_value
FROM users u
INNER JOIN orders o ON u.id = o.user_id
GROUP BY u.id, u.name, u.email
HAVING COUNT(o.id) >= 1
ORDER BY total_spent DESC;

\echo ''

-- Self JOIN demonstration (if we had hierarchical data)
\echo '8. Advanced JOIN scenarios...'
\echo ''
-- Create a simple employee hierarchy table for self-join demo
CREATE TABLE employees (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    manager_id INTEGER,
    department TEXT
);

INSERT INTO employees (id, name, manager_id, department) VALUES 
    (1, 'Alice CEO', NULL, 'Executive'),
    (2, 'Bob VP-Eng', 1, 'Engineering'),
    (3, 'Carol VP-Sales', 1, 'Sales'),
    (4, 'Dave Dev-Lead', 2, 'Engineering'),
    (5, 'Eve Developer', 4, 'Engineering'),
    (6, 'Frank Sales-Rep', 3, 'Sales');

\echo '8.1 Employee hierarchy (self-join):'
SELECT 
    e.name as employee_name,
    e.department,
    COALESCE(m.name, 'No manager') as manager_name
FROM employees e
LEFT JOIN employees m ON e.manager_id = m.id
ORDER BY e.department, e.name;

\echo ''
\echo '✅ JOINs and indices demo completed!'
\echo ''