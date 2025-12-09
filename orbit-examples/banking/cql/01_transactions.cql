-- ============================================================================
-- OrbitRS Banking Examples - Transaction History & Financial Time-Series (CQL)
-- ============================================================================
-- High-volume transaction storage optimized for:
-- - Account statement generation (month-by-month queries)
-- - Fraud analytics (real-time scoring and pattern detection)
-- - Regulatory compliance (7-year retention)
-- - ML model training data
-- ============================================================================

CREATE KEYSPACE IF NOT EXISTS banking_ledger
WITH replication = {'class': 'NetworkTopologyStrategy', 'dc1': 3}
AND durable_writes = true;

USE banking_ledger;

-- ============================================================================
-- CUSTOMER TRANSACTIONS (Primary Statement Table)
-- ============================================================================
-- Partition by account + month for efficient statement generation
-- Clustering by timestamp DESC for most recent first

CREATE TABLE customer_transactions (
    account_id uuid,
    year_month text,  -- Format: '2024-12' for partition key
    transaction_time timestamp,
    transaction_id uuid,
    -- Transaction details
    transaction_type text,  -- DEBIT, CREDIT, TRANSFER, FEE, INTEREST, REFUND
    amount decimal,
    currency text,
    -- Merchant info
    merchant_id text,
    merchant_name text,
    merchant_category_code text,  -- MCC code
    merchant_category text,
    -- Location
    location_city text,
    location_state text,
    location_country text,
    location_lat double,
    location_lon double,
    -- Balance tracking
    balance_before decimal,
    balance_after decimal,
    -- Status
    status text,  -- PENDING, POSTED, DECLINED, REVERSED
    -- Metadata
    channel text,  -- MOBILE, WEB, ATM, POS, BRANCH
    device_id text,
    ip_address text,
    reference_number text,
    description text,
    PRIMARY KEY ((account_id, year_month), transaction_time, transaction_id)
) WITH CLUSTERING ORDER BY (transaction_time DESC, transaction_id ASC)
  AND default_time_to_live = 220752000  -- 7 years retention (regulatory)
  AND compaction = {'class': 'TimeWindowCompactionStrategy', 'compaction_window_size': 7, 'compaction_window_unit': 'DAYS'}
  AND compression = {'sstable_compression': 'LZ4Compressor'};

-- Secondary index for merchant searches (within partition)
CREATE INDEX ON customer_transactions (merchant_category);

-- Insert transactions
INSERT INTO customer_transactions (
    account_id, year_month, transaction_time, transaction_id,
    transaction_type, amount, currency,
    merchant_name, merchant_category_code, merchant_category,
    location_city, location_state, location_country,
    balance_before, balance_after, status, channel
) VALUES (
    11111111-1111-1111-1111-111111111111, '2024-12', '2024-12-09 10:30:00', uuid(),
    'DEBIT', -45.50, 'USD',
    'Whole Foods Market', '5411', 'Grocery Stores',
    'San Francisco', 'CA', 'US',
    1295.50, 1250.00, 'POSTED', 'POS'
);

INSERT INTO customer_transactions (
    account_id, year_month, transaction_time, transaction_id,
    transaction_type, amount, currency,
    merchant_name, merchant_category,
    balance_before, balance_after, status, channel
) VALUES (
    11111111-1111-1111-1111-111111111111, '2024-12', '2024-12-09 14:15:00', uuid(),
    'CREDIT', 2500.00, 'USD',
    'Direct Deposit - Acme Corp', 'Payroll',
    1250.00, 3750.00, 'POSTED', 'ACH'
);

-- Query: Get December 2024 statement
SELECT transaction_time, transaction_type, amount, merchant_name, balance_after
FROM customer_transactions
WHERE account_id = 11111111-1111-1111-1111-111111111111
  AND year_month = '2024-12';

-- Query: Get last 30 days (may span partitions)
SELECT transaction_time, amount, merchant_name
FROM customer_transactions
WHERE account_id = 11111111-1111-1111-1111-111111111111
  AND year_month IN ('2024-11', '2024-12')
  AND transaction_time >= '2024-11-09 00:00:00';

-- ============================================================================
-- FRAUD DETECTION EVENTS
-- ============================================================================
-- Real-time fraud scoring and alert tracking

