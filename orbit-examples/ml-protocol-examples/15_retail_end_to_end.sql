-- End-to-end retail example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: customers, products, orders, order_items
-- 2) Seeding sample data
-- 3) Rolling revenue/time-window analytics
-- 4) Purchase propensity classification: training and inference
-- 5) Vector embeddings for product similarity
-- 6) GraphRAG policy ingestion and Q&A
CREATE TABLE IF NOT EXISTS customers (
    customer_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    segment VARCHAR(20)
);
CREATE TABLE IF NOT EXISTS products (
    product_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    category VARCHAR(50),
    description TEXT,
    price DECIMAL(10,2)
);
CREATE TABLE IF NOT EXISTS orders (
    order_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customers(customer_id),
    ts TIMESTAMP DEFAULT NOW()
);
CREATE TABLE IF NOT EXISTS order_items (
    order_id INTEGER REFERENCES orders(order_id),
    product_id INTEGER REFERENCES products(product_id),
    qty INTEGER,
    price DECIMAL(10,2)
);
-- Seed customers and products
INSERT INTO customers (name, segment) VALUES ('Rita','vip'),('Sam','standard') ON CONFLICT DO NOTHING;
INSERT INTO products (name, category, description, price) VALUES
  ('4K TV','electronics','High-resolution television ideal for home theaters', 799.00),
  ('Espresso Machine','kitchen','Compact espresso maker for coffee enthusiasts', 249.00),
  ('Running Shoes','fashion','Lightweight shoes suitable for daily running', 129.00)
ON CONFLICT DO NOTHING;
-- Seed orders and items
INSERT INTO orders (customer_id, ts) VALUES (1, NOW() - INTERVAL '45 minutes'), (1, NOW() - INTERVAL '10 minutes'), (2, NOW() - INTERVAL '5 minutes');
INSERT INTO order_items (order_id, product_id, qty, price)
SELECT 1, 2, 1, 249.00
UNION ALL SELECT 2, 1, 1, 799.00
UNION ALL SELECT 3, 3, 1, 129.00;
-- Rolling 60-minute revenue per customer and overall
WITH item_revenue AS (
    SELECT o.customer_id, o.ts, (oi.qty * oi.price) AS revenue
    FROM orders o JOIN order_items oi ON oi.order_id = o.order_id
), rev_win AS (
    SELECT customer_id, ts, revenue,
           SUM(revenue) OVER (PARTITION BY customer_id ORDER BY ts ROWS BETWEEN 59 PRECEDING AND CURRENT ROW) AS cust_rev_60m,
           SUM(revenue) OVER (ORDER BY ts ROWS BETWEEN 59 PRECEDING AND CURRENT ROW) AS total_rev_60m
    FROM item_revenue
)
SELECT customer_id, ts, revenue, cust_rev_60m, total_rev_60m
FROM rev_win
ORDER BY ts DESC
LIMIT 20;
-- Feature engineering: recent activity and segment for purchase propensity
WITH cust_features AS (
    SELECT c.customer_id,
           COALESCE(COUNT(o.order_id),0) AS orders_last_24h,
           COALESCE(SUM(oi.qty * oi.price),0) AS revenue_last_24h,
           CASE WHEN c.segment = 'vip' THEN 1 ELSE 0 END AS vip_segment,
           CASE WHEN COALESCE(SUM(oi.qty * oi.price),0) > 300 THEN 1 ELSE 0 END AS high_spend,
           0 AS label
    FROM customers c
    LEFT JOIN orders o ON o.customer_id = c.customer_id AND o.ts > NOW() - INTERVAL '24 hours'
    LEFT JOIN order_items oi ON oi.order_id = o.order_id
    GROUP BY c.customer_id, c.segment
)
-- Train gradient boosting classifier for purchase propensity
SELECT ML_TRAIN_MODEL('retail_propensity_gb','gradient_boosting', ARRAY[orders_last_24h, revenue_last_24h, vip_segment, high_spend], label) FROM cust_features;
-- Predict propensity scores
SELECT customer_id,
       ML_PREDICT('retail_propensity_gb', ARRAY[orders_last_24h, revenue_last_24h, vip_segment, high_spend]) AS propensity_score
FROM cust_features
ORDER BY propensity_score DESC;
-- Vector embeddings for product similarity
CREATE EXTENSION IF NOT EXISTS vector;
ALTER TABLE IF NOT EXISTS products ADD COLUMN IF NOT EXISTS embedding vector(384);
UPDATE products p SET embedding = ML_EMBED_TEXT(p.description,'sentence-transformers') WHERE p.embedding IS NULL;
-- Cosine similarity: find products similar to 'Espresso Machine'
SELECT p2.name, 1 - (p2.embedding <=> p1.embedding) AS similarity
FROM products p1, products p2
WHERE p1.name = 'Espresso Machine' AND p2.product_id <> p1.product_id
ORDER BY similarity DESC
LIMIT 5;
-- GraphRAG: ingest retail policies and ask a question
SELECT * FROM GRAPHRAG_BUILD('retail_kg','policy_1','High-value carts above $500 require address verification before shipping.','{"source":"policy","domain":"retail"}'::json);
SELECT * FROM GRAPHRAG_BUILD('retail_kg','policy_2','VIP customers receive free expedited shipping unless flagged by risk systems.','{"source":"policy","domain":"retail"}'::json);
SELECT * FROM GRAPHRAG_QUERY('retail_kg','What rules apply to VIP customers with high-value carts?',2,2048,'ollama',true);
