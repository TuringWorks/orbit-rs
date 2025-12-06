-- ============================================================================
-- Retail & E-Commerce ML Industry Example - Orbit-RS
-- ============================================================================
-- Use Case: Product Recommendations, Demand Forecasting, Customer Segmentation,
--           Dynamic Pricing, Inventory Optimization
-- Protocols: PostgreSQL (SQL), Vector Search, Time Series
-- ============================================================================

-- ----------------------------------------------------------------------------
-- 1. PRODUCT RECOMMENDATION ENGINE
-- ----------------------------------------------------------------------------

-- Create products table with embeddings
CREATE TABLE IF NOT EXISTS products (
    product_id SERIAL PRIMARY KEY,
    product_name VARCHAR(200) NOT NULL,
    category VARCHAR(100),
    subcategory VARCHAR(100),
    brand VARCHAR(100),
    price DECIMAL(10, 2),
    description TEXT,
    product_embedding vector(256), -- Product feature embedding from description/images
    avg_rating FLOAT,
    num_reviews INTEGER,
    stock_quantity INTEGER
);

-- Insert sample products
INSERT INTO products (product_name, category, subcategory, brand, price, description, product_embedding, avg_rating, num_reviews, stock_quantity) VALUES
('iPhone 15 Pro', 'Electronics', 'Smartphones', 'Apple', 999.99, 'Latest iPhone with A17 Pro chip', 
 array_fill(random()::float, ARRAY[256])::vector(256), 4.8, 1250, 500),
('Samsung Galaxy S24', 'Electronics', 'Smartphones', 'Samsung', 899.99, 'Flagship Android phone with AI features',
 array_fill(random()::float, ARRAY[256])::vector(256), 4.6, 980, 350),
('MacBook Pro 16"', 'Electronics', 'Laptops', 'Apple', 2499.99, 'Professional laptop with M3 Max chip',
 array_fill(random()::float, ARRAY[256])::vector(256), 4.9, 750, 120),
('Nike Air Max', 'Fashion', 'Shoes', 'Nike', 129.99, 'Comfortable running shoes',
 array_fill(random()::float, ARRAY[256])::vector(256), 4.5, 2100, 800),
('Levi''s 501 Jeans', 'Fashion', 'Clothing', 'Levi''s', 69.99, 'Classic straight fit jeans',
 array_fill(random()::float, ARRAY[256])::vector(256), 4.4, 1500, 600);

-- Create customer purchase history
CREATE TABLE IF NOT EXISTS customer_purchases (
    purchase_id SERIAL PRIMARY KEY,
    customer_id INTEGER NOT NULL,
    product_id INTEGER REFERENCES products(product_id),
    purchase_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    quantity INTEGER DEFAULT 1,
    purchase_price DECIMAL(10, 2)
);

-- Insert sample purchase history
INSERT INTO customer_purchases (customer_id, product_id, purchase_date, quantity, purchase_price) VALUES
(1001, 1, CURRENT_TIMESTAMP - INTERVAL '30 days', 1, 999.99),
(1001, 4, CURRENT_TIMESTAMP - INTERVAL '15 days', 2, 129.99),
(1002, 2, CURRENT_TIMESTAMP - INTERVAL '20 days', 1, 899.99),
(1002, 5, CURRENT_TIMESTAMP - INTERVAL '10 days', 1, 69.99),
(1003, 3, CURRENT_TIMESTAMP - INTERVAL '5 days', 1, 2499.99);

-- Create vector index for fast similarity search
CREATE INDEX IF NOT EXISTS products_embedding_idx 
ON products USING ivfflat (product_embedding vector_cosine_ops);

-- Recommend similar products based on purchase history
-- Find products similar to what customer has bought
WITH customer_products AS (
    SELECT DISTINCT p.product_embedding, p.product_id
    FROM customer_purchases cp
    JOIN products p ON cp.product_id = p.product_id
    WHERE cp.customer_id = 1001
)
SELECT 
    p.product_id,
    p.product_name,
    p.category,
    p.brand,
    p.price,
    p.avg_rating,
    1 - (p.product_embedding <=> cp.product_embedding) AS similarity_score
FROM products p
CROSS JOIN customer_products cp
WHERE p.product_id != cp.product_id
  AND p.stock_quantity > 0
