-- =============================================================================
-- OrbitRS Insurance Example: Catastrophe and Disaster Modeling
-- =============================================================================
-- Demonstrates PostgreSQL patterns for catastrophe risk management:
--   - Exposure accumulation tracking
--   - Event scenario modeling
--   - Probable Maximum Loss (PML) analysis
--   - Reinsurance recovery calculations
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_catastrophe_modeling.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS catastrophe;

-- =============================================================================
-- GEOGRAPHIC AND EXPOSURE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS catastrophe.geographic_zones (
    zone_id VARCHAR(20) PRIMARY KEY,
    zone_name VARCHAR(100) NOT NULL,
    zone_type VARCHAR(30), -- CRESTA, AIR_GEOCODE, RMS_GEOCODE, STATE, COUNTY
    parent_zone_id VARCHAR(20),
    state VARCHAR(2),
    country VARCHAR(3) DEFAULT 'USA',
    latitude DECIMAL(10,7),
    longitude DECIMAL(10,7),
    area_sq_miles DECIMAL(12,2),
    population INTEGER,
    -- Risk characteristics
    hurricane_zone VARCHAR(10), -- HVHZ, WIND_BORNE_DEBRIS, STANDARD
    earthquake_zone VARCHAR(10), -- HIGH, MODERATE, LOW
    flood_zone VARCHAR(10), -- A, AE, X, V, VE
    wildfire_zone VARCHAR(10), -- HIGH, MODERATE, LOW
    tornado_zone VARCHAR(10), -- ALLEY, MODERATE, LOW
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS catastrophe.exposure_locations (
    exposure_id VARCHAR(50) PRIMARY KEY,
    policy_id VARCHAR(50) NOT NULL,
    location_id VARCHAR(50),
    zone_id VARCHAR(20) REFERENCES catastrophe.geographic_zones(zone_id),
    address VARCHAR(300),
    latitude DECIMAL(10,7),
    longitude DECIMAL(10,7),
    occupancy_code VARCHAR(20),
    construction_code VARCHAR(20),
    year_built INTEGER,
    num_stories INTEGER,
    building_value DECIMAL(14,2),
    contents_value DECIMAL(14,2),
    bi_value DECIMAL(14,2),
    total_insured_value DECIMAL(14,2),
    -- Vulnerability characteristics
    roof_shape VARCHAR(20),
    roof_cover VARCHAR(20),
    roof_deck_attachment VARCHAR(20),
    wall_siding VARCHAR(20),
    opening_protection VARCHAR(20), -- HURRICANE_RATED, BASIC, NONE
    foundation_type VARCHAR(20),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- PERIL AND EVENT TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS catastrophe.perils (
    peril_id VARCHAR(20) PRIMARY KEY,
    peril_name VARCHAR(50) NOT NULL,
    peril_category VARCHAR(30), -- WIND, FLOOD, EARTHQUAKE, FIRE, SEVERE_STORM
    is_modeled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS catastrophe.historical_events (
    event_id VARCHAR(50) PRIMARY KEY,
    event_name VARCHAR(100) NOT NULL,
    peril_id VARCHAR(20) REFERENCES catastrophe.perils(peril_id),
    event_date DATE NOT NULL,
    landfall_date DATE,
    -- Event parameters
    category INTEGER, -- Hurricane category (1-5), EQ magnitude, etc.
    max_wind_mph INTEGER,
    central_pressure_mb INTEGER,
    magnitude DECIMAL(3,1), -- For earthquakes
    affected_states TEXT[],
    -- Impact metrics
    industry_insured_loss DECIMAL(16,2),
    industry_economic_loss DECIMAL(16,2),
    fatalities INTEGER,
    properties_damaged INTEGER,
    pcs_number VARCHAR(20), -- Property Claims Services number
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS catastrophe.modeled_events (
    scenario_id VARCHAR(50) PRIMARY KEY,
    scenario_name VARCHAR(200) NOT NULL,
    peril_id VARCHAR(20) REFERENCES catastrophe.perils(peril_id),
    event_type VARCHAR(30), -- HISTORICAL, STOCHASTIC, DETERMINISTIC
    return_period_years INTEGER, -- e.g., 100, 250, 500 year event
    annual_exceedance_probability DECIMAL(10,8),
    -- Event footprint
    landfall_zone VARCHAR(20),
    track_path TEXT, -- JSON or GeoJSON
    intensity_parameter VARCHAR(30),
    intensity_value DECIMAL(10,2),
    -- Modeled losses (industry)
    industry_ground_up_loss DECIMAL(16,2),
    industry_gross_loss DECIMAL(16,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- LOSS ANALYSIS TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS catastrophe.exposure_loss_results (
    result_id VARCHAR(50) PRIMARY KEY,
    exposure_id VARCHAR(50) REFERENCES catastrophe.exposure_locations(exposure_id),
    scenario_id VARCHAR(50) REFERENCES catastrophe.modeled_events(scenario_id),
    analysis_date DATE DEFAULT CURRENT_DATE,
    -- Ground-up losses by coverage
    building_loss DECIMAL(14,2) DEFAULT 0,
    contents_loss DECIMAL(14,2) DEFAULT 0,
    bi_loss DECIMAL(14,2) DEFAULT 0,
    total_ground_up_loss DECIMAL(14,2),
    -- Gross losses (after deductible, before reinsurance)
    deductible_applied DECIMAL(12,2),
    gross_loss DECIMAL(14,2),
    -- Damage ratios
    building_damage_ratio DECIMAL(6,4),
    contents_damage_ratio DECIMAL(6,4),
    bi_days_lost INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS catastrophe.aggregate_loss_results (
    aggregate_id VARCHAR(50) PRIMARY KEY,
    scenario_id VARCHAR(50) REFERENCES catastrophe.modeled_events(scenario_id),
    analysis_date DATE DEFAULT CURRENT_DATE,
    analysis_type VARCHAR(30), -- BY_ZONE, BY_STATE, BY_LOB, PORTFOLIO
    grouping_key VARCHAR(100), -- Zone ID, state code, etc.
    -- Exposure summary
    location_count INTEGER,
    total_tiv DECIMAL(16,2),
    -- Loss summary
    total_ground_up_loss DECIMAL(16,2),
    total_gross_loss DECIMAL(16,2),
    total_net_loss DECIMAL(16,2), -- After reinsurance
    -- Statistics
    average_damage_ratio DECIMAL(6,4),
    standard_deviation DECIMAL(14,2),
    coefficient_of_variation DECIMAL(6,4),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- REINSURANCE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS catastrophe.reinsurance_treaties (
    treaty_id VARCHAR(50) PRIMARY KEY,
    treaty_name VARCHAR(200) NOT NULL,
    treaty_type VARCHAR(30), -- QUOTA_SHARE, EXCESS_OF_LOSS, AGGREGATE_XOL, CATASTROPHE_XOL
    inception_date DATE NOT NULL,
    expiration_date DATE NOT NULL,
    peril_covered TEXT[], -- Array of peril IDs
    territory TEXT[], -- Array of zone IDs or state codes
    -- Treaty terms
    attachment_point DECIMAL(14,2),
    limit_amount DECIMAL(14,2),
    ceding_percent DECIMAL(5,2), -- For quota share
    reinstatements INTEGER DEFAULT 0,
    aggregate_limit DECIMAL(14,2),
    annual_aggregate_deductible DECIMAL(14,2),
    rate_on_line DECIMAL(8,6),
    premium DECIMAL(12,2),
    reinsurer_name VARCHAR(200),
    status VARCHAR(20) DEFAULT 'ACTIVE',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS catastrophe.reinsurance_recoveries (
    recovery_id VARCHAR(50) PRIMARY KEY,
    treaty_id VARCHAR(50) REFERENCES catastrophe.reinsurance_treaties(treaty_id),
    scenario_id VARCHAR(50) REFERENCES catastrophe.modeled_events(scenario_id),
    event_id VARCHAR(50) REFERENCES catastrophe.historical_events(event_id),
    gross_loss DECIMAL(14,2),
    ceded_loss DECIMAL(14,2),
    net_loss DECIMAL(14,2),
    reinstatement_premium DECIMAL(12,2),
    recovery_status VARCHAR(20),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- EP CURVE TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS catastrophe.ep_curves (
    curve_id VARCHAR(50) PRIMARY KEY,
    curve_name VARCHAR(200) NOT NULL,
    analysis_date DATE DEFAULT CURRENT_DATE,
    peril_id VARCHAR(20) REFERENCES catastrophe.perils(peril_id),
    perspective VARCHAR(20), -- GROUND_UP, GROSS, NET
    territory VARCHAR(50),
    lob VARCHAR(50),
    model_vendor VARCHAR(30), -- AIR, RMS, CoreLogic
    model_version VARCHAR(20),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS catastrophe.ep_curve_points (
    point_id VARCHAR(50) PRIMARY KEY,
    curve_id VARCHAR(50) REFERENCES catastrophe.ep_curves(curve_id),
    return_period INTEGER NOT NULL, -- 10, 25, 50, 100, 250, 500, 1000
    exceedance_probability DECIMAL(10,8),
    oep_loss DECIMAL(16,2), -- Occurrence Exceedance Probability
    aep_loss DECIMAL(16,2), -- Aggregate Exceedance Probability
    aal DECIMAL(14,2), -- Average Annual Loss (at this point)
    standard_error DECIMAL(14,2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Geographic zones
INSERT INTO catastrophe.geographic_zones (zone_id, zone_name, zone_type, state,
    hurricane_zone, earthquake_zone, flood_zone)
VALUES
    ('FL-001', 'Miami-Dade County', 'COUNTY', 'FL', 'HVHZ', 'LOW', 'AE'),
    ('FL-002', 'Broward County', 'COUNTY', 'FL', 'HVHZ', 'LOW', 'AE'),
    ('FL-003', 'Palm Beach County', 'COUNTY', 'FL', 'WIND_BORNE_DEBRIS', 'LOW', 'X'),
    ('CA-001', 'Los Angeles County', 'COUNTY', 'CA', 'LOW', 'HIGH', 'X'),
    ('CA-002', 'San Francisco County', 'COUNTY', 'CA', 'LOW', 'HIGH', 'AE'),
    ('TX-001', 'Harris County', 'COUNTY', 'TX', 'STANDARD', 'LOW', 'A'),
    ('TX-002', 'Galveston County', 'COUNTY', 'TX', 'HVHZ', 'LOW', 'VE')
ON CONFLICT (zone_id) DO NOTHING;

-- Perils
INSERT INTO catastrophe.perils (peril_id, peril_name, peril_category, is_modeled)
VALUES
    ('HU', 'Hurricane', 'WIND', true),
    ('EQ', 'Earthquake', 'EARTHQUAKE', true),
    ('FL', 'Flood', 'FLOOD', true),
    ('WT', 'Winter Storm', 'SEVERE_STORM', true),
    ('TOR', 'Tornado', 'SEVERE_STORM', true),
    ('WF', 'Wildfire', 'FIRE', true),
    ('CV', 'Convective Storm', 'SEVERE_STORM', true)
ON CONFLICT (peril_id) DO NOTHING;

-- Historical events
INSERT INTO catastrophe.historical_events (event_id, event_name, peril_id, event_date,
    category, max_wind_mph, affected_states, industry_insured_loss, pcs_number)
VALUES
    ('HU-2022-IAN', 'Hurricane Ian', 'HU', '2022-09-28', 4, 150,
     ARRAY['FL', 'SC', 'NC'], 50000000000.00, '2022-65'),
    ('HU-2017-IRMA', 'Hurricane Irma', 'HU', '2017-09-10', 4, 130,
     ARRAY['FL', 'GA', 'SC'], 32000000000.00, '2017-60'),
    ('EQ-1994-NORTH', 'Northridge Earthquake', 'EQ', '1994-01-17', NULL, NULL,
     ARRAY['CA'], 15000000000.00, '1994-03'),
    ('HU-2012-SANDY', 'Hurricane Sandy', 'HU', '2012-10-29', 1, 115,
     ARRAY['NJ', 'NY', 'CT', 'PA'], 18750000000.00, '2012-65')
ON CONFLICT (event_id) DO NOTHING;

-- Modeled scenarios
INSERT INTO catastrophe.modeled_events (scenario_id, scenario_name, peril_id,
    event_type, return_period_years, annual_exceedance_probability,
    landfall_zone, industry_ground_up_loss, industry_gross_loss)
VALUES
    ('HU-100-FL-SE', 'Southeast Florida Cat 4 Hurricane - 100yr', 'HU',
     'STOCHASTIC', 100, 0.01, 'FL-001', 80000000000.00, 65000000000.00),
    ('HU-250-FL-SE', 'Southeast Florida Cat 5 Hurricane - 250yr', 'HU',
     'STOCHASTIC', 250, 0.004, 'FL-001', 150000000000.00, 125000000000.00),
    ('EQ-250-CA-SF', 'San Francisco Bay Area EQ - 250yr', 'EQ',
     'STOCHASTIC', 250, 0.004, 'CA-002', 100000000000.00, 80000000000.00),
    ('HU-100-TX-GC', 'Texas Gulf Coast Hurricane - 100yr', 'HU',
     'STOCHASTIC', 100, 0.01, 'TX-002', 45000000000.00, 38000000000.00)
ON CONFLICT (scenario_id) DO NOTHING;

-- Sample exposure locations
INSERT INTO catastrophe.exposure_locations (exposure_id, policy_id, zone_id,
    address, latitude, longitude, construction_code, year_built, num_stories,
    building_value, contents_value, bi_value, total_insured_value,
    roof_cover, opening_protection)
VALUES
    ('EXP-001', 'POL-PROP-FL-001', 'FL-001', '100 Ocean Drive, Miami Beach, FL 33139',
     25.7617, -80.1918, 'MASONRY', 2005, 3, 2500000.00, 500000.00, 300000.00, 3300000.00,
     'CONCRETE_TILE', 'HURRICANE_RATED'),
    ('EXP-002', 'POL-PROP-FL-002', 'FL-001', '500 Brickell Ave, Miami, FL 33131',
     25.7654, -80.1908, 'STEEL_FRAME', 2018, 50, 150000000.00, 25000000.00, 15000000.00, 190000000.00,
     'BUILT_UP', 'HURRICANE_RATED'),
    ('EXP-003', 'POL-PROP-CA-001', 'CA-002', '1 Market St, San Francisco, CA 94105',
     37.7941, -122.3951, 'STEEL_FRAME', 1975, 40, 200000000.00, 30000000.00, 20000000.00, 250000000.00,
     'BUILT_UP', 'NONE')
ON CONFLICT (exposure_id) DO NOTHING;

-- Reinsurance treaties
INSERT INTO catastrophe.reinsurance_treaties (treaty_id, treaty_name, treaty_type,
    inception_date, expiration_date, peril_covered, territory,
    attachment_point, limit_amount, rate_on_line, premium, reinsurer_name)
VALUES
    ('TR-CAT-2024-001', '2024 Catastrophe Excess of Loss Layer 1', 'CATASTROPHE_XOL',
     '2024-01-01', '2025-01-01', ARRAY['HU', 'EQ'], ARRAY['FL', 'CA', 'TX'],
     50000000.00, 100000000.00, 0.08, 8000000.00, 'Swiss Re'),
    ('TR-CAT-2024-002', '2024 Catastrophe Excess of Loss Layer 2', 'CATASTROPHE_XOL',
     '2024-01-01', '2025-01-01', ARRAY['HU', 'EQ'], ARRAY['FL', 'CA', 'TX'],
     150000000.00, 200000000.00, 0.04, 8000000.00, 'Munich Re'),
    ('TR-QS-2024-001', '2024 Florida Property Quota Share', 'QUOTA_SHARE',
     '2024-01-01', '2025-01-01', ARRAY['HU', 'FL'], ARRAY['FL'],
     0.00, NULL, NULL, 5000000.00, 'Lloyd''s Syndicate 1234')
ON CONFLICT (treaty_id) DO NOTHING;

-- EP Curves
INSERT INTO catastrophe.ep_curves (curve_id, curve_name, analysis_date, peril_id,
    perspective, territory, model_vendor, model_version)
VALUES
    ('EP-HU-FL-2024', 'Florida Hurricane EP Curve 2024', '2024-01-15', 'HU',
     'GROSS', 'FL', 'AIR', 'Touchstone 10.0'),
    ('EP-EQ-CA-2024', 'California Earthquake EP Curve 2024', '2024-01-15', 'EQ',
     'GROSS', 'CA', 'RMS', 'RiskLink 23.0')
ON CONFLICT (curve_id) DO NOTHING;

-- EP Curve points
INSERT INTO catastrophe.ep_curve_points (point_id, curve_id, return_period,
    exceedance_probability, oep_loss, aep_loss, aal)
VALUES
    ('EP-HU-FL-2024-10', 'EP-HU-FL-2024', 10, 0.1, 25000000.00, 45000000.00, 8500000.00),
    ('EP-HU-FL-2024-25', 'EP-HU-FL-2024', 25, 0.04, 75000000.00, 125000000.00, 8500000.00),
    ('EP-HU-FL-2024-50', 'EP-HU-FL-2024', 50, 0.02, 150000000.00, 225000000.00, 8500000.00),
    ('EP-HU-FL-2024-100', 'EP-HU-FL-2024', 100, 0.01, 275000000.00, 375000000.00, 8500000.00),
    ('EP-HU-FL-2024-250', 'EP-HU-FL-2024', 250, 0.004, 450000000.00, 550000000.00, 8500000.00),
    ('EP-HU-FL-2024-500', 'EP-HU-FL-2024', 500, 0.002, 600000000.00, 700000000.00, 8500000.00),
    ('EP-EQ-CA-2024-100', 'EP-EQ-CA-2024', 100, 0.01, 180000000.00, 250000000.00, 5500000.00),
    ('EP-EQ-CA-2024-250', 'EP-EQ-CA-2024', 250, 0.004, 350000000.00, 425000000.00, 5500000.00)
ON CONFLICT (point_id) DO NOTHING;

-- =============================================================================
-- ANALYTICAL QUERIES
-- =============================================================================

-- Exposure accumulation by zone
SELECT
    gz.zone_name,
    gz.state,
    gz.hurricane_zone,
    gz.earthquake_zone,
    COUNT(el.exposure_id) as location_count,
    SUM(el.total_insured_value) as total_tiv,
    AVG(el.total_insured_value) as avg_tiv,
    SUM(el.building_value) as building_exposure,
    SUM(el.bi_value) as bi_exposure
FROM catastrophe.geographic_zones gz
LEFT JOIN catastrophe.exposure_locations el ON gz.zone_id = el.zone_id
GROUP BY gz.zone_id, gz.zone_name, gz.state, gz.hurricane_zone, gz.earthquake_zone
ORDER BY total_tiv DESC NULLS LAST;

-- PML summary by return period
SELECT
    ep.peril_id,
    p.peril_name,
    ep.territory,
    ecp.return_period,
    ecp.oep_loss as occurrence_pml,
    ecp.aep_loss as aggregate_pml,
    ecp.aal as average_annual_loss,
    ROUND(ecp.oep_loss / NULLIF(
        (SELECT SUM(total_insured_value) FROM catastrophe.exposure_locations
         WHERE zone_id LIKE ep.territory || '%'), 0) * 100, 2) as pml_percent
FROM catastrophe.ep_curves ep
JOIN catastrophe.ep_curve_points ecp ON ep.curve_id = ecp.curve_id
JOIN catastrophe.perils p ON ep.peril_id = p.peril_id
WHERE ecp.return_period IN (100, 250, 500)
ORDER BY ep.territory, ecp.return_period;

-- Reinsurance program structure
SELECT
    treaty_name,
    treaty_type,
    attachment_point,
    limit_amount,
    attachment_point + limit_amount as exhaustion_point,
    ROUND(rate_on_line * 100, 2) as rate_on_line_percent,
    premium,
    reinsurer_name
FROM catastrophe.reinsurance_treaties
WHERE status = 'ACTIVE'
AND expiration_date > CURRENT_DATE
ORDER BY attachment_point;

-- Net loss after reinsurance (simplified calculation)
WITH gross_losses AS (
    SELECT
        me.scenario_name,
        me.return_period_years,
        SUM(el.total_insured_value * 0.15) as estimated_gross_loss -- Simplified 15% damage
    FROM catastrophe.modeled_events me
    CROSS JOIN catastrophe.exposure_locations el
    WHERE el.zone_id LIKE SUBSTRING(me.landfall_zone FROM 1 FOR 2) || '%'
    GROUP BY me.scenario_id, me.scenario_name, me.return_period_years
)
SELECT
    gl.scenario_name,
    gl.return_period_years,
    gl.estimated_gross_loss,
    COALESCE(SUM(
        CASE
            WHEN gl.estimated_gross_loss > rt.attachment_point
            THEN LEAST(gl.estimated_gross_loss - rt.attachment_point, rt.limit_amount)
            ELSE 0
        END
    ), 0) as reinsurance_recovery,
    gl.estimated_gross_loss - COALESCE(SUM(
        CASE
            WHEN gl.estimated_gross_loss > rt.attachment_point
            THEN LEAST(gl.estimated_gross_loss - rt.attachment_point, rt.limit_amount)
            ELSE 0
        END
    ), 0) as net_loss
FROM gross_losses gl
LEFT JOIN catastrophe.reinsurance_treaties rt ON rt.status = 'ACTIVE'
GROUP BY gl.scenario_name, gl.return_period_years, gl.estimated_gross_loss
ORDER BY gl.return_period_years;

-- Historical event comparison
SELECT
    event_name,
    peril_id,
    event_date,
    category,
    max_wind_mph,
    industry_insured_loss / 1000000000 as insured_loss_billions,
    ARRAY_TO_STRING(affected_states, ', ') as states_affected
FROM catastrophe.historical_events
ORDER BY industry_insured_loss DESC
LIMIT 10;

-- Concentration risk analysis (top 10 locations)
SELECT
    el.exposure_id,
    el.address,
    gz.zone_name,
    gz.hurricane_zone,
    gz.earthquake_zone,
    el.total_insured_value,
    el.construction_code,
    el.year_built,
    el.opening_protection
FROM catastrophe.exposure_locations el
JOIN catastrophe.geographic_zones gz ON el.zone_id = gz.zone_id
ORDER BY el.total_insured_value DESC
LIMIT 10;
