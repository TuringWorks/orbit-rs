-- ============================================================================
-- OrbitRS Telco Examples - Core Schema
-- ============================================================================
-- Core telecommunications entities: subscribers, accounts, addresses, contacts
-- ============================================================================
-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "postgis";
-- For geospatial data
-- ============================================================================
-- SUBSCRIBERS
-- ============================================================================
CREATE TABLE subscribers (
    subscriber_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Identifiers
    msisdn VARCHAR(15) UNIQUE NOT NULL,
    -- Mobile Station International Subscriber Directory Number (phone number)
    imsi VARCHAR(15) UNIQUE,
    -- International Mobile Subscriber Identity
    subscriber_number VARCHAR(50) UNIQUE NOT NULL,
    -- Internal subscriber number
    -- Personal Information
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    middle_name VARCHAR(100),
    date_of_birth DATE NOT NULL,
    ssn_last4 VARCHAR(4),
    -- Last 4 digits for verification
    email VARCHAR(255) UNIQUE NOT NULL,
    alternate_email VARCHAR(255),
    -- Account Type
    account_type VARCHAR(20) NOT NULL CHECK (
        account_type IN ('INDIVIDUAL', 'BUSINESS', 'GOVERNMENT')
    ),
    credit_class VARCHAR(20) NOT NULL DEFAULT 'POSTPAID' CHECK (
        credit_class IN ('POSTPAID', 'PREPAID', 'HYBRID')
    ),
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'ACTIVE',
            'SUSPENDED',
            'BARRED',
            'TERMINATED',
            'FRAUD_HOLD'
        )
    ),
    status_reason TEXT,
    -- Lifecycle Dates
    registration_date DATE DEFAULT CURRENT_DATE,
    activation_date DATE,
    suspension_date DATE,
    termination_date DATE,
    -- Credit and Risk
    credit_score INTEGER CHECK (
        credit_score BETWEEN 300 AND 850
    ),
    credit_limit DECIMAL(10, 2),
    deposit_required BOOLEAN DEFAULT FALSE,
    deposit_amount DECIMAL(10, 2),
    risk_category VARCHAR(20) CHECK (
        risk_category IN ('LOW', 'MEDIUM', 'HIGH', 'CRITICAL')
    ),
    -- Preferences
    language_preference VARCHAR(10) DEFAULT 'en',
    communication_preference VARCHAR(20) DEFAULT 'EMAIL' CHECK (
        communication_preference IN ('EMAIL', 'SMS', 'PHONE', 'MAIL')
    ),
    paperless_billing BOOLEAN DEFAULT TRUE,
    marketing_opt_in BOOLEAN DEFAULT FALSE,
    data_sharing_consent BOOLEAN DEFAULT FALSE,
    -- KYC (Know Your Customer)
    kyc_status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        kyc_status IN ('PENDING', 'VERIFIED', 'FAILED', 'EXPIRED')
    ),
    kyc_verified_date DATE,
    kyc_document_type VARCHAR(50),
    kyc_document_number VARCHAR(100),
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(100),
    updated_by VARCHAR(100)
);
CREATE INDEX idx_subscribers_msisdn ON subscribers(msisdn);
CREATE INDEX idx_subscribers_imsi ON subscribers(imsi);
CREATE INDEX idx_subscribers_status ON subscribers(status);
CREATE INDEX idx_subscribers_credit_class ON subscribers(credit_class);
CREATE INDEX idx_subscribers_email ON subscribers(email);
-- ============================================================================
-- ACCOUNTS
-- ============================================================================
CREATE TABLE accounts (
    account_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_number VARCHAR(50) UNIQUE NOT NULL,
    -- Account Hierarchy
    parent_account_id UUID REFERENCES accounts(account_id),
    account_level INTEGER DEFAULT 1,
    -- 1=master, 2=sub-account, etc.
    -- Account Owner
    primary_subscriber_id UUID NOT NULL REFERENCES subscribers(subscriber_id),
    -- Account Type
    account_type VARCHAR(20) NOT NULL CHECK (
        account_type IN (
            'INDIVIDUAL',
            'FAMILY',
            'BUSINESS',
            'ENTERPRISE',
            'GOVERNMENT'
        )
    ),
    -- Billing
    billing_cycle_day INTEGER CHECK (
        billing_cycle_day BETWEEN 1 AND 28
    ),
    billing_currency VARCHAR(3) DEFAULT 'USD',
    payment_terms VARCHAR(20) DEFAULT 'NET_30',
    auto_pay_enabled BOOLEAN DEFAULT FALSE,
    -- Credit
    credit_limit DECIMAL(12, 2),
    current_balance DECIMAL(12, 2) DEFAULT 0,
    available_credit DECIMAL(12, 2),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'SUSPENDED',
            'COLLECTIONS',
            'CLOSED'
        )
    ),
    -- Dates
    opened_date DATE DEFAULT CURRENT_DATE,
    closed_date DATE,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_accounts_primary_subscriber ON accounts(primary_subscriber_id);
