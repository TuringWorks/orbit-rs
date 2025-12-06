-- End-to-end fashion example using SQL, time series, ML, vectors, and GraphRAG
-- Sections:
-- 1) Data model: customers, products, sessions, purchases
-- 2) Engagement analytics (rolling session counts)
-- 3) Purchase propensity training and inference
-- 4) Vector embeddings for product text similarity
-- 5) GraphRAG merchandising policy ingestion and Q&A
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
CREATE TABLE IF NOT EXISTS sessions (
    session_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customers(customer_id),
    ts TIMESTAMP NOT NULL,
    page VARCHAR(50)
);
CREATE TABLE IF NOT EXISTS purchases (
    purchase_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customers(customer_id),
    product_id INTEGER REFERENCES products(product_id),
    ts TIMESTAMP NOT NULL,
    qty INTEGER,
    price DECIMAL(10,2)
);
INSERT INTO customers (name, segment) VALUES ('Lina','premium'),('Tom','standard') ON CONFLICT DO NOTHING;
INSERT INTO products (name, category, description, price) VALUES
  ('Silk Dress','apparel','Elegant silk dress suitable for evening events', 299.00),
  ('Sport Jacket','apparel','Breathable jacket designed for outdoor activity', 149.00),
  ('Leather Boots','footwear','Durable boots crafted from premium leather', 199.00)
ON CONFLICT DO NOTHING;
INSERT INTO sessions (customer_id, ts, page)
SELECT 1, NOW() - (INTERVAL '10 minutes' * n), 'product' FROM GENERATE_SERIES(0, 12) AS n;
INSERT INTO sessions (customer_id, ts, page)
SELECT 2, NOW() - (INTERVAL '15 minutes' * n), 'home' FROM GENERATE_SERIES(0, 8) AS n;
INSERT INTO purchases (customer_id, product_id, ts, qty, price)
SELECT 1, 1, NOW() - INTERVAL '25 minutes', 1, 299.00
UNION ALL SELECT 1, 3, NOW() - INTERVAL '5 minutes', 1, 199.00;
-- Rolling engagement: session counts over last 2 hours
WITH sess AS (
    SELECT customer_id, ts, 1 AS s
    FROM sessions
), win AS (
    SELECT customer_id, ts,
           SUM(s) OVER (PARTITION BY customer_id ORDER BY ts ROWS BETWEEN 7 PRECEDING AND CURRENT ROW) AS sessions_2h
    FROM sess
)
SELECT customer_id, ts, sessions_2h
FROM win
ORDER BY ts DESC
LIMIT 20;
-- Purchase propensity features and training
WITH features AS (
    SELECT c.customer_id,
           COALESCE(COUNT(p.purchase_id),0) AS purchases_24h,
           COALESCE(SUM(p.qty * p.price),0) AS spend_24h,
           CASE WHEN c.segment='premium' THEN 1 ELSE 0 END AS premium,
           CASE WHEN COALESCE(SUM(p.qty * p.price),0) > 250 THEN 1 ELSE 0 END AS high_spend,
           0 AS label
    FROM customers c
    LEFT JOIN purchases p ON p.customer_id = c.customer_id AND p.ts > NOW() - INTERVAL '24 hours'
    GROUP BY c.customer_id, c.segment
)
SELECT ML_TRAIN_MODEL('fashion_propensity_gb','gradient_boosting', ARRAY[purchases_24h, spend_24h, premium, high_spend], label) FROM features;
SELECT customer_id,
       ML_PREDICT('fashion_propensity_gb', ARRAY[purchases_24h, spend_24h, premium, high_spend]) AS propensity
FROM features
ORDER BY propensity DESC;
-- Vector embeddings for product similarity
CREATE EXTENSION IF NOT EXISTS vector;
ALTER TABLE IF NOT EXISTS products ADD COLUMN IF NOT EXISTS embedding vector(384);
UPDATE products p SET embedding = ML_EMBED_TEXT(p.description,'sentence-transformers') WHERE p.embedding IS NULL;
SELECT p2.name, 1 - (p2.embedding <=> p1.embedding) AS similarity
FROM products p1, products p2
WHERE p1.name = 'Silk Dress' AND p2.product_id <> p1.product_id
ORDER BY similarity DESC
LIMIT 5;
-- GraphRAG merchandising policies
SELECT * FROM GRAPHRAG_BUILD('fashion_kg','policy_1','Premium customers receive curated recommendations prioritized by similarity and availability.','{"source":"policy","domain":"fashion"}'::json);
SELECT * FROM GRAPHRAG_BUILD('fashion_kg','policy_2','High-value orders require manual review during promotional periods.','{"source":"policy","domain":"fashion"}'::json);
SELECT * FROM GRAPHRAG_QUERY('fashion_kg','What rules apply to premium customers with high-value orders?',2,2048,'ollama',true);