ORDER BY p.product_embedding <=> cp.product_embedding
LIMIT 5;

-- Collaborative filtering - recommend products bought by similar customers
WITH customer_vector AS (
    SELECT 
        customer_id,
        array_agg(product_id ORDER BY product_id) AS purchased_products
    FROM customer_purchases
    GROUP BY customer_id
)
SELECT 
    cv2.customer_id AS similar_customer,
    p.product_id,
    p.product_name,
    p.price,
    COUNT(*) AS purchase_frequency
FROM customer_vector cv1
JOIN customer_vector cv2 ON cv1.customer_id != cv2.customer_id
JOIN customer_purchases cp ON cv2.customer_id = cp.customer_id
JOIN products p ON cp.product_id = p.product_id
WHERE cv1.customer_id = 1001
  AND NOT (p.product_id = ANY(cv1.purchased_products))
GROUP BY cv2.customer_id, p.product_id, p.product_name, p.price
ORDER BY purchase_frequency DESC
LIMIT 5;

-- ----------------------------------------------------------------------------
-- 2. CUSTOMER SEGMENTATION WITH ML
-- ----------------------------------------------------------------------------

-- Create customer profiles table
CREATE TABLE IF NOT EXISTS customer_profiles (
    customer_id INTEGER PRIMARY KEY,
    name VARCHAR(100),
    email VARCHAR(100),
    registration_date DATE,
    total_purchases INTEGER DEFAULT 0,
    total_spent DECIMAL(12, 2) DEFAULT 0,
    avg_order_value DECIMAL(10, 2),
    days_since_last_purchase INTEGER,
    favorite_category VARCHAR(100),
    customer_embedding vector(128), -- Behavioral embedding
    segment VARCHAR(50), -- 'VIP', 'Regular', 'At-Risk', 'New'
    lifetime_value DECIMAL(12, 2)
);

-- Insert sample customer profiles
INSERT INTO customer_profiles (customer_id, name, email, registration_date, total_purchases, total_spent, 
                               days_since_last_purchase, favorite_category, customer_embedding) VALUES
(1001, 'John Doe', 'john@example.com', '2023-01-15', 15, 3500.00, 5, 'Electronics', 
 array_fill(random()::float, ARRAY[128])::vector(128)),
(1002, 'Jane Smith', 'jane@example.com', '2023-06-20', 8, 1200.00, 10, 'Fashion',
 array_fill(random()::float, ARRAY[128])::vector(128)),
(1003, 'Bob Johnson', 'bob@example.com', '2024-01-10', 2, 2600.00, 5, 'Electronics',
 array_fill(random()::float, ARRAY[128])::vector(128)),
(1004, 'Alice Williams', 'alice@example.com', '2022-03-05', 45, 12000.00, 2, 'Fashion',
 array_fill(random()::float, ARRAY[128])::vector(128));

-- Calculate customer metrics
UPDATE customer_profiles cp
SET 
    avg_order_value = cp.total_spent / NULLIF(cp.total_purchases, 0),
    lifetime_value = cp.total_spent * (1 + (365.0 / NULLIF(CURRENT_DATE - cp.registration_date, 0)));

-- ML-based customer segmentation
UPDATE customer_profiles
SET segment = CASE
    WHEN total_spent > 10000 AND days_since_last_purchase < 30 THEN 'VIP'
    WHEN total_spent > 5000 AND days_since_last_purchase < 60 THEN 'High Value'
    WHEN days_since_last_purchase > 90 THEN 'At-Risk'
    WHEN total_purchases < 3 THEN 'New'
    ELSE 'Regular'
END;

-- Find similar customers for targeted marketing
SELECT 
    c1.customer_id,
    c1.name,
    c1.segment,
    c2.customer_id AS similar_customer_id,
    c2.name AS similar_customer_name,
    c2.segment AS similar_segment,
    1 - (c1.customer_embedding <=> c2.customer_embedding) AS similarity
FROM customer_profiles c1
CROSS JOIN customer_profiles c2
WHERE c1.customer_id = 1001 AND c2.customer_id != 1001
ORDER BY c1.customer_embedding <=> c2.customer_embedding
LIMIT 5;

