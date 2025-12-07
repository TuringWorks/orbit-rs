-- ============================================================================
-- OrbitRS Retail Examples - Cassandra Time-Series Analytics
-- ============================================================================
-- Sales trends, inventory turnover, customer behavior
-- ============================================================================

-- ============================================================================
-- SALES BY DAY
-- ============================================================================

CREATE TABLE sales_by_day (
    product_id UUID,
    sale_date DATE,
    hour INT,
    
    -- Metrics
    units_sold INT,
    revenue DECIMAL,
    discount_amount DECIMAL,
    
    -- Aggregates
    order_count INT,
    unique_customers INT,
    
    PRIMARY KEY ((product_id), sale_date, hour)
) WITH CLUSTERING ORDER BY (sale_date DESC, hour DESC);

-- Sample data
INSERT INTO sales_by_day (product_id, sale_date, hour, units_sold, revenue, discount_amount, order_count, unique_customers)
VALUES (uuid(), '2024-12-06', 10, 15, 1499.85, 150.00, 12, 12);

INSERT INTO sales_by_day (product_id, sale_date, hour, units_sold, revenue, discount_amount, order_count, unique_customers)
VALUES (uuid(), '2024-12-06', 11, 23, 2299.77, 230.00, 18, 17);

-- Query: Daily sales trend
SELECT sale_date, SUM(units_sold) AS total_units, SUM(revenue) AS total_revenue
FROM sales_by_day
WHERE product_id = ? AND sale_date >= '2024-12-01'
GROUP BY sale_date;

-- ============================================================================
-- INVENTORY TURNOVER
-- ============================================================================

CREATE TABLE inventory_turnover (
    warehouse_id UUID,
    sku TEXT,
    date DATE,
    
    -- Stock levels
    opening_stock INT,
    closing_stock INT,
    units_received INT,
    units_sold INT,
    units_adjusted INT,
    
    -- Metrics
    turnover_rate DECIMAL,
    days_of_supply INT,
    
    PRIMARY KEY ((warehouse_id, sku), date)
) WITH CLUSTERING ORDER BY (date DESC);

-- Sample data
INSERT INTO inventory_turnover (warehouse_id, sku, date, opening_stock, closing_stock, units_received, units_sold, turnover_rate, days_of_supply)
VALUES (uuid(), 'SKU-001', '2024-12-06', 150, 135, 0, 15, 0.10, 9);

-- Query: Inventory turnover trend
SELECT date, opening_stock, closing_stock, units_sold, turnover_rate
FROM inventory_turnover
WHERE warehouse_id = ? AND sku = 'SKU-001' AND date >= '2024-11-01'
ORDER BY date DESC;

-- ============================================================================
-- CUSTOMER BEHAVIOR ANALYTICS
-- ============================================================================

CREATE TABLE customer_activity (
    customer_id UUID,
    activity_date DATE,
    activity_hour INT,
    
    -- Activity types
    page_views INT,
    product_views INT,
    cart_adds INT,
    purchases INT,
    
    -- Engagement
    session_duration_seconds INT,
    bounce_rate DECIMAL,
    
    PRIMARY KEY ((customer_id), activity_date, activity_hour)
) WITH CLUSTERING ORDER BY (activity_date DESC, activity_hour DESC);

-- Sample data
INSERT INTO customer_activity (customer_id, activity_date, activity_hour, page_views, product_views, cart_adds, purchases, session_duration_seconds)
VALUES (uuid(), '2024-12-06', 10, 25, 12, 3, 1, 1200);

-- Query: Customer engagement over time
SELECT activity_date, SUM(page_views) AS total_views, SUM(purchases) AS total_purchases
FROM customer_activity
WHERE customer_id = ? AND activity_date >= '2024-11-01'
GROUP BY activity_date;

-- ============================================================================
-- MARKETING CAMPAIGN PERFORMANCE
-- ============================================================================

CREATE TABLE campaign_metrics (
    campaign_id UUID,
    metric_date DATE,
    metric_hour INT,
    
    -- Email metrics
    emails_sent INT,
    emails_opened INT,
    emails_clicked INT,
    
    -- Conversion metrics
    conversions INT,
    revenue DECIMAL,
    
    -- Calculated rates
    open_rate DECIMAL,
    click_rate DECIMAL,
    conversion_rate DECIMAL,
    
    PRIMARY KEY ((campaign_id), metric_date, metric_hour)
) WITH CLUSTERING ORDER BY (metric_date DESC, metric_hour DESC);

