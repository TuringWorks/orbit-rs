-- ============================================================================
-- OrbitRS Banking Examples - Core Banking Schema
-- ============================================================================
-- Accounts, customers, transactions with ML fraud detection
-- ============================================================================
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- CUSTOMERS
-- ============================================================================
CREATE TABLE customers (
    customer_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_number VARCHAR(20) UNIQUE NOT NULL,
    -- Personal Info
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE,
    ssn_encrypted VARCHAR(255),
    -- Encrypted SSN
    -- Contact
    email VARCHAR(255),
    phone VARCHAR(20),
    address_line1 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(2),
    zip_code VARCHAR(10),
    country VARCHAR(2) DEFAULT 'US',
    -- KYC/AML
    kyc_status VARCHAR(20) CHECK (
        kyc_status IN ('PENDING', 'VERIFIED', 'FAILED', 'EXPIRED')
    ),
    kyc_verified_at TIMESTAMP,
    risk_rating VARCHAR(10) CHECK (risk_rating IN ('LOW', 'MEDIUM', 'HIGH')),
    -- ML Scores
    credit_score INTEGER CHECK (
        credit_score BETWEEN 300 AND 850
    ),
    fraud_risk_score DECIMAL(5, 4),
    -- 0.0000 to 1.0000
    churn_probability DECIMAL(5, 4),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'SUSPENDED', 'CLOSED')),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_customers_email ON customers(email);
CREATE INDEX idx_customers_status ON customers(status);
CREATE INDEX idx_customers_risk ON customers(risk_rating);
-- ============================================================================
-- ACCOUNTS
-- ============================================================================
CREATE TABLE accounts (
    account_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_number VARCHAR(20) UNIQUE NOT NULL,
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    -- Account Type
    account_type VARCHAR(20) NOT NULL CHECK (
        account_type IN (
            'CHECKING',
            'SAVINGS',
            'CREDIT_CARD',
            'LOAN',
            'INVESTMENT'
        )
    ),
    -- Balances
    balance DECIMAL(15, 2) DEFAULT 0,
    available_balance DECIMAL(15, 2) DEFAULT 0,
    pending_balance DECIMAL(15, 2) DEFAULT 0,
    -- Credit Accounts
    credit_limit DECIMAL(15, 2),
    apr DECIMAL(5, 2),
    -- Annual Percentage Rate
    -- Interest
    interest_rate DECIMAL(5, 4),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'FROZEN',
            'CLOSED',
            'DORMANT'
        )
    ),
    -- Overdraft
    overdraft_protection BOOLEAN DEFAULT FALSE,
    overdraft_limit DECIMAL(10, 2) DEFAULT 0,
    opened_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_accounts_customer ON accounts(customer_id);
CREATE INDEX idx_accounts_type ON accounts(account_type);
CREATE INDEX idx_accounts_status ON accounts(status);
-- ============================================================================
-- TRANSACTIONS
-- ============================================================================
CREATE TABLE transactions (
    transaction_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    transaction_number VARCHAR(30) UNIQUE NOT NULL,
    -- Account
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    -- Transaction Details
    transaction_type VARCHAR(30) CHECK (
        transaction_type IN (
            'DEPOSIT',
            'WITHDRAWAL',
            'TRANSFER',
            'PAYMENT',
            'FEE',
            'INTEREST',
            'REFUND',
            'ADJUSTMENT'
        )
    ),
    -- Amount
    amount DECIMAL(15, 2) NOT NULL,
    currency VARCHAR(3) DEFAULT 'USD',
    -- Balances (snapshot)
    balance_before DECIMAL(15, 2),
    balance_after DECIMAL(15, 2),
    -- Transfer Details
    from_account_id UUID REFERENCES accounts(account_id),
    to_account_id UUID REFERENCES accounts(account_id),
    -- External Transfer
    external_account_number VARCHAR(50),
    routing_number VARCHAR(20),
    -- Description
    description TEXT,
    merchant_name VARCHAR(200),
    merchant_category VARCHAR(50),
    -- Location
    location_city VARCHAR(100),
    location_state VARCHAR(2),
    location_country VARCHAR(2),
    -- Fraud Detection
    fraud_score DECIMAL(5, 4),
    -- ML fraud probability
    fraud_status VARCHAR(20) DEFAULT 'CLEAR' CHECK (
        fraud_status IN (
            'CLEAR',
            'REVIEW',
            'BLOCKED',
            'CONFIRMED_FRAUD'
        )
    ),
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'POSTED',
            'DECLINED',
            'REVERSED',
            'FAILED'
        )
    ),
    -- Timestamps
    transaction_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    posted_date TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_transactions_account ON transactions(account_id);
CREATE INDEX idx_transactions_date ON transactions(transaction_date);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_fraud ON transactions(fraud_status);
CREATE INDEX idx_transactions_type ON transactions(transaction_type);
-- ============================================================================
-- CARDS
-- ============================================================================
CREATE TABLE cards (
    card_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    card_number_encrypted VARCHAR(255) NOT NULL,
    -- Encrypted PAN
    card_number_last4 VARCHAR(4) NOT NULL,
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    -- Card Details
    card_type VARCHAR(20) CHECK (card_type IN ('DEBIT', 'CREDIT', 'PREPAID')),
    card_brand VARCHAR(20) CHECK (
        card_brand IN ('VISA', 'MASTERCARD', 'AMEX', 'DISCOVER')
    ),
    -- Expiration
    expiration_month INTEGER CHECK (
        expiration_month BETWEEN 1 AND 12
    ),
    expiration_year INTEGER,
    -- CVV (encrypted)
    cvv_encrypted VARCHAR(255),
    -- PIN
    pin_hash VARCHAR(255),
    pin_attempts INTEGER DEFAULT 0,
    -- Limits
    daily_limit DECIMAL(10, 2),
    transaction_limit DECIMAL(10, 2),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'BLOCKED',
            'LOST',
            'STOLEN',
            'EXPIRED',
            'CANCELLED'
        )
    ),
    issued_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    activated_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_cards_account ON cards(account_id);