CREATE INDEX idx_accounts_parent ON accounts(parent_account_id);
CREATE INDEX idx_accounts_status ON accounts(status);
-- ============================================================================
-- ADDRESSES
-- ============================================================================
CREATE TABLE addresses (
    address_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subscriber_id UUID REFERENCES subscribers(subscriber_id) ON DELETE CASCADE,
    account_id UUID REFERENCES accounts(account_id) ON DELETE CASCADE,
    address_type VARCHAR(20) NOT NULL CHECK (
        address_type IN (
            'SERVICE',
            'BILLING',
            'SHIPPING',
            'INSTALLATION'
        )
    ),
    -- Address Components
    street_address_1 VARCHAR(255) NOT NULL,
    street_address_2 VARCHAR(255),
    city VARCHAR(100) NOT NULL,
    state VARCHAR(50) NOT NULL,
    postal_code VARCHAR(20) NOT NULL,
    country VARCHAR(2) DEFAULT 'US',
    -- Geospatial
    location GEOGRAPHY(POINT, 4326),
    -- PostGIS point (longitude, latitude)
    latitude DECIMAL(10, 8),
    longitude DECIMAL(11, 8),
    -- Coverage
    coverage_area_id UUID,
    -- Reference to coverage areas
    cell_tower_id UUID,
    -- Nearest cell tower
    -- Validation
    is_validated BOOLEAN DEFAULT FALSE,
    validation_date DATE,
    is_serviceable BOOLEAN DEFAULT TRUE,
    -- Primary Address
    is_primary BOOLEAN DEFAULT FALSE,
    -- Dates
    valid_from DATE DEFAULT CURRENT_DATE,
    valid_to DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_addresses_subscriber ON addresses(subscriber_id);
CREATE INDEX idx_addresses_account ON addresses(account_id);
CREATE INDEX idx_addresses_type ON addresses(address_type);
CREATE INDEX idx_addresses_location ON addresses USING GIST(location);
-- ============================================================================
-- CONTACTS
-- ============================================================================
CREATE TABLE contacts (
    contact_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subscriber_id UUID REFERENCES subscribers(subscriber_id) ON DELETE CASCADE,
    account_id UUID REFERENCES accounts(account_id) ON DELETE CASCADE,
    contact_type VARCHAR(20) NOT NULL CHECK (
        contact_type IN (
            'PRIMARY',
            'SECONDARY',
            'EMERGENCY',
            'BILLING',
            'TECHNICAL'
        )
    ),
    -- Contact Information
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    relationship VARCHAR(50),
    -- Phone Numbers
    phone_number VARCHAR(20),
    phone_type VARCHAR(20) CHECK (phone_type IN ('MOBILE', 'HOME', 'WORK', 'FAX')),
    -- Email
    email VARCHAR(255),
    -- Preferences
    preferred_contact_method VARCHAR(20) CHECK (
        preferred_contact_method IN ('PHONE', 'EMAIL', 'SMS')
    ),
    can_authorize_changes BOOLEAN DEFAULT FALSE,
    -- Verification
    is_verified BOOLEAN DEFAULT FALSE,
    verified_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_contacts_subscriber ON contacts(subscriber_id);
CREATE INDEX idx_contacts_account ON contacts(account_id);
CREATE INDEX idx_contacts_type ON contacts(contact_type);
-- ============================================================================
-- CREDIT PROFILES
-- ============================================================================
CREATE TABLE credit_profiles (
    credit_profile_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subscriber_id UUID NOT NULL UNIQUE REFERENCES subscribers(subscriber_id) ON DELETE CASCADE,
    -- Credit Score
    credit_score INTEGER CHECK (
        credit_score BETWEEN 300 AND 850
    ),
    credit_bureau VARCHAR(50),
    -- Equifax, Experian, TransUnion
    score_date DATE,
    -- Credit Limits
    approved_credit_limit DECIMAL(10, 2),
    current_credit_limit DECIMAL(10, 2),
    temporary_credit_increase DECIMAL(10, 2),
    temporary_increase_expiry DATE,
    -- Deposit
    deposit_required BOOLEAN DEFAULT FALSE,
    deposit_amount DECIMAL(10, 2),
    deposit_status VARCHAR(20) CHECK (
        deposit_status IN ('PENDING', 'PAID', 'REFUNDED', 'FORFEITED')
    ),
    deposit_refund_date DATE,
    -- Payment History
    on_time_payments INTEGER DEFAULT 0,
    late_payments INTEGER DEFAULT 0,
    missed_payments INTEGER DEFAULT 0,
    payment_score DECIMAL(5, 2),
    -- 0-100
    -- Risk Assessment
    risk_category VARCHAR(20) CHECK (
        risk_category IN ('LOW', 'MEDIUM', 'HIGH', 'CRITICAL')
    ),
    fraud_score INTEGER CHECK (
        fraud_score BETWEEN 0 AND 100
    ),
    collection_risk BOOLEAN DEFAULT FALSE,
    -- Credit Review
    last_review_date DATE,
    next_review_date DATE,
    review_frequency_days INTEGER DEFAULT 90,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_credit_profiles_subscriber ON credit_profiles(subscriber_id);
CREATE INDEX idx_credit_profiles_risk ON credit_profiles(risk_category);
-- ============================================================================
-- SUBSCRIBER NOTES
-- ============================================================================
CREATE TABLE subscriber_notes (
    note_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    subscriber_id UUID NOT NULL REFERENCES subscribers(subscriber_id) ON DELETE CASCADE,
    note_type VARCHAR(20) CHECK (
        note_type IN (
            'GENERAL',
            'BILLING',
            'TECHNICAL',
            'FRAUD',
            'COLLECTION',
            'COMPLAINT'
        )
    ),
    note_category VARCHAR(50),
    note_text TEXT NOT NULL,
    -- Visibility
    is_internal BOOLEAN DEFAULT TRUE,
    is_sensitive BOOLEAN DEFAULT FALSE,
    -- Author
    created_by VARCHAR(100) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_notes_subscriber ON subscriber_notes(subscriber_id);
CREATE INDEX idx_notes_type ON subscriber_notes(note_type);
CREATE INDEX idx_notes_created_at ON subscriber_notes(created_at);
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
    user_agent TEXT,
    -- Context
    session_id VARCHAR(100),
    transaction_id VARCHAR(100)
);
CREATE INDEX idx_audit_table_record ON audit_log(table_name, record_id);
CREATE INDEX idx_audit_changed_at ON audit_log(changed_at);
CREATE INDEX idx_audit_changed_by ON audit_log(changed_by);
-- ============================================================================
-- TRIGGERS FOR UPDATED_AT
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = CURRENT_TIMESTAMP;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER update_subscribers_updated_at BEFORE
UPDATE ON subscribers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_accounts_updated_at BEFORE
UPDATE ON accounts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_addresses_updated_at BEFORE
UPDATE ON addresses FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_contacts_updated_at BEFORE
UPDATE ON contacts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_credit_profiles_updated_at BEFORE
UPDATE ON credit_profiles FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active subscribers with account info
CREATE VIEW v_active_subscribers AS
SELECT s.subscriber_id,
    s.msisdn,
    s.imsi,
    s.first_name,
    s.last_name,
    s.email,
    s.credit_class,
    s.status,
    a.account_id,
    a.account_number,
    a.account_type,
    a.current_balance,
    cp.credit_score,
    cp.risk_category
FROM subscribers s
    LEFT JOIN accounts a ON s.subscriber_id = a.primary_subscriber_id
    LEFT JOIN credit_profiles cp ON s.subscriber_id = cp.subscriber_id
WHERE s.status = 'ACTIVE';
-- Subscriber summary
CREATE VIEW v_subscriber_summary AS
SELECT s.subscriber_id,
    s.msisdn,
    s.first_name,
    s.last_name,
    s.status,
    s.credit_class,
    s.activation_date,
    EXTRACT(
        DAYS
        FROM (CURRENT_DATE - s.activation_date)
    ) AS days_active,
    a.account_number,
    a.current_balance,
    cp.credit_score,
    cp.risk_category,
    COUNT(DISTINCT c.contact_id) AS contact_count,
    COUNT(DISTINCT addr.address_id) AS address_count
FROM subscribers s
    LEFT JOIN accounts a ON s.subscriber_id = a.primary_subscriber_id
    LEFT JOIN credit_profiles cp ON s.subscriber_id = cp.subscriber_id
    LEFT JOIN contacts c ON s.subscriber_id = c.subscriber_id
    LEFT JOIN addresses addr ON s.subscriber_id = addr.subscriber_id
GROUP BY s.subscriber_id,
    s.msisdn,
    s.first_name,
    s.last_name,
    s.status,
    s.credit_class,
    s.activation_date,
    a.account_number,
    a.current_balance,
    cp.credit_score,
    cp.risk_category;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE subscribers IS 'Core subscriber master data with MSISDN and IMSI';
COMMENT ON TABLE accounts IS 'Account hierarchy and billing relationships';
COMMENT ON TABLE addresses IS 'Service and billing addresses with geospatial data';
COMMENT ON TABLE contacts IS 'Contact information for subscribers';
COMMENT ON TABLE credit_profiles IS 'Credit scoring and risk assessment';
COMMENT ON TABLE subscriber_notes IS 'Notes and comments about subscribers';
COMMENT ON TABLE audit_log IS 'Audit trail for all data changes';
COMMENT ON COLUMN subscribers.msisdn IS 'Mobile phone number in E.164 format';
COMMENT ON COLUMN subscribers.imsi IS 'Unique identifier for SIM card';
COMMENT ON COLUMN subscribers.credit_class IS 'Postpaid or prepaid billing';
COMMENT ON COLUMN addresses.location IS 'PostGIS geography point for spatial queries';