-- Sample data
INSERT INTO campaign_metrics (campaign_id, metric_date, metric_hour, emails_sent, emails_opened, emails_clicked, conversions, revenue, open_rate, click_rate, conversion_rate)
VALUES (uuid(), '2024-12-06', 9, 10000, 4500, 1200, 87, 8750.00, 0.45, 0.27, 0.07);

-- Query: Campaign performance trend
SELECT metric_date, SUM(emails_sent) AS sent, SUM(conversions) AS conversions, SUM(revenue) AS revenue
FROM campaign_metrics
WHERE campaign_id = ? AND metric_date >= '2024-12-01'
GROUP BY metric_date;

-- ============================================================================
-- PRODUCT VIEWS ANALYTICS
-- ============================================================================

CREATE TABLE product_views (
    product_id UUID,
    view_date DATE,
    view_hour INT,
    
    -- View metrics
    total_views INT,
    unique_viewers INT,
    avg_view_duration_seconds INT,
    
    -- Conversion
    add_to_cart_count INT,
    purchase_count INT,
    conversion_rate DECIMAL,
    
    PRIMARY KEY ((product_id), view_date, view_hour)
) WITH CLUSTERING ORDER BY (view_date DESC, view_hour DESC);

-- Sample data
INSERT INTO product_views (product_id, view_date, view_hour, total_views, unique_viewers, add_to_cart_count, purchase_count, conversion_rate)
VALUES (uuid(), '2024-12-06', 10, 450, 380, 45, 12, 0.027);

-- Query: Product view trends
SELECT view_date, SUM(total_views) AS views, SUM(purchase_count) AS purchases
FROM product_views
WHERE product_id = ? AND view_date >= '2024-11-01'
GROUP BY view_date;

-- ============================================================================
-- WAREHOUSE EFFICIENCY METRICS
-- ============================================================================

CREATE TABLE warehouse_metrics (
    warehouse_id UUID,
    metric_date DATE,
    
    -- Order fulfillment
    orders_processed INT,
    orders_shipped INT,
    avg_pick_time_minutes DECIMAL,
    avg_pack_time_minutes DECIMAL,
    
    -- Accuracy
    pick_accuracy_rate DECIMAL,
    shipping_accuracy_rate DECIMAL,
    
    -- Capacity
    capacity_utilization_pct DECIMAL,
    
    PRIMARY KEY ((warehouse_id), metric_date)
) WITH CLUSTERING ORDER BY (metric_date DESC);

-- Sample data
INSERT INTO warehouse_metrics (warehouse_id, metric_date, orders_processed, orders_shipped, avg_pick_time_minutes, avg_pack_time_minutes, pick_accuracy_rate, capacity_utilization_pct)
VALUES (uuid(), '2024-12-06', 1250, 1235, 8.5, 5.2, 0.998, 0.75);

-- Query: Warehouse efficiency trend
SELECT metric_date, orders_processed, avg_pick_time_minutes, pick_accuracy_rate
FROM warehouse_metrics
WHERE warehouse_id = ? AND metric_date >= '2024-11-01'
ORDER BY metric_date DESC;

-- ============================================================================
-- CATEGORY PERFORMANCE
-- ============================================================================

CREATE TABLE category_sales (
    category_id UUID,
    sale_date DATE,
    
    -- Sales metrics
    units_sold INT,
    revenue DECIMAL,
    avg_order_value DECIMAL,
    
    -- Customer metrics
    unique_customers INT,
    repeat_customer_pct DECIMAL,
    
    PRIMARY KEY ((category_id), sale_date)
) WITH CLUSTERING ORDER BY (sale_date DESC);

-- Sample data
INSERT INTO category_sales (category_id, sale_date, units_sold, revenue, avg_order_value, unique_customers, repeat_customer_pct)
VALUES (uuid(), '2024-12-06', 345, 34500.00, 100.00, 280, 0.35);

-- Query: Category performance comparison
SELECT sale_date, SUM(revenue) AS total_revenue, AVG(avg_order_value) AS avg_order_value
FROM category_sales
WHERE category_id IN (?, ?, ?) AND sale_date >= '2024-11-01'
GROUP BY sale_date;

-- ============================================================================
-- COMMENTS
-- ============================================================================

-- These time-series tables enable:
-- 1. Real-time analytics dashboards
-- 2. Trend analysis and forecasting
-- 3. Performance monitoring
-- 4. Historical reporting
-- 5. Anomaly detection