CREATE INDEX idx_cards_last4 ON cards(card_number_last4);
CREATE INDEX idx_cards_status ON cards(status);
-- ============================================================================
-- LOANS
-- ============================================================================
CREATE TABLE loans (
    loan_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    loan_number VARCHAR(20) UNIQUE NOT NULL,
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    account_id UUID REFERENCES accounts(account_id),
    -- Loan Type
    loan_type VARCHAR(30) CHECK (
        loan_type IN (
            'PERSONAL',
            'AUTO',
            'MORTGAGE',
            'STUDENT',
            'BUSINESS',
            'LINE_OF_CREDIT'
        )
    ),
    -- Amounts
    principal_amount DECIMAL(15, 2) NOT NULL,
    outstanding_balance DECIMAL(15, 2),
    interest_rate DECIMAL(5, 4) NOT NULL,
    -- Terms
    term_months INTEGER NOT NULL,
    payment_amount DECIMAL(10, 2),
    payment_frequency VARCHAR(20) CHECK (
        payment_frequency IN ('MONTHLY', 'BIWEEKLY', 'WEEKLY')
    ),
    -- ML Credit Assessment
    credit_score_at_origination INTEGER,
    default_probability DECIMAL(5, 4),
    -- ML prediction
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'APPROVED',
            'ACTIVE',
            'PAID_OFF',
            'DEFAULTED',
            'CHARGED_OFF'
        )
    ),
    -- Dates
    origination_date DATE,
    first_payment_date DATE,
    maturity_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_loans_customer ON loans(customer_id);
CREATE INDEX idx_loans_status ON loans(status);
CREATE INDEX idx_loans_type ON loans(loan_type);
-- ============================================================================
-- FRAUD_ALERTS
-- ============================================================================
CREATE TABLE fraud_alerts (
    alert_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    transaction_id UUID REFERENCES transactions(transaction_id),
    customer_id UUID REFERENCES customers(customer_id),
    -- Alert Details
    alert_type VARCHAR(50) CHECK (
        alert_type IN (
            'UNUSUAL_LOCATION',
            'UNUSUAL_AMOUNT',
            'VELOCITY_CHECK',
            'DUPLICATE_TRANSACTION',
            'COMPROMISED_MERCHANT',
            'ML_DETECTION'
        )
    ),
    -- ML Scores
    fraud_score DECIMAL(5, 4) NOT NULL,
    confidence DECIMAL(5, 4),
    -- Risk Factors
    risk_factors JSONB,
    -- Status
    status VARCHAR(20) DEFAULT 'OPEN' CHECK (
        status IN (
            'OPEN',
            'INVESTIGATING',
            'CONFIRMED',
            'FALSE_POSITIVE',
            'CLOSED'
        )
    ),
    -- Resolution
    resolved_by UUID,
    resolved_at TIMESTAMP,
    resolution_notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_fraud_alerts_transaction ON fraud_alerts(transaction_id);
CREATE INDEX idx_fraud_alerts_customer ON fraud_alerts(customer_id);
CREATE INDEX idx_fraud_alerts_status ON fraud_alerts(status);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = CURRENT_TIMESTAMP;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER update_customers_updated_at BEFORE
UPDATE ON customers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_accounts_updated_at BEFORE
UPDATE ON accounts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_loans_updated_at BEFORE
UPDATE ON loans FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Customer account summary
CREATE VIEW v_customer_accounts AS
SELECT c.customer_id,
    c.customer_number,
    c.first_name,
    c.last_name,
    c.credit_score,
    c.risk_rating,
    COUNT(a.account_id) AS account_count,
    SUM(
        CASE
            WHEN a.account_type = 'CHECKING' THEN a.balance
            ELSE 0
        END
    ) AS checking_balance,
    SUM(
        CASE
            WHEN a.account_type = 'SAVINGS' THEN a.balance
            ELSE 0
        END
    ) AS savings_balance,
    SUM(
        CASE
            WHEN a.account_type = 'CREDIT_CARD' THEN a.balance
            ELSE 0
        END
    ) AS credit_card_balance
FROM customers c
    LEFT JOIN accounts a ON c.customer_id = a.customer_id
    AND a.status = 'ACTIVE'
GROUP BY c.customer_id,
    c.customer_number,
    c.first_name,
    c.last_name,
    c.credit_score,
    c.risk_rating;
-- High-risk transactions
CREATE VIEW v_high_risk_transactions AS
SELECT t.transaction_id,
    t.transaction_number,
    t.amount,
    t.transaction_type,
    t.fraud_score,
    t.fraud_status,
    c.customer_number,
    c.first_name,
    c.last_name,
    a.account_number
FROM transactions t
    JOIN accounts a ON t.account_id = a.account_id
    JOIN customers c ON a.customer_id = c.customer_id
WHERE t.fraud_score > 0.7
    OR t.fraud_status IN ('REVIEW', 'BLOCKED')
ORDER BY t.fraud_score DESC,
    t.transaction_date DESC;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE customers IS 'Bank customers with KYC and ML risk scores';
COMMENT ON TABLE accounts IS 'Customer accounts (checking, savings, credit, loans)';
COMMENT ON TABLE transactions IS 'All financial transactions with fraud detection';
COMMENT ON TABLE fraud_alerts IS 'ML-powered fraud detection alerts';