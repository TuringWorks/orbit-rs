-- ============================================================================
-- Orbit-RS MySQL Protocol Examples - Basic SQL Operations
-- ============================================================================
-- This file demonstrates basic SQL operations using the MySQL wire protocol
-- with Orbit-RS.
--
-- Prerequisites:
-- 1. Start Orbit server: cargo run --bin orbit-server
-- 2. Run with: mysql -h localhost -P 3306 -u orbit < 01_basic_sql.sql
-- ============================================================================
SELECT '============================================================' AS '';
SELECT 'MySQL Basic SQL Operations with Orbit-RS' AS '';
SELECT '============================================================' AS '';
-- ============================================================================
-- 1. DATABASE AND TABLE CREATION
-- ============================================================================
SELECT '' AS '';
SELECT '1. DATABASE AND TABLE CREATION' AS '';
SELECT '------------------------------------------------------------' AS '';
-- Create database (if not exists)
CREATE DATABASE IF NOT EXISTS orbit_mysql_examples;
USE orbit_mysql_examples;
-- Drop existing tables for clean start
DROP TABLE IF EXISTS order_items;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS products;
DROP TABLE IF EXISTS customers;
-- Create customers table
CREATE TABLE customers (
    customer_id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    phone VARCHAR(20),
    city VARCHAR(50),
    state VARCHAR(2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_email (email),
    INDEX idx_city_state (city, state)
);
SELECT 'Created customers table' AS '';
-- Create products table
CREATE TABLE products (
    product_id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    category VARCHAR(50),
    price DECIMAL(10, 2) NOT NULL,
    stock INT DEFAULT 0,
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_category (category),
    INDEX idx_price (price)
);
SELECT 'Created products table' AS '';
-- Create orders table
CREATE TABLE orders (
    order_id INT AUTO_INCREMENT PRIMARY KEY,
    customer_id INT NOT NULL,
    order_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    status VARCHAR(20) DEFAULT 'pending',
    total_amount DECIMAL(10, 2),
    FOREIGN KEY (customer_id) REFERENCES customers(customer_id),
    INDEX idx_customer (customer_id),
    INDEX idx_status (status),
    INDEX idx_order_date (order_date)
);
SELECT 'Created orders table' AS '';
-- Create order_items table
CREATE TABLE order_items (
    item_id INT AUTO_INCREMENT PRIMARY KEY,
    order_id INT NOT NULL,
    product_id INT NOT NULL,
    quantity INT NOT NULL,
    unit_price DECIMAL(10, 2) NOT NULL,
    FOREIGN KEY (order_id) REFERENCES orders(order_id),
    FOREIGN KEY (product_id) REFERENCES products(product_id),
    INDEX idx_order (order_id),
    INDEX idx_product (product_id)
);
SELECT 'Created order_items table' AS '';
-- ============================================================================
-- 2. INSERT OPERATIONS
-- ============================================================================
SELECT '' AS '';
SELECT '2. INSERT OPERATIONS' AS '';
SELECT '------------------------------------------------------------' AS '';
-- Insert customers
INSERT INTO customers (name, email, phone, city, state)
VALUES (
        'Alice Johnson',
        'alice@example.com',
        '555-0101',
        'New York',
        'NY'
    ),
    (
        'Bob Smith',
        'bob@example.com',
        '555-0102',
        'Los Angeles',
        'CA'
    ),
    (
        'Carol White',
        'carol@example.com',
        '555-0103',
        'Chicago',
        'IL'
    ),
    (
        'David Brown',
        'david@example.com',
        '555-0104',
        'Houston',
        'TX'
    ),
    (
        'Eve Davis',
        'eve@example.com',
        '555-0105',
        'Phoenix',
        'AZ'
    );
SELECT CONCAT('Inserted ', ROW_COUNT(), ' customers') AS '';
-- Insert products
INSERT INTO products (name, category, price, stock, description)
VALUES (
        'Laptop Pro 15',
        'Electronics',
        1299.99,
        50,
        'High-performance laptop'
    ),
    (
        'Wireless Mouse',
        'Accessories',
        29.99,
        200,
        'Ergonomic wireless mouse'
    ),
    (
        'USB-C Hub',
        'Accessories',
        49.99,
        150,
        '7-port USB-C hub'
    ),
    (
        '4K Monitor',
        'Electronics',
        599.99,
        30,
        '27-inch 4K display'
    ),
    (
        'Mechanical Keyboard',
        'Accessories',
        149.99,
        75,
        'RGB mechanical keyboard'
    ),
    (
        'Webcam HD',
        'Electronics',
        89.99,
        100,
        '1080p webcam'
    ),
    (
        'Laptop Stand',
        'Accessories',
        39.99,
        120,
        'Adjustable laptop stand'
    ),
    (
        'External SSD 1TB',
        'Storage',
        129.99,
        80,
        'Portable SSD drive'
    );
SELECT CONCAT('Inserted ', ROW_COUNT(), ' products') AS '';
-- Insert orders
INSERT INTO orders (customer_id, status, total_amount)
VALUES (1, 'completed', 1329.98),
    (2, 'completed', 599.99),
    (3, 'shipped', 1449.98),
    (4, 'pending', 149.99),
    (5, 'completed', 149.95);
SELECT CONCAT('Inserted ', ROW_COUNT(), ' orders') AS '';
-- Insert order items
INSERT INTO order_items (order_id, product_id, quantity, unit_price)
VALUES (1, 1, 1, 1299.99),
    (1, 2, 1, 29.99),
    (2, 4, 1, 599.99),
    (3, 1, 1, 1299.99),
    (3, 3, 3, 49.99),
    (4, 5, 1, 149.99),
    (5, 2, 5, 29.99);
SELECT CONCAT('Inserted ', ROW_COUNT(), ' order items') AS '';
-- ============================================================================
-- 3. SELECT OPERATIONS
-- ============================================================================
SELECT '' AS '';
SELECT '3. SELECT OPERATIONS' AS '';
SELECT '------------------------------------------------------------' AS '';
-- Select all customers
SELECT '' AS '';
SELECT '3.1 All Customers:' AS '';
SELECT name,
    email,
    city,
    state
FROM customers;
-- Select with WHERE clause
SELECT '' AS '';
SELECT '3.2 Customers in California:' AS '';
SELECT name,
    city
FROM customers
WHERE state = 'CA';
-- Select with ORDER BY
SELECT '' AS '';
SELECT '3.3 Products Sorted by Price (Descending):' AS '';
SELECT name,
    category,
    price
FROM products
ORDER BY price DESC;
-- Select with LIMIT
SELECT '' AS '';
SELECT '3.4 Top 3 Most Expensive Products:' AS '';
SELECT name,
    price
FROM products
ORDER BY price DESC
LIMIT 3;
-- Select with aggregate functions
SELECT '' AS '';
SELECT '3.5 Product Statistics:' AS '';
SELECT COUNT(*) as total_products,
    AVG(price) as avg_price,
    MIN(price) as min_price,
    MAX(price) as max_price,
    SUM(stock) as total_stock
FROM products;
-- Group by
SELECT '' AS '';
SELECT '3.6 Products by Category:' AS '';
SELECT category,
    COUNT(*) as count,
    AVG(price) as avg_price
FROM products
GROUP BY category
ORDER BY count DESC;
-- Having clause
SELECT '' AS '';
SELECT '3.7 Categories with Avg Price > $100:' AS '';
SELECT category,
    COUNT(*) as count,
    AVG(price) as avg_price
FROM products
GROUP BY category
HAVING AVG(price) > 100
ORDER BY avg_price DESC;
-- ============================================================================
-- 4. JOIN OPERATIONS
-- ============================================================================
SELECT '' AS '';
SELECT '4. JOIN OPERATIONS' AS '';
SELECT '------------------------------------------------------------' AS '';
-- Inner join
SELECT '' AS '';
SELECT '4.1 Orders with Customer Names:' AS '';
SELECT o.order_id,
    c.name as customer_name,
    o.status,
    o.total_amount
FROM orders o
    INNER JOIN customers c ON o.customer_id = c.customer_id
ORDER BY o.order_id;
-- Multiple joins
SELECT '' AS '';
SELECT '4.2 Order Details with Products:' AS '';
SELECT o.order_id,
    c.name as customer,
    p.name as product,
    oi.quantity,
    oi.unit_price,
    (oi.quantity * oi.unit_price) as line_total
FROM orders o
    INNER JOIN customers c ON o.customer_id = c.customer_id
    INNER JOIN order_items oi ON o.order_id = oi.order_id
    INNER JOIN products p ON oi.product_id = p.product_id
ORDER BY o.order_id,
    oi.item_id;
-- Left join
SELECT '' AS '';
SELECT '4.3 All Customers with Order Count:' AS '';
SELECT c.name,
    c.email,
    COUNT(o.order_id) as order_count,
    COALESCE(SUM(o.total_amount), 0) as total_spent
FROM customers c
    LEFT JOIN orders o ON c.customer_id = o.customer_id
GROUP BY c.customer_id,
    c.name,
    c.email
ORDER BY total_spent DESC;
-- ============================================================================
-- 5. UPDATE OPERATIONS
-- ============================================================================
SELECT '' AS '';
SELECT '5. UPDATE OPERATIONS' AS '';
SELECT '------------------------------------------------------------' AS '';
-- Update single record
UPDATE products
SET price = 34.99
WHERE product_id = 2;
SELECT '5.1 Updated product price' AS '';
-- Update with calculation
UPDATE products
SET price = price * 0.9
WHERE category = 'Accessories';
SELECT '5.2 Applied 10% discount to Accessories' AS '';
-- Update with subquery
UPDATE orders o
SET total_amount = (
        SELECT SUM(oi.quantity * oi.unit_price)
        FROM order_items oi
        WHERE oi.order_id = o.order_id
    )
WHERE o.order_id IN (1, 2, 3);
SELECT '5.3 Recalculated order totals' AS '';
-- ============================================================================
-- 6. DELETE OPERATIONS
-- ============================================================================
SELECT '' AS '';
SELECT '6. DELETE OPERATIONS' AS '';
SELECT '------------------------------------------------------------' AS '';
-- Insert test record
INSERT INTO customers (name, email, city, state)
VALUES (
        'Test User',
        'test@example.com',
        'Test City',
        'TS'
    );
-- Delete single record
DELETE FROM customers
WHERE email = 'test@example.com';
SELECT '6.1 Deleted test customer' AS '';
-- ============================================================================
-- 7. SUBQUERIES
-- ============================================================================
SELECT '' AS '';
SELECT '7. SUBQUERIES' AS '';
SELECT '------------------------------------------------------------' AS '';
-- Subquery in WHERE
SELECT '' AS '';
SELECT '7.1 Products More Expensive Than Average:' AS '';
SELECT name,
    price
FROM products
WHERE price > (
        SELECT AVG(price)
        FROM products
    )
ORDER BY price DESC;
-- Subquery in SELECT
SELECT '' AS '';
SELECT '7.2 Customers with Order Count:' AS '';
SELECT c.name,
    c.email,
    (
        SELECT COUNT(*)
        FROM orders o
        WHERE o.customer_id = c.customer_id
    ) as order_count
FROM customers c;
-- ============================================================================
-- 8. CASE EXPRESSIONS
-- ============================================================================
SELECT '' AS '';
SELECT '8. CASE EXPRESSIONS' AS '';
SELECT '------------------------------------------------------------' AS '';
SELECT '' AS '';
SELECT '8.1 Product Price Categories:' AS '';
SELECT name,
    price,
    CASE
        WHEN price < 50 THEN 'Budget'
        WHEN price < 200 THEN 'Mid-Range'
        ELSE 'Premium'
    END as price_category
FROM products
ORDER BY price;
-- ============================================================================
-- 9. STRING FUNCTIONS
-- ============================================================================
SELECT '' AS '';
SELECT '9. STRING FUNCTIONS' AS '';
SELECT '------------------------------------------------------------' AS '';
SELECT '' AS '';
SELECT '9.1 String Manipulations:' AS '';
SELECT name,
    UPPER(name) as uppercase,
    LOWER(name) as lowercase,
    LENGTH(name) as name_length,
    CONCAT(name, ' (', category, ')') as full_description
FROM products
LIMIT 3;
-- ============================================================================
-- 10. DATE FUNCTIONS
-- ============================================================================
SELECT '' AS '';
SELECT '10. DATE FUNCTIONS' AS '';
SELECT '------------------------------------------------------------' AS '';
SELECT '' AS '';
SELECT '10.1 Order Date Analysis:' AS '';
SELECT order_id,
    order_date,
    DATE(order_date) as order_day,
    YEAR(order_date) as order_year,
    MONTH(order_date) as order_month,
    DAY(order_date) as order_day_of_month
FROM orders
LIMIT 3;
-- ============================================================================
-- 11. SUMMARY
-- ============================================================================
SELECT '' AS '';
SELECT '11. SUMMARY' AS '';
SELECT '------------------------------------------------------------' AS '';
SELECT (
        SELECT COUNT(*)
        FROM customers
    ) as total_customers,
    (
        SELECT COUNT(*)
        FROM products
    ) as total_products,
    (
        SELECT COUNT(*)
        FROM orders
    ) as total_orders,
    (
        SELECT SUM(total_amount)
        FROM orders
    ) as total_revenue;
SELECT '' AS '';
SELECT '============================================================' AS '';
SELECT 'MySQL Basic SQL Operations Complete!' AS '';
SELECT '============================================================' AS '';