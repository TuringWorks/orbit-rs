-- ============================================================================
-- OrbitRS Insurance Examples - Home Insurance Schema
-- ============================================================================
-- Home insurance specific tables: properties, home policies, natural disasters
-- ============================================================================
-- ============================================================================
-- PROPERTIES
-- ============================================================================
CREATE TABLE properties (
    property_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    address_id UUID NOT NULL REFERENCES addresses(address_id),
    -- Property type
    property_type VARCHAR(30) CHECK (
        property_type IN (
            'SINGLE_FAMILY',
            'CONDO',
            'TOWNHOUSE',
            'MULTI_FAMILY',
            'MOBILE_HOME',
            'MANUFACTURED',
            'VACATION_HOME'
        )
    ),
    -- Construction
    year_built INTEGER CHECK (
        year_built BETWEEN 1700 AND 2100
    ),
    square_footage INTEGER,
    number_of_stories DECIMAL(3, 1),
    number_of_bedrooms INTEGER,
    number_of_bathrooms DECIMAL(3, 1),
    construction_type VARCHAR(30) CHECK (
        construction_type IN (
            'FRAME',
            'MASONRY',
            'BRICK',
            'STONE',
            'CONCRETE',
            'LOG',
            'STEEL'
        )
    ),
    roof_type VARCHAR(30) CHECK (
        roof_type IN (
            'ASPHALT_SHINGLE',
            'METAL',
            'TILE',
            'SLATE',
            'WOOD_SHAKE',
            'FLAT'
        )
    ),
    roof_age INTEGER,
    foundation_type VARCHAR(30) CHECK (
        foundation_type IN (
            'SLAB',
            'CRAWL_SPACE',
            'BASEMENT',
            'PIER_BEAM'
        )
    ),
    -- Systems
    heating_type VARCHAR(30),
    cooling_type VARCHAR(30),
    electrical_updated BOOLEAN DEFAULT FALSE,
    plumbing_updated BOOLEAN DEFAULT FALSE,
    -- Features
    has_basement BOOLEAN DEFAULT FALSE,
    basement_finished BOOLEAN DEFAULT FALSE,
    has_garage BOOLEAN DEFAULT FALSE,
    garage_spaces INTEGER,
    has_pool BOOLEAN DEFAULT FALSE,
    pool_type VARCHAR(20) CHECK (pool_type IN ('INGROUND', 'ABOVEGROUND', 'NONE')),
    has_fireplace BOOLEAN DEFAULT FALSE,
    fireplace_count INTEGER DEFAULT 0,
    -- Safety features
    smoke_detectors BOOLEAN DEFAULT FALSE,
    carbon_monoxide_detectors BOOLEAN DEFAULT FALSE,
    fire_extinguishers BOOLEAN DEFAULT FALSE,
    sprinkler_system BOOLEAN DEFAULT FALSE,
    security_system BOOLEAN DEFAULT FALSE,
    security_system_monitored BOOLEAN DEFAULT FALSE,
    -- Occupancy
    occupancy_type VARCHAR(30) CHECK (
        occupancy_type IN (
            'OWNER_OCCUPIED',
            'TENANT_OCCUPIED',
            'VACANT',
            'SEASONAL'
        )
    ),
    primary_residence BOOLEAN DEFAULT TRUE,
    -- Value
    purchase_price DECIMAL(12, 2),
    purchase_date DATE,
    current_market_value DECIMAL(12, 2),
    assessed_value DECIMAL(12, 2),
    replacement_cost DECIMAL(12, 2),
    -- Risk factors
    distance_to_fire_station DECIMAL(5, 2),
    -- in miles
    distance_to_fire_hydrant DECIMAL(5, 2),
    -- in miles
    fire_protection_class INTEGER CHECK (
        fire_protection_class BETWEEN 1 AND 10
    ),
    flood_zone VARCHAR(10),
    in_wildfire_zone BOOLEAN DEFAULT FALSE,
    in_earthquake_zone BOOLEAN DEFAULT FALSE,
    in_hurricane_zone BOOLEAN DEFAULT FALSE,
    in_tornado_zone BOOLEAN DEFAULT FALSE,
    -- HOA
    hoa_managed BOOLEAN DEFAULT FALSE,
    hoa_name VARCHAR(255),
    hoa_fee_monthly DECIMAL(10, 2),
    -- Status
    property_status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        property_status IN (
            'ACTIVE',
            'SOLD',
            'DEMOLISHED',
            'CONDEMNED'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_properties_customer ON properties(customer_id);
CREATE INDEX idx_properties_address ON properties(address_id);
CREATE INDEX idx_properties_type ON properties(property_type);
CREATE INDEX idx_properties_status ON properties(property_status);
-- ============================================================================
-- HOME POLICIES
-- ============================================================================
CREATE TABLE home_policies (
    home_policy_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID NOT NULL UNIQUE REFERENCES policies(policy_id) ON DELETE CASCADE,
    property_id UUID NOT NULL REFERENCES properties(property_id),
    -- Coverage A - Dwelling
    dwelling_coverage DECIMAL(12, 2) NOT NULL,
    dwelling_deductible DECIMAL(10, 2) NOT NULL,
    replacement_cost_coverage BOOLEAN DEFAULT TRUE,
    -- Coverage B - Other Structures
    other_structures_coverage DECIMAL(12, 2),
    other_structures_percentage DECIMAL(5, 2) DEFAULT 10.00,
    -- % of dwelling
    -- Coverage C - Personal Property
    personal_property_coverage DECIMAL(12, 2),
    personal_property_percentage DECIMAL(5, 2) DEFAULT 50.00,
    -- % of dwelling
    personal_property_replacement_cost BOOLEAN DEFAULT FALSE,
    -- Coverage D - Loss of Use
    loss_of_use_coverage DECIMAL(12, 2),
    loss_of_use_percentage DECIMAL(5, 2) DEFAULT 20.00,
    -- % of dwelling
    -- Coverage E - Personal Liability
    personal_liability_coverage DECIMAL(12, 2) NOT NULL DEFAULT 100000,
    -- Coverage F - Medical Payments
    medical_payments_coverage DECIMAL(10, 2) DEFAULT 1000,
    -- Additional coverages
    water_backup_coverage BOOLEAN DEFAULT FALSE,
    water_backup_limit DECIMAL(10, 2),
    equipment_breakdown_coverage BOOLEAN DEFAULT FALSE,
    equipment_breakdown_limit DECIMAL(10, 2),
    identity_theft_coverage BOOLEAN DEFAULT FALSE,
    identity_theft_limit DECIMAL(10, 2),
    ordinance_law_coverage BOOLEAN DEFAULT FALSE,
    ordinance_law_limit DECIMAL(10, 2),
    -- Scheduled personal property (jewelry, art, etc.)
    scheduled_property_coverage BOOLEAN DEFAULT FALSE,
    scheduled_property_limit DECIMAL(12, 2),
    -- Flood insurance (separate policy often)
    flood_coverage BOOLEAN DEFAULT FALSE,
    flood_limit DECIMAL(12, 2),
    flood_deductible DECIMAL(10, 2),
    -- Earthquake coverage
    earthquake_coverage BOOLEAN DEFAULT FALSE,
    earthquake_limit DECIMAL(12, 2),
    earthquake_deductible_percentage DECIMAL(5, 2),
    -- Often % of dwelling
    -- Discounts
    multi_policy_discount BOOLEAN DEFAULT FALSE,
    security_system_discount BOOLEAN DEFAULT FALSE,
    fire_resistant_discount BOOLEAN DEFAULT FALSE,
    new_home_discount BOOLEAN DEFAULT FALSE,
    claims_free_discount BOOLEAN DEFAULT FALSE,
    senior_discount BOOLEAN DEFAULT FALSE,
    non_smoker_discount BOOLEAN DEFAULT FALSE,
    -- Mortgagee information
    mortgagee_name VARCHAR(255),
    mortgagee_loan_number VARCHAR(100),
    mortgagee_address TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_home_policies_policy ON home_policies(policy_id);
CREATE INDEX idx_home_policies_property ON home_policies(property_id);
-- ============================================================================
-- HOME CLAIMS
-- ============================================================================
CREATE TABLE home_claims (
    home_claim_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    claim_id UUID NOT NULL UNIQUE REFERENCES claims(claim_id) ON DELETE CASCADE,
    property_id UUID NOT NULL REFERENCES properties(property_id),
    -- Claim category
    claim_category VARCHAR(30) CHECK (
        claim_category IN (
            'FIRE',
            'WATER_DAMAGE',
            'WIND',
            'HAIL',
            'LIGHTNING',
            'THEFT',
            'VANDALISM',
            'LIABILITY',
            'MEDICAL_PAYMENTS',
            'OTHER'
        )
    ),
    -- Damage details
    damage_area VARCHAR(50),
    -- Kitchen, Bathroom, Roof, etc.
    damage_severity VARCHAR(20) CHECK (
        damage_severity IN (
            'MINOR',
            'MODERATE',
            'SEVERE',
            'TOTAL_LOSS'
        )
    ),
    -- Property habitability
    property_habitable BOOLEAN DEFAULT TRUE,
    temporary_housing_needed BOOLEAN DEFAULT FALSE,
    temporary_housing_start_date DATE,
    temporary_housing_end_date DATE,
    temporary_housing_daily_cost DECIMAL(10, 2),
    -- Emergency services
    emergency_services_required BOOLEAN DEFAULT FALSE,
    emergency_services_cost DECIMAL(10, 2),
    -- Contractor information
    contractor_name VARCHAR(255),
    contractor_phone VARCHAR(20),
    contractor_license VARCHAR(50),
    -- Estimates
    initial_estimate DECIMAL(12, 2),
    final_estimate DECIMAL(12, 2),
    actual_repair_cost DECIMAL(12, 2),
    -- Contents claim
    contents_damaged BOOLEAN DEFAULT FALSE,
    contents_claim_amount DECIMAL(12, 2),
    -- Liability claim
    liability_claim BOOLEAN DEFAULT FALSE,
    injured_party_name VARCHAR(200),
    injured_party_contact TEXT,
    medical_treatment_required BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_home_claims_claim ON home_claims(claim_id);
CREATE INDEX idx_home_claims_property ON home_claims(property_id);
CREATE INDEX idx_home_claims_category ON home_claims(claim_category);
-- ============================================================================
-- PROPERTY INSPECTIONS
-- ============================================================================
CREATE TABLE property_inspections (
    inspection_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    property_id UUID NOT NULL REFERENCES properties(property_id),
    policy_id UUID REFERENCES policies(policy_id),
    inspection_type VARCHAR(30) CHECK (
        inspection_type IN (
            'INITIAL',
            'RENEWAL',
            'CLAIM',
            'RISK_ASSESSMENT',
            'REINSPECTION'
        )
    ),
    inspection_date DATE NOT NULL,
    inspector_name VARCHAR(200),
    inspector_company VARCHAR(255),
    inspector_license VARCHAR(50),
    -- Findings
    overall_condition VARCHAR(20) CHECK (
        overall_condition IN (
            'EXCELLENT',
            'GOOD',
            'FAIR',
            'POOR',
            'UNACCEPTABLE'
        )
    ),
    roof_condition VARCHAR(20),
    electrical_condition VARCHAR(20),
    plumbing_condition VARCHAR(20),
    hvac_condition VARCHAR(20),
    structural_condition VARCHAR(20),
    -- Issues found
    issues_found BOOLEAN DEFAULT FALSE,
    issues_description TEXT,
    repairs_required BOOLEAN DEFAULT FALSE,
    repairs_description TEXT,
    repairs_deadline DATE,
    -- Photos and reports (stored in MongoDB)
    photo_count INTEGER DEFAULT 0,
    report_document_id UUID REFERENCES documents(document_id),
    -- Follow-up
    follow_up_required BOOLEAN DEFAULT FALSE,
    follow_up_date DATE,
    follow_up_completed BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_inspections_property ON property_inspections(property_id);
CREATE INDEX idx_inspections_policy ON property_inspections(policy_id);
CREATE INDEX idx_inspections_date ON property_inspections(inspection_date);
-- ============================================================================
-- NATURAL DISASTERS
-- ============================================================================
CREATE TABLE natural_disasters (
    disaster_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    disaster_type VARCHAR(30) CHECK (
        disaster_type IN (
            'HURRICANE',
            'TORNADO',
            'FLOOD',
            'WILDFIRE',
            'EARTHQUAKE',
            'HAIL_STORM',
            'WINTER_STORM',
            'DROUGHT',
            'TSUNAMI'
        )
    ),
    disaster_name VARCHAR(255),
    -- e.g., "Hurricane Katrina"
    -- Location
    affected_states TEXT [],
    affected_counties TEXT [],
    affected_zip_codes TEXT [],
    -- Geographic bounds
    min_latitude DECIMAL(10, 8),
    max_latitude DECIMAL(10, 8),
    min_longitude DECIMAL(11, 8),
    max_longitude DECIMAL(11, 8),
    -- Timing
    start_date TIMESTAMP NOT NULL,
    end_date TIMESTAMP,
    peak_date TIMESTAMP,
    -- Severity
    severity_level VARCHAR(20) CHECK (
        severity_level IN (
            'MINOR',
            'MODERATE',
            'MAJOR',
            'CATASTROPHIC'
        )
    ),
    category_rating VARCHAR(10),
    -- For hurricanes, tornadoes
    -- Impact
    estimated_damage_total DECIMAL(15, 2),
    estimated_insured_losses DECIMAL(15, 2),
    properties_affected INTEGER,
    casualties INTEGER,
    -- FEMA
    fema_disaster_number VARCHAR(50),
    fema_declaration_date DATE,
    -- Insurance response
    catastrophe_number VARCHAR(50),
    -- Industry CAT number
    claims_expected INTEGER,
    claims_received INTEGER,
    -- Status
    disaster_status VARCHAR(20) CHECK (
        disaster_status IN (
            'ACTIVE',
            'CONTAINED',
            'RESOLVED',
            'MONITORING'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_disasters_type ON natural_disasters(disaster_type);
CREATE INDEX idx_disasters_start_date ON natural_disasters(start_date);
CREATE INDEX idx_disasters_status ON natural_disasters(disaster_status);
-- ============================================================================
-- PROPERTY VALUATIONS
-- ============================================================================
CREATE TABLE property_valuations (
    valuation_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    property_id UUID NOT NULL REFERENCES properties(property_id),
    valuation_type VARCHAR(30) CHECK (
        valuation_type IN (
            'MARKET',
            'REPLACEMENT_COST',
            'ASSESSED',
            'APPRAISAL'
        )
    ),
    valuation_date DATE NOT NULL,
    valuation_amount DECIMAL(12, 2) NOT NULL,
    -- Valuation source
    valuator_name VARCHAR(255),
    valuator_company VARCHAR(255),
    valuator_license VARCHAR(50),
    -- Method
    valuation_method VARCHAR(50),
    -- Comparative Market Analysis, Cost Approach, etc.
    comparable_properties_used INTEGER,
    -- Details
    land_value DECIMAL(12, 2),
    improvement_value DECIMAL(12, 2),
    depreciation_amount DECIMAL(12, 2),
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_valuations_property ON property_valuations(property_id);
CREATE INDEX idx_valuations_date ON property_valuations(valuation_date);
CREATE INDEX idx_valuations_type ON property_valuations(valuation_type);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_properties_updated_at BEFORE
UPDATE ON properties FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_home_policies_updated_at BEFORE
UPDATE ON home_policies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_home_claims_updated_at BEFORE
UPDATE ON home_claims FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_disasters_updated_at BEFORE
UPDATE ON natural_disasters FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Complete home policy details
CREATE VIEW v_home_policy_details AS
SELECT p.policy_id,
    p.policy_number,
    p.policy_status,
    p.effective_date,
    p.expiration_date,
    p.premium_amount,
    c.customer_id,
    c.first_name,
    c.last_name,
    c.email,
    pr.property_id,
    pr.property_type,
    a.street_address_1,
    a.city,
    a.state,
    a.zip_code,
    hp.dwelling_coverage,
    hp.personal_liability_coverage,
    hp.flood_coverage,
    hp.earthquake_coverage
FROM policies p
    JOIN customers c ON p.customer_id = c.customer_id
    JOIN home_policies hp ON p.policy_id = hp.policy_id
    JOIN properties pr ON hp.property_id = pr.property_id
    JOIN addresses a ON pr.address_id = a.address_id
WHERE p.policy_type = 'HOME';
-- Properties at risk
CREATE VIEW v_properties_at_risk AS
SELECT pr.property_id,
    pr.property_type,
    a.street_address_1,
    a.city,
    a.state,
    a.zip_code,
    pr.flood_zone,
    pr.in_wildfire_zone,
    pr.in_earthquake_zone,
    pr.in_hurricane_zone,
    pr.fire_protection_class,
    pr.year_built,
    pr.roof_age,
    COUNT(hc.home_claim_id) AS claim_count,
    SUM(cl.claim_amount) AS total_claimed
FROM properties pr
    JOIN addresses a ON pr.address_id = a.address_id
    LEFT JOIN home_claims hc ON pr.property_id = hc.property_id
    LEFT JOIN claims cl ON hc.claim_id = cl.claim_id
WHERE pr.in_wildfire_zone = TRUE
    OR pr.in_earthquake_zone = TRUE
    OR pr.in_hurricane_zone = TRUE
    OR pr.flood_zone IN ('A', 'AE', 'V', 'VE')
    OR pr.fire_protection_class > 5
GROUP BY pr.property_id,
    pr.property_type,
    a.street_address_1,
    a.city,
    a.state,
    a.zip_code,
    pr.flood_zone,
    pr.in_wildfire_zone,
    pr.in_earthquake_zone,
    pr.in_hurricane_zone,
    pr.fire_protection_class,
    pr.year_built,
    pr.roof_age;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE properties IS 'Property master data with construction details and risk factors';
COMMENT ON TABLE home_policies IS 'Home insurance policy coverage details';
COMMENT ON TABLE home_claims IS 'Home-specific claim details';
COMMENT ON TABLE property_inspections IS 'Property inspection reports and findings';
COMMENT ON TABLE natural_disasters IS 'Natural disaster events and impact tracking';
COMMENT ON TABLE property_valuations IS 'Property valuation history';