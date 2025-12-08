-- =============================================================================
-- OrbitRS Insurance Example: Shared Utilities and Functions
-- =============================================================================
-- Common SQL utilities for insurance applications:
--   - Premium calculation functions
--   - Risk scoring utilities
--   - Date/age calculation helpers
--   - Loss ratio calculations
--   - Reporting views
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i insurance_utils.sql
-- =============================================================================

-- Create schema for shared utilities
CREATE SCHEMA IF NOT EXISTS insurance_utils;

-- =============================================================================
-- UTILITY FUNCTIONS
-- =============================================================================

-- Calculate age from date of birth
CREATE OR REPLACE FUNCTION insurance_utils.calculate_age(birth_date DATE)
RETURNS INTEGER AS $$
BEGIN
    RETURN EXTRACT(YEAR FROM AGE(birth_date));
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Calculate policy term in months
CREATE OR REPLACE FUNCTION insurance_utils.policy_term_months(
    effective_date DATE,
    expiration_date DATE
) RETURNS INTEGER AS $$
BEGIN
    RETURN (EXTRACT(YEAR FROM expiration_date) - EXTRACT(YEAR FROM effective_date)) * 12 +
           (EXTRACT(MONTH FROM expiration_date) - EXTRACT(MONTH FROM effective_date));
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Calculate earned premium (pro-rata)
CREATE OR REPLACE FUNCTION insurance_utils.earned_premium(
    annual_premium DECIMAL,
    effective_date DATE,
    as_of_date DATE DEFAULT CURRENT_DATE
) RETURNS DECIMAL AS $$
DECLARE
    days_in_term INTEGER;
    days_earned INTEGER;
BEGIN
    days_in_term := 365;
    days_earned := LEAST(as_of_date - effective_date, days_in_term);

    IF days_earned < 0 THEN
        RETURN 0;
    END IF;

    RETURN ROUND(annual_premium * (days_earned::DECIMAL / days_in_term), 2);
END;
$$ LANGUAGE plpgsql STABLE;

-- Calculate loss ratio
CREATE OR REPLACE FUNCTION insurance_utils.loss_ratio(
    earned_premium DECIMAL,
    incurred_losses DECIMAL
) RETURNS DECIMAL AS $$
BEGIN
    IF earned_premium IS NULL OR earned_premium = 0 THEN
        RETURN NULL;
    END IF;

    RETURN ROUND((COALESCE(incurred_losses, 0) / earned_premium) * 100, 2);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Calculate combined ratio
CREATE OR REPLACE FUNCTION insurance_utils.combined_ratio(
    earned_premium DECIMAL,
    incurred_losses DECIMAL,
    expenses DECIMAL
) RETURNS DECIMAL AS $$
BEGIN
    IF earned_premium IS NULL OR earned_premium = 0 THEN
        RETURN NULL;
    END IF;

    RETURN ROUND(((COALESCE(incurred_losses, 0) + COALESCE(expenses, 0)) / earned_premium) * 100, 2);
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Risk tier based on score
CREATE OR REPLACE FUNCTION insurance_utils.risk_tier(risk_score INTEGER)
RETURNS VARCHAR AS $$
BEGIN
    RETURN CASE
        WHEN risk_score >= 90 THEN 'PREFERRED_PLUS'
        WHEN risk_score >= 80 THEN 'PREFERRED'
        WHEN risk_score >= 70 THEN 'STANDARD_PLUS'
        WHEN risk_score >= 60 THEN 'STANDARD'
        WHEN risk_score >= 50 THEN 'SUBSTANDARD'
        ELSE 'DECLINED'
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Calculate days until expiration
CREATE OR REPLACE FUNCTION insurance_utils.days_until_expiry(expiration_date DATE)
RETURNS INTEGER AS $$
BEGIN
    RETURN expiration_date - CURRENT_DATE;
END;
$$ LANGUAGE plpgsql STABLE;

-- Format currency
CREATE OR REPLACE FUNCTION insurance_utils.format_currency(amount DECIMAL)
RETURNS VARCHAR AS $$
BEGIN
    RETURN '$' || TO_CHAR(COALESCE(amount, 0), 'FM999,999,999.00');
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- =============================================================================
-- PREMIUM CALCULATION FUNCTIONS
-- =============================================================================

