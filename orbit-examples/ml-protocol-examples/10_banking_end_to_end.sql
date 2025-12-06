CREATE TABLE IF NOT EXISTS customers (customer_id SERIAL PRIMARY KEY, name VARCHAR(100), risk_segment VARCHAR(20));
CREATE TABLE IF NOT EXISTS accounts (account_id SERIAL PRIMARY KEY, customer_id INTEGER REFERENCES customers(customer_id), account_type VARCHAR(20), opened_at TIMESTAMP DEFAULT NOW());
CREATE TABLE IF NOT EXISTS merchants (merchant_id SERIAL PRIMARY KEY, merchant_name VARCHAR(100), category VARCHAR(50));
CREATE TABLE IF NOT EXISTS transactions (txn_id SERIAL PRIMARY KEY, account_id INTEGER REFERENCES accounts(account_id), merchant_id INTEGER REFERENCES merchants(merchant_id), amount DECIMAL(10,2), ts TIMESTAMP DEFAULT NOW(), channel VARCHAR(20), description TEXT);
INSERT INTO customers (name, risk_segment) VALUES ('Alice','low'),('Bob','medium') ON CONFLICT DO NOTHING;
INSERT INTO accounts (customer_id, account_type) VALUES (1,'checking'),(1,'credit'),(2,'checking') ON CONFLICT DO NOTHING;
INSERT INTO merchants (merchant_name, category) VALUES ('SuperMart','grocery'),('TechWorld','electronics'),('CafeRio','restaurant') ON CONFLICT DO NOTHING;
INSERT INTO transactions (account_id, merchant_id, amount, ts, channel, description)
SELECT 1,1, 45.20, NOW() - INTERVAL '50 minutes', 'pos', 'grocery purchase'
UNION ALL SELECT 1,2, 799.00, NOW() - INTERVAL '10 minutes', 'online', 'electronics purchase'
UNION ALL SELECT 2,3, 12.75, NOW() - INTERVAL '5 minutes', 'pos', 'coffee';
WITH ts_win AS (
  SELECT account_id, ts, amount,
         SUM(amount) OVER (PARTITION BY account_id ORDER BY ts ROWS BETWEEN 59 PRECEDING AND CURRENT ROW) AS sum_60m,
         COUNT(*) OVER (PARTITION BY account_id ORDER BY ts ROWS BETWEEN 59 PRECEDING AND CURRENT ROW) AS count_60m
  FROM transactions
)
SELECT account_id, ts, amount, sum_60m, count_60m FROM ts_win ORDER BY ts DESC LIMIT 20;
WITH fraud_features AS (
  SELECT t.txn_id, t.amount, EXTRACT(HOUR FROM t.ts) AS hour, m.category,
         CASE WHEN t.channel='online' THEN 1 ELSE 0 END AS is_online,
         CASE WHEN m.category IN ('electronics','luxury') THEN 1 ELSE 0 END AS high_risk_cat,
         CASE WHEN t.amount > 500 THEN 1 ELSE 0 END AS large_amount,
         0 AS label
  FROM transactions t JOIN merchants m ON t.merchant_id=m.merchant_id
)
SELECT ML_TRAIN_MODEL('bank_fraud_rf','random_forest', ARRAY[is_online, high_risk_cat, large_amount, hour], label) FROM fraud_features;
SELECT txn_id, amount,
       ML_PREDICT('bank_fraud_rf', ARRAY[is_online, high_risk_cat, large_amount, hour]) AS fraud_score
FROM fraud_features ORDER BY fraud_score DESC;
WITH credit_features AS (
  SELECT c.customer_id, COALESCE(SUM(t.amount),0) AS monthly_spend, COUNT(t.txn_id) AS txn_count,
         CASE WHEN c.risk_segment='low' THEN 1 WHEN c.risk_segment='medium' THEN 0.5 ELSE 0 END AS segment_score,
         CASE WHEN COUNT(t.txn_id)>10 THEN 1 ELSE 0 END AS activity_high,
         0 AS label
  FROM customers c
  LEFT JOIN accounts a ON a.customer_id=c.customer_id
  LEFT JOIN transactions t ON t.account_id=a.account_id AND t.ts> NOW()- INTERVAL '30 days'
  GROUP BY c.customer_id, c.risk_segment
)
SELECT ML_TRAIN_MODEL('credit_score_gb','gradient_boosting', ARRAY[monthly_spend, txn_count, segment_score, activity_high], label) FROM credit_features;
SELECT customer_id,
       ML_PREDICT('credit_score_gb', ARRAY[monthly_spend, txn_count, segment_score, activity_high]) AS credit_score
FROM credit_features ORDER BY credit_score DESC;
SELECT * FROM GRAPHRAG_BUILD('banking_kg','policy_1','Online electronics purchases over $500 require secondary verification.','{"source":"policy","domain":"banking"}'::json);
SELECT * FROM GRAPHRAG_BUILD('banking_kg','policy_2','Transactions in luxury categories after midnight are flagged for review.','{"source":"policy","domain":"banking"}'::json);
SELECT * FROM GRAPHRAG_QUERY('banking_kg','Explain rules affecting high-value online purchases.',2,2048,'ollama',true);