-- Customer segment analysis
SELECT 
    segment,
    COUNT(*) AS num_customers,
    AVG(total_spent) AS avg_total_spent,
    AVG(total_purchases) AS avg_purchases,
    AVG(days_since_last_purchase) AS avg_days_since_purchase,
    SUM(total_spent) AS segment_revenue
FROM customer_profiles
GROUP BY segment
ORDER BY segment_revenue DESC;

ALTER TABLE customer_profiles ADD COLUMN IF NOT EXISTS is_vip BOOLEAN;
UPDATE customer_profiles SET is_vip = (segment = 'VIP');
SELECT ML_TRAIN_MODEL(
  'vip_classifier_lr',
  'logistic_regression',
  ARRAY[
    total_spent,
    total_purchases,
    avg_order_value,
    days_since_last_purchase
  ],
  is_vip
) FROM customer_profiles;
UPDATE customer_profiles
SET segment = CASE
  WHEN ML_PREDICT(
    'vip_classifier_lr',
    ARRAY[
      total_spent,
      total_purchases,
      avg_order_value,
      days_since_last_purchase
    ]
  ) > 0.5 THEN 'VIP'
  ELSE segment
END;

-- ----------------------------------------------------------------------------
-- 3. DEMAND FORECASTING - TIME SERIES
-- ----------------------------------------------------------------------------

-- Create sales time series table
CREATE TABLE IF NOT EXISTS daily_sales (
    sale_id SERIAL PRIMARY KEY,
    product_id INTEGER REFERENCES products(product_id),
    sale_date DATE NOT NULL,
    units_sold INTEGER NOT NULL,
    revenue DECIMAL(12, 2),
    day_of_week INTEGER, -- 0=Sunday, 6=Saturday
    is_weekend BOOLEAN,
    is_holiday BOOLEAN DEFAULT FALSE,
    promotion_active BOOLEAN DEFAULT FALSE
);

-- Insert sample sales data (last 30 days)
INSERT INTO daily_sales (product_id, sale_date, units_sold, revenue, day_of_week, is_weekend, promotion_active)
SELECT 
    1, -- iPhone 15 Pro
    CURRENT_DATE - (n || ' days')::INTERVAL,
    (random() * 50 + 10)::INTEGER,
    (random() * 50 + 10)::INTEGER * 999.99,
    EXTRACT(DOW FROM CURRENT_DATE - (n || ' days')::INTERVAL)::INTEGER,
    EXTRACT(DOW FROM CURRENT_DATE - (n || ' days')::INTERVAL) IN (0, 6),
    (random() > 0.8)
FROM generate_series(1, 30) AS n;

-- Calculate moving averages for demand forecasting
SELECT 
    product_id,
    sale_date,
    units_sold,
    AVG(units_sold) OVER (
        PARTITION BY product_id 
        ORDER BY sale_date 
        ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
    ) AS ma_7day,
    AVG(units_sold) OVER (
        PARTITION BY product_id 
        ORDER BY sale_date 
        ROWS BETWEEN 29 PRECEDING AND CURRENT ROW
    ) AS ma_30day,
    STDDEV(units_sold) OVER (
        PARTITION BY product_id 
        ORDER BY sale_date 
        ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
    ) AS volatility_7day
FROM daily_sales
WHERE product_id = 1
ORDER BY sale_date DESC
LIMIT 14;

-- Detect sales trends and seasonality
WITH sales_trends AS (
    SELECT 
        product_id,
        sale_date,
        units_sold,
        AVG(units_sold) OVER (
            PARTITION BY product_id 
            ORDER BY sale_date 
            ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
        ) AS ma_7day,
        LAG(AVG(units_sold) OVER (
            PARTITION BY product_id 
            ORDER BY sale_date 
            ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
        )) OVER (PARTITION BY product_id ORDER BY sale_date) AS prev_ma_7day
    FROM daily_sales
)
SELECT 
    product_id,
    sale_date,
    units_sold,
    ma_7day,
    CASE 
        WHEN ma_7day > prev_ma_7day * 1.1 THEN 'STRONG UPTREND'
        WHEN ma_7day > prev_ma_7day THEN 'UPTREND'
        WHEN ma_7day < prev_ma_7day * 0.9 THEN 'STRONG DOWNTREND'
        WHEN ma_7day < prev_ma_7day THEN 'DOWNTREND'
        ELSE 'STABLE'
    END AS trend
