-- =============================================================================
-- OrbitRS Insurance Example: Home Insurance Policy Management
-- =============================================================================
-- Demonstrates PostgreSQL patterns for homeowners/renters insurance:
--   - Property information and valuations
--   - Coverage types (dwelling, personal property, liability)
--   - Endorsements and riders
--   - Claims for property damage
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_home_policy_postgres.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS home_insurance;

-- =============================================================================
-- PROPERTY TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS home_insurance.properties (
    property_id VARCHAR(50) PRIMARY KEY,
    customer_id VARCHAR(50) NOT NULL,
    address_street VARCHAR(200) NOT NULL,
    address_city VARCHAR(100) NOT NULL,
    address_state VARCHAR(2) NOT NULL,
    address_zip VARCHAR(10) NOT NULL,
    property_type VARCHAR(30), -- SINGLE_FAMILY, CONDO, TOWNHOUSE, RENTAL, MOBILE_HOME
    occupancy_type VARCHAR(30), -- OWNER_OCCUPIED, RENTAL, SEASONAL, VACANT
    year_built INTEGER,
    square_footage INTEGER,
    num_stories INTEGER DEFAULT 1,
    num_bedrooms INTEGER,
    num_bathrooms DECIMAL(3,1),
    construction_type VARCHAR(30), -- FRAME, MASONRY, BRICK, STUCCO, STEEL
    roof_type VARCHAR(30), -- ASPHALT_SHINGLE, TILE, METAL, SLATE, FLAT
    roof_age_years INTEGER,
    foundation_type VARCHAR(30), -- SLAB, CRAWL_SPACE, BASEMENT, PIER
    heating_type VARCHAR(30), -- CENTRAL_GAS, ELECTRIC, OIL, HEAT_PUMP, RADIANT
    electrical_type VARCHAR(30), -- CIRCUIT_BREAKER, FUSE, UPDATED
    plumbing_type VARCHAR(30), -- COPPER, PVC, GALVANIZED, PEX
    pool BOOLEAN DEFAULT false,
    pool_type VARCHAR(20), -- IN_GROUND, ABOVE_GROUND
    trampoline BOOLEAN DEFAULT false,
    dog_breed_restricted BOOLEAN DEFAULT false,
    fire_alarm BOOLEAN DEFAULT false,
    burglar_alarm BOOLEAN DEFAULT false,
    smoke_detectors BOOLEAN DEFAULT true,
    sprinkler_system BOOLEAN DEFAULT false,
    gated_community BOOLEAN DEFAULT false,
    distance_fire_station_miles DECIMAL(5,2),
    distance_fire_hydrant_feet INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS home_insurance.property_valuations (
    valuation_id VARCHAR(50) PRIMARY KEY,
    property_id VARCHAR(50) REFERENCES home_insurance.properties(property_id),
    valuation_date DATE NOT NULL,
    valuation_type VARCHAR(30), -- PURCHASE, APPRAISAL, MARKET, REPLACEMENT_COST
    dwelling_value DECIMAL(12,2),
    land_value DECIMAL(12,2),
    total_value DECIMAL(12,2),
    replacement_cost DECIMAL(12,2),
    cost_per_sqft DECIMAL(8,2),
    valuation_source VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- POLICY AND COVERAGE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS home_insurance.policies (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    policy_form VARCHAR(10), -- HO-1, HO-2, HO-3, HO-4, HO-5, HO-6, HO-8
    property_id VARCHAR(50) REFERENCES home_insurance.properties(property_id),
    customer_id VARCHAR(50) NOT NULL,
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    total_premium DECIMAL(10,2),
    payment_plan VARCHAR(20),
    deductible DECIMAL(10,2),
    hurricane_deductible_percent DECIMAL(5,2),
    earthquake_deductible_percent DECIMAL(5,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS home_insurance.coverages (
    coverage_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES home_insurance.policies(policy_id),
    coverage_code VARCHAR(10) NOT NULL,
    -- COVERAGE A: Dwelling
    -- COVERAGE B: Other Structures
    -- COVERAGE C: Personal Property
    -- COVERAGE D: Loss of Use
    -- COVERAGE E: Personal Liability
    -- COVERAGE F: Medical Payments
    coverage_name VARCHAR(100),
    coverage_limit DECIMAL(12,2),
    deductible DECIMAL(10,2),
    premium DECIMAL(10,2),
    coinsurance_percent INTEGER DEFAULT 80,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS home_insurance.endorsements (
    endorsement_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES home_insurance.policies(policy_id),
    endorsement_code VARCHAR(20),
    endorsement_name VARCHAR(100),
    -- Common endorsements:
    -- SCHEDULED_PERSONAL_PROPERTY, WATER_BACKUP, IDENTITY_THEFT,
    -- EQUIPMENT_BREAKDOWN, SERVICE_LINE, INFLATION_GUARD,
    -- REPLACEMENT_COST, ORDINANCE_LAW, EARTHQUAKE, FLOOD
    description TEXT,
    coverage_limit DECIMAL(12,2),
    deductible DECIMAL(10,2),
    additional_premium DECIMAL(10,2),
    effective_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS home_insurance.scheduled_items (
    item_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) REFERENCES home_insurance.policies(policy_id),
    endorsement_id VARCHAR(50) REFERENCES home_insurance.endorsements(endorsement_id),
    item_category VARCHAR(50), -- JEWELRY, ART, FURS, ELECTRONICS, COLLECTIBLES, MUSICAL_INSTRUMENTS
    item_description TEXT NOT NULL,
    appraised_value DECIMAL(12,2),
    appraisal_date DATE,
    serial_number VARCHAR(100),
    purchase_date DATE,
    purchase_price DECIMAL(12,2),
    premium DECIMAL(10,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- CLAIMS TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS home_insurance.claims (
    claim_id VARCHAR(50) PRIMARY KEY,
    claim_number VARCHAR(30) UNIQUE NOT NULL,
    policy_id VARCHAR(50) REFERENCES home_insurance.policies(policy_id),
    property_id VARCHAR(50) REFERENCES home_insurance.properties(property_id),
    loss_date TIMESTAMP NOT NULL,
    report_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    loss_type VARCHAR(30),
    -- FIRE, LIGHTNING, WINDSTORM, HAIL, WATER_DAMAGE, THEFT, VANDALISM,
    -- FALLING_OBJECTS, WEIGHT_OF_ICE_SNOW, ACCIDENTAL_DISCHARGE,
    -- SMOKE, LIABILITY, EXPLOSION
    loss_description TEXT,
    rooms_affected TEXT[],
    is_catastrophe BOOLEAN DEFAULT false,
    catastrophe_code VARCHAR(20),
    dwelling_damage DECIMAL(12,2) DEFAULT 0,
    other_structures_damage DECIMAL(12,2) DEFAULT 0,
    personal_property_damage DECIMAL(12,2) DEFAULT 0,
    loss_of_use_expense DECIMAL(12,2) DEFAULT 0,
    liability_expense DECIMAL(12,2) DEFAULT 0,
    total_claimed DECIMAL(12,2),
    total_paid DECIMAL(12,2) DEFAULT 0,
    deductible_applied DECIMAL(10,2),
    status VARCHAR(20) DEFAULT 'OPEN',
    adjuster_id VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS home_insurance.claim_items (
    item_id VARCHAR(50) PRIMARY KEY,
    claim_id VARCHAR(50) REFERENCES home_insurance.claims(claim_id),
    item_category VARCHAR(50),
    item_description TEXT NOT NULL,
    room_location VARCHAR(50),
    quantity INTEGER DEFAULT 1,
    age_years INTEGER,
    purchase_price DECIMAL(12,2),
    replacement_cost DECIMAL(12,2),
    actual_cash_value DECIMAL(12,2),
    depreciation_percent DECIMAL(5,2),
    claimed_amount DECIMAL(12,2),
    approved_amount DECIMAL(12,2),
    status VARCHAR(20) DEFAULT 'PENDING',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Properties
INSERT INTO home_insurance.properties (property_id, customer_id, address_street, address_city,
    address_state, address_zip, property_type, occupancy_type, year_built, square_footage,
    num_stories, num_bedrooms, num_bathrooms, construction_type, roof_type, roof_age_years,
    foundation_type, pool, fire_alarm, burglar_alarm, smoke_detectors, sprinkler_system,
    distance_fire_station_miles, distance_fire_hydrant_feet)
VALUES
    ('PROP-001', 'CUST-001', '123 Oak Street', 'Pleasantville', 'NY', '10570',
     'SINGLE_FAMILY', 'OWNER_OCCUPIED', 1995, 2400, 2, 4, 2.5, 'FRAME', 'ASPHALT_SHINGLE', 8,
     'BASEMENT', false, true, true, true, false, 1.5, 300),
    ('PROP-002', 'CUST-002', '456 Palm Drive', 'Miami', 'FL', '33101',
     'CONDO', 'OWNER_OCCUPIED', 2015, 1200, 1, 2, 2.0, 'MASONRY', 'TILE', 3,
     'SLAB', true, true, true, true, true, 0.8, 150),
    ('PROP-003', 'CUST-003', '789 Mountain View', 'Denver', 'CO', '80201',
     'TOWNHOUSE', 'OWNER_OCCUPIED', 2008, 1800, 3, 3, 2.5, 'FRAME', 'ASPHALT_SHINGLE', 5,
     'CRAWL_SPACE', false, true, false, true, false, 2.0, 500)
ON CONFLICT (property_id) DO NOTHING;

-- Property valuations
INSERT INTO home_insurance.property_valuations (valuation_id, property_id, valuation_date,
    valuation_type, dwelling_value, land_value, total_value, replacement_cost, cost_per_sqft)
VALUES
    ('VAL-001', 'PROP-001', '2024-01-15', 'REPLACEMENT_COST', 450000.00, 150000.00, 600000.00, 550000.00, 229.17),
    ('VAL-002', 'PROP-002', '2024-02-01', 'REPLACEMENT_COST', 350000.00, 0.00, 350000.00, 320000.00, 266.67),
    ('VAL-003', 'PROP-003', '2024-01-20', 'REPLACEMENT_COST', 380000.00, 80000.00, 460000.00, 420000.00, 233.33)
ON CONFLICT (valuation_id) DO NOTHING;

-- Policies
INSERT INTO home_insurance.policies (policy_id, policy_number, policy_form, property_id,
    customer_id, effective_date, expiration_date, status, total_premium, payment_plan,
    deductible, hurricane_deductible_percent)
VALUES
    ('POL-HOME-001', 'HO-2024-00001', 'HO-3', 'PROP-001', 'CUST-001',
     '2024-01-01', '2025-01-01', 'ACTIVE', 1850.00, 'ANNUAL', 1000.00, NULL),
    ('POL-HOME-002', 'HO-2024-00002', 'HO-6', 'PROP-002', 'CUST-002',
     '2024-02-01', '2025-02-01', 'ACTIVE', 1200.00, 'MONTHLY', 2500.00, 2.00),
    ('POL-HOME-003', 'HO-2024-00003', 'HO-3', 'PROP-003', 'CUST-003',
     '2024-01-15', '2025-01-15', 'ACTIVE', 1650.00, 'QUARTERLY', 1000.00, NULL)
ON CONFLICT (policy_id) DO NOTHING;

-- Coverages
INSERT INTO home_insurance.coverages (coverage_id, policy_id, coverage_code, coverage_name,
    coverage_limit, deductible, premium, coinsurance_percent)
VALUES
    ('COV-H-001', 'POL-HOME-001', 'A', 'Dwelling', 550000.00, 1000.00, 850.00, 80),
    ('COV-H-002', 'POL-HOME-001', 'B', 'Other Structures', 55000.00, 1000.00, 85.00, 80),
    ('COV-H-003', 'POL-HOME-001', 'C', 'Personal Property', 275000.00, 1000.00, 425.00, 80),
    ('COV-H-004', 'POL-HOME-001', 'D', 'Loss of Use', 110000.00, 0.00, 100.00, NULL),
    ('COV-H-005', 'POL-HOME-001', 'E', 'Personal Liability', 300000.00, 0.00, 250.00, NULL),
    ('COV-H-006', 'POL-HOME-001', 'F', 'Medical Payments', 5000.00, 0.00, 50.00, NULL)
ON CONFLICT (coverage_id) DO NOTHING;

-- Endorsements
INSERT INTO home_insurance.endorsements (endorsement_id, policy_id, endorsement_code,
    endorsement_name, description, coverage_limit, additional_premium, effective_date)
VALUES
    ('END-001', 'POL-HOME-001', 'HO-04-61', 'Scheduled Personal Property',
     'Coverage for specifically listed high-value items', 50000.00, 150.00, '2024-01-01'),
    ('END-002', 'POL-HOME-001', 'HO-04-36', 'Water Backup and Sump Overflow',
     'Coverage for damage from water backup through sewers or drains', 25000.00, 75.00, '2024-01-01'),
    ('END-003', 'POL-HOME-002', 'HO-17-32', 'Unit Owners Coverage A Special',
     'Replacement cost coverage for condo unit improvements', 100000.00, 85.00, '2024-02-01')
ON CONFLICT (endorsement_id) DO NOTHING;

-- Scheduled items
INSERT INTO home_insurance.scheduled_items (item_id, policy_id, endorsement_id,
    item_category, item_description, appraised_value, appraisal_date, serial_number, premium)
VALUES
    ('ITEM-001', 'POL-HOME-001', 'END-001', 'JEWELRY',
     'Diamond engagement ring, 2 carat princess cut, platinum setting', 15000.00, '2023-12-01', NULL, 75.00),
    ('ITEM-002', 'POL-HOME-001', 'END-001', 'ART',
     'Original oil painting by local artist, "Sunset over Hudson"', 8000.00, '2023-11-15', NULL, 40.00),
    ('ITEM-003', 'POL-HOME-001', 'END-001', 'ELECTRONICS',
     'Vintage Rolex Submariner watch, 1970s era', 25000.00, '2024-01-05', 'RSM1234567', 125.00)
ON CONFLICT (item_id) DO NOTHING;

-- Claims
INSERT INTO home_insurance.claims (claim_id, claim_number, policy_id, property_id,
    loss_date, loss_type, loss_description, rooms_affected, is_catastrophe,
    dwelling_damage, personal_property_damage, total_claimed, status)
VALUES
    ('CLM-H-001', 'HO-CLM-2024-00001', 'POL-HOME-001', 'PROP-001',
     '2024-04-15 03:30:00', 'WATER_DAMAGE',
     'Pipe burst in second floor bathroom causing water damage to ceiling and floor below',
     ARRAY['Bathroom', 'Kitchen', 'Living Room'],
     false, 12000.00, 3500.00, 15500.00, 'OPEN')
ON CONFLICT (claim_id) DO NOTHING;

-- Claim items
INSERT INTO home_insurance.claim_items (item_id, claim_id, item_category,
    item_description, room_location, quantity, age_years, replacement_cost,
    actual_cash_value, depreciation_percent, claimed_amount, status)
VALUES
    ('CI-001', 'CLM-H-001', 'FLOORING', 'Hardwood flooring - water damaged', 'Kitchen', 1, 10, 3500.00, 2100.00, 40.00, 3500.00, 'APPROVED'),
    ('CI-002', 'CLM-H-001', 'APPLIANCES', 'Microwave - water damaged', 'Kitchen', 1, 5, 400.00, 200.00, 50.00, 400.00, 'APPROVED'),
    ('CI-003', 'CLM-H-001', 'FURNITURE', 'Dining table - water stained', 'Living Room', 1, 3, 1200.00, 960.00, 20.00, 1200.00, 'PENDING')
ON CONFLICT (item_id) DO NOTHING;

-- =============================================================================
-- ANALYTICAL QUERIES
-- =============================================================================

-- Policy coverage summary
SELECT
    p.policy_number,
    p.policy_form,
    pr.address_city || ', ' || pr.address_state as location,
    pr.property_type,
    pr.year_built,
    p.total_premium,
    p.deductible,
    SUM(c.coverage_limit) as total_coverage
FROM home_insurance.policies p
JOIN home_insurance.properties pr ON p.property_id = pr.property_id
JOIN home_insurance.coverages c ON p.policy_id = c.policy_id
WHERE p.status = 'ACTIVE'
GROUP BY p.policy_id, p.policy_number, p.policy_form, pr.address_city, pr.address_state,
         pr.property_type, pr.year_built, p.total_premium, p.deductible
ORDER BY p.policy_number;

-- Property risk assessment
SELECT
    pr.property_id,
    pr.address_city || ', ' || pr.address_state as location,
    pr.year_built,
    2024 - pr.year_built as property_age,
    pr.roof_age_years,
    pr.construction_type,
    CASE
        WHEN pr.fire_alarm AND pr.burglar_alarm AND pr.sprinkler_system THEN 'LOW'
        WHEN pr.fire_alarm OR pr.burglar_alarm THEN 'MEDIUM'
        ELSE 'HIGH'
    END as security_risk,
    CASE
        WHEN pr.roof_age_years > 20 THEN 'HIGH'
        WHEN pr.roof_age_years > 10 THEN 'MEDIUM'
        ELSE 'LOW'
    END as roof_risk,
    pr.distance_fire_station_miles,
    pr.distance_fire_hydrant_feet
FROM home_insurance.properties pr
ORDER BY pr.year_built;

-- Claims by loss type
SELECT
    loss_type,
    COUNT(*) as claim_count,
    SUM(total_claimed) as total_claimed,
    SUM(total_paid) as total_paid,
    AVG(total_claimed) as avg_claim_amount
FROM home_insurance.claims
WHERE loss_date >= CURRENT_DATE - INTERVAL '1 year'
GROUP BY loss_type
ORDER BY claim_count DESC;

-- High-value scheduled items report
SELECT
    p.policy_number,
    si.item_category,
    si.item_description,
    si.appraised_value,
    si.premium,
    e.endorsement_name
FROM home_insurance.scheduled_items si
JOIN home_insurance.policies p ON si.policy_id = p.policy_id
JOIN home_insurance.endorsements e ON si.endorsement_id = e.endorsement_id
WHERE si.appraised_value >= 10000
ORDER BY si.appraised_value DESC;

-- Premium analysis by property characteristics
SELECT
    pr.construction_type,
    pr.roof_type,
    CASE
        WHEN pr.pool THEN 'With Pool'
        ELSE 'No Pool'
    END as pool_status,
    COUNT(*) as policy_count,
    AVG(p.total_premium) as avg_premium,
    AVG(pv.replacement_cost) as avg_replacement_cost
FROM home_insurance.policies p
JOIN home_insurance.properties pr ON p.property_id = pr.property_id
LEFT JOIN home_insurance.property_valuations pv ON pr.property_id = pv.property_id
    AND pv.valuation_type = 'REPLACEMENT_COST'
WHERE p.status = 'ACTIVE'
GROUP BY pr.construction_type, pr.roof_type, pr.pool
ORDER BY avg_premium DESC;