-- Base rate lookup (simplified)
CREATE OR REPLACE FUNCTION insurance_utils.get_base_rate(
    policy_type VARCHAR,
    territory VARCHAR DEFAULT 'DEFAULT'
) RETURNS DECIMAL AS $$
BEGIN
    RETURN CASE policy_type
        WHEN 'AUTO' THEN
            CASE territory
                WHEN 'URBAN' THEN 1.25
                WHEN 'SUBURBAN' THEN 1.00
                WHEN 'RURAL' THEN 0.85
                ELSE 1.00
            END
        WHEN 'HOME' THEN
            CASE territory
                WHEN 'COASTAL' THEN 1.50
                WHEN 'URBAN' THEN 1.10
                WHEN 'SUBURBAN' THEN 1.00
                WHEN 'RURAL' THEN 0.90
                ELSE 1.00
            END
        WHEN 'LIFE' THEN 1.00
        ELSE 1.00
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Age factor for auto insurance
CREATE OR REPLACE FUNCTION insurance_utils.auto_age_factor(driver_age INTEGER)
RETURNS DECIMAL AS $$
BEGIN
    RETURN CASE
        WHEN driver_age < 21 THEN 1.75
        WHEN driver_age < 25 THEN 1.35
        WHEN driver_age < 30 THEN 1.10
        WHEN driver_age < 65 THEN 1.00
        WHEN driver_age < 75 THEN 1.10
        ELSE 1.25
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Deductible credit factor
CREATE OR REPLACE FUNCTION insurance_utils.deductible_credit(
    deductible_amount DECIMAL,
    base_deductible DECIMAL DEFAULT 500
) RETURNS DECIMAL AS $$
BEGIN
    IF deductible_amount <= base_deductible THEN
        RETURN 1.00;
    ELSIF deductible_amount <= 1000 THEN
        RETURN 0.95;
    ELSIF deductible_amount <= 2500 THEN
        RETURN 0.88;
    ELSIF deductible_amount <= 5000 THEN
        RETURN 0.80;
    ELSE
        RETURN 0.75;
    END IF;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- =============================================================================
-- CLAIM HELPER FUNCTIONS
-- =============================================================================

-- Claim age in days
CREATE OR REPLACE FUNCTION insurance_utils.claim_age_days(
    loss_date DATE,
    close_date DATE DEFAULT NULL
) RETURNS INTEGER AS $$
BEGIN
    RETURN COALESCE(close_date, CURRENT_DATE) - loss_date;
END;
$$ LANGUAGE plpgsql STABLE;

-- Claim severity classification
CREATE OR REPLACE FUNCTION insurance_utils.claim_severity(claim_amount DECIMAL)
RETURNS VARCHAR AS $$
BEGIN
    RETURN CASE
        WHEN claim_amount < 1000 THEN 'MINOR'
        WHEN claim_amount < 10000 THEN 'MODERATE'
        WHEN claim_amount < 100000 THEN 'MAJOR'
        ELSE 'CATASTROPHIC'
    END;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Reserve adequacy check
CREATE OR REPLACE FUNCTION insurance_utils.reserve_status(
    incurred_amount DECIMAL,
    paid_amount DECIMAL,
    reserved_amount DECIMAL
) RETURNS VARCHAR AS $$
DECLARE
    remaining DECIMAL;
BEGIN
    remaining := COALESCE(reserved_amount, 0) - COALESCE(paid_amount, 0);

    IF remaining < 0 THEN
        RETURN 'OVER_PAID';
    ELSIF remaining < (incurred_amount * 0.1) THEN
        RETURN 'LOW_RESERVE';
    ELSIF remaining > (incurred_amount * 0.5) THEN
        RETURN 'HIGH_RESERVE';
    ELSE
        RETURN 'ADEQUATE';
    END IF;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- =============================================================================
-- REFERENCE DATA TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS insurance_utils.loss_types (
    loss_type_code VARCHAR(30) PRIMARY KEY,
    loss_type_name VARCHAR(100) NOT NULL,
    policy_types TEXT[], -- Which policy types this applies to
    severity_weight DECIMAL(3,2) DEFAULT 1.00,
    description TEXT
);

INSERT INTO insurance_utils.loss_types (loss_type_code, loss_type_name, policy_types, severity_weight, description)
VALUES
    ('COLLISION', 'Collision', ARRAY['AUTO'], 1.00, 'Vehicle collision with another vehicle or object'),
    ('COMPREHENSIVE', 'Comprehensive', ARRAY['AUTO'], 0.80, 'Non-collision auto damage (theft, weather, animals)'),
    ('THEFT', 'Theft', ARRAY['AUTO', 'HOME'], 1.20, 'Theft of property'),
    ('FIRE', 'Fire', ARRAY['HOME', 'PROPERTY'], 1.50, 'Fire damage'),
    ('WATER_DAMAGE', 'Water Damage', ARRAY['HOME', 'PROPERTY'], 1.00, 'Water damage from various sources'),
    ('WIND_HAIL', 'Wind/Hail', ARRAY['HOME', 'PROPERTY', 'AUTO'], 1.10, 'Weather-related damage'),
    ('LIABILITY_BI', 'Bodily Injury Liability', ARRAY['AUTO', 'HOME'], 1.50, 'Third party bodily injury claims'),
    ('LIABILITY_PD', 'Property Damage Liability', ARRAY['AUTO', 'HOME'], 1.00, 'Third party property damage claims'),
    ('MEDICAL', 'Medical', ARRAY['AUTO', 'HEALTH', 'LIFE'], 1.30, 'Medical expenses'),
    ('DEATH', 'Death Claim', ARRAY['LIFE'], 2.00, 'Life insurance death claim')