FROM sales_trends
WHERE product_id = 1
ORDER BY sale_date DESC
LIMIT 7;

-- Weekend vs weekday sales analysis
SELECT 
    product_id,
    is_weekend,
    COUNT(*) AS num_days,
    AVG(units_sold) AS avg_units_sold,
    SUM(revenue) AS total_revenue
FROM daily_sales
GROUP BY product_id, is_weekend
ORDER BY product_id, is_weekend;

-- ----------------------------------------------------------------------------
-- 4. DYNAMIC PRICING OPTIMIZATION
-- ----------------------------------------------------------------------------

-- Create pricing history table
CREATE TABLE IF NOT EXISTS pricing_history (
    pricing_id SERIAL PRIMARY KEY,
    product_id INTEGER REFERENCES products(product_id),
    price DECIMAL(10, 2) NOT NULL,
    effective_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    competitor_avg_price DECIMAL(10, 2),
    demand_level VARCHAR(20), -- 'low', 'medium', 'high'
    inventory_level INTEGER,
    conversion_rate FLOAT
);

-- ML-based dynamic pricing function
CREATE OR REPLACE FUNCTION calculate_optimal_price(
    p_base_price DECIMAL,
    p_competitor_price DECIMAL,
    p_demand_level VARCHAR,
    p_inventory_level INTEGER,
    p_stock_quantity INTEGER
) RETURNS DECIMAL AS $$
DECLARE
    optimal_price DECIMAL;
    demand_multiplier FLOAT;
    inventory_multiplier FLOAT;
    competition_factor FLOAT;
BEGIN
    -- Base price
    optimal_price := p_base_price;
    
    -- Demand adjustment
    demand_multiplier := CASE p_demand_level
        WHEN 'high' THEN 1.15
        WHEN 'medium' THEN 1.0
        WHEN 'low' THEN 0.90
        ELSE 1.0
    END;
    
    -- Inventory adjustment (clearance pricing)
    inventory_multiplier := CASE
        WHEN p_inventory_level > p_stock_quantity * 0.8 THEN 0.85 -- Overstocked
        WHEN p_inventory_level < p_stock_quantity * 0.2 THEN 1.10 -- Low stock
        ELSE 1.0
    END;
    
    -- Competition factor
    IF p_competitor_price IS NOT NULL THEN
        competition_factor := LEAST(1.05, p_competitor_price / p_base_price);
    ELSE
        competition_factor := 1.0;
    END IF;
    
    optimal_price := p_base_price * demand_multiplier * inventory_multiplier * competition_factor;
    
    RETURN ROUND(optimal_price, 2);
END;
$$ LANGUAGE plpgsql;

-- Apply dynamic pricing
SELECT 
    p.product_id,
    p.product_name,
    p.price AS current_price,
    p.stock_quantity,
    calculate_optimal_price(
        p.price,
        p.price * (0.95 + random() * 0.1), -- Simulated competitor price
        CASE 
            WHEN p.num_reviews > 1000 THEN 'high'
            WHEN p.num_reviews > 500 THEN 'medium'
            ELSE 'low'
        END,
        p.stock_quantity,
        p.stock_quantity
    ) AS optimal_price,
    ((calculate_optimal_price(
        p.price,
        p.price * (0.95 + random() * 0.1),
        CASE 
            WHEN p.num_reviews > 1000 THEN 'high'
            WHEN p.num_reviews > 500 THEN 'medium'
            ELSE 'low'
        END,
        p.stock_quantity,
        p.stock_quantity
    ) - p.price) / p.price * 100) AS price_change_pct
FROM products p
WHERE p.stock_quantity > 0;

-- ----------------------------------------------------------------------------
-- 5. INVENTORY OPTIMIZATION
-- ----------------------------------------------------------------------------

-- Create inventory movements table
CREATE TABLE IF NOT EXISTS inventory_movements (
    movement_id SERIAL PRIMARY KEY,
    product_id INTEGER REFERENCES products(product_id),
    movement_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    movement_type VARCHAR(20), -- 'INBOUND', 'OUTBOUND', 'ADJUSTMENT'
    quantity INTEGER,
    reason VARCHAR(100)
);

