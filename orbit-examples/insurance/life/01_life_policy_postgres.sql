-- =============================================================================
-- OrbitRS Insurance Example: Life Insurance Policy Management
-- =============================================================================
-- Demonstrates PostgreSQL patterns for life insurance:
--   - Policy types (Term, Whole, Universal, Variable)
--   - Underwriting and health assessments
--   - Beneficiary management
--   - Cash value and dividends tracking
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_life_policy_postgres.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS life_insurance;

-- =============================================================================
-- INSURED AND APPLICANT TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS life_insurance.insureds (
    insured_id VARCHAR(50) PRIMARY KEY,
    customer_id VARCHAR(50) NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE NOT NULL,
    gender VARCHAR(10),
    ssn_last_four VARCHAR(4),
    email VARCHAR(200),
    phone VARCHAR(20),
    address_street VARCHAR(200),
    address_city VARCHAR(100),
    address_state VARCHAR(2),
    address_zip VARCHAR(10),
    occupation VARCHAR(100),
    annual_income DECIMAL(12,2),
    net_worth DECIMAL(14,2),
    marital_status VARCHAR(20),
    num_dependents INTEGER DEFAULT 0,
    is_smoker BOOLEAN DEFAULT false,
    tobacco_use_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS life_insurance.health_history (
    record_id VARCHAR(50) PRIMARY KEY,
    insured_id VARCHAR(50) REFERENCES life_insurance.insureds(insured_id),
    condition_type VARCHAR(50),
    -- HEART_DISEASE, CANCER, DIABETES, HYPERTENSION, STROKE,
    -- MENTAL_HEALTH, RESPIRATORY, NEUROLOGICAL, HIV_AIDS, OTHER
    condition_name VARCHAR(200),
    diagnosis_date DATE,
    current_status VARCHAR(20), -- ACTIVE, MANAGED, RESOLVED
    treatment VARCHAR(500),
    medications TEXT[],
    physician_name VARCHAR(200),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS life_insurance.underwriting (
    underwriting_id VARCHAR(50) PRIMARY KEY,
    insured_id VARCHAR(50) REFERENCES life_insurance.insureds(insured_id),
    application_id VARCHAR(50),
    underwriting_date DATE,
    height_inches INTEGER,
    weight_lbs INTEGER,
    bmi DECIMAL(4,1),
    blood_pressure_systolic INTEGER,
    blood_pressure_diastolic INTEGER,
    cholesterol_total INTEGER,
    cholesterol_hdl INTEGER,
    cholesterol_ldl INTEGER,
    glucose_level INTEGER,
    nicotine_test BOOLEAN,
    drug_test_result VARCHAR(20),
    driving_record VARCHAR(20), -- CLEAN, MINOR_VIOLATIONS, MAJOR_VIOLATIONS, DUI
    criminal_record BOOLEAN DEFAULT false,
    hazardous_activities TEXT[], -- SKYDIVING, SCUBA, MOUNTAINEERING, RACING
    foreign_travel TEXT[],
    risk_class VARCHAR(20),
    -- PREFERRED_PLUS, PREFERRED, STANDARD_PLUS, STANDARD, SUBSTANDARD, DECLINED
    table_rating INTEGER, -- For substandard cases (1-10)
    flat_extra_per_thousand DECIMAL(6,2),
    decision VARCHAR(20), -- APPROVED, DECLINED, POSTPONED, COUNTER_OFFER
    decision_date DATE,
    underwriter_id VARCHAR(50),
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- POLICY TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS life_insurance.policies (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    policy_type VARCHAR(20) NOT NULL,
    -- TERM_10, TERM_15, TERM_20, TERM_30, WHOLE_LIFE,
    -- UNIVERSAL_LIFE, VARIABLE_LIFE, VARIABLE_UNIVERSAL
    insured_id VARCHAR(50) REFERENCES life_insurance.insureds(insured_id),
    owner_id VARCHAR(50), -- May be different from insured
    issue_date DATE NOT NULL,
    effective_date DATE NOT NULL,
    maturity_date DATE,
    expiration_date DATE, -- For term policies
    face_amount DECIMAL(14,2) NOT NULL,
    premium_mode VARCHAR(20), -- ANNUAL, SEMI_ANNUAL, QUARTERLY, MONTHLY
    modal_premium DECIMAL(10,2),
    annual_premium DECIMAL(10,2),
    risk_class VARCHAR(20),
    status VARCHAR(20) DEFAULT 'ACTIVE',
    -- ACTIVE, LAPSED, SURRENDERED, PAID_UP, MATURED, DEATH_CLAIM, CONVERTED
    last_premium_date DATE,
    next_premium_due DATE,
    grace_period_end DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS life_insurance.beneficiaries (
    beneficiary_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES life_insurance.policies(policy_id),
    beneficiary_type VARCHAR(20), -- PRIMARY, CONTINGENT
    relationship VARCHAR(30), -- SPOUSE, CHILD, PARENT, SIBLING, TRUST, ESTATE, OTHER
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    entity_name VARCHAR(200), -- For trusts/organizations
    date_of_birth DATE,
    ssn_last_four VARCHAR(4),
    percentage DECIMAL(5,2), -- Percentage of benefit
    is_irrevocable BOOLEAN DEFAULT false,
    designation_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS life_insurance.riders (
    rider_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES life_insurance.policies(policy_id),
    rider_type VARCHAR(30),
    -- WAIVER_OF_PREMIUM, ACCIDENTAL_DEATH, GUARANTEED_INSURABILITY,
    -- CHILD_TERM, ACCELERATED_DEATH_BENEFIT, LONG_TERM_CARE,
    -- RETURN_OF_PREMIUM, DISABILITY_INCOME
    rider_name VARCHAR(100),
    benefit_amount DECIMAL(12,2),
    additional_premium DECIMAL(10,2),
    effective_date DATE,
    expiration_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- CASH VALUE AND PERFORMANCE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS life_insurance.policy_values (
    value_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES life_insurance.policies(policy_id),
    as_of_date DATE NOT NULL,
    cash_value DECIMAL(14,2),
    surrender_value DECIMAL(14,2),
    loan_value DECIMAL(14,2),
    death_benefit DECIMAL(14,2),
    accumulated_dividends DECIMAL(14,2),
    loan_balance DECIMAL(14,2) DEFAULT 0,
    loan_interest_accrued DECIMAL(12,2) DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS life_insurance.premium_payments (
    payment_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES life_insurance.policies(policy_id),
    payment_date DATE NOT NULL,
    due_date DATE,
    amount DECIMAL(10,2),
    payment_type VARCHAR(20), -- PREMIUM, LOAN_REPAYMENT, ADDITIONAL
    payment_method VARCHAR(20), -- CHECK, ACH, CREDIT_CARD, WIRE
    confirmation_number VARCHAR(50),
    status VARCHAR(20) DEFAULT 'COMPLETED',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS life_insurance.dividends (
    dividend_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES life_insurance.policies(policy_id),
    dividend_date DATE NOT NULL,
    dividend_amount DECIMAL(12,2),
    dividend_option VARCHAR(30),
    -- CASH, REDUCE_PREMIUM, PAID_UP_ADDITIONS, ACCUMULATE_INTEREST, TERM_PURCHASE
    applied_amount DECIMAL(12,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- CLAIMS TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS life_insurance.claims (
    claim_id VARCHAR(50) PRIMARY KEY,
    claim_number VARCHAR(30) UNIQUE NOT NULL,
    policy_id VARCHAR(50) REFERENCES life_insurance.policies(policy_id),
    claim_type VARCHAR(30),
    -- DEATH, ACCELERATED_DEATH_BENEFIT, WAIVER_OF_PREMIUM,
    -- DISABILITY, ACCIDENTAL_DEATH, TERMINAL_ILLNESS
    date_of_loss DATE NOT NULL,
    date_reported DATE DEFAULT CURRENT_DATE,
    cause_of_death VARCHAR(50),
    place_of_death VARCHAR(200),
    death_certificate_number VARCHAR(50),
    autopsy_performed BOOLEAN DEFAULT false,
    contestability_status VARCHAR(20), -- WITHIN_PERIOD, OUTSIDE_PERIOD
    investigation_required BOOLEAN DEFAULT false,
    face_amount DECIMAL(14,2),
    accidental_death_benefit DECIMAL(14,2) DEFAULT 0,
    other_benefits DECIMAL(14,2) DEFAULT 0,
    total_benefit DECIMAL(14,2),
    loan_offset DECIMAL(14,2) DEFAULT 0,
    net_benefit DECIMAL(14,2),
    status VARCHAR(20) DEFAULT 'PENDING',
    -- PENDING, UNDER_REVIEW, APPROVED, DENIED, PAID
    adjuster_id VARCHAR(50),
    decision_date DATE,
    payment_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Insureds
INSERT INTO life_insurance.insureds (insured_id, customer_id, first_name, last_name,
    date_of_birth, gender, email, phone, address_city, address_state, address_zip,
    occupation, annual_income, net_worth, marital_status, num_dependents, is_smoker)
VALUES
    ('INS-001', 'CUST-001', 'Michael', 'Johnson', '1980-05-20', 'MALE',
     'michael.j@email.com', '+1-555-0101', 'Chicago', 'IL', '60601',
     'Software Engineer', 150000.00, 500000.00, 'MARRIED', 2, false),
    ('INS-002', 'CUST-002', 'Sarah', 'Williams', '1975-09-12', 'FEMALE',
     'sarah.w@email.com', '+1-555-0102', 'Boston', 'MA', '02101',
     'Financial Analyst', 120000.00, 350000.00, 'MARRIED', 1, false),
    ('INS-003', 'CUST-003', 'David', 'Brown', '1985-03-08', 'MALE',
     'david.b@email.com', '+1-555-0103', 'Seattle', 'WA', '98101',
     'Construction Manager', 95000.00, 200000.00, 'SINGLE', 0, true)
ON CONFLICT (insured_id) DO NOTHING;

-- Underwriting records
INSERT INTO life_insurance.underwriting (underwriting_id, insured_id, application_id,
    underwriting_date, height_inches, weight_lbs, bmi, blood_pressure_systolic,
    blood_pressure_diastolic, cholesterol_total, nicotine_test, driving_record,
    risk_class, decision, decision_date)
VALUES
    ('UW-001', 'INS-001', 'APP-2024-001', '2024-01-10', 72, 185, 25.1, 120, 78,
     195, false, 'CLEAN', 'PREFERRED_PLUS', 'APPROVED', '2024-01-15'),
    ('UW-002', 'INS-002', 'APP-2024-002', '2024-01-20', 65, 140, 23.3, 118, 75,
     180, false, 'CLEAN', 'PREFERRED', 'APPROVED', '2024-01-25'),
    ('UW-003', 'INS-003', 'APP-2024-003', '2024-02-01', 70, 190, 27.3, 135, 88,
     220, true, 'MINOR_VIOLATIONS', 'STANDARD', 'APPROVED', '2024-02-10')
ON CONFLICT (underwriting_id) DO NOTHING;

-- Policies
INSERT INTO life_insurance.policies (policy_id, policy_number, policy_type, insured_id,
    owner_id, issue_date, effective_date, expiration_date, face_amount, premium_mode,
    modal_premium, annual_premium, risk_class, status, last_premium_date, next_premium_due)
VALUES
    ('POL-LIFE-001', 'LF-2024-00001', 'TERM_20', 'INS-001', 'INS-001',
     '2024-02-01', '2024-02-01', '2044-02-01', 1000000.00, 'MONTHLY',
     85.00, 1020.00, 'PREFERRED_PLUS', 'ACTIVE', '2024-11-01', '2024-12-01'),
    ('POL-LIFE-002', 'LF-2024-00002', 'WHOLE_LIFE', 'INS-002', 'INS-002',
     '2024-02-15', '2024-02-15', NULL, 500000.00, 'ANNUAL',
     4500.00, 4500.00, 'PREFERRED', 'ACTIVE', '2024-02-15', '2025-02-15'),
    ('POL-LIFE-003', 'LF-2024-00003', 'TERM_30', 'INS-003', 'INS-003',
     '2024-03-01', '2024-03-01', '2054-03-01', 750000.00, 'MONTHLY',
     125.00, 1500.00, 'STANDARD', 'ACTIVE', '2024-11-01', '2024-12-01')
ON CONFLICT (policy_id) DO NOTHING;

-- Beneficiaries
INSERT INTO life_insurance.beneficiaries (beneficiary_id, policy_id, beneficiary_type,
    relationship, first_name, last_name, percentage, designation_date)
VALUES
    ('BEN-001', 'POL-LIFE-001', 'PRIMARY', 'SPOUSE', 'Jennifer', 'Johnson', 100.00, '2024-02-01'),
    ('BEN-002', 'POL-LIFE-002', 'PRIMARY', 'SPOUSE', 'Robert', 'Williams', 60.00, '2024-02-15'),
    ('BEN-003', 'POL-LIFE-002', 'PRIMARY', 'CHILD', 'Emma', 'Williams', 40.00, '2024-02-15'),
    ('BEN-004', 'POL-LIFE-002', 'CONTINGENT', 'PARENT', 'Helen', 'Carter', 100.00, '2024-02-15'),
    ('BEN-005', 'POL-LIFE-003', 'PRIMARY', 'PARENT', 'Margaret', 'Brown', 100.00, '2024-03-01')
ON CONFLICT (beneficiary_id) DO NOTHING;

-- Riders
INSERT INTO life_insurance.riders (rider_id, policy_id, rider_type, rider_name,
    benefit_amount, additional_premium, effective_date, status)
VALUES
    ('RDR-001', 'POL-LIFE-001', 'WAIVER_OF_PREMIUM', 'Waiver of Premium', NULL, 8.50, '2024-02-01', 'ACTIVE'),
    ('RDR-002', 'POL-LIFE-001', 'ACCIDENTAL_DEATH', 'Accidental Death Benefit', 1000000.00, 12.00, '2024-02-01', 'ACTIVE'),
    ('RDR-003', 'POL-LIFE-002', 'ACCELERATED_DEATH_BENEFIT', 'Living Benefits Rider', 250000.00, 0.00, '2024-02-15', 'ACTIVE'),
    ('RDR-004', 'POL-LIFE-002', 'GUARANTEED_INSURABILITY', 'Future Increase Option', 250000.00, 25.00, '2024-02-15', 'ACTIVE')
ON CONFLICT (rider_id) DO NOTHING;

-- Policy values (for whole life policy)
INSERT INTO life_insurance.policy_values (value_id, policy_id, as_of_date,
    cash_value, surrender_value, loan_value, death_benefit, accumulated_dividends)
VALUES
    ('VAL-L-001', 'POL-LIFE-002', '2024-02-28', 0.00, 0.00, 0.00, 500000.00, 0.00),
    ('VAL-L-002', 'POL-LIFE-002', '2024-06-30', 850.00, 425.00, 765.00, 500000.00, 0.00),
    ('VAL-L-003', 'POL-LIFE-002', '2024-09-30', 2100.00, 1680.00, 1890.00, 500000.00, 45.00)
ON CONFLICT (value_id) DO NOTHING;

-- Premium payments
INSERT INTO life_insurance.premium_payments (payment_id, policy_id, payment_date,
    due_date, amount, payment_type, payment_method, status)
VALUES
    ('PAY-001', 'POL-LIFE-001', '2024-02-01', '2024-02-01', 85.00, 'PREMIUM', 'ACH', 'COMPLETED'),
    ('PAY-002', 'POL-LIFE-001', '2024-03-01', '2024-03-01', 85.00, 'PREMIUM', 'ACH', 'COMPLETED'),
    ('PAY-003', 'POL-LIFE-002', '2024-02-15', '2024-02-15', 4500.00, 'PREMIUM', 'CHECK', 'COMPLETED')
ON CONFLICT (payment_id) DO NOTHING;

-- =============================================================================
-- ANALYTICAL QUERIES
-- =============================================================================

-- Policy summary with beneficiaries
SELECT
    p.policy_number,
    p.policy_type,
    i.first_name || ' ' || i.last_name as insured_name,
    EXTRACT(YEAR FROM AGE(i.date_of_birth)) as current_age,
    p.face_amount,
    p.annual_premium,
    p.risk_class,
    p.status,
    STRING_AGG(b.first_name || ' ' || b.last_name || ' (' || b.percentage || '%)', ', ') as beneficiaries
FROM life_insurance.policies p
JOIN life_insurance.insureds i ON p.insured_id = i.insured_id
LEFT JOIN life_insurance.beneficiaries b ON p.policy_id = b.policy_id AND b.beneficiary_type = 'PRIMARY'
WHERE p.status = 'ACTIVE'
GROUP BY p.policy_id, p.policy_number, p.policy_type, i.first_name, i.last_name,
         i.date_of_birth, p.face_amount, p.annual_premium, p.risk_class, p.status
ORDER BY p.policy_number;

-- Underwriting risk distribution
SELECT
    risk_class,
    COUNT(*) as policy_count,
    SUM(face_amount) as total_face_amount,
    AVG(annual_premium) as avg_premium,
    AVG(face_amount) as avg_face_amount
FROM life_insurance.policies
WHERE status = 'ACTIVE'
GROUP BY risk_class
ORDER BY policy_count DESC;

-- Age band analysis
SELECT
    CASE
        WHEN EXTRACT(YEAR FROM AGE(i.date_of_birth)) < 30 THEN 'Under 30'
        WHEN EXTRACT(YEAR FROM AGE(i.date_of_birth)) < 40 THEN '30-39'
        WHEN EXTRACT(YEAR FROM AGE(i.date_of_birth)) < 50 THEN '40-49'
        WHEN EXTRACT(YEAR FROM AGE(i.date_of_birth)) < 60 THEN '50-59'
        ELSE '60+'
    END as age_band,
    p.policy_type,
    COUNT(*) as policy_count,
    AVG(p.face_amount) as avg_face_amount,
    AVG(p.annual_premium / (p.face_amount / 1000)) as avg_rate_per_thousand
FROM life_insurance.policies p
JOIN life_insurance.insureds i ON p.insured_id = i.insured_id
WHERE p.status = 'ACTIVE'
GROUP BY age_band, p.policy_type
ORDER BY age_band, p.policy_type;

-- Cash value growth (for permanent policies)
SELECT
    p.policy_number,
    p.policy_type,
    pv.as_of_date,
    pv.cash_value,
    pv.death_benefit,
    pv.accumulated_dividends,
    pv.cash_value - LAG(pv.cash_value) OVER (PARTITION BY p.policy_id ORDER BY pv.as_of_date) as value_change
FROM life_insurance.policies p
JOIN life_insurance.policy_values pv ON p.policy_id = pv.policy_id
WHERE p.policy_type IN ('WHOLE_LIFE', 'UNIVERSAL_LIFE', 'VARIABLE_LIFE')
ORDER BY p.policy_number, pv.as_of_date;

-- Rider summary
SELECT
    r.rider_type,
    COUNT(*) as rider_count,
    SUM(r.additional_premium) as total_additional_premium,
    COUNT(DISTINCT r.policy_id) as policies_with_rider
FROM life_insurance.riders r
WHERE r.status = 'ACTIVE'
GROUP BY r.rider_type
ORDER BY rider_count DESC;

-- Premium payment status
SELECT
    p.policy_number,
    p.next_premium_due,
    p.grace_period_end,
    CASE
        WHEN p.next_premium_due > CURRENT_DATE THEN 'CURRENT'
        WHEN p.grace_period_end >= CURRENT_DATE THEN 'IN_GRACE'
        ELSE 'PAST_DUE'
    END as payment_status,
    CURRENT_DATE - p.next_premium_due as days_past_due
FROM life_insurance.policies p
WHERE p.status = 'ACTIVE'
ORDER BY p.next_premium_due;

-- Smoker vs non-smoker premium comparison
SELECT
    i.is_smoker,
    p.policy_type,
    COUNT(*) as policy_count,
    AVG(p.annual_premium) as avg_annual_premium,
    AVG(p.annual_premium / (p.face_amount / 1000)) as avg_rate_per_thousand,
    AVG(p.face_amount) as avg_face_amount
FROM life_insurance.policies p
JOIN life_insurance.insureds i ON p.insured_id = i.insured_id
WHERE p.status = 'ACTIVE'
GROUP BY i.is_smoker, p.policy_type
ORDER BY i.is_smoker, p.policy_type;