CREATE TABLE fraud_events (
    customer_id uuid,
    event_date date,
    event_time timestamp,
    event_id uuid,
    -- Transaction reference
    transaction_id uuid,
    account_id uuid,
    amount decimal,
    -- Fraud scoring
    fraud_score double,      -- ML model probability (0.0 - 1.0)
    risk_level text,         -- LOW, MEDIUM, HIGH, CRITICAL
    model_version text,
    -- Detection details
    detection_type text,     -- VELOCITY, GEOGRAPHIC, AMOUNT, BEHAVIOR, ML_ANOMALY
    risk_factors list<text>, -- List of contributing factors
    -- Resolution
    alert_status text,       -- OPEN, INVESTIGATING, CONFIRMED_FRAUD, FALSE_POSITIVE, CLOSED
    resolution_time timestamp,
    resolved_by text,
    resolution_notes text,
    PRIMARY KEY ((customer_id, event_date), event_time, event_id)
) WITH CLUSTERING ORDER BY (event_time DESC, event_id ASC)
  AND default_time_to_live = 31536000;  -- 1 year

-- Insert fraud event
INSERT INTO fraud_events (
    customer_id, event_date, event_time, event_id,
    transaction_id, account_id, amount,
    fraud_score, risk_level, model_version,
    detection_type, risk_factors, alert_status
) VALUES (
    22222222-2222-2222-2222-222222222222, '2024-12-09', '2024-12-09 15:30:00', uuid(),
    33333333-3333-3333-3333-333333333333, 11111111-1111-1111-1111-111111111111, 5000.00,
    0.87, 'HIGH', 'fraud_model_v3.2',
    'ML_ANOMALY', ['unusual_amount', 'new_merchant', 'late_night_txn'], 'OPEN'
);

-- Query high-risk events today
SELECT event_time, amount, fraud_score, risk_factors, alert_status
FROM fraud_events
WHERE customer_id = 22222222-2222-2222-2222-222222222222
  AND event_date = '2024-12-09'
  AND fraud_score > 0.7
ALLOW FILTERING;

-- ============================================================================
-- DAILY ACCOUNT BALANCE SNAPSHOTS
-- ============================================================================
-- End-of-day balances for reporting and reconciliation

CREATE TABLE daily_balances (
    account_id uuid,
    balance_date date,
    -- Balances
    opening_balance decimal,
    closing_balance decimal,
    available_balance decimal,
    pending_balance decimal,
    -- Activity summary
    total_credits decimal,
    total_debits decimal,
    transaction_count int,
    -- Interest (for savings/money market)
    interest_accrued decimal,
    interest_rate double,
    -- Status
    account_status text,
    PRIMARY KEY (account_id, balance_date)
) WITH CLUSTERING ORDER BY (balance_date DESC)
  AND default_time_to_live = 220752000;  -- 7 years

-- Insert daily balance
INSERT INTO daily_balances (
    account_id, balance_date,
    opening_balance, closing_balance, available_balance,
    total_credits, total_debits, transaction_count,
    interest_accrued, account_status
) VALUES (
    11111111-1111-1111-1111-111111111111, '2024-12-09',
    1295.50, 3750.00, 3750.00,
    2500.00, 45.50, 2,
    0.12, 'ACTIVE'
);

-- Query balance history for account
SELECT balance_date, opening_balance, closing_balance, transaction_count
FROM daily_balances
WHERE account_id = 11111111-1111-1111-1111-111111111111
LIMIT 30;

-- ============================================================================
-- CARD AUTHORIZATION EVENTS
-- ============================================================================
-- Real-time card swipe/chip/tap events before posting

CREATE TABLE card_authorizations (
    card_id uuid,
    auth_date date,
    auth_time timestamp,
    auth_id uuid,
    -- Authorization details
    amount decimal,
    currency text,
    auth_code text,
    response_code text,  -- 00=Approved, 05=Do Not Honor, 51=Insufficient Funds
    -- Merchant
    merchant_id text,
    merchant_name text,
    merchant_mcc text,
    terminal_id text,
    -- Location
    location_city text,
    location_country text,
    pos_entry_mode text,  -- CHIP, SWIPE, TAP, MANUAL, ECOMMERCE
    -- Real-time checks
    avs_result text,      -- Address Verification
    cvv_result text,      -- CVV check
    fraud_check_score double,
    -- Status
    auth_status text,     -- APPROVED, DECLINED, PENDING, REVERSED
    decline_reason text,
    PRIMARY KEY ((card_id, auth_date), auth_time, auth_id)
) WITH CLUSTERING ORDER BY (auth_time DESC, auth_id ASC)
  AND default_time_to_live = 7776000;  -- 90 days