-- Calculate reorder points using ML
CREATE OR REPLACE FUNCTION calculate_reorder_point(
    p_avg_daily_sales FLOAT,
    p_lead_time_days INTEGER,
    p_safety_stock_days INTEGER
) RETURNS INTEGER AS $$
BEGIN
    RETURN CEIL(p_avg_daily_sales * (p_lead_time_days + p_safety_stock_days));
END;
$$ LANGUAGE plpgsql;

-- Inventory analysis with reorder recommendations
WITH sales_velocity AS (
    SELECT 
        product_id,
        AVG(units_sold) AS avg_daily_sales,
        STDDEV(units_sold) AS sales_volatility
    FROM daily_sales
    WHERE sale_date > CURRENT_DATE - INTERVAL '30 days'
    GROUP BY product_id
)
SELECT 
    p.product_id,
    p.product_name,
    p.stock_quantity AS current_stock,
    sv.avg_daily_sales,
    sv.sales_volatility,
    calculate_reorder_point(sv.avg_daily_sales, 7, 3) AS reorder_point,
    CASE 
        WHEN p.stock_quantity < calculate_reorder_point(sv.avg_daily_sales, 7, 3) 
        THEN 'REORDER NOW'
        WHEN p.stock_quantity < calculate_reorder_point(sv.avg_daily_sales, 7, 3) * 1.5 
        THEN 'REORDER SOON'
        ELSE 'SUFFICIENT'
    END AS inventory_status,
    CEIL(calculate_reorder_point(sv.avg_daily_sales, 7, 3) * 2 - p.stock_quantity) AS suggested_order_qty
FROM products p
JOIN sales_velocity sv ON p.product_id = sv.product_id
ORDER BY 
    CASE 
        WHEN p.stock_quantity < calculate_reorder_point(sv.avg_daily_sales, 7, 3) THEN 1
        WHEN p.stock_quantity < calculate_reorder_point(sv.avg_daily_sales, 7, 3) * 1.5 THEN 2
        ELSE 3
    END;

-- ----------------------------------------------------------------------------
-- 6. SHOPPING CART ABANDONMENT PREDICTION
-- ----------------------------------------------------------------------------

-- Create shopping carts table
CREATE TABLE IF NOT EXISTS shopping_carts (
    cart_id SERIAL PRIMARY KEY,
    customer_id INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    total_items INTEGER DEFAULT 0,
    total_value DECIMAL(10, 2) DEFAULT 0,
    is_abandoned BOOLEAN DEFAULT FALSE,
    abandonment_probability FLOAT,
    session_duration_minutes INTEGER,
    num_page_views INTEGER
);

-- Insert sample cart data
INSERT INTO shopping_carts (customer_id, created_at, last_updated, total_items, total_value, 
                            session_duration_minutes, num_page_views) VALUES
(1001, CURRENT_TIMESTAMP - INTERVAL '2 hours', CURRENT_TIMESTAMP - INTERVAL '1 hour 45 minutes', 
 3, 450.00, 15, 25),
(1002, CURRENT_TIMESTAMP - INTERVAL '30 minutes', CURRENT_TIMESTAMP - INTERVAL '5 minutes',
 1, 999.99, 45, 50),
(1003, CURRENT_TIMESTAMP - INTERVAL '3 days', CURRENT_TIMESTAMP - INTERVAL '3 days',
 2, 200.00, 5, 8);

-- ML-based cart abandonment prediction
CREATE OR REPLACE FUNCTION predict_cart_abandonment(
    p_minutes_since_update INTEGER,
    p_total_value DECIMAL,
    p_session_duration INTEGER,
    p_num_page_views INTEGER,
    p_total_items INTEGER
) RETURNS FLOAT AS $$
DECLARE
    abandonment_score FLOAT := 0.0;
BEGIN
    -- Time since last update
    IF p_minutes_since_update > 60 THEN abandonment_score := abandonment_score + 0.4; END IF;
    IF p_minutes_since_update > 120 THEN abandonment_score := abandonment_score + 0.2; END IF;
    
    -- High cart value (price shock)
    IF p_total_value > 500 THEN abandonment_score := abandonment_score + 0.15; END IF;
    
    -- Short session (not engaged)
    IF p_session_duration < 10 THEN abandonment_score := abandonment_score + 0.15; END IF;
    
    -- Few page views (not browsing)
    IF p_num_page_views < 10 THEN abandonment_score := abandonment_score + 0.1; END IF;
    
    RETURN LEAST(abandonment_score, 1.0);
