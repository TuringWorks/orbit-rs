-- ============================================================================
-- Finance & Banking ML Industry Example - Orbit-RS
-- ============================================================================
-- Use Case: Fraud Detection, Credit Scoring, Algorithmic Trading, Risk Management
-- Protocols: PostgreSQL (SQL), Vector Search, Time Series
-- ============================================================================

-- ----------------------------------------------------------------------------
-- 1. FRAUD DETECTION WITH ML
-- ----------------------------------------------------------------------------

-- Create transactions table
CREATE TABLE IF NOT EXISTS transactions (
    transaction_id SERIAL PRIMARY KEY,
    account_id INTEGER NOT NULL,
    transaction_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    amount DECIMAL(15, 2) NOT NULL,
    merchant_name VARCHAR(200),
    merchant_category VARCHAR(50),
    location VARCHAR(100),
    transaction_type VARCHAR(50), -- 'purchase', 'withdrawal', 'transfer'
    is_online BOOLEAN DEFAULT FALSE,
    device_fingerprint VARCHAR(100),
    ip_address INET,
    fraud_score FLOAT,
    is_fraud BOOLEAN DEFAULT FALSE
);

-- Insert sample transactions
INSERT INTO transactions (account_id, amount, merchant_name, merchant_category, location, transaction_type, is_online, fraud_score) VALUES
(1001, 45.99, 'Amazon', 'E-commerce', 'Seattle, WA', 'purchase', TRUE, 0.15),
(1001, 2500.00, 'Best Buy', 'Electronics', 'New York, NY', 'purchase', FALSE, 0.35),
(1002, 15000.00, 'Unknown Merchant', 'Unknown', 'Lagos, Nigeria', 'purchase', TRUE, 0.92),
(1003, 89.50, 'Starbucks', 'Food & Beverage', 'San Francisco, CA', 'purchase', FALSE, 0.08),
(1002, 500.00, 'ATM Withdrawal', 'Cash', 'Moscow, Russia', 'withdrawal', FALSE, 0.78);

-- ML-based fraud detection function
CREATE OR REPLACE FUNCTION detect_fraud(
    p_amount DECIMAL,
    p_is_online BOOLEAN,
    p_merchant_category VARCHAR,
    p_location VARCHAR,
    p_account_avg_transaction DECIMAL
) RETURNS FLOAT AS $$
DECLARE
    fraud_score FLOAT := 0.0;
BEGIN
    -- Amount anomaly
    IF p_amount > p_account_avg_transaction * 5 THEN
        fraud_score := fraud_score + 0.3;
    END IF;
    
    -- High-risk location
    IF p_location ILIKE '%nigeria%' OR p_location ILIKE '%russia%' THEN
        fraud_score := fraud_score + 0.4;
    END IF;
    
    -- Online transaction risk
    IF p_is_online THEN
        fraud_score := fraud_score + 0.1;
    END IF;
    
    -- Unknown merchant
    IF p_merchant_category = 'Unknown' THEN
        fraud_score := fraud_score + 0.2;
    END IF;
    
    RETURN LEAST(fraud_score, 1.0);
END;
$$ LANGUAGE plpgsql;

-- Update fraud scores for all transactions
WITH account_averages AS (
    SELECT account_id, AVG(amount) AS avg_amount
    FROM transactions
    GROUP BY account_id
)
UPDATE transactions t
SET fraud_score = detect_fraud(
    t.amount,
    t.is_online,
    t.merchant_category,
    t.location,
    COALESCE(aa.avg_amount, 100.0)
)
FROM account_averages aa
WHERE t.account_id = aa.account_id;

-- Flag high-risk transactions for review
UPDATE transactions
SET is_fraud = TRUE
WHERE fraud_score > 0.7;

-- Query suspicious transactions
SELECT 
    transaction_id,
    account_id,
    amount,
    merchant_name,
    location,
    fraud_score,
    CASE 
        WHEN fraud_score > 0.8 THEN 'CRITICAL'
        WHEN fraud_score > 0.6 THEN 'HIGH'
        WHEN fraud_score > 0.4 THEN 'MEDIUM'
        ELSE 'LOW'
    END AS risk_level