-- Insert authorization
INSERT INTO card_authorizations (
    card_id, auth_date, auth_time, auth_id,
    amount, currency, auth_code, response_code,
    merchant_name, merchant_mcc,
    location_city, location_country, pos_entry_mode,
    avs_result, cvv_result, fraud_check_score, auth_status
) VALUES (
    44444444-4444-4444-4444-444444444444, '2024-12-09', '2024-12-09 16:45:00', uuid(),
    125.99, 'USD', 'A12345', '00',
    'Amazon.com', '5942',
    'Seattle', 'US', 'ECOMMERCE',
    'Y', 'M', 0.15, 'APPROVED'
);

-- Query today's authorizations
SELECT auth_time, amount, merchant_name, auth_status, fraud_check_score
FROM card_authorizations
WHERE card_id = 44444444-4444-4444-4444-444444444444
  AND auth_date = '2024-12-09';

-- ============================================================================
-- PAYMENT SCHEDULES (Bill Pay, Recurring)
-- ============================================================================

CREATE TABLE payment_schedules (
    customer_id uuid,
    schedule_id uuid,
    -- Payment details
    payee_name text,
    payee_account text,
    payment_amount decimal,
    currency text,
    -- Schedule
    frequency text,        -- ONCE, WEEKLY, BIWEEKLY, MONTHLY, QUARTERLY, ANNUALLY
    next_payment_date date,
    end_date date,
    -- Status
    status text,           -- ACTIVE, PAUSED, COMPLETED, CANCELLED
    payments_made int,
    total_paid decimal,
    last_payment_date date,
    last_payment_status text,
    -- Source
    source_account_id uuid,
    created_at timestamp,
    PRIMARY KEY (customer_id, schedule_id)
);

-- Insert recurring payment
INSERT INTO payment_schedules (
    customer_id, schedule_id,
    payee_name, payee_account, payment_amount, currency,
    frequency, next_payment_date,
    status, payments_made, total_paid,
    source_account_id, created_at
) VALUES (
    22222222-2222-2222-2222-222222222222, uuid(),
    'Pacific Gas & Electric', 'PGE-123456789', 150.00, 'USD',
    'MONTHLY', '2024-12-15',
    'ACTIVE', 6, 900.00,
    11111111-1111-1111-1111-111111111111, toTimestamp(now())
);

-- Query active payments
SELECT payee_name, payment_amount, frequency, next_payment_date
FROM payment_schedules
WHERE customer_id = 22222222-2222-2222-2222-222222222222
  AND status = 'ACTIVE'
ALLOW FILTERING;

-- ============================================================================
-- LOAN PAYMENT HISTORY
-- ============================================================================

CREATE TABLE loan_payments (
    loan_id uuid,
    payment_year int,
    payment_date date,
    payment_id uuid,
    -- Payment details
    payment_amount decimal,
    principal_amount decimal,
    interest_amount decimal,
    fees_amount decimal,
    escrow_amount decimal,
    -- Balance tracking
    principal_balance_before decimal,
    principal_balance_after decimal,
    -- Status
    payment_status text,    -- SCHEDULED, PAID, PARTIAL, MISSED, LATE
    days_late int,
    late_fee decimal,
    -- Source
    source_account_id uuid,
    payment_method text,
    PRIMARY KEY ((loan_id, payment_year), payment_date, payment_id)
) WITH CLUSTERING ORDER BY (payment_date DESC, payment_id ASC);

-- Insert loan payment
INSERT INTO loan_payments (
    loan_id, payment_year, payment_date, payment_id,
    payment_amount, principal_amount, interest_amount,
    principal_balance_before, principal_balance_after,
    payment_status, days_late, source_account_id, payment_method
) VALUES (
    55555555-5555-5555-5555-555555555555, 2024, '2024-12-01', uuid(),
    1250.00, 850.00, 400.00,
    125000.00, 124150.00,
    'PAID', 0, 11111111-1111-1111-1111-111111111111, 'ACH'
);

-- Query payment history for loan
SELECT payment_date, payment_amount, principal_amount, interest_amount, principal_balance_after
FROM loan_payments
WHERE loan_id = 55555555-5555-5555-5555-555555555555
  AND payment_year = 2024;

