-- =============================================================================
-- OrbitRS Insurance Example: Industrial and Commercial Insurance
-- =============================================================================
-- Demonstrates PostgreSQL patterns for industrial/commercial insurance:
--   - Workers compensation
--   - General liability
--   - Professional liability (E&O)
--   - Product liability
--   - Cyber liability
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_commercial_industrial.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS commercial_insurance;

-- =============================================================================
-- BUSINESS AND CLASSIFICATION TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS commercial_insurance.businesses (
    business_id VARCHAR(50) PRIMARY KEY,
    legal_name VARCHAR(200) NOT NULL,
    dba_name VARCHAR(200),
    entity_type VARCHAR(30), -- CORPORATION, LLC, PARTNERSHIP, SOLE_PROP
    tax_id VARCHAR(20),
    naics_code VARCHAR(10),
    sic_code VARCHAR(10),
    industry_description VARCHAR(200),
    -- Business details
    year_established INTEGER,
    num_employees INTEGER,
    annual_revenue DECIMAL(16,2),
    annual_payroll DECIMAL(14,2),
    -- Location
    hq_address_street VARCHAR(200),
    hq_address_city VARCHAR(100),
    hq_address_state VARCHAR(2),
    hq_address_zip VARCHAR(10),
    num_locations INTEGER DEFAULT 1,
    -- Risk factors
    years_in_business INTEGER,
    prior_losses_3yr INTEGER DEFAULT 0,
    prior_claims_amount DECIMAL(14,2) DEFAULT 0,
    experience_mod DECIMAL(4,3) DEFAULT 1.000, -- Workers comp mod
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS commercial_insurance.class_codes (
    class_code VARCHAR(10) PRIMARY KEY,
    class_description VARCHAR(300) NOT NULL,
    line_of_business VARCHAR(30), -- WC, GL, AUTO, PROPERTY
    hazard_group VARCHAR(5), -- I, II, III, IV (increasing risk)
    base_rate DECIMAL(10,4),
    effective_date DATE,
    state VARCHAR(2), -- NULL for countrywide
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS commercial_insurance.business_operations (
    operation_id VARCHAR(50) PRIMARY KEY,
    business_id VARCHAR(50) REFERENCES commercial_insurance.businesses(business_id),
    class_code VARCHAR(10) REFERENCES commercial_insurance.class_codes(class_code),
    operation_description VARCHAR(300),
    annual_payroll DECIMAL(14,2), -- For WC
    annual_receipts DECIMAL(16,2), -- For GL
    num_employees INTEGER,
    square_footage INTEGER,
    exposure_units DECIMAL(12,2), -- Varies by class
    exposure_base VARCHAR(30), -- PAYROLL, SALES, AREA, PER_UNIT
    is_primary BOOLEAN DEFAULT false,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- WORKERS COMPENSATION TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS commercial_insurance.wc_policies (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    business_id VARCHAR(50) REFERENCES commercial_insurance.businesses(business_id),
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    -- Coverage
    coverage_states TEXT[],
    employer_liability_limit_each DECIMAL(12,2) DEFAULT 1000000,
    employer_liability_limit_disease DECIMAL(12,2) DEFAULT 1000000,
    employer_liability_limit_policy DECIMAL(12,2) DEFAULT 1000000,
    -- Rating
    total_payroll DECIMAL(14,2),
    manual_premium DECIMAL(12,2),
    experience_mod DECIMAL(4,3) DEFAULT 1.000,
    schedule_mod DECIMAL(4,3) DEFAULT 1.000,
    modified_premium DECIMAL(12,2),
    expense_constant DECIMAL(8,2),
    total_premium DECIMAL(12,2),
    -- Deductible options
    deductible_type VARCHAR(20), -- NONE, MEDICAL, INDEMNITY, COMBINED
    deductible_amount DECIMAL(10,2) DEFAULT 0,
    deductible_aggregate DECIMAL(12,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS commercial_insurance.wc_class_details (
    detail_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES commercial_insurance.wc_policies(policy_id),
    class_code VARCHAR(10),
    class_description VARCHAR(300),
    state VARCHAR(2),
    payroll DECIMAL(14,2),
    rate_per_hundred DECIMAL(8,4),
    manual_premium DECIMAL(12,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS commercial_insurance.wc_claims (
    claim_id VARCHAR(50) PRIMARY KEY,
    claim_number VARCHAR(30) UNIQUE NOT NULL,
    policy_id VARCHAR(50) REFERENCES commercial_insurance.wc_policies(policy_id),
    injury_date DATE NOT NULL,
    report_date DATE DEFAULT CURRENT_DATE,
    employee_name VARCHAR(200),
    employee_id VARCHAR(50),
    class_code VARCHAR(10),
    -- Injury details
    injury_type VARCHAR(50), -- STRAIN, LACERATION, FRACTURE, BURN, CONTUSION, AMPUTATION
    body_part VARCHAR(50),
    cause_of_injury VARCHAR(100),
    nature_of_injury VARCHAR(100),
    location VARCHAR(200),
    -- Claim type
    claim_type VARCHAR(30), -- MEDICAL_ONLY, LOST_TIME, PERMANENT_PARTIAL, PERMANENT_TOTAL, FATALITY
    days_away INTEGER DEFAULT 0,
    days_restricted INTEGER DEFAULT 0,
    return_to_work_date DATE,
    -- Financials
    medical_paid DECIMAL(12,2) DEFAULT 0,
    medical_reserved DECIMAL(12,2) DEFAULT 0,
    indemnity_paid DECIMAL(12,2) DEFAULT 0,
    indemnity_reserved DECIMAL(12,2) DEFAULT 0,
    expense_paid DECIMAL(12,2) DEFAULT 0,
    total_incurred DECIMAL(12,2),
    status VARCHAR(20) DEFAULT 'OPEN',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- GENERAL LIABILITY TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS commercial_insurance.gl_policies (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    business_id VARCHAR(50) REFERENCES commercial_insurance.businesses(business_id),
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    policy_form VARCHAR(20), -- OCCURRENCE, CLAIMS_MADE
    -- Limits
    each_occurrence_limit DECIMAL(12,2) DEFAULT 1000000,
    general_aggregate_limit DECIMAL(12,2) DEFAULT 2000000,
    products_aggregate_limit DECIMAL(12,2) DEFAULT 2000000,
    personal_injury_limit DECIMAL(12,2) DEFAULT 1000000,
    damage_to_rented_premises DECIMAL(12,2) DEFAULT 100000,
    medical_expense_limit DECIMAL(10,2) DEFAULT 5000,
    -- Premium
    total_exposure DECIMAL(16,2),
    base_premium DECIMAL(12,2),
    schedule_mod DECIMAL(4,3) DEFAULT 1.000,
    total_premium DECIMAL(12,2),
    -- Deductible
    deductible DECIMAL(10,2) DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS commercial_insurance.gl_claims (
    claim_id VARCHAR(50) PRIMARY KEY,
    claim_number VARCHAR(30) UNIQUE NOT NULL,
    policy_id VARCHAR(50) REFERENCES commercial_insurance.gl_policies(policy_id),
    occurrence_date DATE NOT NULL,
    report_date DATE DEFAULT CURRENT_DATE,
    -- Claim details
    claim_type VARCHAR(50),
    -- BODILY_INJURY, PROPERTY_DAMAGE, PERSONAL_INJURY, ADVERTISING_INJURY,
    -- PRODUCTS_LIABILITY, COMPLETED_OPERATIONS
    claimant_name VARCHAR(200),
    claimant_attorney VARCHAR(200),
    loss_location VARCHAR(300),
    loss_description TEXT,
    -- Coverage
    coverage_applicable VARCHAR(50),
    coverage_limit_applicable DECIMAL(12,2),
    deductible_applicable DECIMAL(10,2),
    -- Financials
    indemnity_paid DECIMAL(12,2) DEFAULT 0,
    indemnity_reserved DECIMAL(12,2) DEFAULT 0,
    expense_paid DECIMAL(12,2) DEFAULT 0,
    expense_reserved DECIMAL(12,2) DEFAULT 0,
    total_incurred DECIMAL(12,2),
    status VARCHAR(20) DEFAULT 'OPEN',
    litigation_status VARCHAR(20), -- PRE_SUIT, SUIT_FILED, TRIAL_SET, SETTLED
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- PROFESSIONAL LIABILITY (E&O) TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS commercial_insurance.professional_liability (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    business_id VARCHAR(50) REFERENCES commercial_insurance.businesses(business_id),
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    policy_form VARCHAR(20) DEFAULT 'CLAIMS_MADE',
    retroactive_date DATE,
    -- Coverage
    professional_type VARCHAR(50), -- TECHNOLOGY, ACCOUNTING, LEGAL, MEDICAL, CONSULTING, ARCHITECT
    each_claim_limit DECIMAL(12,2),
    aggregate_limit DECIMAL(12,2),
    deductible DECIMAL(10,2),
    -- Extended reporting period options
    erp_available BOOLEAN DEFAULT true,
    erp_1yr_cost_percent DECIMAL(5,2),
    erp_3yr_cost_percent DECIMAL(5,2),
    -- Premium
    annual_revenue DECIMAL(16,2),
    num_professionals INTEGER,
    base_premium DECIMAL(12,2),
    total_premium DECIMAL(12,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- CYBER LIABILITY TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS commercial_insurance.cyber_policies (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    business_id VARCHAR(50) REFERENCES commercial_insurance.businesses(business_id),
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    -- First party coverages
    data_breach_limit DECIMAL(12,2),
    business_interruption_limit DECIMAL(12,2),
    cyber_extortion_limit DECIMAL(12,2),
    data_restoration_limit DECIMAL(12,2),
    -- Third party coverages
    privacy_liability_limit DECIMAL(12,2),
    network_security_limit DECIMAL(12,2),
    media_liability_limit DECIMAL(12,2),
    -- Aggregate and deductible
    aggregate_limit DECIMAL(12,2),
    retention DECIMAL(10,2),
    waiting_period_hours INTEGER DEFAULT 8,
    -- Underwriting factors
    annual_revenue DECIMAL(16,2),
    records_count INTEGER, -- PII/PHI records
    pci_compliant BOOLEAN DEFAULT false,
    prior_breaches INTEGER DEFAULT 0,
    security_score INTEGER, -- 0-100
    -- Premium
    base_premium DECIMAL(12,2),
    total_premium DECIMAL(12,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS commercial_insurance.cyber_incidents (
    incident_id VARCHAR(50) PRIMARY KEY,
    claim_number VARCHAR(30) UNIQUE NOT NULL,
    policy_id VARCHAR(50) REFERENCES commercial_insurance.cyber_policies(policy_id),
    discovery_date DATE NOT NULL,
    incident_date DATE,
    report_date DATE DEFAULT CURRENT_DATE,
    -- Incident details
    incident_type VARCHAR(50),
    -- RANSOMWARE, DATA_BREACH, PHISHING, DDOS, MALWARE, SOCIAL_ENGINEERING,
    -- INSIDER_THREAT, SYSTEM_FAILURE, VENDOR_BREACH
    attack_vector VARCHAR(50),
    records_compromised INTEGER,
    data_types_affected TEXT[], -- PII, PHI, PCI, CREDENTIALS
    systems_affected TEXT[],
    downtime_hours INTEGER,
    -- Response and costs
    forensic_cost DECIMAL(12,2) DEFAULT 0,
    notification_cost DECIMAL(12,2) DEFAULT 0,
    credit_monitoring_cost DECIMAL(12,2) DEFAULT 0,
    legal_cost DECIMAL(12,2) DEFAULT 0,
    regulatory_fines DECIMAL(12,2) DEFAULT 0,
    ransom_paid DECIMAL(12,2) DEFAULT 0,
    business_interruption_loss DECIMAL(12,2) DEFAULT 0,
    third_party_claims DECIMAL(12,2) DEFAULT 0,
    total_loss DECIMAL(14,2),
    total_paid DECIMAL(14,2) DEFAULT 0,
    status VARCHAR(20) DEFAULT 'OPEN',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Businesses
INSERT INTO commercial_insurance.businesses (business_id, legal_name, dba_name,
    entity_type, naics_code, industry_description, year_established,
    num_employees, annual_revenue, annual_payroll, hq_address_city, hq_address_state,
    years_in_business, prior_losses_3yr, experience_mod)
VALUES
    ('BUS-001', 'TechStart Software Inc', 'TechStart', 'CORPORATION', '541511',
     'Custom Computer Programming Services', 2018, 75, 12000000.00, 6500000.00,
     'Austin', 'TX', 6, 1, 0.950),
    ('BUS-002', 'Acme Manufacturing Corp', 'Acme Mfg', 'CORPORATION', '332710',
     'Machine Shops', 1985, 250, 45000000.00, 18000000.00,
     'Cleveland', 'OH', 39, 8, 1.150),
    ('BUS-003', 'HealthFirst Medical Group', 'HealthFirst', 'LLC', '621111',
     'Offices of Physicians', 2010, 120, 28000000.00, 12000000.00,
     'Phoenix', 'AZ', 14, 3, 1.000)
ON CONFLICT (business_id) DO NOTHING;

-- Class codes
INSERT INTO commercial_insurance.class_codes (class_code, class_description,
    line_of_business, hazard_group, base_rate)
VALUES
    ('8810', 'Clerical Office Employees', 'WC', 'I', 0.18),
    ('8742', 'Salespersons - Outside', 'WC', 'II', 0.45),
    ('3632', 'Machine Shop NOC', 'WC', 'III', 3.85),
    ('8832', 'Physician & Clerical', 'WC', 'I', 0.25),
    ('8868', 'College - Professional Employees', 'WC', 'I', 0.15),
    ('41668', 'Computer Programming', 'GL', 'I', 0.25),
    ('59999', 'Manufacturing - Metal Products', 'GL', 'III', 1.85)
ON CONFLICT (class_code) DO NOTHING;

-- Workers Comp policies
INSERT INTO commercial_insurance.wc_policies (policy_id, policy_number, business_id,
    effective_date, expiration_date, status, coverage_states,
    total_payroll, manual_premium, experience_mod, schedule_mod, modified_premium, total_premium)
VALUES
    ('WC-001', 'WC-2024-00001', 'BUS-001', '2024-01-01', '2025-01-01', 'ACTIVE',
     ARRAY['TX'], 6500000.00, 11700.00, 0.950, 0.950, 10556.00, 10856.00),
    ('WC-002', 'WC-2024-00002', 'BUS-002', '2024-01-01', '2025-01-01', 'ACTIVE',
     ARRAY['OH', 'MI', 'IN'], 18000000.00, 693000.00, 1.150, 1.050, 837157.50, 837457.50),
    ('WC-003', 'WC-2024-00003', 'BUS-003', '2024-01-01', '2025-01-01', 'ACTIVE',
     ARRAY['AZ'], 12000000.00, 30000.00, 1.000, 0.900, 27000.00, 27300.00)
ON CONFLICT (policy_id) DO NOTHING;

-- General Liability policies
INSERT INTO commercial_insurance.gl_policies (policy_id, policy_number, business_id,
    effective_date, expiration_date, status, policy_form,
    each_occurrence_limit, general_aggregate_limit, products_aggregate_limit,
    total_exposure, base_premium, total_premium)
VALUES
    ('GL-001', 'GL-2024-00001', 'BUS-001', '2024-01-01', '2025-01-01', 'ACTIVE', 'OCCURRENCE',
     1000000.00, 2000000.00, 2000000.00, 12000000.00, 4500.00, 4500.00),
    ('GL-002', 'GL-2024-00002', 'BUS-002', '2024-01-01', '2025-01-01', 'ACTIVE', 'OCCURRENCE',
     2000000.00, 4000000.00, 4000000.00, 45000000.00, 85000.00, 89250.00),
    ('GL-003', 'GL-2024-00003', 'BUS-003', '2024-01-01', '2025-01-01', 'ACTIVE', 'OCCURRENCE',
     1000000.00, 3000000.00, 3000000.00, 28000000.00, 12000.00, 12000.00)
ON CONFLICT (policy_id) DO NOTHING;

-- Professional Liability
INSERT INTO commercial_insurance.professional_liability (policy_id, policy_number,
    business_id, effective_date, expiration_date, status, retroactive_date,
    professional_type, each_claim_limit, aggregate_limit, deductible,
    annual_revenue, num_professionals, total_premium)
VALUES
    ('PL-001', 'PL-2024-00001', 'BUS-001', '2024-01-01', '2025-01-01', 'ACTIVE', '2018-01-01',
     'TECHNOLOGY', 2000000.00, 4000000.00, 25000.00, 12000000.00, 75, 35000.00),
    ('PL-002', 'PL-2024-00002', 'BUS-003', '2024-01-01', '2025-01-01', 'ACTIVE', '2010-01-01',
     'MEDICAL', 1000000.00, 3000000.00, 10000.00, 28000000.00, 45, 125000.00)
ON CONFLICT (policy_id) DO NOTHING;

-- Cyber policies
INSERT INTO commercial_insurance.cyber_policies (policy_id, policy_number, business_id,
    effective_date, expiration_date, status,
    data_breach_limit, business_interruption_limit, cyber_extortion_limit,
    privacy_liability_limit, network_security_limit, aggregate_limit, retention,
    annual_revenue, records_count, pci_compliant, security_score, total_premium)
VALUES
    ('CY-001', 'CY-2024-00001', 'BUS-001', '2024-01-01', '2025-01-01', 'ACTIVE',
     1000000.00, 500000.00, 250000.00, 1000000.00, 1000000.00, 2000000.00, 10000.00,
     12000000.00, 50000, true, 85, 18500.00),
    ('CY-002', 'CY-2024-00002', 'BUS-003', '2024-01-01', '2025-01-01', 'ACTIVE',
     2000000.00, 1000000.00, 500000.00, 2000000.00, 2000000.00, 5000000.00, 25000.00,
     28000000.00, 500000, true, 78, 45000.00)
ON CONFLICT (policy_id) DO NOTHING;

-- Sample WC claim
INSERT INTO commercial_insurance.wc_claims (claim_id, claim_number, policy_id,
    injury_date, employee_name, class_code, injury_type, body_part, cause_of_injury,
    claim_type, days_away, medical_paid, medical_reserved, indemnity_paid, indemnity_reserved,
    total_incurred, status)
VALUES
    ('WC-CLM-001', 'WC-CLM-2024-00001', 'WC-002', '2024-05-15',
     'John Smith', '3632', 'LACERATION', 'Hand - Right', 'Caught in machinery',
     'LOST_TIME', 14, 8500.00, 2000.00, 4200.00, 1000.00, 15700.00, 'OPEN')
ON CONFLICT (claim_id) DO NOTHING;

-- Sample cyber incident
INSERT INTO commercial_insurance.cyber_incidents (incident_id, claim_number, policy_id,
    discovery_date, incident_date, incident_type, attack_vector, records_compromised,
    data_types_affected, downtime_hours, forensic_cost, notification_cost, legal_cost,
    business_interruption_loss, total_loss, status)
VALUES
    ('CY-INC-001', 'CY-CLM-2024-00001', 'CY-001', '2024-07-10', '2024-07-08',
     'RANSOMWARE', 'Phishing Email', 0, ARRAY['PII', 'CREDENTIALS'],
     72, 75000.00, 0.00, 25000.00, 180000.00, 280000.00, 'OPEN')
ON CONFLICT (incident_id) DO NOTHING;

-- =============================================================================
-- ANALYTICAL QUERIES
-- =============================================================================

-- Workers compensation loss ratio by class
SELECT
    wcd.class_code,
    wcd.class_description,
    SUM(wcd.payroll) as total_payroll,
    SUM(wcd.manual_premium) as total_premium,
    COUNT(DISTINCT wcc.claim_id) as claim_count,
    COALESCE(SUM(wcc.total_incurred), 0) as total_incurred,
    ROUND(COALESCE(SUM(wcc.total_incurred), 0) / NULLIF(SUM(wcd.manual_premium), 0) * 100, 2) as loss_ratio
FROM commercial_insurance.wc_class_details wcd
LEFT JOIN commercial_insurance.wc_claims wcc ON wcd.class_code = wcc.class_code
    AND wcd.policy_id = wcc.policy_id
GROUP BY wcd.class_code, wcd.class_description
ORDER BY total_premium DESC;

-- General liability claims by type
SELECT
    claim_type,
    COUNT(*) as claim_count,
    SUM(total_incurred) as total_incurred,
    AVG(total_incurred) as avg_claim_size,
    SUM(CASE WHEN litigation_status = 'SUIT_FILED' THEN 1 ELSE 0 END) as suits_filed
FROM commercial_insurance.gl_claims
GROUP BY claim_type
ORDER BY total_incurred DESC;

-- Cyber risk exposure by industry
SELECT
    b.industry_description,
    COUNT(DISTINCT cp.policy_id) as policy_count,
    SUM(cp.aggregate_limit) as total_limit,
    SUM(cp.total_premium) as total_premium,
    AVG(cp.security_score) as avg_security_score,
    SUM(cp.records_count) as total_records
FROM commercial_insurance.cyber_policies cp
JOIN commercial_insurance.businesses b ON cp.business_id = b.business_id
WHERE cp.status = 'ACTIVE'
GROUP BY b.industry_description
ORDER BY total_limit DESC;

-- Combined commercial program summary
SELECT
    b.legal_name,
    b.industry_description,
    b.num_employees,
    b.annual_revenue,
    COALESCE(wc.total_premium, 0) as wc_premium,
    COALESCE(gl.total_premium, 0) as gl_premium,
    COALESCE(pl.total_premium, 0) as pl_premium,
    COALESCE(cy.total_premium, 0) as cyber_premium,
    COALESCE(wc.total_premium, 0) + COALESCE(gl.total_premium, 0) +
    COALESCE(pl.total_premium, 0) + COALESCE(cy.total_premium, 0) as total_premium
FROM commercial_insurance.businesses b
LEFT JOIN commercial_insurance.wc_policies wc ON b.business_id = wc.business_id AND wc.status = 'ACTIVE'
LEFT JOIN commercial_insurance.gl_policies gl ON b.business_id = gl.business_id AND gl.status = 'ACTIVE'
LEFT JOIN commercial_insurance.professional_liability pl ON b.business_id = pl.business_id AND pl.status = 'ACTIVE'
LEFT JOIN commercial_insurance.cyber_policies cy ON b.business_id = cy.business_id AND cy.status = 'ACTIVE'
ORDER BY total_premium DESC;

-- Experience mod impact on WC premium
SELECT
    wc.policy_number,
    b.legal_name,
    wc.total_payroll,
    wc.manual_premium,
    wc.experience_mod,
    wc.schedule_mod,
    wc.modified_premium,
    wc.manual_premium - wc.modified_premium as mod_savings_or_surcharge,
    ROUND((wc.experience_mod - 1) * 100, 1) as exp_mod_impact_percent
FROM commercial_insurance.wc_policies wc
JOIN commercial_insurance.businesses b ON wc.business_id = b.business_id
WHERE wc.status = 'ACTIVE'
ORDER BY wc.experience_mod DESC;

-- Cyber incident trends
SELECT
    incident_type,
    COUNT(*) as incident_count,
    AVG(records_compromised) as avg_records_compromised,
    AVG(downtime_hours) as avg_downtime_hours,
    SUM(total_loss) as total_loss,
    AVG(total_loss) as avg_loss_per_incident
FROM commercial_insurance.cyber_incidents
GROUP BY incident_type
ORDER BY total_loss DESC;
