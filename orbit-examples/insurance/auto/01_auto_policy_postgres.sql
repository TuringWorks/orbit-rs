-- =============================================================================
-- OrbitRS Insurance Example: Auto Insurance Policy Management
-- =============================================================================
-- Demonstrates PostgreSQL patterns for auto insurance:
--   - Vehicle and driver information
--   - Coverage types and limits
--   - Premium calculation factors
--   - Claims processing
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_auto_policy_postgres.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS auto_insurance;

-- =============================================================================
-- DRIVER TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS auto_insurance.drivers (
    driver_id VARCHAR(50) PRIMARY KEY,
    customer_id VARCHAR(50) NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE NOT NULL,
    license_number VARCHAR(50) UNIQUE,
    license_state VARCHAR(2),
    license_expiry DATE,
    license_status VARCHAR(20) DEFAULT 'VALID',
    years_licensed INTEGER,
    is_primary_driver BOOLEAN DEFAULT false,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS auto_insurance.driver_history (
    record_id VARCHAR(50) PRIMARY KEY,
    driver_id VARCHAR(50) REFERENCES auto_insurance.drivers(driver_id),
    record_type VARCHAR(20), -- ACCIDENT, VIOLATION, DUI, CLAIM
    record_date DATE,
    description TEXT,
    points INTEGER DEFAULT 0,
    severity VARCHAR(20), -- MINOR, MODERATE, MAJOR
    at_fault BOOLEAN,
    expires_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- VEHICLE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS auto_insurance.vehicles (
    vehicle_id VARCHAR(50) PRIMARY KEY,
    vin VARCHAR(17) UNIQUE NOT NULL,
    year INTEGER NOT NULL,
    make VARCHAR(50) NOT NULL,
    model VARCHAR(100) NOT NULL,
    trim_level VARCHAR(50),
    body_type VARCHAR(30), -- SEDAN, SUV, TRUCK, COUPE, VAN, MOTORCYCLE
    engine_type VARCHAR(30), -- GAS, DIESEL, HYBRID, ELECTRIC
    color VARCHAR(30),
    msrp DECIMAL(12,2),
    current_value DECIMAL(12,2),
    annual_mileage INTEGER,
    primary_use VARCHAR(30), -- COMMUTE, PLEASURE, BUSINESS
    garage_type VARCHAR(30), -- GARAGE, CARPORT, STREET, DRIVEWAY
    anti_theft_device BOOLEAN DEFAULT false,
    safety_features TEXT[], -- Array of safety features
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS auto_insurance.vehicle_drivers (
    vehicle_id VARCHAR(50) REFERENCES auto_insurance.vehicles(vehicle_id),
    driver_id VARCHAR(50) REFERENCES auto_insurance.drivers(driver_id),
    is_primary BOOLEAN DEFAULT false,
    usage_percent INTEGER DEFAULT 100,
    PRIMARY KEY (vehicle_id, driver_id)
);

-- =============================================================================
-- POLICY AND COVERAGE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS auto_insurance.policies (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    customer_id VARCHAR(50) NOT NULL,
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    policy_term_months INTEGER DEFAULT 6,
    payment_plan VARCHAR(20), -- FULL_PAY, MONTHLY, QUARTERLY
    total_premium DECIMAL(10,2),
    down_payment DECIMAL(10,2),
    installment_amount DECIMAL(10,2),
    billing_day INTEGER,
    paperless_discount BOOLEAN DEFAULT false,
    multi_policy_discount BOOLEAN DEFAULT false,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS auto_insurance.policy_vehicles (
    policy_id VARCHAR(50) REFERENCES auto_insurance.policies(policy_id),
    vehicle_id VARCHAR(50) REFERENCES auto_insurance.vehicles(vehicle_id),
    vehicle_premium DECIMAL(10,2),
    PRIMARY KEY (policy_id, vehicle_id)
);

CREATE TABLE IF NOT EXISTS auto_insurance.coverages (
    coverage_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES auto_insurance.policies(policy_id),
    vehicle_id VARCHAR(50) REFERENCES auto_insurance.vehicles(vehicle_id),
    coverage_type VARCHAR(30) NOT NULL,
    -- LIABILITY_BI, LIABILITY_PD, COLLISION, COMPREHENSIVE,
    -- UNINSURED_MOTORIST, UNDERINSURED_MOTORIST, MEDICAL, PIP, RENTAL, ROADSIDE
    limit_per_person DECIMAL(12,2),
    limit_per_accident DECIMAL(12,2),
    limit_property DECIMAL(12,2),
    deductible DECIMAL(10,2),
    premium DECIMAL(10,2),
    is_required BOOLEAN DEFAULT false,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- CLAIMS TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS auto_insurance.claims (
    claim_id VARCHAR(50) PRIMARY KEY,
    claim_number VARCHAR(30) UNIQUE NOT NULL,
    policy_id VARCHAR(50) REFERENCES auto_insurance.policies(policy_id),
    vehicle_id VARCHAR(50) REFERENCES auto_insurance.vehicles(vehicle_id),
    driver_id VARCHAR(50) REFERENCES auto_insurance.drivers(driver_id),
    loss_date TIMESTAMP NOT NULL,
    report_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    loss_type VARCHAR(30), -- COLLISION, COMPREHENSIVE, LIABILITY, MEDICAL, PIP
    loss_description TEXT,
    loss_location TEXT,
    police_report_number VARCHAR(50),
    at_fault BOOLEAN,
    fault_percent INTEGER,
    total_claimed DECIMAL(12,2),
    total_paid DECIMAL(12,2) DEFAULT 0,
    deductible_applied DECIMAL(10,2),
    status VARCHAR(20) DEFAULT 'OPEN',
    adjuster_id VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS auto_insurance.claim_payments (
    payment_id VARCHAR(50) PRIMARY KEY,
    claim_id VARCHAR(50) REFERENCES auto_insurance.claims(claim_id),
    payment_type VARCHAR(30), -- REPAIR, REPLACEMENT, MEDICAL, RENTAL, TOTAL_LOSS
    payee_name VARCHAR(200),
    payee_type VARCHAR(20), -- INSURED, CLAIMANT, VENDOR, MEDICAL_PROVIDER
    amount DECIMAL(12,2),
    payment_date DATE,
    check_number VARCHAR(30),
    status VARCHAR(20) DEFAULT 'PENDING',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- RATING TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS auto_insurance.rating_factors (
    factor_id VARCHAR(50) PRIMARY KEY,
    factor_name VARCHAR(100) NOT NULL,
    factor_category VARCHAR(50), -- DRIVER, VEHICLE, TERRITORY, DISCOUNT
    min_value DECIMAL(5,3),
    max_value DECIMAL(5,3),
    effective_date DATE,
    expiration_date DATE
);

CREATE TABLE IF NOT EXISTS auto_insurance.territory_rates (
    territory_code VARCHAR(10) PRIMARY KEY,
    state VARCHAR(2) NOT NULL,
    zip_codes TEXT[], -- Array of zip codes
    base_rate_factor DECIMAL(5,3) DEFAULT 1.000,
    theft_factor DECIMAL(5,3) DEFAULT 1.000,
    collision_factor DECIMAL(5,3) DEFAULT 1.000,
    comprehensive_factor DECIMAL(5,3) DEFAULT 1.000
);

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Drivers
INSERT INTO auto_insurance.drivers (driver_id, customer_id, first_name, last_name,
    date_of_birth, license_number, license_state, license_expiry, years_licensed, is_primary_driver)
VALUES
    ('DRV-001', 'CUST-001', 'John', 'Smith', '1985-03-15', 'S123456789', 'NY', '2027-03-15', 15, true),
    ('DRV-002', 'CUST-001', 'Jane', 'Smith', '1987-07-22', 'S987654321', 'NY', '2026-07-22', 12, false),
    ('DRV-003', 'CUST-002', 'Robert', 'Johnson', '1975-11-08', 'J111222333', 'CA', '2025-11-08', 25, true)
ON CONFLICT (driver_id) DO NOTHING;

-- Driver history
INSERT INTO auto_insurance.driver_history (record_id, driver_id, record_type, record_date,
    description, points, severity, at_fault, expires_date)
VALUES
    ('REC-001', 'DRV-001', 'VIOLATION', '2022-05-10', 'Speeding 15 over limit', 2, 'MINOR', true, '2025-05-10'),
    ('REC-002', 'DRV-003', 'ACCIDENT', '2021-08-22', 'Fender bender in parking lot', 0, 'MINOR', true, '2024-08-22')
ON CONFLICT (record_id) DO NOTHING;

-- Vehicles
INSERT INTO auto_insurance.vehicles (vehicle_id, vin, year, make, model, trim_level,
    body_type, engine_type, msrp, current_value, annual_mileage, primary_use,
    garage_type, anti_theft_device, safety_features)
VALUES
    ('VEH-001', '1HGCM82633A123456', 2023, 'Honda', 'Accord', 'EX-L', 'SEDAN', 'GAS',
     35000.00, 32000.00, 12000, 'COMMUTE', 'GARAGE', true,
     ARRAY['ABS', 'Airbags', 'Backup Camera', 'Lane Departure Warning', 'Adaptive Cruise Control']),
    ('VEH-002', '5YJSA1E26MF123456', 2023, 'Tesla', 'Model 3', 'Long Range', 'SEDAN', 'ELECTRIC',
     50000.00, 48000.00, 10000, 'COMMUTE', 'GARAGE', true,
     ARRAY['Autopilot', 'ABS', 'Airbags', 'Collision Avoidance', 'Sentry Mode']),
    ('VEH-003', '1FTFW1ET5EKE12345', 2022, 'Ford', 'F-150', 'XLT', 'TRUCK', 'GAS',
     55000.00, 45000.00, 15000, 'BUSINESS', 'DRIVEWAY', false,
     ARRAY['ABS', 'Airbags', 'Trailer Assist'])
ON CONFLICT (vehicle_id) DO NOTHING;

-- Policies
INSERT INTO auto_insurance.policies (policy_id, policy_number, customer_id,
    effective_date, expiration_date, status, policy_term_months, payment_plan,
    total_premium, down_payment, installment_amount, billing_day,
    paperless_discount, multi_policy_discount)
VALUES
    ('POL-AUTO-001', 'AUT-2024-00001', 'CUST-001', '2024-01-01', '2024-07-01', 'ACTIVE',
     6, 'MONTHLY', 1200.00, 200.00, 166.67, 15, true, true),
    ('POL-AUTO-002', 'AUT-2024-00002', 'CUST-002', '2024-02-01', '2024-08-01', 'ACTIVE',
     6, 'FULL_PAY', 950.00, 950.00, 0.00, 1, true, false)
ON CONFLICT (policy_id) DO NOTHING;

-- Coverages
INSERT INTO auto_insurance.coverages (coverage_id, policy_id, vehicle_id, coverage_type,
    limit_per_person, limit_per_accident, limit_property, deductible, premium, is_required)
VALUES
    ('COV-001', 'POL-AUTO-001', 'VEH-001', 'LIABILITY_BI', 100000, 300000, NULL, 0, 250.00, true),
    ('COV-002', 'POL-AUTO-001', 'VEH-001', 'LIABILITY_PD', NULL, NULL, 100000, 0, 150.00, true),
    ('COV-003', 'POL-AUTO-001', 'VEH-001', 'COLLISION', NULL, NULL, NULL, 500, 300.00, false),
    ('COV-004', 'POL-AUTO-001', 'VEH-001', 'COMPREHENSIVE', NULL, NULL, NULL, 250, 150.00, false),
    ('COV-005', 'POL-AUTO-001', 'VEH-001', 'UNINSURED_MOTORIST', 100000, 300000, NULL, 0, 75.00, true),
    ('COV-006', 'POL-AUTO-001', 'VEH-001', 'RENTAL', NULL, 30, NULL, 0, 25.00, false)
ON CONFLICT (coverage_id) DO NOTHING;

-- Claims
INSERT INTO auto_insurance.claims (claim_id, claim_number, policy_id, vehicle_id, driver_id,
    loss_date, loss_type, loss_description, loss_location, police_report_number,
    at_fault, fault_percent, total_claimed, status)
VALUES
    ('CLM-001', 'CLM-2024-00001', 'POL-AUTO-001', 'VEH-001', 'DRV-001',
     '2024-03-15 14:30:00', 'COLLISION',
     'Rear-ended at stop light. Minor bumper damage.',
     'Main St & 5th Ave, New York, NY',
     'NYPD-2024-12345', false, 0, 2500.00, 'OPEN')
ON CONFLICT (claim_id) DO NOTHING;

-- =============================================================================
-- ANALYTICAL QUERIES
-- =============================================================================

-- Policy summary with coverage details
SELECT
    p.policy_number,
    p.status,
    p.effective_date,
    p.expiration_date,
    p.total_premium,
    COUNT(DISTINCT pv.vehicle_id) as vehicle_count,
    COUNT(c.coverage_id) as coverage_count,
    SUM(c.premium) as total_coverage_premium
FROM auto_insurance.policies p
LEFT JOIN auto_insurance.policy_vehicles pv ON p.policy_id = pv.policy_id
LEFT JOIN auto_insurance.coverages c ON p.policy_id = c.policy_id
WHERE p.status = 'ACTIVE'
GROUP BY p.policy_id, p.policy_number, p.status, p.effective_date, p.expiration_date, p.total_premium
ORDER BY p.policy_number;

-- Driver risk analysis
SELECT
    d.driver_id,
    d.first_name || ' ' || d.last_name as driver_name,
    d.years_licensed,
    EXTRACT(YEAR FROM AGE(d.date_of_birth)) as age,
    COUNT(dh.record_id) as incident_count,
    COALESCE(SUM(dh.points), 0) as total_points,
    CASE
        WHEN SUM(dh.points) IS NULL OR SUM(dh.points) = 0 THEN 'PREFERRED'
        WHEN SUM(dh.points) <= 3 THEN 'STANDARD'
        ELSE 'HIGH_RISK'
    END as risk_tier
FROM auto_insurance.drivers d
LEFT JOIN auto_insurance.driver_history dh ON d.driver_id = dh.driver_id
    AND dh.expires_date > CURRENT_DATE
GROUP BY d.driver_id, d.first_name, d.last_name, d.years_licensed, d.date_of_birth
ORDER BY total_points DESC;

-- Claims loss ratio by vehicle type
SELECT
    v.body_type,
    v.make,
    COUNT(DISTINCT c.claim_id) as claim_count,
    COALESCE(SUM(c.total_claimed), 0) as total_claimed,
    COALESCE(SUM(c.total_paid), 0) as total_paid,
    COUNT(DISTINCT v.vehicle_id) as vehicle_count
FROM auto_insurance.vehicles v
LEFT JOIN auto_insurance.claims c ON v.vehicle_id = c.vehicle_id
GROUP BY v.body_type, v.make
ORDER BY claim_count DESC;

-- Coverage distribution
SELECT
    coverage_type,
    COUNT(*) as policy_count,
    AVG(premium) as avg_premium,
    AVG(deductible) as avg_deductible,
    SUM(premium) as total_premium_collected
FROM auto_insurance.coverages
GROUP BY coverage_type
ORDER BY total_premium_collected DESC;

-- Upcoming policy renewals (next 30 days)
SELECT
    p.policy_number,
    p.customer_id,
    p.expiration_date,
    p.total_premium,
    p.expiration_date - CURRENT_DATE as days_until_expiry
FROM auto_insurance.policies p
WHERE p.status = 'ACTIVE'
AND p.expiration_date BETWEEN CURRENT_DATE AND CURRENT_DATE + INTERVAL '30 days'
ORDER BY p.expiration_date;
