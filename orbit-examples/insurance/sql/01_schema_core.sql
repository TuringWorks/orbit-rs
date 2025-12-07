-- ============================================================================
-- OrbitRS Insurance Examples - Core Schema
-- ============================================================================
-- This schema defines the core insurance entities shared across all domains:
-- customers, policies, claims, agents, addresses, and payments
-- ============================================================================
-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- CUSTOMERS
-- ============================================================================
CREATE TABLE customers (
    customer_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_number VARCHAR(50) UNIQUE NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    middle_name VARCHAR(100),
    date_of_birth DATE NOT NULL,
    ssn_last4 VARCHAR(4),
    -- Last 4 digits only for privacy
    email VARCHAR(255) UNIQUE NOT NULL,
    phone VARCHAR(20),
    mobile_phone VARCHAR(20),
    credit_score INTEGER CHECK (
        credit_score BETWEEN 300 AND 850
    ),
    customer_since DATE DEFAULT CURRENT_DATE,
    customer_status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        customer_status IN ('ACTIVE', 'INACTIVE', 'SUSPENDED', 'DECEASED')
    ),
    preferred_language VARCHAR(10) DEFAULT 'en',
    marketing_opt_in BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(100),
    updated_by VARCHAR(100)
);
CREATE INDEX idx_customers_email ON customers(email);
CREATE INDEX idx_customers_customer_number ON customers(customer_number);
CREATE INDEX idx_customers_status ON customers(customer_status);
-- ============================================================================
-- ADDRESSES
-- ============================================================================
CREATE TABLE addresses (
    address_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID REFERENCES customers(customer_id) ON DELETE CASCADE,
    address_type VARCHAR(20) CHECK (
        address_type IN ('HOME', 'MAILING', 'BUSINESS', 'PROPERTY')
    ),
    street_address_1 VARCHAR(255) NOT NULL,
    street_address_2 VARCHAR(255),
    city VARCHAR(100) NOT NULL,
    state VARCHAR(2) NOT NULL,
    zip_code VARCHAR(10) NOT NULL,
    county VARCHAR(100),
    country VARCHAR(2) DEFAULT 'US',
    latitude DECIMAL(10, 8),
    longitude DECIMAL(11, 8),
    is_primary BOOLEAN DEFAULT FALSE,
    valid_from DATE DEFAULT CURRENT_DATE,
    valid_to DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_addresses_customer ON addresses(customer_id);
CREATE INDEX idx_addresses_zip ON addresses(zip_code);
CREATE INDEX idx_addresses_location ON addresses(latitude, longitude);
-- ============================================================================
-- AGENTS
-- ============================================================================
CREATE TABLE agents (
    agent_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agent_number VARCHAR(50) UNIQUE NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    phone VARCHAR(20),
    license_number VARCHAR(50) UNIQUE NOT NULL,
    license_state VARCHAR(2) NOT NULL,
    license_expiration DATE NOT NULL,
    agent_type VARCHAR(20) CHECK (
        agent_type IN ('CAPTIVE', 'INDEPENDENT', 'BROKER')
    ),
    commission_rate DECIMAL(5, 4) DEFAULT 0.10,
    -- 10% default
    territory_states TEXT [],
    -- Array of state codes
    specializations TEXT [],
    -- Array of insurance types
    agent_status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        agent_status IN ('ACTIVE', 'INACTIVE', 'SUSPENDED')
    ),
    hire_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_agents_license ON agents(license_number);
CREATE INDEX idx_agents_status ON agents(agent_status);
-- ============================================================================
-- POLICIES (Master Table)
-- ============================================================================
CREATE TABLE policies (
    policy_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_number VARCHAR(50) UNIQUE NOT NULL,
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    agent_id UUID REFERENCES agents(agent_id),
    policy_type VARCHAR(50) NOT NULL CHECK (
        policy_type IN (
            'AUTO',
            'HOME',
            'LIFE',
            'PROPERTY',
            'DISASTER',
            'EARTHQUAKE',
            'INDUSTRIAL'
        )
    ),
    policy_status VARCHAR(20) DEFAULT 'QUOTED' CHECK (
        policy_status IN (
            'QUOTED',
            'PENDING',
            'ACTIVE',
            'SUSPENDED',
            'CANCELLED',
            'EXPIRED',
            'LAPSED'
        )
    ),
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    issue_date DATE,
    cancellation_date DATE,
    cancellation_reason TEXT,
    -- Financial
    premium_amount DECIMAL(10, 2) NOT NULL,
    premium_frequency VARCHAR(20) DEFAULT 'MONTHLY' CHECK (
        premium_frequency IN (
            'MONTHLY',
            'QUARTERLY',
            'SEMI_ANNUAL',
            'ANNUAL'
        )
    ),
    coverage_amount DECIMAL(12, 2) NOT NULL,
    deductible_amount DECIMAL(10, 2) DEFAULT 0,
    -- Risk
    risk_score INTEGER CHECK (
        risk_score BETWEEN 0 AND 100
    ),
    underwriting_tier VARCHAR(20) CHECK (
        underwriting_tier IN (
            'PREFERRED',
            'STANDARD',
            'SUBSTANDARD',
            'DECLINED'
        )
    ),
    -- Renewal
    auto_renew BOOLEAN DEFAULT TRUE,
    renewal_date DATE,
    prior_policy_id UUID REFERENCES policies(policy_id),
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(100),
    updated_by VARCHAR(100),
    CONSTRAINT valid_dates CHECK (expiration_date > effective_date)
);
CREATE INDEX idx_policies_customer ON policies(customer_id);
CREATE INDEX idx_policies_agent ON policies(agent_id);
CREATE INDEX idx_policies_type ON policies(policy_type);
CREATE INDEX idx_policies_status ON policies(policy_status);
CREATE INDEX idx_policies_effective_date ON policies(effective_date);
CREATE INDEX idx_policies_number ON policies(policy_number);
-- ============================================================================
-- CLAIMS (Master Table)
-- ============================================================================
CREATE TABLE claims (
    claim_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    claim_number VARCHAR(50) UNIQUE NOT NULL,
    policy_id UUID NOT NULL REFERENCES policies(policy_id),
    claim_type VARCHAR(50) NOT NULL,
    claim_status VARCHAR(20) DEFAULT 'REPORTED' CHECK (
        claim_status IN (
            'REPORTED',
            'INVESTIGATING',
            'PENDING_INFO',
            'APPROVED',
            'DENIED',
            'SETTLED',
            'CLOSED',
            'WITHDRAWN'
        )
    ),
    -- Dates
    incident_date TIMESTAMP NOT NULL,
    reported_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    closed_date TIMESTAMP,
    -- Location
    incident_location TEXT,
    incident_latitude DECIMAL(10, 8),
    incident_longitude DECIMAL(11, 8),
    -- Description
    incident_description TEXT NOT NULL,
    police_report_number VARCHAR(50),
    -- Financial
    claim_amount DECIMAL(12, 2) NOT NULL,
    deductible_applied DECIMAL(10, 2) DEFAULT 0,
    approved_amount DECIMAL(12, 2),
    paid_amount DECIMAL(12, 2) DEFAULT 0,
    reserve_amount DECIMAL(12, 2),
    -- Assignment
    adjuster_id UUID REFERENCES agents(agent_id),
    assigned_date TIMESTAMP,
    -- Fraud indicators
    fraud_score INTEGER CHECK (
        fraud_score BETWEEN 0 AND 100
    ),
    fraud_flag BOOLEAN DEFAULT FALSE,
    fraud_investigation_notes TEXT,
    -- Subrogation
    subrogation_potential BOOLEAN DEFAULT FALSE,
    subrogation_amount DECIMAL(12, 2),
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(100),
    updated_by VARCHAR(100),
    CONSTRAINT valid_claim_dates CHECK (reported_date >= incident_date)
);
CREATE INDEX idx_claims_policy ON claims(policy_id);
CREATE INDEX idx_claims_status ON claims(claim_status);
CREATE INDEX idx_claims_incident_date ON claims(incident_date);
CREATE INDEX idx_claims_reported_date ON claims(reported_date);
CREATE INDEX idx_claims_adjuster ON claims(adjuster_id);
CREATE INDEX idx_claims_fraud_flag ON claims(fraud_flag);
CREATE INDEX idx_claims_number ON claims(claim_number);
-- ============================================================================
-- PAYMENTS
-- ============================================================================
CREATE TABLE payments (
    payment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    payment_number VARCHAR(50) UNIQUE NOT NULL,
    policy_id UUID REFERENCES policies(policy_id),
    claim_id UUID REFERENCES claims(claim_id),
    payment_type VARCHAR(20) NOT NULL CHECK (
        payment_type IN (
            'PREMIUM',
            'CLAIM',
            'REFUND',
            'COMMISSION'
        )
    ),
    payment_direction VARCHAR(10) NOT NULL CHECK (payment_direction IN ('INBOUND', 'OUTBOUND')),
    -- Amount
    amount DECIMAL(12, 2) NOT NULL,
    currency VARCHAR(3) DEFAULT 'USD',
    -- Payment details
    payment_method VARCHAR(20) CHECK (
        payment_method IN (
            'CREDIT_CARD',
            'DEBIT_CARD',
            'ACH',
            'CHECK',
            'WIRE',
            'CASH'
        )
    ),
    payment_status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        payment_status IN (
            'PENDING',
            'PROCESSING',
            'COMPLETED',
            'FAILED',
            'REVERSED',
            'CANCELLED'
        )
    ),
    -- Dates
    payment_date DATE NOT NULL,
    due_date DATE,
    processed_date TIMESTAMP,
    -- Payment details
    transaction_id VARCHAR(100),
    confirmation_number VARCHAR(100),
    payment_processor VARCHAR(50),
    -- Failure handling
    failure_reason TEXT,
    retry_count INTEGER DEFAULT 0,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(100),
    CONSTRAINT payment_reference CHECK (
        (
            payment_type = 'PREMIUM'
            AND policy_id IS NOT NULL
        )
        OR (
            payment_type = 'CLAIM'
            AND claim_id IS NOT NULL
        )
        OR (payment_type IN ('REFUND', 'COMMISSION'))
    )
);
CREATE INDEX idx_payments_policy ON payments(policy_id);
CREATE INDEX idx_payments_claim ON payments(claim_id);
CREATE INDEX idx_payments_type ON payments(payment_type);
CREATE INDEX idx_payments_status ON payments(payment_status);
CREATE INDEX idx_payments_date ON payments(payment_date);
CREATE INDEX idx_payments_number ON payments(payment_number);
-- ============================================================================
-- DOCUMENTS
-- ============================================================================
CREATE TABLE documents (
    document_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID REFERENCES policies(policy_id),
    claim_id UUID REFERENCES claims(claim_id),
    customer_id UUID REFERENCES customers(customer_id),
    document_type VARCHAR(50) NOT NULL CHECK (
        document_type IN (
            'POLICY_CONTRACT',
            'DECLARATION',
            'ENDORSEMENT',
            'CLAIM_FORM',
            'PHOTO',
            'VIDEO',
            'REPORT',
            'ESTIMATE',
            'INVOICE',
            'CORRESPONDENCE'
        )
    ),
    document_name VARCHAR(255) NOT NULL,
    document_description TEXT,
    -- Storage (reference to MongoDB or object storage)
    storage_type VARCHAR(20) CHECK (
        storage_type IN ('MONGODB', 'S3', 'AZURE', 'LOCAL')
    ),
    storage_reference VARCHAR(500),
    -- MongoDB ObjectId or S3 key
    -- File metadata
    file_size_bytes BIGINT,
    mime_type VARCHAR(100),
    file_hash VARCHAR(64),
    -- SHA-256 hash
    -- Metadata
    uploaded_by UUID REFERENCES customers(customer_id),
    uploaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_documents_policy ON documents(policy_id);
CREATE INDEX idx_documents_claim ON documents(claim_id);
CREATE INDEX idx_documents_customer ON documents(customer_id);
CREATE INDEX idx_documents_type ON documents(document_type);
-- ============================================================================
-- AUDIT LOG
-- ============================================================================
CREATE TABLE audit_log (
    audit_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    table_name VARCHAR(100) NOT NULL,
    record_id UUID NOT NULL,
    action VARCHAR(20) NOT NULL CHECK (action IN ('INSERT', 'UPDATE', 'DELETE')),
    old_values JSONB,
    new_values JSONB,
    changed_by VARCHAR(100),
    changed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    ip_address INET,
    user_agent TEXT
);
CREATE INDEX idx_audit_table_record ON audit_log(table_name, record_id);
CREATE INDEX idx_audit_changed_at ON audit_log(changed_at);
-- ============================================================================
-- TRIGGERS FOR UPDATED_AT
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = CURRENT_TIMESTAMP;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER update_customers_updated_at BEFORE
UPDATE ON customers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_addresses_updated_at BEFORE
UPDATE ON addresses FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_agents_updated_at BEFORE
UPDATE ON agents FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_policies_updated_at BEFORE
UPDATE ON policies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_claims_updated_at BEFORE
UPDATE ON claims FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_payments_updated_at BEFORE
UPDATE ON payments FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active policies with customer info
CREATE VIEW v_active_policies AS
SELECT p.policy_id,
    p.policy_number,
    p.policy_type,
    p.policy_status,
    p.effective_date,
    p.expiration_date,
    p.premium_amount,
    p.coverage_amount,
    c.customer_id,
    c.customer_number,
    c.first_name,
    c.last_name,
    c.email,
    a.agent_id,
    a.first_name AS agent_first_name,
    a.last_name AS agent_last_name
FROM policies p
    JOIN customers c ON p.customer_id = c.customer_id
    LEFT JOIN agents a ON p.agent_id = a.agent_id
WHERE p.policy_status = 'ACTIVE'
    AND p.effective_date <= CURRENT_DATE
    AND p.expiration_date >= CURRENT_DATE;
-- Open claims summary
CREATE VIEW v_open_claims AS
SELECT cl.claim_id,
    cl.claim_number,
    cl.claim_type,
    cl.claim_status,
    cl.incident_date,
    cl.reported_date,
    cl.claim_amount,
    cl.approved_amount,
    cl.paid_amount,
    p.policy_number,
    p.policy_type,
    c.customer_number,
    c.first_name,
    c.last_name,
    adj.first_name AS adjuster_first_name,
    adj.last_name AS adjuster_last_name
FROM claims cl
    JOIN policies p ON cl.policy_id = p.policy_id
    JOIN customers c ON p.customer_id = c.customer_id
    LEFT JOIN agents adj ON cl.adjuster_id = adj.agent_id
WHERE cl.claim_status NOT IN ('SETTLED', 'CLOSED', 'DENIED', 'WITHDRAWN');
-- Customer policy summary
CREATE VIEW v_customer_policy_summary AS
SELECT c.customer_id,
    c.customer_number,
    c.first_name,
    c.last_name,
    c.email,
    COUNT(DISTINCT p.policy_id) AS total_policies,
    COUNT(
        DISTINCT CASE
            WHEN p.policy_status = 'ACTIVE' THEN p.policy_id
        END
    ) AS active_policies,
    SUM(
        CASE
            WHEN p.policy_status = 'ACTIVE' THEN p.premium_amount
            ELSE 0
        END
    ) AS total_premium,
    COUNT(DISTINCT cl.claim_id) AS total_claims,
    SUM(cl.claim_amount) AS total_claimed,
    SUM(cl.paid_amount) AS total_paid
FROM customers c
    LEFT JOIN policies p ON c.customer_id = p.customer_id
    LEFT JOIN claims cl ON p.policy_id = cl.policy_id
GROUP BY c.customer_id,
    c.customer_number,
    c.first_name,
    c.last_name,
    c.email;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE customers IS 'Core customer master data';
COMMENT ON TABLE addresses IS 'Customer and property addresses';
COMMENT ON TABLE agents IS 'Insurance agents and brokers';
COMMENT ON TABLE policies IS 'Master policy table for all insurance types';
COMMENT ON TABLE claims IS 'Master claims table for all insurance types';
COMMENT ON TABLE payments IS 'Payment transactions for premiums and claims';
COMMENT ON TABLE documents IS 'Document metadata with references to external storage';
COMMENT ON TABLE audit_log IS 'Audit trail for all data changes';
-- ============================================================================
-- GRANTS (Example - adjust based on your security model)
-- ============================================================================
-- GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO insurance_app;
-- GRANT SELECT ON ALL TABLES IN SCHEMA public TO insurance_readonly;