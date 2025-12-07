-- ============================================================================
-- OrbitRS Insurance Examples - Life, Property, Disaster, Earthquake, Industrial
-- ============================================================================
-- Combined schema for remaining insurance domains
-- ============================================================================
-- ============================================================================
-- LIFE INSURANCE
-- ============================================================================
CREATE TABLE life_policies (
    life_policy_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID NOT NULL UNIQUE REFERENCES policies(policy_id) ON DELETE CASCADE,
    policy_subtype VARCHAR(30) CHECK (
        policy_subtype IN (
            'TERM',
            'WHOLE_LIFE',
            'UNIVERSAL',
            'VARIABLE',
            'INDEXED_UNIVERSAL'
        )
    ),
    term_length_years INTEGER,
    -- For term life
    face_amount DECIMAL(12, 2) NOT NULL,
    -- Death benefit
    cash_value DECIMAL(12, 2) DEFAULT 0,
    -- Underwriting
    health_class VARCHAR(20) CHECK (
        health_class IN (
            'PREFERRED_PLUS',
            'PREFERRED',
            'STANDARD_PLUS',
            'STANDARD',
            'SUBSTANDARD'
        )
    ),
    tobacco_user BOOLEAN DEFAULT FALSE,
    -- Riders
    accelerated_death_benefit BOOLEAN DEFAULT FALSE,
    waiver_of_premium BOOLEAN DEFAULT FALSE,
    accidental_death_benefit BOOLEAN DEFAULT FALSE,
    child_rider BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE beneficiaries (
    beneficiary_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID NOT NULL REFERENCES policies(policy_id) ON DELETE CASCADE,
    beneficiary_type VARCHAR(20) CHECK (beneficiary_type IN ('PRIMARY', 'CONTINGENT')),
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    relationship VARCHAR(50),
    percentage DECIMAL(5, 2) CHECK (
        percentage BETWEEN 0 AND 100
    ),
    ssn_last4 VARCHAR(4),
    date_of_birth DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_life_policies_policy ON life_policies(policy_id);
CREATE INDEX idx_beneficiaries_policy ON beneficiaries(policy_id);
-- ============================================================================
-- COMMERCIAL PROPERTY INSURANCE
-- ============================================================================
CREATE TABLE commercial_properties (
    commercial_property_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    address_id UUID NOT NULL REFERENCES addresses(address_id),
    property_use VARCHAR(50) CHECK (
        property_use IN (
            'OFFICE',
            'RETAIL',
            'WAREHOUSE',
            'MANUFACTURING',
            'RESTAURANT',
            'HOTEL',
            'APARTMENT',
            'MIXED_USE'
        )
    ),
    building_value DECIMAL(12, 2),
    business_personal_property_value DECIMAL(12, 2),
    loss_of_income_value DECIMAL(12, 2),
    number_of_employees INTEGER,
    annual_revenue DECIMAL(15, 2),
    sprinkler_system BOOLEAN DEFAULT FALSE,
    fire_alarm_monitored BOOLEAN DEFAULT FALSE,
    security_guard BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_commercial_properties_customer ON commercial_properties(customer_id);
-- ============================================================================
-- DISASTER/CATASTROPHE INSURANCE
-- ============================================================================
CREATE TABLE disaster_policies (
    disaster_policy_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID NOT NULL UNIQUE REFERENCES policies(policy_id) ON DELETE CASCADE,
    property_id UUID REFERENCES properties(property_id),
    covered_perils TEXT [],
    -- Array: ['FLOOD', 'EARTHQUAKE', 'HURRICANE', 'WILDFIRE']
    aggregate_limit DECIMAL(12, 2),
    per_occurrence_limit DECIMAL(12, 2),
    waiting_period_days INTEGER DEFAULT 30,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE disaster_response_teams (
    team_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    disaster_id UUID REFERENCES natural_disasters(disaster_id),
    team_name VARCHAR(255),
    team_type VARCHAR(30) CHECK (
        team_type IN (
            'CLAIMS_ADJUSTERS',
            'EMERGENCY_RESPONSE',
            'CUSTOMER_SERVICE',
            'FIELD_INSPECTORS'
        )
    ),
    team_size INTEGER,
    deployment_date DATE,
    assigned_region VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_disaster_policies_policy ON disaster_policies(policy_id);
CREATE INDEX idx_response_teams_disaster ON disaster_response_teams(disaster_id);
-- ============================================================================
-- EARTHQUAKE INSURANCE
-- ============================================================================
CREATE TABLE seismic_zones (
    zone_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    zone_name VARCHAR(100),
    zone_code VARCHAR(20),
    risk_level VARCHAR(20) CHECK (
        risk_level IN ('LOW', 'MODERATE', 'HIGH', 'VERY_HIGH')
    ),
    -- Geographic bounds
    min_latitude DECIMAL(10, 8),
    max_latitude DECIMAL(10, 8),
    min_longitude DECIMAL(11, 8),
    max_longitude DECIMAL(11, 8),
    affected_zip_codes TEXT [],
    peak_ground_acceleration DECIMAL(5, 2),
    -- in g
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE structural_assessments (
    assessment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    property_id UUID NOT NULL REFERENCES properties(property_id),
    assessment_date DATE NOT NULL,
    seismic_rating VARCHAR(20) CHECK (
        seismic_rating IN (
            'EXCELLENT',
            'GOOD',
            'FAIR',
            'POOR',
            'UNSAFE'
        )
    ),
    foundation_bolted BOOLEAN DEFAULT FALSE,
    cripple_walls_braced BOOLEAN DEFAULT FALSE,
    soft_story_retrofit BOOLEAN DEFAULT FALSE,
    retrofit_cost_estimate DECIMAL(10, 2),
    retrofit_completed BOOLEAN DEFAULT FALSE,
    retrofit_completion_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_structural_assessments_property ON structural_assessments(property_id);
-- ============================================================================
-- INDUSTRIAL INSURANCE
-- ============================================================================
CREATE TABLE industrial_facilities (
    facility_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    address_id UUID NOT NULL REFERENCES addresses(address_id),
    facility_type VARCHAR(50) CHECK (
        facility_type IN (
            'MANUFACTURING',
            'PROCESSING',
            'ASSEMBLY',
            'DISTRIBUTION',
            'POWER_PLANT'
        )
    ),
    facility_size_sqft INTEGER,
    number_of_employees INTEGER,
    shifts_per_day INTEGER,
    operates_24_7 BOOLEAN DEFAULT FALSE,
    hazardous_materials BOOLEAN DEFAULT FALSE,
    hazmat_types TEXT [],
    annual_payroll DECIMAL(15, 2),
    annual_revenue DECIMAL(15, 2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE equipment_inventory (
    equipment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    facility_id UUID NOT NULL REFERENCES industrial_facilities(facility_id),
    equipment_type VARCHAR(100),
    manufacturer VARCHAR(100),
    model VARCHAR(100),
    serial_number VARCHAR(100),
    purchase_date DATE,
    purchase_price DECIMAL(12, 2),
    current_value DECIMAL(12, 2),
    replacement_cost DECIMAL(12, 2),
    maintenance_schedule VARCHAR(50),
    last_maintenance_date DATE,
    next_maintenance_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE workers_compensation_policies (
    wc_policy_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID NOT NULL UNIQUE REFERENCES policies(policy_id) ON DELETE CASCADE,
    facility_id UUID NOT NULL REFERENCES industrial_facilities(facility_id),
    payroll_amount DECIMAL(15, 2),
    employee_count INTEGER,
    -- Class codes and rates
    class_codes JSONB,
    -- Array of {code, description, payroll, rate}
    experience_mod DECIMAL(5, 4) DEFAULT 1.0000,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE safety_incidents (
    incident_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    facility_id UUID NOT NULL REFERENCES industrial_facilities(facility_id),
    incident_date TIMESTAMP NOT NULL,
    incident_type VARCHAR(50) CHECK (
        incident_type IN (
            'INJURY',
            'ILLNESS',
            'NEAR_MISS',
            'PROPERTY_DAMAGE',
            'ENVIRONMENTAL'
        )
    ),
    severity VARCHAR(20) CHECK (
        severity IN ('MINOR', 'MODERATE', 'SERIOUS', 'FATAL')
    ),
    employee_name VARCHAR(200),
    body_part_affected VARCHAR(100),
    days_away_from_work INTEGER DEFAULT 0,
    osha_recordable BOOLEAN DEFAULT FALSE,
    osha_report_filed BOOLEAN DEFAULT FALSE,
    root_cause TEXT,
    corrective_actions TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_industrial_facilities_customer ON industrial_facilities(customer_id);
CREATE INDEX idx_equipment_facility ON equipment_inventory(facility_id);
CREATE INDEX idx_wc_policies_policy ON workers_compensation_policies(policy_id);
CREATE INDEX idx_safety_incidents_facility ON safety_incidents(facility_id);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_life_policies_updated_at BEFORE
UPDATE ON life_policies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_commercial_properties_updated_at BEFORE
UPDATE ON commercial_properties FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_industrial_facilities_updated_at BEFORE
UPDATE ON industrial_facilities FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE life_policies IS 'Life insurance policy details and riders';
COMMENT ON TABLE beneficiaries IS 'Life insurance policy beneficiaries';
COMMENT ON TABLE commercial_properties IS 'Commercial property information';
COMMENT ON TABLE disaster_policies IS 'Catastrophe insurance coverage';
COMMENT ON TABLE seismic_zones IS 'Earthquake risk zones';
COMMENT ON TABLE structural_assessments IS 'Building seismic assessments';
COMMENT ON TABLE industrial_facilities IS 'Industrial/manufacturing facilities';
COMMENT ON TABLE equipment_inventory IS 'Insured equipment tracking';
COMMENT ON TABLE workers_compensation_policies IS 'Workers compensation coverage';
COMMENT ON TABLE safety_incidents IS 'Workplace safety incident tracking';