-- ============================================================================
-- SPENDING ANALYTICS (Pre-aggregated)
-- ============================================================================

CREATE TABLE spending_by_category (
    customer_id uuid,
    year_month text,
    category text,
    -- Aggregates
    total_amount decimal,
    transaction_count int,
    avg_transaction decimal,
    max_transaction decimal,
    -- Trend
    mom_change_pct double,  -- Month-over-month change
    PRIMARY KEY ((customer_id, year_month), category)
);

-- Insert spending aggregate
INSERT INTO spending_by_category (
    customer_id, year_month, category,
    total_amount, transaction_count, avg_transaction, max_transaction,
    mom_change_pct
) VALUES (
    22222222-2222-2222-2222-222222222222, '2024-12', 'Grocery Stores',
    450.00, 8, 56.25, 125.00,
    5.5
);

-- Query spending by category
SELECT category, total_amount, transaction_count, mom_change_pct
FROM spending_by_category
WHERE customer_id = 22222222-2222-2222-2222-222222222222
  AND year_month = '2024-12';

-- ============================================================================
-- TRANSACTION EMBEDDINGS (OrbitRS Extension)
-- ============================================================================
-- Vector storage for ML-based fraud detection and transaction clustering

CREATE TABLE transaction_embeddings (
    account_id uuid,
    transaction_id uuid,
    -- Embedding vector
    embedding vector<float, 128>,
    -- Transaction metadata for filtering
    transaction_date date,
    amount decimal,
    merchant_category text,
    -- Clustering results
    cluster_id int,
    anomaly_score double,
    PRIMARY KEY (account_id, transaction_id)
);

-- Insert embedding
INSERT INTO transaction_embeddings (
    account_id, transaction_id,
    transaction_date, amount, merchant_category,
    cluster_id, anomaly_score
) VALUES (
    11111111-1111-1111-1111-111111111111, 66666666-6666-6666-6666-666666666666,
    '2024-12-09', 45.50, 'Grocery Stores',
    3, 0.12
);

-- ANN query to find similar transactions (OrbitRS extension)
-- SELECT transaction_id, amount, merchant_category, anomaly_score
-- FROM transaction_embeddings
-- WHERE account_id = 11111111-1111-1111-1111-111111111111
-- ORDER BY embedding ANN OF [0.1, 0.2, ...]
-- LIMIT 10;

-- ============================================================================
-- COMPLIANCE: SUSPICIOUS ACTIVITY REPORTS
-- ============================================================================

CREATE TABLE sar_reports (
    report_year int,
    report_id uuid,
    filing_date date,
    -- Subject
    customer_id uuid,
    customer_name text,
    -- Activity
    activity_type text,      -- STRUCTURING, MONEY_LAUNDERING, FRAUD, UNUSUAL_ACTIVITY
    activity_start_date date,
    activity_end_date date,
    total_amount decimal,
    -- Report details
    narrative text,
    supporting_transactions list<uuid>,
    -- Status
    filing_status text,      -- DRAFT, SUBMITTED, ACKNOWLEDGED
    fincen_tracking_number text,
    submitted_at timestamp,
    submitted_by text,
    PRIMARY KEY (report_year, report_id)
);

-- ============================================================================
-- AUDIT: DATA ACCESS LOG
-- ============================================================================

CREATE TABLE data_access_log (
    access_date date,
    access_time timestamp,
    access_id uuid,
    -- User
    user_id text,
    user_role text,
    -- Access details
    action_type text,        -- VIEW, EXPORT, MODIFY
    resource_type text,      -- ACCOUNT, TRANSACTION, CUSTOMER
    resource_id text,
    -- Context
    ip_address text,
    user_agent text,
    session_id text,
    -- Justification (for sensitive access)
    business_justification text,
    PRIMARY KEY (access_date, access_time, access_id)
) WITH CLUSTERING ORDER BY (access_time DESC, access_id ASC)
  AND default_time_to_live = 220752000;  -- 7 years

-- Insert access log
INSERT INTO data_access_log (
    access_date, access_time, access_id,
    user_id, user_role,
    action_type, resource_type, resource_id,
    ip_address, session_id
) VALUES (
    '2024-12-09', toTimestamp(now()), uuid(),
    'analyst_001', 'FRAUD_ANALYST',
    'VIEW', 'TRANSACTION', 'TXN-12345',
    '10.0.0.50', 'sess_abc123'
);