FROM transactions
WHERE fraud_score > 0.5
ORDER BY fraud_score DESC;

-- ----------------------------------------------------------------------------
-- 2. CREDIT SCORING WITH ML
-- ----------------------------------------------------------------------------

-- Create customer credit profiles
CREATE TABLE IF NOT EXISTS credit_profiles (
    customer_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    age INTEGER,
    annual_income DECIMAL(15, 2),
    employment_years INTEGER,
    num_credit_cards INTEGER,
    total_credit_limit DECIMAL(15, 2),
    credit_utilization FLOAT, -- 0.0 to 1.0
    num_late_payments INTEGER,
    num_bankruptcies INTEGER,
    credit_score INTEGER, -- 300-850
    credit_embedding vector(128) -- ML-generated credit profile embedding
);

-- Insert sample credit profiles
INSERT INTO credit_profiles (name, age, annual_income, employment_years, num_credit_cards, 
                             total_credit_limit, credit_utilization, num_late_payments, 
                             num_bankruptcies, credit_embedding) VALUES
('Alice Johnson', 35, 85000, 8, 3, 25000, 0.35, 0, 0, array_fill(random()::float, ARRAY[128])::vector(128)),
('Bob Smith', 42, 120000, 15, 5, 50000, 0.20, 1, 0, array_fill(random()::float, ARRAY[128])::vector(128)),
('Charlie Brown', 28, 45000, 3, 2, 10000, 0.85, 5, 1, array_fill(random()::float, ARRAY[128])::vector(128)),
('Diana Prince', 50, 150000, 20, 4, 75000, 0.15, 0, 0, array_fill(random()::float, ARRAY[128])::vector(128));

-- ML-based credit score calculation
CREATE OR REPLACE FUNCTION calculate_credit_score(
    p_annual_income DECIMAL,
    p_employment_years INTEGER,
    p_credit_utilization FLOAT,
    p_num_late_payments INTEGER,
    p_num_bankruptcies INTEGER
) RETURNS INTEGER AS $$
DECLARE
    base_score INTEGER := 300;
    score INTEGER;
BEGIN
    score := base_score;
    
    -- Income factor
    score := score + LEAST((p_annual_income / 1000)::INTEGER, 200);
    
    -- Employment stability
    score := score + (p_employment_years * 10);
    
    -- Credit utilization (lower is better)
    score := score + (100 - (p_credit_utilization * 100))::INTEGER;
    
    -- Payment history
    score := score - (p_num_late_payments * 30);
    
    -- Bankruptcies
    score := score - (p_num_bankruptcies * 150);
    
    -- Cap between 300 and 850
    RETURN GREATEST(300, LEAST(850, score));
END;
$$ LANGUAGE plpgsql;

-- Update credit scores
UPDATE credit_profiles
SET credit_score = calculate_credit_score(
    annual_income,
    employment_years,
    credit_utilization,
    num_late_payments,
    num_bankruptcies
);

-- Find similar credit profiles using vector embeddings
-- Useful for peer comparison and risk assessment
SELECT 
    c1.customer_id,
    c1.name,
    c1.credit_score,
    c2.name AS similar_customer,
    c2.credit_score AS similar_score,
    1 - (c1.credit_embedding <=> c2.credit_embedding) AS similarity
FROM credit_profiles c1
CROSS JOIN credit_profiles c2
WHERE c1.customer_id = 1 AND c2.customer_id != 1
ORDER BY c1.credit_embedding <=> c2.credit_embedding
LIMIT 3;

-- ----------------------------------------------------------------------------
-- 3. ALGORITHMIC TRADING - TIME SERIES ANALYSIS
-- ----------------------------------------------------------------------------

-- Create stock prices time series table
CREATE TABLE IF NOT EXISTS stock_prices (
    price_id SERIAL PRIMARY KEY,
    symbol VARCHAR(10) NOT NULL,
    timestamp BIGINT NOT NULL, -- Unix timestamp in milliseconds
    open_price DECIMAL(15, 4),
    high_price DECIMAL(15, 4),
    low_price DECIMAL(15, 4),
    close_price DECIMAL(15, 4),
    volume BIGINT,
    vwap DECIMAL(15, 4) -- Volume-weighted average price
);

