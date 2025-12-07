-- ============================================================================
-- OrbitRS Insurance Examples - Auto Insurance Schema
-- ============================================================================
-- Auto insurance specific tables: vehicles, drivers, auto policies, accidents
-- ============================================================================
-- ============================================================================
-- VEHICLES
-- ============================================================================
CREATE TABLE vehicles (
    vehicle_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    vin VARCHAR(17) UNIQUE NOT NULL CHECK (LENGTH(vin) = 17),
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    -- Vehicle details
    year INTEGER NOT NULL CHECK (
        year BETWEEN 1900 AND 2100
    ),
    make VARCHAR(50) NOT NULL,
    model VARCHAR(100) NOT NULL,
    trim VARCHAR(50),
    body_style VARCHAR(30) CHECK (
        body_style IN (
            'SEDAN',
            'COUPE',
            'SUV',
            'TRUCK',
            'VAN',
            'WAGON',
            'CONVERTIBLE',
            'HATCHBACK'
        )
    ),
    -- Technical specs
    engine_size DECIMAL(3, 1),
    -- in liters
    cylinders INTEGER,
    fuel_type VARCHAR(20) CHECK (
        fuel_type IN (
            'GASOLINE',
            'DIESEL',
            'ELECTRIC',
            'HYBRID',
            'PLUG_IN_HYBRID'
        )
    ),
    transmission VARCHAR(20) CHECK (transmission IN ('AUTOMATIC', 'MANUAL', 'CVT')),
    -- Value
    purchase_price DECIMAL(10, 2),
    current_value DECIMAL(10, 2),
    msrp DECIMAL(10, 2),
    -- Usage
    primary_use VARCHAR(30) CHECK (
        primary_use IN (
            'PERSONAL',
            'BUSINESS',
            'COMMERCIAL',
            'RIDESHARE',
            'DELIVERY'
        )
    ),
    annual_mileage INTEGER,
    current_odometer INTEGER,
    -- Safety features
    safety_features TEXT [],
    -- Array: ['ABS', 'AIRBAGS', 'BACKUP_CAMERA', 'BLIND_SPOT', 'LANE_ASSIST']
    anti_theft_devices TEXT [],
    -- Array: ['ALARM', 'GPS_TRACKER', 'IMMOBILIZER', 'VIN_ETCHING']
    -- Registration
    license_plate VARCHAR(20),
    registration_state VARCHAR(2),
    registration_expiration DATE,
    -- Ownership
    ownership_type VARCHAR(20) CHECK (
        ownership_type IN ('OWNED', 'LEASED', 'FINANCED')
    ),
    lienholder_name VARCHAR(255),
    lienholder_address TEXT,
    -- Garaging
    garaging_address_id UUID REFERENCES addresses(address_id),
    garage_type VARCHAR(20) CHECK (
        garage_type IN ('GARAGE', 'CARPORT', 'DRIVEWAY', 'STREET')
    ),
    -- Status
    vehicle_status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        vehicle_status IN (
            'ACTIVE',
            'SOLD',
            'TOTALED',
            'STOLEN',
            'INACTIVE'
        )
    ),
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_vehicles_vin ON vehicles(vin);
CREATE INDEX idx_vehicles_customer ON vehicles(customer_id);
CREATE INDEX idx_vehicles_status ON vehicles(vehicle_status);
CREATE INDEX idx_vehicles_make_model ON vehicles(make, model);
-- ============================================================================
-- DRIVERS
-- ============================================================================
CREATE TABLE drivers (
    driver_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    -- Personal info
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE NOT NULL,
    gender VARCHAR(1) CHECK (gender IN ('M', 'F', 'X')),
    marital_status VARCHAR(20) CHECK (
        marital_status IN (
            'SINGLE',
            'MARRIED',
            'DIVORCED',
            'WIDOWED',
            'DOMESTIC_PARTNER'
        )
    ),
    -- License info
    license_number VARCHAR(50) NOT NULL,
    license_state VARCHAR(2) NOT NULL,
    license_status VARCHAR(20) DEFAULT 'VALID' CHECK (
        license_status IN (
            'VALID',
            'SUSPENDED',
            'REVOKED',
            'EXPIRED',
            'LEARNERS_PERMIT'
        )
    ),
    license_issue_date DATE,
    license_expiration_date DATE,
    license_class VARCHAR(10),
    -- Class A, B, C, etc.
    -- Driving history
    years_licensed INTEGER,
    age_first_licensed INTEGER,
    -- Relationship to policyholder
    relationship_to_policyholder VARCHAR(30) CHECK (
        relationship_to_policyholder IN (
            'SELF',
            'SPOUSE',
            'CHILD',
            'PARENT',
            'SIBLING',
            'OTHER'
        )
    ),
    -- Driver classification
    driver_type VARCHAR(30) CHECK (
        driver_type IN (
            'PRIMARY',
            'SECONDARY',
            'OCCASIONAL',
            'EXCLUDED'
        )
    ),
    -- Risk factors
    driver_training_completed BOOLEAN DEFAULT FALSE,
    good_student BOOLEAN DEFAULT FALSE,
    -- For young drivers
    mature_driver_course BOOLEAN DEFAULT FALSE,
    -- For senior drivers
    -- Status
    driver_status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        driver_status IN (
            'ACTIVE',
            'EXCLUDED',
            'REMOVED',
            'DECEASED'
        )
    ),
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(customer_id, license_number, license_state)
);
CREATE INDEX idx_drivers_customer ON drivers(customer_id);
CREATE INDEX idx_drivers_license ON drivers(license_number, license_state);
CREATE INDEX idx_drivers_status ON drivers(driver_status);
-- ============================================================================
-- DRIVING VIOLATIONS
-- ============================================================================
CREATE TABLE driving_violations (
    violation_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    driver_id UUID NOT NULL REFERENCES drivers(driver_id),
    violation_type VARCHAR(50) NOT NULL CHECK (
        violation_type IN (
            'SPEEDING',
            'DUI',
            'DWI',
            'RECKLESS_DRIVING',
            'RUNNING_RED_LIGHT',
            'RUNNING_STOP_SIGN',
            'IMPROPER_LANE_CHANGE',
            'FOLLOWING_TOO_CLOSE',
            'FAILURE_TO_YIELD',
            'CELL_PHONE_USE',
            'SEAT_BELT',
            'OTHER'
        )
    ),
    violation_date DATE NOT NULL,
    violation_description TEXT,
    -- Location
    violation_state VARCHAR(2),
    violation_city VARCHAR(100),
    -- Severity
    points_assessed INTEGER DEFAULT 0,
    fine_amount DECIMAL(10, 2),
    -- Status
    conviction_status VARCHAR(20) CHECK (
        conviction_status IN (
            'PENDING',
            'CONVICTED',
            'DISMISSED',
            'REDUCED',
            'EXPUNGED'
        )
    ),
    conviction_date DATE,
    -- Impact
    affects_insurance BOOLEAN DEFAULT TRUE,
    removed_from_record_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_violations_driver ON driving_violations(driver_id);
CREATE INDEX idx_violations_date ON driving_violations(violation_date);
CREATE INDEX idx_violations_type ON driving_violations(violation_type);
-- ============================================================================
-- AUTO POLICIES
-- ============================================================================
CREATE TABLE auto_policies (
    auto_policy_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID NOT NULL UNIQUE REFERENCES policies(policy_id) ON DELETE CASCADE,
    -- Coverage details
    liability_bodily_injury_per_person DECIMAL(10, 2) NOT NULL,
    liability_bodily_injury_per_accident DECIMAL(10, 2) NOT NULL,
    liability_property_damage DECIMAL(10, 2) NOT NULL,
    -- Optional coverages
    collision_coverage BOOLEAN DEFAULT FALSE,
    collision_deductible DECIMAL(10, 2),
    comprehensive_coverage BOOLEAN DEFAULT FALSE,
    comprehensive_deductible DECIMAL(10, 2),
    uninsured_motorist_coverage BOOLEAN DEFAULT FALSE,
    uninsured_motorist_limit DECIMAL(10, 2),
    underinsured_motorist_coverage BOOLEAN DEFAULT FALSE,
    underinsured_motorist_limit DECIMAL(10, 2),
    medical_payments_coverage BOOLEAN DEFAULT FALSE,
    medical_payments_limit DECIMAL(10, 2),
    personal_injury_protection BOOLEAN DEFAULT FALSE,
    pip_limit DECIMAL(10, 2),
    -- Additional coverages
    rental_reimbursement BOOLEAN DEFAULT FALSE,
    rental_daily_limit DECIMAL(10, 2),
    rental_max_days INTEGER,
    roadside_assistance BOOLEAN DEFAULT FALSE,
    towing_labor_limit DECIMAL(10, 2),
    -- Discounts applied
    multi_car_discount BOOLEAN DEFAULT FALSE,
    multi_policy_discount BOOLEAN DEFAULT FALSE,
    good_driver_discount BOOLEAN DEFAULT FALSE,
    defensive_driver_discount BOOLEAN DEFAULT FALSE,
    low_mileage_discount BOOLEAN DEFAULT FALSE,
    anti_theft_discount BOOLEAN DEFAULT FALSE,
    safety_features_discount BOOLEAN DEFAULT FALSE,
    paperless_discount BOOLEAN DEFAULT FALSE,
    pay_in_full_discount BOOLEAN DEFAULT FALSE,
    -- Telematics
    telematics_enrolled BOOLEAN DEFAULT FALSE,
    telematics_device_id VARCHAR(100),
    telematics_discount_percentage DECIMAL(5, 2),
    -- Policy specifics
    state_minimum_coverage BOOLEAN DEFAULT FALSE,
    state_filed_rate BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_auto_policies_policy ON auto_policies(policy_id);
-- ============================================================================
-- POLICY VEHICLES (Junction table)
-- ============================================================================
CREATE TABLE policy_vehicles (
    policy_vehicle_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID NOT NULL REFERENCES policies(policy_id) ON DELETE CASCADE,
    vehicle_id UUID NOT NULL REFERENCES vehicles(vehicle_id),
    -- Coverage for this specific vehicle
    coverage_type VARCHAR(30) CHECK (
        coverage_type IN (
            'FULL',
            'LIABILITY_ONLY',
            'COMPREHENSIVE_ONLY',
            'EXCLUDED'
        )
    ),
    -- Vehicle-specific premium
    vehicle_premium DECIMAL(10, 2),
    -- Primary driver for this vehicle
    primary_driver_id UUID REFERENCES drivers(driver_id),
    -- Dates
    added_to_policy_date DATE DEFAULT CURRENT_DATE,
    removed_from_policy_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(policy_id, vehicle_id)
);
CREATE INDEX idx_policy_vehicles_policy ON policy_vehicles(policy_id);
CREATE INDEX idx_policy_vehicles_vehicle ON policy_vehicles(vehicle_id);
-- ============================================================================
-- POLICY DRIVERS (Junction table)
-- ============================================================================
CREATE TABLE policy_drivers (
    policy_driver_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    policy_id UUID NOT NULL REFERENCES policies(policy_id) ON DELETE CASCADE,
    driver_id UUID NOT NULL REFERENCES drivers(driver_id),
    -- Driver status on policy
    driver_status VARCHAR(20) CHECK (
        driver_status IN ('LISTED', 'EXCLUDED', 'REMOVED')
    ),
    -- Dates
    added_to_policy_date DATE DEFAULT CURRENT_DATE,
    removed_from_policy_date DATE,
    exclusion_reason TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(policy_id, driver_id)
);
CREATE INDEX idx_policy_drivers_policy ON policy_drivers(policy_id);
CREATE INDEX idx_policy_drivers_driver ON policy_drivers(driver_id);
-- ============================================================================
-- AUTO CLAIMS
-- ============================================================================
CREATE TABLE auto_claims (
    auto_claim_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    claim_id UUID NOT NULL UNIQUE REFERENCES claims(claim_id) ON DELETE CASCADE,
    vehicle_id UUID NOT NULL REFERENCES vehicles(vehicle_id),
    driver_id UUID REFERENCES drivers(driver_id),
    -- Accident details
    accident_type VARCHAR(30) CHECK (
        accident_type IN (
            'COLLISION',
            'COMPREHENSIVE',
            'THEFT',
            'VANDALISM',
            'WEATHER',
            'ANIMAL',
            'FIRE',
            'GLASS',
            'HIT_AND_RUN',
            'PARKING_LOT'
        )
    ),
    at_fault BOOLEAN,
    fault_percentage INTEGER CHECK (
        fault_percentage BETWEEN 0 AND 100
    ),
    -- Other parties
    other_parties_involved INTEGER DEFAULT 0,
    police_notified BOOLEAN DEFAULT FALSE,
    injuries_reported BOOLEAN DEFAULT FALSE,
    fatalities BOOLEAN DEFAULT FALSE,
    -- Vehicle damage
    vehicle_drivable BOOLEAN,
    total_loss BOOLEAN DEFAULT FALSE,
    repair_facility_name VARCHAR(255),
    repair_facility_phone VARCHAR(20),
    -- Rental car
    rental_car_needed BOOLEAN DEFAULT FALSE,
    rental_start_date DATE,
    rental_end_date DATE,
    rental_daily_cost DECIMAL(10, 2),
    -- Towing
    towing_required BOOLEAN DEFAULT FALSE,
    towing_company VARCHAR(255),
    towing_cost DECIMAL(10, 2),
    -- Subrogation
    other_party_insurer VARCHAR(255),
    other_party_policy_number VARCHAR(50),
    other_party_claim_number VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_auto_claims_claim ON auto_claims(claim_id);
CREATE INDEX idx_auto_claims_vehicle ON auto_claims(vehicle_id);
CREATE INDEX idx_auto_claims_driver ON auto_claims(driver_id);
CREATE INDEX idx_auto_claims_type ON auto_claims(accident_type);
-- ============================================================================
-- ACCIDENTS (Third-party incidents)
-- ============================================================================
CREATE TABLE accidents (
    accident_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    claim_id UUID REFERENCES claims(claim_id),
    -- Other party information
    other_driver_name VARCHAR(200),
    other_driver_phone VARCHAR(20),
    other_driver_license VARCHAR(50),
    other_driver_license_state VARCHAR(2),
    other_vehicle_vin VARCHAR(17),
    other_vehicle_year INTEGER,
    other_vehicle_make VARCHAR(50),
    other_vehicle_model VARCHAR(100),
    other_vehicle_plate VARCHAR(20),
    other_vehicle_plate_state VARCHAR(2),
    other_insurance_company VARCHAR(255),
    other_policy_number VARCHAR(50),
    other_insurance_phone VARCHAR(20),
    -- Witnesses
    witnesses JSONB,
    -- Array of witness objects with name, phone, statement
    -- Damage description
    damage_description TEXT,
    damage_photos_count INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_accidents_claim ON accidents(claim_id);
-- ============================================================================
-- TELEMATICS DATA (Summary table - detailed data in Cassandra)
-- ============================================================================
CREATE TABLE telematics_devices (
    device_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    device_serial VARCHAR(100) UNIQUE NOT NULL,
    vehicle_id UUID NOT NULL REFERENCES vehicles(vehicle_id),
    policy_id UUID NOT NULL REFERENCES policies(policy_id),
    device_type VARCHAR(30) CHECK (device_type IN ('OBD', 'MOBILE_APP', 'BUILT_IN')),
    device_status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        device_status IN (
            'ACTIVE',
            'INACTIVE',
            'MALFUNCTIONING',
            'REMOVED'
        )
    ),
    -- Installation
    installed_date DATE,
    activated_date DATE,
    deactivated_date DATE,
    -- Current metrics (summary)
    total_miles_tracked INTEGER DEFAULT 0,
    total_trips INTEGER DEFAULT 0,
    last_data_received TIMESTAMP,
    -- Scoring (calculated periodically)
    overall_score INTEGER CHECK (
        overall_score BETWEEN 0 AND 100
    ),
    acceleration_score INTEGER CHECK (
        acceleration_score BETWEEN 0 AND 100
    ),
    braking_score INTEGER CHECK (
        braking_score BETWEEN 0 AND 100
    ),
    cornering_score INTEGER CHECK (
        cornering_score BETWEEN 0 AND 100
    ),
    speeding_score INTEGER CHECK (
        speeding_score BETWEEN 0 AND 100
    ),
    time_of_day_score INTEGER CHECK (
        time_of_day_score BETWEEN 0 AND 100
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_telematics_vehicle ON telematics_devices(vehicle_id);
CREATE INDEX idx_telematics_policy ON telematics_devices(policy_id);
CREATE INDEX idx_telematics_status ON telematics_devices(device_status);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_vehicles_updated_at BEFORE
UPDATE ON vehicles FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_drivers_updated_at BEFORE
UPDATE ON drivers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_auto_policies_updated_at BEFORE
UPDATE ON auto_policies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_auto_claims_updated_at BEFORE
UPDATE ON auto_claims FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_telematics_updated_at BEFORE
UPDATE ON telematics_devices FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Complete auto policy view with all details
CREATE VIEW v_auto_policy_details AS
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
    ap.liability_bodily_injury_per_person,
    ap.liability_bodily_injury_per_accident,
    ap.liability_property_damage,
    ap.collision_coverage,
    ap.comprehensive_coverage,
    ap.telematics_enrolled,
    COUNT(DISTINCT pv.vehicle_id) AS vehicle_count,
    COUNT(DISTINCT pd.driver_id) AS driver_count
FROM policies p
    JOIN customers c ON p.customer_id = c.customer_id
    JOIN auto_policies ap ON p.policy_id = ap.policy_id
    LEFT JOIN policy_vehicles pv ON p.policy_id = pv.policy_id
    LEFT JOIN policy_drivers pd ON p.policy_id = pd.policy_id
WHERE p.policy_type = 'AUTO'
GROUP BY p.policy_id,
    p.policy_number,
    p.policy_status,
    p.effective_date,
    p.expiration_date,
    p.premium_amount,
    c.customer_id,
    c.first_name,
    c.last_name,
    c.email,
    ap.liability_bodily_injury_per_person,
    ap.liability_bodily_injury_per_accident,
    ap.liability_property_damage,
    ap.collision_coverage,
    ap.comprehensive_coverage,
    ap.telematics_enrolled;
-- Driver risk profile
CREATE VIEW v_driver_risk_profile AS
SELECT d.driver_id,
    d.first_name,
    d.last_name,
    d.date_of_birth,
    EXTRACT(
        YEAR
        FROM AGE(d.date_of_birth)
    ) AS age,
    d.years_licensed,
    d.license_status,
    COUNT(dv.violation_id) AS total_violations,
    COUNT(
        CASE
            WHEN dv.violation_date > CURRENT_DATE - INTERVAL '3 years' THEN 1
        END
    ) AS violations_last_3_years,
    COUNT(
        CASE
            WHEN dv.violation_type IN ('DUI', 'DWI', 'RECKLESS_DRIVING') THEN 1
        END
    ) AS major_violations,
    SUM(dv.points_assessed) AS total_points,
    COUNT(ac.auto_claim_id) AS total_claims,
    COUNT(
        CASE
            WHEN ac.at_fault = TRUE THEN 1
        END
    ) AS at_fault_claims
FROM drivers d
    LEFT JOIN driving_violations dv ON d.driver_id = dv.driver_id
    LEFT JOIN auto_claims ac ON d.driver_id = ac.driver_id
GROUP BY d.driver_id,
    d.first_name,
    d.last_name,
    d.date_of_birth,
    d.years_licensed,
    d.license_status;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE vehicles IS 'Vehicle master data with VIN, specs, and usage';
COMMENT ON TABLE drivers IS 'Driver information and license details';
COMMENT ON TABLE driving_violations IS 'Traffic violations and moving violations';
COMMENT ON TABLE auto_policies IS 'Auto insurance policy coverage details';
COMMENT ON TABLE policy_vehicles IS 'Vehicles covered under each policy';
COMMENT ON TABLE policy_drivers IS 'Drivers listed on each policy';
COMMENT ON TABLE auto_claims IS 'Auto-specific claim details';
COMMENT ON TABLE accidents IS 'Third-party accident information';
COMMENT ON TABLE telematics_devices IS 'Telematics device tracking and scoring';