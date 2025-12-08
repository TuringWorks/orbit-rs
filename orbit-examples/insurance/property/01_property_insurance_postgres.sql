-- =============================================================================
-- OrbitRS Insurance Example: Commercial Property Insurance
-- =============================================================================
-- Demonstrates PostgreSQL patterns for commercial property insurance:
--   - Building and location management
--   - Business interruption coverage
--   - Equipment and inventory coverage
--   - Multi-location policies
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_property_insurance_postgres.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS property_insurance;

-- =============================================================================
-- LOCATION AND BUILDING TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS property_insurance.locations (
    location_id VARCHAR(50) PRIMARY KEY,
    customer_id VARCHAR(50) NOT NULL,
    location_number INTEGER,
    location_name VARCHAR(200),
    address_street VARCHAR(200) NOT NULL,
    address_city VARCHAR(100) NOT NULL,
    address_state VARCHAR(2) NOT NULL,
    address_zip VARCHAR(10) NOT NULL,
    county VARCHAR(100),
    country VARCHAR(50) DEFAULT 'USA',
    latitude DECIMAL(10,7),
    longitude DECIMAL(10,7),
    territory_code VARCHAR(10),
    protection_class VARCHAR(10), -- ISO 1-10 fire protection class
    occupancy_code VARCHAR(20),
    construction_class VARCHAR(20), -- ISO construction class
    year_built INTEGER,
    num_stories INTEGER,
    total_area_sqft INTEGER,
    sprinklered_percent DECIMAL(5,2),
    alarm_type VARCHAR(30),
    is_headquarters BOOLEAN DEFAULT false,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS property_insurance.buildings (
    building_id VARCHAR(50) PRIMARY KEY,
    location_id VARCHAR(50) REFERENCES property_insurance.locations(location_id),
    building_number INTEGER,
    building_name VARCHAR(200),
    construction_type VARCHAR(30),
    -- FRAME, JOISTED_MASONRY, NON_COMBUSTIBLE, MASONRY_NON_COMBUSTIBLE,
    -- MODIFIED_FIRE_RESISTIVE, FIRE_RESISTIVE
    roof_type VARCHAR(30),
    roof_age_years INTEGER,
    electrical_update_year INTEGER,
    plumbing_update_year INTEGER,
    hvac_update_year INTEGER,
    building_value DECIMAL(14,2),
    replacement_cost DECIMAL(14,2),
    functional_replacement_cost DECIMAL(14,2),
    actual_cash_value DECIMAL(14,2),
    square_footage INTEGER,
    num_units INTEGER DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- POLICY AND COVERAGE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS property_insurance.policies (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    policy_form VARCHAR(20), -- BOP, CPP, SMP, DIFFERENCE_IN_CONDITIONS
    customer_id VARCHAR(50) NOT NULL,
    named_insured VARCHAR(200) NOT NULL,
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    total_premium DECIMAL(12,2),
    total_insured_value DECIMAL(16,2),
    blanket_coverage BOOLEAN DEFAULT false,
    agreed_value BOOLEAN DEFAULT false,
    coinsurance_percent INTEGER DEFAULT 80,
    valuation_method VARCHAR(30), -- REPLACEMENT_COST, ACV, FUNCTIONAL_RC
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS property_insurance.policy_locations (
    policy_location_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES property_insurance.policies(policy_id),
    location_id VARCHAR(50) REFERENCES property_insurance.locations(location_id),
    location_schedule_number INTEGER,
    building_limit DECIMAL(14,2),
    bpp_limit DECIMAL(14,2), -- Business Personal Property
    bi_limit DECIMAL(14,2), -- Business Interruption
    extra_expense_limit DECIMAL(14,2),
    deductible DECIMAL(10,2),
    wind_deductible_percent DECIMAL(5,2),
    flood_deductible_percent DECIMAL(5,2),
    earthquake_deductible_percent DECIMAL(5,2),
    location_premium DECIMAL(10,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS property_insurance.coverages (
    coverage_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES property_insurance.policies(policy_id),
    policy_location_id VARCHAR(50) REFERENCES property_insurance.policy_locations(policy_location_id),
    coverage_type VARCHAR(50) NOT NULL,
    -- BUILDING, BUSINESS_PERSONAL_PROPERTY, BUSINESS_INCOME,
    -- EXTRA_EXPENSE, EQUIPMENT_BREAKDOWN, SPOILAGE,
    -- VALUABLE_PAPERS, ACCOUNTS_RECEIVABLE, EDP_EQUIPMENT,
    -- SIGNS, GLASS, OUTDOOR_PROPERTY
    coverage_limit DECIMAL(14,2),
    deductible DECIMAL(10,2),
    waiting_period_hours INTEGER, -- For time element coverages
    coinsurance_percent INTEGER,
    premium DECIMAL(10,2),
    rate_per_hundred DECIMAL(8,4),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS property_insurance.endorsements (
    endorsement_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES property_insurance.policies(policy_id),
    endorsement_form VARCHAR(30),
    endorsement_name VARCHAR(200),
    -- EARTHQUAKE, FLOOD, ORDINANCE_OR_LAW, UTILITY_SERVICES,
    -- CONTINGENT_BI, SERVICE_INTERRUPTION, CYBER_COVERAGE,
    -- TERRORISM, NAMED_STORM_DEDUCTIBLE
    description TEXT,
    sublimit DECIMAL(14,2),
    deductible DECIMAL(10,2),
    additional_premium DECIMAL(10,2),
    effective_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- EQUIPMENT AND INVENTORY TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS property_insurance.scheduled_equipment (
    equipment_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES property_insurance.policies(policy_id),
    location_id VARCHAR(50) REFERENCES property_insurance.locations(location_id),
    equipment_type VARCHAR(50),
    description TEXT NOT NULL,
    manufacturer VARCHAR(100),
    model VARCHAR(100),
    serial_number VARCHAR(100),
    year_manufactured INTEGER,
    purchase_price DECIMAL(12,2),
    insured_value DECIMAL(12,2),
    premium DECIMAL(10,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS property_insurance.inventory_values (
    inventory_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES property_insurance.policies(policy_id),
    location_id VARCHAR(50) REFERENCES property_insurance.locations(location_id),
    reporting_date DATE NOT NULL,
    raw_materials DECIMAL(14,2) DEFAULT 0,
    work_in_progress DECIMAL(14,2) DEFAULT 0,
    finished_goods DECIMAL(14,2) DEFAULT 0,
    supplies DECIMAL(14,2) DEFAULT 0,
    total_inventory DECIMAL(14,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- CLAIMS TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS property_insurance.claims (
    claim_id VARCHAR(50) PRIMARY KEY,
    claim_number VARCHAR(30) UNIQUE NOT NULL,
    policy_id VARCHAR(50) REFERENCES property_insurance.policies(policy_id),
    location_id VARCHAR(50) REFERENCES property_insurance.locations(location_id),
    loss_date TIMESTAMP NOT NULL,
    report_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    cause_of_loss VARCHAR(50),
    -- FIRE, LIGHTNING, WINDSTORM, HAIL, EXPLOSION, RIOT,
    -- SMOKE, AIRCRAFT, VEHICLES, VANDALISM, SPRINKLER_LEAKAGE,
    -- SINKHOLE, WEIGHT_OF_SNOW, WATER_DAMAGE, THEFT, EARTHQUAKE, FLOOD
    loss_description TEXT,
    is_catastrophe BOOLEAN DEFAULT false,
    catastrophe_number VARCHAR(20),
    building_damage DECIMAL(14,2) DEFAULT 0,
    bpp_damage DECIMAL(14,2) DEFAULT 0,
    bi_loss DECIMAL(14,2) DEFAULT 0,
    extra_expense DECIMAL(14,2) DEFAULT 0,
    debris_removal DECIMAL(12,2) DEFAULT 0,
    total_claimed DECIMAL(14,2),
    total_paid DECIMAL(14,2) DEFAULT 0,
    deductible_applied DECIMAL(10,2),
    salvage_recovered DECIMAL(12,2) DEFAULT 0,
    subrogation_recovered DECIMAL(12,2) DEFAULT 0,
    status VARCHAR(20) DEFAULT 'OPEN',
    adjuster_id VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS property_insurance.claim_payments (
    payment_id VARCHAR(50) PRIMARY KEY,
    claim_id VARCHAR(50) REFERENCES property_insurance.claims(claim_id),
    payment_date DATE NOT NULL,
    payment_type VARCHAR(30),
    -- BUILDING_REPAIR, BPP_REPLACEMENT, BI_INDEMNITY,
    -- EXTRA_EXPENSE, DEBRIS_REMOVAL, ALE
    payee_name VARCHAR(200),
    payee_type VARCHAR(20), -- INSURED, CONTRACTOR, VENDOR, MORTGAGEE
    amount DECIMAL(12,2),
    check_number VARCHAR(30),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Locations
INSERT INTO property_insurance.locations (location_id, customer_id, location_number,
    location_name, address_street, address_city, address_state, address_zip,
    protection_class, construction_class, year_built, num_stories, total_area_sqft,
    sprinklered_percent, alarm_type, is_headquarters)
VALUES
    ('LOC-001', 'CUST-COMM-001', 1, 'Corporate Headquarters',
     '100 Commerce Way', 'Hartford', 'CT', '06101',
     '3', 'FIRE_RESISTIVE', 2010, 5, 75000, 100.00, 'CENTRAL_STATION', true),
    ('LOC-002', 'CUST-COMM-001', 2, 'Manufacturing Plant',
     '500 Industrial Blvd', 'Springfield', 'MA', '01101',
     '4', 'NON_COMBUSTIBLE', 1985, 2, 150000, 85.00, 'CENTRAL_STATION', false),
    ('LOC-003', 'CUST-COMM-001', 3, 'Distribution Center',
     '200 Logistics Dr', 'Newark', 'NJ', '07101',
     '5', 'JOISTED_MASONRY', 1995, 1, 200000, 100.00, 'LOCAL_ALARM', false),
    ('LOC-004', 'CUST-COMM-002', 1, 'Retail Store - Downtown',
     '50 Main Street', 'Boston', 'MA', '02101',
     '2', 'MASONRY_NON_COMBUSTIBLE', 1920, 3, 15000, 0.00, 'CENTRAL_STATION', true)
ON CONFLICT (location_id) DO NOTHING;

-- Buildings
INSERT INTO property_insurance.buildings (building_id, location_id, building_number,
    building_name, construction_type, roof_type, roof_age_years, building_value,
    replacement_cost, square_footage)
VALUES
    ('BLD-001', 'LOC-001', 1, 'Main Office Building', 'FIRE_RESISTIVE', 'FLAT_MEMBRANE', 10,
     15000000.00, 18000000.00, 75000),
    ('BLD-002', 'LOC-002', 1, 'Production Facility', 'NON_COMBUSTIBLE', 'METAL', 15,
     25000000.00, 30000000.00, 120000),
    ('BLD-003', 'LOC-002', 2, 'Warehouse Wing', 'NON_COMBUSTIBLE', 'METAL', 15,
     5000000.00, 6000000.00, 30000),
    ('BLD-004', 'LOC-003', 1, 'Distribution Warehouse', 'JOISTED_MASONRY', 'BUILT_UP', 20,
     12000000.00, 15000000.00, 200000)
ON CONFLICT (building_id) DO NOTHING;

-- Policies
INSERT INTO property_insurance.policies (policy_id, policy_number, policy_form,
    customer_id, named_insured, effective_date, expiration_date, status,
    total_premium, total_insured_value, blanket_coverage, agreed_value, valuation_method)
VALUES
    ('POL-PROP-001', 'CP-2024-00001', 'CPP', 'CUST-COMM-001',
     'Acme Manufacturing Corp', '2024-01-01', '2025-01-01', 'ACTIVE',
     125000.00, 85000000.00, true, true, 'REPLACEMENT_COST'),
    ('POL-PROP-002', 'CP-2024-00002', 'BOP', 'CUST-COMM-002',
     'Downtown Retail LLC', '2024-03-01', '2025-03-01', 'ACTIVE',
     8500.00, 2500000.00, false, false, 'REPLACEMENT_COST')
ON CONFLICT (policy_id) DO NOTHING;

-- Policy locations with coverages
INSERT INTO property_insurance.policy_locations (policy_location_id, policy_id, location_id,
    location_schedule_number, building_limit, bpp_limit, bi_limit, extra_expense_limit,
    deductible, wind_deductible_percent, location_premium)
VALUES
    ('PL-001', 'POL-PROP-001', 'LOC-001', 1, 18000000.00, 5000000.00, 3000000.00, 500000.00,
     25000.00, NULL, 35000.00),
    ('PL-002', 'POL-PROP-001', 'LOC-002', 2, 36000000.00, 15000000.00, 8000000.00, 1000000.00,
     50000.00, NULL, 55000.00),
    ('PL-003', 'POL-PROP-001', 'LOC-003', 3, 15000000.00, 10000000.00, 5000000.00, 500000.00,
     25000.00, 2.00, 35000.00)
ON CONFLICT (policy_location_id) DO NOTHING;

-- Coverages
INSERT INTO property_insurance.coverages (coverage_id, policy_id, policy_location_id,
    coverage_type, coverage_limit, deductible, waiting_period_hours, premium, rate_per_hundred)
VALUES
    ('COV-P-001', 'POL-PROP-001', 'PL-001', 'BUILDING', 18000000.00, 25000.00, NULL, 15000.00, 0.0833),
    ('COV-P-002', 'POL-PROP-001', 'PL-001', 'BUSINESS_PERSONAL_PROPERTY', 5000000.00, 25000.00, NULL, 8000.00, 0.1600),
    ('COV-P-003', 'POL-PROP-001', 'PL-001', 'BUSINESS_INCOME', 3000000.00, 0.00, 72, 10000.00, 0.3333),
    ('COV-P-004', 'POL-PROP-001', 'PL-002', 'BUILDING', 36000000.00, 50000.00, NULL, 28000.00, 0.0778),
    ('COV-P-005', 'POL-PROP-001', 'PL-002', 'BUSINESS_PERSONAL_PROPERTY', 15000000.00, 50000.00, NULL, 18000.00, 0.1200),
    ('COV-P-006', 'POL-PROP-001', 'PL-002', 'EQUIPMENT_BREAKDOWN', 5000000.00, 25000.00, 24, 5000.00, 0.1000)
ON CONFLICT (coverage_id) DO NOTHING;

-- Endorsements
INSERT INTO property_insurance.endorsements (endorsement_id, policy_id, endorsement_form,
    endorsement_name, description, sublimit, deductible, additional_premium, effective_date)
VALUES
    ('END-P-001', 'POL-PROP-001', 'CP-1040', 'Earthquake Coverage',
     'Coverage for earthquake damage at all locations', 10000000.00, 100000.00, 15000.00, '2024-01-01'),
    ('END-P-002', 'POL-PROP-001', 'CP-1045', 'Flood Coverage',
     'Coverage for flood damage - excess of NFIP', 5000000.00, 50000.00, 12000.00, '2024-01-01'),
    ('END-P-003', 'POL-PROP-001', 'CP-1532', 'Utility Services - Time Element',
     'Coverage for off-premises utility failure', 1000000.00, 0.00, 2500.00, '2024-01-01')
ON CONFLICT (endorsement_id) DO NOTHING;

-- Scheduled equipment
INSERT INTO property_insurance.scheduled_equipment (equipment_id, policy_id, location_id,
    equipment_type, description, manufacturer, model, year_manufactured, insured_value, premium)
VALUES
    ('EQ-001', 'POL-PROP-001', 'LOC-002', 'CNC_MACHINE',
     'CNC Milling Center - 5-axis', 'Haas', 'UMC-750', 2020, 450000.00, 1500.00),
    ('EQ-002', 'POL-PROP-001', 'LOC-002', 'INJECTION_MOLDER',
     'Plastic Injection Molding Machine', 'Arburg', 'Allrounder 570', 2018, 350000.00, 1200.00),
    ('EQ-003', 'POL-PROP-001', 'LOC-001', 'DATA_CENTER',
     'Server Room Equipment and UPS', 'Various', 'N/A', 2022, 2000000.00, 5000.00)
ON CONFLICT (equipment_id) DO NOTHING;

-- Claims
INSERT INTO property_insurance.claims (claim_id, claim_number, policy_id, location_id,
    loss_date, cause_of_loss, loss_description, is_catastrophe,
    building_damage, bpp_damage, bi_loss, total_claimed, status)
VALUES
    ('CLM-P-001', 'CP-CLM-2024-00001', 'POL-PROP-001', 'LOC-002',
     '2024-07-15 02:30:00', 'FIRE',
     'Electrical fire in production area. Fire suppression limited damage but smoke and water damage to equipment.',
     false, 75000.00, 250000.00, 180000.00, 505000.00, 'OPEN')
ON CONFLICT (claim_id) DO NOTHING;

-- =============================================================================
-- ANALYTICAL QUERIES
-- =============================================================================

-- Total insured values by location
SELECT
    l.location_name,
    l.address_city || ', ' || l.address_state as location,
    l.protection_class,
    pl.building_limit,
    pl.bpp_limit,
    pl.bi_limit,
    pl.building_limit + pl.bpp_limit + pl.bi_limit + COALESCE(pl.extra_expense_limit, 0) as total_tiv,
    pl.location_premium
FROM property_insurance.policy_locations pl
JOIN property_insurance.locations l ON pl.location_id = l.location_id
JOIN property_insurance.policies p ON pl.policy_id = p.policy_id
WHERE p.status = 'ACTIVE'
ORDER BY total_tiv DESC;

-- Coverage distribution
SELECT
    coverage_type,
    COUNT(*) as coverage_count,
    SUM(coverage_limit) as total_limit,
    SUM(premium) as total_premium,
    AVG(rate_per_hundred) as avg_rate
FROM property_insurance.coverages
GROUP BY coverage_type
ORDER BY total_premium DESC;

-- Protection class analysis
SELECT
    l.protection_class,
    COUNT(DISTINCT l.location_id) as location_count,
    SUM(pl.building_limit) as total_building_limit,
    AVG(pl.location_premium) as avg_location_premium
FROM property_insurance.locations l
JOIN property_insurance.policy_locations pl ON l.location_id = pl.location_id
GROUP BY l.protection_class
ORDER BY l.protection_class;

-- Claims loss ratio by cause
SELECT
    cause_of_loss,
    COUNT(*) as claim_count,
    SUM(total_claimed) as total_incurred,
    SUM(total_paid) as total_paid,
    AVG(total_claimed) as avg_claim_size
FROM property_insurance.claims
GROUP BY cause_of_loss
ORDER BY total_incurred DESC;

-- High-value equipment report
SELECT
    se.description,
    se.manufacturer,
    se.model,
    l.location_name,
    se.insured_value,
    se.premium,
    ROUND(se.premium / (se.insured_value / 100), 4) as rate_per_hundred
FROM property_insurance.scheduled_equipment se
JOIN property_insurance.locations l ON se.location_id = l.location_id
WHERE se.insured_value >= 100000
ORDER BY se.insured_value DESC;

-- Business interruption exposure analysis
SELECT
    p.policy_number,
    p.named_insured,
    SUM(c.coverage_limit) as total_bi_limit,
    SUM(c.premium) as bi_premium,
    c.waiting_period_hours
FROM property_insurance.policies p
JOIN property_insurance.coverages c ON p.policy_id = c.policy_id
WHERE c.coverage_type IN ('BUSINESS_INCOME', 'EXTRA_EXPENSE')
AND p.status = 'ACTIVE'
GROUP BY p.policy_id, p.policy_number, p.named_insured, c.waiting_period_hours
ORDER BY total_bi_limit DESC;