ON CONFLICT (loss_type_code) DO NOTHING;

CREATE TABLE IF NOT EXISTS insurance_utils.policy_status_codes (
    status_code VARCHAR(20) PRIMARY KEY,
    status_name VARCHAR(50) NOT NULL,
    is_active BOOLEAN DEFAULT false,
    allows_claims BOOLEAN DEFAULT false,
    description TEXT
);

INSERT INTO insurance_utils.policy_status_codes (status_code, status_name, is_active, allows_claims, description)
VALUES
    ('ACTIVE', 'Active', true, true, 'Policy is in force'),
    ('PENDING', 'Pending', false, false, 'Policy pending issuance'),
    ('EXPIRED', 'Expired', false, false, 'Policy has expired'),
    ('CANCELLED', 'Cancelled', false, false, 'Policy was cancelled'),
    ('LAPSED', 'Lapsed', false, false, 'Policy lapsed due to non-payment'),
    ('SUSPENDED', 'Suspended', false, false, 'Policy temporarily suspended'),
    ('RENEWED', 'Renewed', true, true, 'Policy renewed for new term')
ON CONFLICT (status_code) DO NOTHING;

CREATE TABLE IF NOT EXISTS insurance_utils.claim_status_codes (
    status_code VARCHAR(20) PRIMARY KEY,
    status_name VARCHAR(50) NOT NULL,
    is_open BOOLEAN DEFAULT true,
    workflow_order INTEGER,
    description TEXT
);

INSERT INTO insurance_utils.claim_status_codes (status_code, status_name, is_open, workflow_order, description)
VALUES
    ('FILED', 'Filed', true, 1, 'Claim has been filed'),
    ('ASSIGNED', 'Assigned', true, 2, 'Claim assigned to adjuster'),
    ('UNDER_REVIEW', 'Under Review', true, 3, 'Claim is being reviewed'),
    ('INVESTIGATION', 'Investigation', true, 4, 'Claim under investigation'),
    ('APPROVED', 'Approved', true, 5, 'Claim has been approved'),
    ('DENIED', 'Denied', false, 6, 'Claim has been denied'),
    ('PAID', 'Paid', false, 7, 'Claim payment issued'),
    ('CLOSED', 'Closed', false, 8, 'Claim file closed')
ON CONFLICT (status_code) DO NOTHING;

-- =============================================================================
-- EXAMPLE USAGE
-- =============================================================================

/*
-- Calculate age
SELECT insurance_utils.calculate_age('1985-03-15');

-- Calculate earned premium
SELECT insurance_utils.earned_premium(1200.00, '2024-01-01', '2024-08-15');

-- Get risk tier
SELECT insurance_utils.risk_tier(85);

-- Calculate loss ratio
SELECT insurance_utils.loss_ratio(50000.00, 35000.00);

-- Get base rate for policy type
SELECT insurance_utils.get_base_rate('AUTO', 'URBAN');

-- Format currency
SELECT insurance_utils.format_currency(1234567.89);

-- Claim severity
SELECT insurance_utils.claim_severity(25000.00);
*/

-- =============================================================================
-- REPORTING VIEWS (for use with actual insurance tables)
-- =============================================================================

/*
-- These views can be created once actual insurance tables exist:

CREATE OR REPLACE VIEW insurance_utils.v_policy_metrics AS
SELECT
    policy_type,
    COUNT(*) as policy_count,
    SUM(premium) as total_premium,
    AVG(premium) as avg_premium,
    SUM(coverage_limit) as total_exposure,
    AVG(insurance_utils.days_until_expiry(expiration_date)) as avg_days_to_expiry
FROM insurance.policies
WHERE status = 'ACTIVE'
GROUP BY policy_type;

CREATE OR REPLACE VIEW insurance_utils.v_claims_summary AS
SELECT
    loss_type,
    COUNT(*) as claim_count,
    SUM(claimed_amount) as total_claimed,
    SUM(paid_amount) as total_paid,
    AVG(claimed_amount) as avg_claim_size,
    insurance_utils.claim_severity(AVG(claimed_amount)) as avg_severity
FROM insurance.claims
WHERE status != 'DENIED'
GROUP BY loss_type;
*/

-- Completion message
SELECT 'Insurance utilities created successfully' as status;