-- Insert sample stock data
INSERT INTO stock_prices (symbol, timestamp, open_price, high_price, low_price, close_price, volume) VALUES
('AAPL', extract(epoch from now() - interval '5 minutes')::bigint * 1000, 175.50, 176.20, 175.30, 176.00, 1000000),
('AAPL', extract(epoch from now() - interval '4 minutes')::bigint * 1000, 176.00, 176.50, 175.80, 176.30, 1200000),
('AAPL', extract(epoch from now() - interval '3 minutes')::bigint * 1000, 176.30, 177.00, 176.20, 176.80, 1500000),
('AAPL', extract(epoch from now() - interval '2 minutes')::bigint * 1000, 176.80, 177.20, 176.50, 176.90, 1100000),
('AAPL', extract(epoch from now() - interval '1 minute')::bigint * 1000, 176.90, 177.50, 176.70, 177.20, 1300000);

-- Calculate moving averages for trading signals
SELECT 
    symbol,
    timestamp,
    close_price,
    AVG(close_price) OVER (
        PARTITION BY symbol 
        ORDER BY timestamp 
        ROWS BETWEEN 4 PRECEDING AND CURRENT ROW
    ) AS sma_5,
    AVG(close_price) OVER (
        PARTITION BY symbol 
        ORDER BY timestamp 
        ROWS BETWEEN 19 PRECEDING AND CURRENT ROW
    ) AS sma_20
FROM stock_prices
WHERE symbol = 'AAPL'
ORDER BY timestamp DESC;

-- Detect trading signals (Golden Cross / Death Cross)
WITH moving_averages AS (
    SELECT 
        symbol,
        timestamp,
        close_price,
        AVG(close_price) OVER (
            PARTITION BY symbol 
            ORDER BY timestamp 
            ROWS BETWEEN 4 PRECEDING AND CURRENT ROW
        ) AS sma_5,
        AVG(close_price) OVER (
            PARTITION BY symbol 
            ORDER BY timestamp 
            ROWS BETWEEN 9 PRECEDING AND CURRENT ROW
        ) AS sma_10,
        LAG(AVG(close_price) OVER (
            PARTITION BY symbol 
            ORDER BY timestamp 
            ROWS BETWEEN 4 PRECEDING AND CURRENT ROW
        )) OVER (PARTITION BY symbol ORDER BY timestamp) AS prev_sma_5,
        LAG(AVG(close_price) OVER (
            PARTITION BY symbol 
            ORDER BY timestamp 
            ROWS BETWEEN 9 PRECEDING AND CURRENT ROW
        )) OVER (PARTITION BY symbol ORDER BY timestamp) AS prev_sma_10
    FROM stock_prices
)
SELECT 
    symbol,
    timestamp,
    close_price,
    sma_5,
    sma_10,
    CASE 
        WHEN sma_5 > sma_10 AND prev_sma_5 <= prev_sma_10 THEN 'BUY SIGNAL (Golden Cross)'
        WHEN sma_5 < sma_10 AND prev_sma_5 >= prev_sma_10 THEN 'SELL SIGNAL (Death Cross)'
        ELSE 'HOLD'
    END AS trading_signal
FROM moving_averages
WHERE symbol = 'AAPL'
ORDER BY timestamp DESC;

-- ----------------------------------------------------------------------------
-- 4. PORTFOLIO RISK MANAGEMENT
-- ----------------------------------------------------------------------------

-- Create portfolio holdings table
CREATE TABLE IF NOT EXISTS portfolio_holdings (
    holding_id SERIAL PRIMARY KEY,
    portfolio_id INTEGER NOT NULL,
    symbol VARCHAR(10) NOT NULL,
    quantity INTEGER NOT NULL,
    purchase_price DECIMAL(15, 4),
    current_price DECIMAL(15, 4),
    asset_class VARCHAR(50), -- 'equity', 'bond', 'commodity', 'crypto'
    sector VARCHAR(50),
    risk_rating VARCHAR(20) -- 'low', 'medium', 'high'
);