END;
$$ LANGUAGE plpgsql;

-- Update abandonment probabilities
UPDATE shopping_carts
SET abandonment_probability = predict_cart_abandonment(
    EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - last_updated))::INTEGER / 60,
    total_value,
    session_duration_minutes,
    num_page_views,
    total_items
);

-- Identify carts for recovery campaigns
SELECT 
    cart_id,
    customer_id,
    total_items,
    total_value,
    EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - last_updated))::INTEGER / 60 AS minutes_inactive,
    abandonment_probability,
    CASE 
        WHEN abandonment_probability > 0.7 THEN 'Send discount code immediately'
        WHEN abandonment_probability > 0.5 THEN 'Send reminder email'
        WHEN abandonment_probability > 0.3 THEN 'Monitor'
        ELSE 'Active'
    END AS recovery_action
FROM shopping_carts
WHERE is_abandoned = FALSE
  AND abandonment_probability > 0.3
ORDER BY abandonment_probability DESC, total_value DESC;

-- ----------------------------------------------------------------------------
-- 7. PRODUCT REVIEW SENTIMENT ANALYSIS
-- ----------------------------------------------------------------------------

-- Create product reviews table
CREATE TABLE IF NOT EXISTS product_reviews (
    review_id SERIAL PRIMARY KEY,
    product_id INTEGER REFERENCES products(product_id),
    customer_id INTEGER,
    rating INTEGER CHECK (rating BETWEEN 1 AND 5),
    review_text TEXT,
    review_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    sentiment_score FLOAT, -- -1.0 (negative) to 1.0 (positive)
    sentiment_label VARCHAR(20), -- 'positive', 'neutral', 'negative'
    is_verified_purchase BOOLEAN DEFAULT FALSE
);

-- Insert sample reviews
INSERT INTO product_reviews (product_id, customer_id, rating, review_text, is_verified_purchase) VALUES
(1, 1001, 5, 'Amazing phone! The camera quality is outstanding and battery life is excellent.', TRUE),
(1, 1002, 4, 'Great phone but a bit expensive. Worth it for the features though.', TRUE),
(2, 1003, 3, 'Good phone but has some software bugs. Customer service was helpful.', TRUE),
(4, 1004, 5, 'Most comfortable running shoes I have ever owned! Highly recommend.', TRUE),
(5, 1001, 2, 'Quality is not as good as expected. Fabric feels cheap.', TRUE);

-- Simple sentiment analysis (in production, use NLP model)
UPDATE product_reviews
SET 
    sentiment_score = (rating - 3.0) / 2.0, -- Normalize rating to -1 to 1
    sentiment_label = CASE 
        WHEN rating >= 4 THEN 'positive'
        WHEN rating = 3 THEN 'neutral'
        ELSE 'negative'
    END;

-- Aggregate sentiment by product
SELECT 
    p.product_id,
    p.product_name,
    COUNT(pr.review_id) AS num_reviews,
    AVG(pr.rating) AS avg_rating,
    AVG(pr.sentiment_score) AS avg_sentiment,
    SUM(CASE WHEN pr.sentiment_label = 'positive' THEN 1 ELSE 0 END)::FLOAT / COUNT(*) * 100 AS positive_pct,
    SUM(CASE WHEN pr.sentiment_label = 'negative' THEN 1 ELSE 0 END)::FLOAT / COUNT(*) * 100 AS negative_pct
FROM products p
LEFT JOIN product_reviews pr ON p.product_id = pr.product_id
GROUP BY p.product_id, p.product_name
HAVING COUNT(pr.review_id) > 0
ORDER BY avg_sentiment DESC;

-- ============================================================================
-- SUMMARY: Retail & E-Commerce ML Use Cases Demonstrated
-- ============================================================================
-- 1. Product recommendations using vector similarity search
-- 2. Customer segmentation with behavioral embeddings
-- 3. Demand forecasting with time series analysis
-- 4. Dynamic pricing optimization based on demand and inventory
-- 5. Inventory optimization with reorder point calculations
-- 6. Shopping cart abandonment prediction
-- 7. Product review sentiment analysis
-- ============================================================================
