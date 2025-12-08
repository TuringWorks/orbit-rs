-- =============================================================================
-- OrbitRS Insurance Example: Earthquake Insurance Coverage
-- =============================================================================
-- Demonstrates PostgreSQL patterns for earthquake insurance:
--   - Seismic zone classification
--   - Deductible structures (percentage-based)
--   - Building vulnerability assessment
--   - CEA (California Earthquake Authority) style policies
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_earthquake_coverage.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS earthquake_insurance;

-- =============================================================================
-- SEISMIC ZONE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS earthquake_insurance.seismic_zones (
    zone_id VARCHAR(20) PRIMARY KEY,
    zone_name VARCHAR(100) NOT NULL,
    state VARCHAR(2) NOT NULL,
    fault_proximity VARCHAR(30), -- ON_FAULT, NEAR_FAULT, AWAY_FROM_FAULT
    seismic_design_category VARCHAR(5), -- A, B, C, D, E, F (per building codes)
    pga_500yr DECIMAL(5,3), -- Peak Ground Acceleration (g) for 500-year event
    pga_2500yr DECIMAL(5,3), -- PGA for 2500-year event
    liquefaction_susceptibility VARCHAR(20), -- VERY_HIGH, HIGH, MODERATE, LOW, VERY_LOW
    landslide_susceptibility VARCHAR(20),
    soil_class VARCHAR(5), -- A (hard rock) to F (soft soil)
    base_rate_factor DECIMAL(5,3) DEFAULT 1.000,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS earthquake_insurance.fault_lines (
    fault_id VARCHAR(50) PRIMARY KEY,
    fault_name VARCHAR(100) NOT NULL,
    fault_type VARCHAR(30), -- STRIKE_SLIP, THRUST, NORMAL, OBLIQUE
    length_miles DECIMAL(8,2),
    slip_rate_mm_yr DECIMAL(6,2), -- Annual slip rate in mm
    max_magnitude DECIMAL(3,1), -- Maximum expected magnitude
    last_rupture_date DATE,
    recurrence_interval_years INTEGER,
    states TEXT[],
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- PROPERTY AND STRUCTURE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS earthquake_insurance.insured_properties (
    property_id VARCHAR(50) PRIMARY KEY,
    customer_id VARCHAR(50) NOT NULL,
    zone_id VARCHAR(20) REFERENCES earthquake_insurance.seismic_zones(zone_id),
    address_street VARCHAR(200) NOT NULL,
    address_city VARCHAR(100) NOT NULL,
    address_state VARCHAR(2) NOT NULL,
    address_zip VARCHAR(10) NOT NULL,
    latitude DECIMAL(10,7),
    longitude DECIMAL(10,7),
    property_type VARCHAR(30), -- SINGLE_FAMILY, CONDO, MOBILE_HOME, RENTAL
    dwelling_type VARCHAR(30), -- WOOD_FRAME, MASONRY, CONCRETE, STEEL
    year_built INTEGER,
    square_footage INTEGER,
    num_stories INTEGER,
    foundation_type VARCHAR(30), -- CRIPPLE_WALL, RAISED_PERIMETER, SLAB, BASEMENT, HILLSIDE
    foundation_bolted BOOLEAN DEFAULT false,
    cripple_wall_braced BOOLEAN DEFAULT false,
    water_heater_strapped BOOLEAN DEFAULT false,
    soft_story BOOLEAN DEFAULT false, -- Weak first floor (e.g., parking)
    chimney_type VARCHAR(20), -- MASONRY, PREFAB, NONE
    retrofit_completed BOOLEAN DEFAULT false,
    retrofit_year INTEGER,
    dwelling_value DECIMAL(12,2),
    contents_value DECIMAL(12,2),
    loss_of_use_value DECIMAL(12,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS earthquake_insurance.building_assessments (
    assessment_id VARCHAR(50) PRIMARY KEY,
    property_id VARCHAR(50) REFERENCES earthquake_insurance.insured_properties(property_id),
    assessment_date DATE NOT NULL,
    assessor_id VARCHAR(50),
    -- Structural assessment
    structural_score INTEGER, -- 1-100
    foundation_score INTEGER,
    chimney_score INTEGER,
    attachment_score INTEGER, -- How well attached to foundation
    shear_wall_score INTEGER,
    -- Vulnerability factors
    age_factor DECIMAL(4,3), -- Multiplier based on age
    construction_factor DECIMAL(4,3),
    soil_factor DECIMAL(4,3),
    foundation_factor DECIMAL(4,3),
    overall_vulnerability DECIMAL(4,3),
    risk_grade VARCHAR(5), -- A, B, C, D, F
    recommendations TEXT[],
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- POLICY AND COVERAGE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS earthquake_insurance.policies (
    policy_id VARCHAR(50) PRIMARY KEY,
    policy_number VARCHAR(30) UNIQUE NOT NULL,
    policy_type VARCHAR(30), -- STAND_ALONE, ENDORSEMENT, CEA, DIC
    property_id VARCHAR(50) REFERENCES earthquake_insurance.insured_properties(property_id),
    customer_id VARCHAR(50) NOT NULL,
    effective_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    -- Coverage limits
    dwelling_limit DECIMAL(12,2),
    contents_limit DECIMAL(12,2),
    loss_of_use_limit DECIMAL(12,2),
    -- Deductibles (typically percentage-based for EQ)
    dwelling_deductible_percent DECIMAL(4,2), -- 5%, 10%, 15%, 20%, 25%
    contents_deductible_percent DECIMAL(4,2),
    -- Premium
    base_premium DECIMAL(10,2),
    retrofit_discount DECIMAL(8,2) DEFAULT 0,
    mitigation_discount DECIMAL(8,2) DEFAULT 0,
    total_premium DECIMAL(10,2),
    -- Masonry veneer coverage
    masonry_veneer_coverage BOOLEAN DEFAULT false,
    masonry_veneer_limit DECIMAL(10,2),
    -- Building code upgrade coverage
    building_code_coverage BOOLEAN DEFAULT false,
    building_code_limit DECIMAL(10,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS earthquake_insurance.premium_factors (
    factor_id VARCHAR(50) PRIMARY KEY,
    zone_id VARCHAR(20) REFERENCES earthquake_insurance.seismic_zones(zone_id),
    dwelling_type VARCHAR(30),
    year_built_range VARCHAR(20), -- PRE_1940, 1940_1970, 1970_1990, 1990_2000, POST_2000
    foundation_type VARCHAR(30),
    num_stories INTEGER,
    base_rate_per_thousand DECIMAL(8,4),
    effective_date DATE,
    expiration_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- CLAIMS TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS earthquake_insurance.earthquake_events (
    event_id VARCHAR(50) PRIMARY KEY,
    event_name VARCHAR(100),
    event_date TIMESTAMP NOT NULL,
    magnitude DECIMAL(3,1) NOT NULL,
    depth_km DECIMAL(6,2),
    epicenter_latitude DECIMAL(10,7),
    epicenter_longitude DECIMAL(10,7),
    epicenter_location VARCHAR(200),
    fault_id VARCHAR(50) REFERENCES earthquake_insurance.fault_lines(fault_id),
    affected_zones TEXT[],
    aftershock_count INTEGER,
    tsunami_generated BOOLEAN DEFAULT false,
    pcs_number VARCHAR(20),
    state_declaration BOOLEAN DEFAULT false,
    federal_declaration BOOLEAN DEFAULT false,
    estimated_insured_loss DECIMAL(16,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS earthquake_insurance.claims (
    claim_id VARCHAR(50) PRIMARY KEY,
    claim_number VARCHAR(30) UNIQUE NOT NULL,
    policy_id VARCHAR(50) REFERENCES earthquake_insurance.policies(policy_id),
    event_id VARCHAR(50) REFERENCES earthquake_insurance.earthquake_events(event_id),
    property_id VARCHAR(50) REFERENCES earthquake_insurance.insured_properties(property_id),
    report_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    inspection_date DATE,
    -- Damage assessment
    damage_grade VARCHAR(20), -- NONE, SLIGHT, MODERATE, EXTENSIVE, COMPLETE
    habitability_status VARCHAR(20), -- HABITABLE, LIMITED_ENTRY, RED_TAGGED
    structural_damage DECIMAL(12,2),
    chimney_damage DECIMAL(12,2),
    foundation_damage DECIMAL(12,2),
    contents_damage DECIMAL(12,2),
    loss_of_use_expense DECIMAL(12,2),
    -- Claims processing
    total_damage DECIMAL(12,2),
    dwelling_deductible DECIMAL(12,2),
    contents_deductible DECIMAL(12,2),
    total_deductible DECIMAL(12,2),
    net_claim_amount DECIMAL(12,2),
    total_paid DECIMAL(12,2) DEFAULT 0,
    status VARCHAR(20) DEFAULT 'OPEN',
    adjuster_id VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMP
);

CREATE TABLE IF NOT EXISTS earthquake_insurance.damage_details (
    detail_id VARCHAR(50) PRIMARY KEY,
    claim_id VARCHAR(50) REFERENCES earthquake_insurance.claims(claim_id),
    damage_category VARCHAR(50),
    -- FOUNDATION_CRACK, FOUNDATION_SHIFT, CRIPPLE_WALL_COLLAPSE,
    -- CHIMNEY_COLLAPSE, CHIMNEY_CRACK, WALL_CRACK, STRUCTURAL_CRACK,
    -- WATER_HEATER, GAS_LINE, ELECTRICAL, BROKEN_GLASS, FALLEN_OBJECTS
    location_in_structure VARCHAR(100),
    description TEXT,
    repair_estimate DECIMAL(12,2),
    actual_cost DECIMAL(12,2),
    repair_contractor VARCHAR(200),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Seismic zones (California focused)
INSERT INTO earthquake_insurance.seismic_zones (zone_id, zone_name, state,
    fault_proximity, seismic_design_category, pga_500yr, pga_2500yr,
    liquefaction_susceptibility, soil_class, base_rate_factor)
VALUES
    ('CA-SF-001', 'San Francisco Downtown', 'CA', 'NEAR_FAULT', 'D', 0.65, 1.10,
     'HIGH', 'D', 2.500),
    ('CA-SF-002', 'San Francisco Marina', 'CA', 'NEAR_FAULT', 'E', 0.70, 1.20,
     'VERY_HIGH', 'E', 3.200),
    ('CA-LA-001', 'Los Angeles Downtown', 'CA', 'NEAR_FAULT', 'D', 0.55, 0.95,
     'MODERATE', 'D', 2.000),
    ('CA-LA-002', 'Los Angeles Hollywood Hills', 'CA', 'ON_FAULT', 'D', 0.75, 1.25,
     'LOW', 'C', 2.800),
    ('CA-SD-001', 'San Diego Central', 'CA', 'AWAY_FROM_FAULT', 'C', 0.35, 0.60,
     'LOW', 'C', 1.200),
    ('WA-SEA-001', 'Seattle Downtown', 'WA', 'NEAR_FAULT', 'D', 0.45, 0.80,
     'HIGH', 'D', 1.800)
ON CONFLICT (zone_id) DO NOTHING;

-- Major fault lines
INSERT INTO earthquake_insurance.fault_lines (fault_id, fault_name, fault_type,
    length_miles, slip_rate_mm_yr, max_magnitude, recurrence_interval_years, states)
VALUES
    ('SAF-001', 'San Andreas Fault - Northern Section', 'STRIKE_SLIP',
     300.00, 24.0, 8.0, 150, ARRAY['CA']),
    ('SAF-002', 'San Andreas Fault - Southern Section', 'STRIKE_SLIP',
     350.00, 25.0, 7.8, 175, ARRAY['CA']),
    ('HAY-001', 'Hayward Fault', 'STRIKE_SLIP',
     62.00, 9.0, 7.0, 161, ARRAY['CA']),
    ('CSZ-001', 'Cascadia Subduction Zone', 'THRUST',
     700.00, 40.0, 9.0, 500, ARRAY['WA', 'OR', 'CA'])
ON CONFLICT (fault_id) DO NOTHING;

-- Insured properties
INSERT INTO earthquake_insurance.insured_properties (property_id, customer_id, zone_id,
    address_street, address_city, address_state, address_zip, latitude, longitude,
    property_type, dwelling_type, year_built, square_footage, num_stories,
    foundation_type, foundation_bolted, cripple_wall_braced, water_heater_strapped,
    soft_story, retrofit_completed, dwelling_value, contents_value, loss_of_use_value)
VALUES
    ('PROP-EQ-001', 'CUST-001', 'CA-SF-001',
     '1234 Nob Hill Ave', 'San Francisco', 'CA', '94109',
     37.7930, -122.4130, 'SINGLE_FAMILY', 'WOOD_FRAME', 1925, 2200, 2,
     'CRIPPLE_WALL', false, false, true, false, false,
     1500000.00, 200000.00, 150000.00),
    ('PROP-EQ-002', 'CUST-002', 'CA-SF-002',
     '567 Marina Blvd', 'San Francisco', 'CA', '94123',
     37.8035, -122.4370, 'SINGLE_FAMILY', 'WOOD_FRAME', 1920, 1800, 2,
     'CRIPPLE_WALL', true, true, true, false, true,
     2200000.00, 300000.00, 200000.00),
    ('PROP-EQ-003', 'CUST-003', 'CA-LA-002',
     '890 Hollywood Hills Dr', 'Los Angeles', 'CA', '90068',
     34.1160, -118.3400, 'SINGLE_FAMILY', 'WOOD_FRAME', 1965, 3500, 1,
     'HILLSIDE', false, false, true, false, false,
     2800000.00, 400000.00, 250000.00)
ON CONFLICT (property_id) DO NOTHING;

-- Building assessments
INSERT INTO earthquake_insurance.building_assessments (assessment_id, property_id,
    assessment_date, structural_score, foundation_score, chimney_score,
    age_factor, construction_factor, soil_factor, foundation_factor,
    overall_vulnerability, risk_grade, recommendations)
VALUES
    ('ASSESS-001', 'PROP-EQ-001', '2024-01-15',
     65, 45, 30,
     1.350, 0.900, 1.200, 1.400, 1.620, 'C',
     ARRAY['Bolt foundation to sill plate', 'Brace cripple walls', 'Strap water heater', 'Evaluate masonry chimney']),
    ('ASSESS-002', 'PROP-EQ-002', '2024-02-01',
     85, 80, 75,
     1.400, 0.900, 1.350, 0.950, 1.135, 'B',
     ARRAY['Continue monitoring', 'Consider soft-story retrofit if adding garage']),
    ('ASSESS-003', 'PROP-EQ-003', '2024-01-20',
     70, 55, 90,
     1.100, 0.950, 1.000, 1.300, 1.360, 'C',
     ARRAY['Hillside foundation inspection recommended', 'Install flexible gas connections'])
ON CONFLICT (assessment_id) DO NOTHING;

-- Policies
INSERT INTO earthquake_insurance.policies (policy_id, policy_number, policy_type,
    property_id, customer_id, effective_date, expiration_date, status,
    dwelling_limit, contents_limit, loss_of_use_limit,
    dwelling_deductible_percent, contents_deductible_percent,
    base_premium, retrofit_discount, total_premium,
    masonry_veneer_coverage, building_code_coverage)
VALUES
    ('POL-EQ-001', 'EQ-2024-00001', 'CEA', 'PROP-EQ-001', 'CUST-001',
     '2024-02-01', '2025-02-01', 'ACTIVE',
     1500000.00, 100000.00, 75000.00,
     15.00, 10.00, 8500.00, 0.00, 8500.00,
     true, true),
    ('POL-EQ-002', 'EQ-2024-00002', 'CEA', 'PROP-EQ-002', 'CUST-002',
     '2024-03-01', '2025-03-01', 'ACTIVE',
     2200000.00, 200000.00, 100000.00,
     10.00, 10.00, 12000.00, 1800.00, 10200.00,
     true, true),
    ('POL-EQ-003', 'EQ-2024-00003', 'STAND_ALONE', 'PROP-EQ-003', 'CUST-003',
     '2024-02-15', '2025-02-15', 'ACTIVE',
     2800000.00, 300000.00, 150000.00,
     15.00, 15.00, 15000.00, 0.00, 15000.00,
     false, true)
ON CONFLICT (policy_id) DO NOTHING;

-- Historical earthquake event
INSERT INTO earthquake_insurance.earthquake_events (event_id, event_name, event_date,
    magnitude, depth_km, epicenter_latitude, epicenter_longitude, epicenter_location,
    affected_zones, pcs_number, federal_declaration, estimated_insured_loss)
VALUES
    ('EQ-2024-NAPA', 'Napa Valley Earthquake', '2024-08-15 03:20:44',
     6.0, 10.5, 38.2200, -122.3100, 'American Canyon, Napa County, CA',
     ARRAY['CA-SF-001', 'CA-SF-002'], '2024-45', true, 500000000.00)
ON CONFLICT (event_id) DO NOTHING;

-- =============================================================================
-- ANALYTICAL QUERIES
-- =============================================================================

-- Policy exposure by seismic zone
SELECT
    sz.zone_name,
    sz.seismic_design_category,
    sz.pga_500yr as expected_pga,
    sz.liquefaction_susceptibility,
    COUNT(p.policy_id) as policy_count,
    SUM(p.dwelling_limit) as total_dwelling_exposure,
    SUM(p.contents_limit) as total_contents_exposure,
    SUM(p.total_premium) as total_premium
FROM earthquake_insurance.seismic_zones sz
LEFT JOIN earthquake_insurance.insured_properties ip ON sz.zone_id = ip.zone_id
LEFT JOIN earthquake_insurance.policies p ON ip.property_id = p.property_id AND p.status = 'ACTIVE'
GROUP BY sz.zone_id, sz.zone_name, sz.seismic_design_category, sz.pga_500yr, sz.liquefaction_susceptibility
ORDER BY total_dwelling_exposure DESC NULLS LAST;

-- Retrofit vs non-retrofit risk analysis
SELECT
    ip.retrofit_completed,
    COUNT(*) as property_count,
    AVG(ba.overall_vulnerability) as avg_vulnerability,
    AVG(p.total_premium) as avg_premium,
    SUM(p.dwelling_limit) as total_exposure,
    SUM(p.retrofit_discount) as total_retrofit_savings
FROM earthquake_insurance.insured_properties ip
JOIN earthquake_insurance.building_assessments ba ON ip.property_id = ba.property_id
JOIN earthquake_insurance.policies p ON ip.property_id = p.property_id
WHERE p.status = 'ACTIVE'
GROUP BY ip.retrofit_completed
ORDER BY ip.retrofit_completed;

-- Deductible analysis (potential out-of-pocket for insureds)
SELECT
    p.policy_number,
    ip.address_city,
    p.dwelling_limit,
    p.dwelling_deductible_percent,
    ROUND(p.dwelling_limit * p.dwelling_deductible_percent / 100, 2) as dwelling_deductible_amount,
    p.contents_limit,
    p.contents_deductible_percent,
    ROUND(p.contents_limit * p.contents_deductible_percent / 100, 2) as contents_deductible_amount,
    ROUND(p.dwelling_limit * p.dwelling_deductible_percent / 100 +
          p.contents_limit * p.contents_deductible_percent / 100, 2) as total_potential_deductible
FROM earthquake_insurance.policies p
JOIN earthquake_insurance.insured_properties ip ON p.property_id = ip.property_id
WHERE p.status = 'ACTIVE'
ORDER BY total_potential_deductible DESC;

-- Foundation type risk distribution
SELECT
    ip.foundation_type,
    ip.foundation_bolted,
    ip.cripple_wall_braced,
    COUNT(*) as property_count,
    AVG(ba.foundation_factor) as avg_foundation_factor,
    AVG(p.total_premium) as avg_premium
FROM earthquake_insurance.insured_properties ip
LEFT JOIN earthquake_insurance.building_assessments ba ON ip.property_id = ba.property_id
LEFT JOIN earthquake_insurance.policies p ON ip.property_id = p.property_id
GROUP BY ip.foundation_type, ip.foundation_bolted, ip.cripple_wall_braced
ORDER BY avg_foundation_factor DESC NULLS LAST;

-- Building age and vulnerability correlation
SELECT
    CASE
        WHEN ip.year_built < 1940 THEN 'Pre-1940'
        WHEN ip.year_built < 1970 THEN '1940-1969'
        WHEN ip.year_built < 1990 THEN '1970-1989'
        WHEN ip.year_built < 2000 THEN '1990-1999'
        ELSE '2000+'
    END as era,
    COUNT(*) as property_count,
    AVG(ba.structural_score) as avg_structural_score,
    AVG(ba.overall_vulnerability) as avg_vulnerability,
    STRING_AGG(DISTINCT ba.risk_grade, ', ') as risk_grades
FROM earthquake_insurance.insured_properties ip
LEFT JOIN earthquake_insurance.building_assessments ba ON ip.property_id = ba.property_id
GROUP BY era
ORDER BY era;

-- Fault proximity exposure
SELECT
    sz.fault_proximity,
    COUNT(DISTINCT ip.property_id) as property_count,
    SUM(p.dwelling_limit) as total_dwelling_exposure,
    AVG(sz.pga_500yr) as avg_expected_pga,
    SUM(p.total_premium) as total_premium,
    ROUND(SUM(p.total_premium) * 1000 / NULLIF(SUM(p.dwelling_limit), 0), 4) as rate_per_thousand
FROM earthquake_insurance.seismic_zones sz
JOIN earthquake_insurance.insured_properties ip ON sz.zone_id = ip.zone_id
JOIN earthquake_insurance.policies p ON ip.property_id = p.property_id
WHERE p.status = 'ACTIVE'
GROUP BY sz.fault_proximity
ORDER BY total_dwelling_exposure DESC;