-- Insert sample portfolio
INSERT INTO portfolio_holdings (portfolio_id, symbol, quantity, purchase_price, current_price, asset_class, sector, risk_rating) VALUES
(1, 'AAPL', 100, 150.00, 177.20, 'equity', 'Technology', 'medium'),
(1, 'GOOGL', 50, 2800.00, 2950.00, 'equity', 'Technology', 'medium'),
(1, 'JNJ', 75, 160.00, 165.00, 'equity', 'Healthcare', 'low'),
(1, 'TSLA', 25, 700.00, 850.00, 'equity', 'Automotive', 'high'),
(1, 'BTC', 2, 45000.00, 52000.00, 'crypto', 'Cryptocurrency', 'high');

-- Calculate portfolio metrics
SELECT 
    portfolio_id,
    symbol,
    quantity,
    purchase_price,
    current_price,
    (current_price - purchase_price) AS price_change,
    ((current_price - purchase_price) / purchase_price * 100) AS return_pct,
    (quantity * current_price) AS current_value,
    (quantity * (current_price - purchase_price)) AS unrealized_pnl
FROM portfolio_holdings
WHERE portfolio_id = 1
ORDER BY unrealized_pnl DESC;

-- Portfolio diversification analysis
SELECT 
    asset_class,
    COUNT(*) AS num_holdings,
    SUM(quantity * current_price) AS total_value,
    SUM(quantity * current_price) / (SELECT SUM(quantity * current_price) FROM portfolio_holdings WHERE portfolio_id = 1) * 100 AS allocation_pct
FROM portfolio_holdings
WHERE portfolio_id = 1
GROUP BY asset_class
ORDER BY total_value DESC;

-- Risk-adjusted portfolio analysis
SELECT 
    portfolio_id,
    SUM(quantity * current_price) AS total_portfolio_value,
    SUM(quantity * (current_price - purchase_price)) AS total_unrealized_pnl,
    AVG(((current_price - purchase_price) / purchase_price * 100)) AS avg_return_pct,
    SUM(CASE WHEN risk_rating = 'high' THEN quantity * current_price ELSE 0 END) / 
        SUM(quantity * current_price) * 100 AS high_risk_allocation_pct
FROM portfolio_holdings
WHERE portfolio_id = 1
GROUP BY portfolio_id;

-- ----------------------------------------------------------------------------
-- 5. CUSTOMER CHURN PREDICTION
-- ----------------------------------------------------------------------------

-- Create customer banking activity table
CREATE TABLE IF NOT EXISTS customer_activity (
    customer_id INTEGER PRIMARY KEY,
    account_age_months INTEGER,
    num_products INTEGER, -- Number of banking products
    avg_monthly_balance DECIMAL(15, 2),
    num_transactions_monthly INTEGER,
    num_customer_service_calls INTEGER,
    has_credit_card BOOLEAN,
    has_mortgage BOOLEAN,
    last_login_days_ago INTEGER,
    churn_probability FLOAT,
    is_churned BOOLEAN DEFAULT FALSE
);

-- Insert sample customer data
INSERT INTO customer_activity VALUES
(1001, 48, 3, 15000.00, 25, 1, TRUE, TRUE, 2, NULL, FALSE),
(1002, 12, 1, 2500.00, 5, 8, FALSE, FALSE, 45, NULL, FALSE),
(1003, 120, 5, 85000.00, 40, 0, TRUE, TRUE, 1, NULL, FALSE),
(1004, 6, 1, 500.00, 2, 15, FALSE, FALSE, 90, NULL, TRUE);

-- ML-based churn prediction
CREATE OR REPLACE FUNCTION predict_churn(
    p_account_age_months INTEGER,
    p_num_products INTEGER,
    p_avg_balance DECIMAL,
    p_num_transactions INTEGER,
    p_num_service_calls INTEGER,
    p_last_login_days INTEGER
) RETURNS FLOAT AS $$
DECLARE
    churn_score FLOAT := 0.0;
BEGIN
    -- Low engagement indicators
    IF p_num_transactions < 10 THEN churn_score := churn_score + 0.2; END IF;
    IF p_last_login_days > 30 THEN churn_score := churn_score + 0.25; END IF;
    
    -- Low product adoption
    IF p_num_products = 1 THEN churn_score := churn_score + 0.15; END IF;
    
    -- High service calls (dissatisfaction)
    IF p_num_service_calls > 5 THEN churn_score := churn_score + 0.2; END IF;
    
    -- Low balance
    IF p_avg_balance < 5000 THEN churn_score := churn_score + 0.1; END IF;
    
    -- New customer risk
    IF p_account_age_months < 12 THEN churn_score := churn_score + 0.1; END IF;
    
    RETURN LEAST(churn_score, 1.0);
END;
$$ LANGUAGE plpgsql;

-- Update churn probabilities
UPDATE customer_activity
SET churn_probability = predict_churn(
    account_age_months,
    num_products,
    avg_monthly_balance,
    num_transactions_monthly,
    num_customer_service_calls,
    last_login_days_ago
);

-- Identify at-risk customers for retention campaigns
SELECT 
    customer_id,
    account_age_months,
    num_products,
    avg_monthly_balance,
    churn_probability,
    CASE 
        WHEN churn_probability > 0.7 THEN 'CRITICAL - Immediate Action'
        WHEN churn_probability > 0.5 THEN 'HIGH - Retention Campaign'
        WHEN churn_probability > 0.3 THEN 'MEDIUM - Monitor'
        ELSE 'LOW - Stable'
    END AS retention_priority
FROM customer_activity
WHERE churn_probability > 0.3
ORDER BY churn_probability DESC;

-- ----------------------------------------------------------------------------
-- 6. ANTI-MONEY LAUNDERING (AML) DETECTION
-- ----------------------------------------------------------------------------

-- Create suspicious activity patterns table
CREATE TABLE IF NOT EXISTS aml_alerts (
    alert_id SERIAL PRIMARY KEY,
    account_id INTEGER NOT NULL,
    alert_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    alert_type VARCHAR(100),
    description TEXT,
    transaction_ids INTEGER[],
    risk_score FLOAT,
    status VARCHAR(50) DEFAULT 'PENDING', -- 'PENDING', 'INVESTIGATING', 'CLEARED', 'REPORTED'
    assigned_analyst VARCHAR(100)
);

-- Detect structuring (smurfing) - multiple transactions just below reporting threshold
INSERT INTO aml_alerts (account_id, alert_type, description, transaction_ids, risk_score)
SELECT 
    account_id,
    'STRUCTURING' AS alert_type,
    'Multiple transactions just below $10,000 threshold within 24 hours' AS description,
    array_agg(transaction_id) AS transaction_ids,
    0.85 AS risk_score
FROM transactions
WHERE amount BETWEEN 9000 AND 9999
  AND transaction_date > CURRENT_TIMESTAMP - INTERVAL '24 hours'
GROUP BY account_id
HAVING COUNT(*) >= 3;

-- Detect unusual geographic patterns
INSERT INTO aml_alerts (account_id, alert_type, description, transaction_ids, risk_score)
SELECT 
    account_id,
    'GEOGRAPHIC_ANOMALY' AS alert_type,
    'Transactions in multiple high-risk countries within short timeframe' AS description,
    array_agg(transaction_id) AS transaction_ids,
    0.75 AS risk_score
FROM transactions
WHERE location ILIKE ANY(ARRAY['%nigeria%', '%russia%', '%iran%', '%north korea%'])
  AND transaction_date > CURRENT_TIMESTAMP - INTERVAL '7 days'
GROUP BY account_id
HAVING COUNT(DISTINCT location) >= 2;

-- Query high-priority AML alerts
SELECT 
    alert_id,
    account_id,
    alert_type,
    description,
    risk_score,
    status,
    alert_date
FROM aml_alerts
WHERE status = 'PENDING'
  AND risk_score > 0.7
ORDER BY risk_score DESC, alert_date DESC;

-- ============================================================================
-- SUMMARY: Finance ML Use Cases Demonstrated
-- ============================================================================
-- 1. Real-time fraud detection with ML scoring
-- 2. Credit scoring and risk assessment with embeddings
-- 3. Algorithmic trading with time series analysis
-- 4. Portfolio risk management and optimization
-- 5. Customer churn prediction for retention
-- 6. Anti-money laundering (AML) pattern detection
-- ============================================================================
