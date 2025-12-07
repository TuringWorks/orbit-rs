-- ============================================================================
-- OrbitRS Telco Examples - Network Infrastructure Schema
-- ============================================================================
-- Cell towers, base stations, network elements, coverage, roaming
-- ============================================================================
-- ============================================================================
-- CELL TOWERS
-- ============================================================================
CREATE TABLE cell_towers (
    tower_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    tower_code VARCHAR(50) UNIQUE NOT NULL,
    -- Location
    tower_name VARCHAR(200),
    location GEOGRAPHY(POINT, 4326),
    -- PostGIS point
    latitude DECIMAL(10, 8) NOT NULL,
    longitude DECIMAL(11, 8) NOT NULL,
    altitude_meters DECIMAL(8, 2),
    -- Address
    street_address VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(50),
    postal_code VARCHAR(20),
    country VARCHAR(2) DEFAULT 'US',
    -- Technology
    technology TEXT [] DEFAULT ARRAY ['4G', '5G'],
    -- Array of supported technologies
    frequency_bands TEXT [],
    -- e.g., ['700MHz', '1900MHz', '2.5GHz']
    -- Capacity
    max_capacity_gbps DECIMAL(10, 2),
    max_concurrent_connections INTEGER,
    -- Coverage
    coverage_radius_km DECIMAL(6, 2),
    coverage_area GEOGRAPHY(POLYGON, 4326),
    -- Coverage polygon
    -- Equipment
    manufacturer VARCHAR(100),
    model VARCHAR(100),
    installation_date DATE,
    last_maintenance_date DATE,
    next_maintenance_date DATE,
    -- Power
    power_source VARCHAR(50) CHECK (
        power_source IN ('GRID', 'SOLAR', 'GENERATOR', 'HYBRID')
    ),
    backup_power BOOLEAN DEFAULT TRUE,
    backup_hours INTEGER,
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'PLANNING',
            'CONSTRUCTION',
            'ACTIVE',
            'MAINTENANCE',
            'DEGRADED',
            'OFFLINE',
            'DECOMMISSIONED'
        )
    ),
    status_reason TEXT,
    -- Ownership
    ownership_type VARCHAR(20) CHECK (ownership_type IN ('OWNED', 'LEASED', 'SHARED')),
    owner_name VARCHAR(200),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_cell_towers_location ON cell_towers USING GIST(location);
CREATE INDEX idx_cell_towers_status ON cell_towers(status);
CREATE INDEX idx_cell_towers_technology ON cell_towers USING GIN(technology);
-- ============================================================================
-- BASE STATIONS
-- ============================================================================
CREATE TABLE base_stations (
    base_station_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    station_code VARCHAR(50) UNIQUE NOT NULL,
    tower_id UUID NOT NULL REFERENCES cell_towers(tower_id),
    -- Station Details
    station_type VARCHAR(50) CHECK (
        station_type IN ('MACRO', 'MICRO', 'PICO', 'FEMTO', 'SMALL_CELL')
    ),
    technology VARCHAR(20) CHECK (technology IN ('2G', '3G', '4G', '5G')),
    -- Configuration
    sector_count INTEGER DEFAULT 3,
    antenna_count INTEGER,
    max_power_watts DECIMAL(10, 2),
    -- Capacity
    max_bandwidth_mhz DECIMAL(10, 2),
    max_throughput_mbps DECIMAL(10, 2),
    max_users INTEGER,
    -- Equipment
    equipment_id VARCHAR(100),
    manufacturer VARCHAR(100),
    model VARCHAR(100),
    firmware_version VARCHAR(50),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'STANDBY',
            'MAINTENANCE',
            'FAULTY',
            'OFFLINE'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_base_stations_tower ON base_stations(tower_id);
CREATE INDEX idx_base_stations_status ON base_stations(status);
CREATE INDEX idx_base_stations_technology ON base_stations(technology);
-- ============================================================================
-- NETWORK ELEMENTS
-- ============================================================================
CREATE TABLE network_elements (
    element_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    element_code VARCHAR(50) UNIQUE NOT NULL,
    -- Element Type
    element_type VARCHAR(50) NOT NULL CHECK (
        element_type IN (
            'MSC',
            'BSC',
            'RNC',
            'MME',
            'SGW',
            'PGW',
            'HSS',
            'PCRF',
            'IMS',
            'ROUTER',
            'SWITCH'
        )
    ),
    element_name VARCHAR(200),
    -- Location
    data_center VARCHAR(100),
    rack_location VARCHAR(50),
    -- Configuration
    ip_address INET,
    management_ip INET,
    capacity_percentage INTEGER CHECK (
        capacity_percentage BETWEEN 0 AND 100
    ),
    -- Equipment
    manufacturer VARCHAR(100),
    model VARCHAR(100),
    serial_number VARCHAR(100),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'STANDBY',
            'MAINTENANCE',
            'FAULTY',
            'OFFLINE'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_network_elements_type ON network_elements(element_type);
CREATE INDEX idx_network_elements_status ON network_elements(status);
-- ============================================================================
-- COVERAGE AREAS
-- ============================================================================
CREATE TABLE coverage_areas (
    coverage_area_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    area_name VARCHAR(200) NOT NULL,
    -- Geographic Boundary
    boundary GEOGRAPHY(POLYGON, 4326),
    -- Coverage Type
    coverage_type VARCHAR(20) CHECK (
        coverage_type IN (
            'EXCELLENT',
            'GOOD',
            'FAIR',
            'POOR',
            'NO_COVERAGE'
        )
    ),
    technology VARCHAR(20) CHECK (technology IN ('2G', '3G', '4G', '5G')),
    -- Signal Strength (dBm)
    avg_signal_strength DECIMAL(6, 2),
    min_signal_strength DECIMAL(6, 2),
    max_signal_strength DECIMAL(6, 2),
    -- Serving Towers
    primary_tower_id UUID REFERENCES cell_towers(tower_id),
    backup_tower_ids UUID [],
    -- Population
    estimated_population INTEGER,
    estimated_subscribers INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_coverage_areas_boundary ON coverage_areas USING GIST(boundary);
CREATE INDEX idx_coverage_areas_type ON coverage_areas(coverage_type);
-- ============================================================================
-- ROAMING AGREEMENTS
-- ============================================================================
CREATE TABLE roaming_agreements (
    agreement_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    agreement_number VARCHAR(50) UNIQUE NOT NULL,
    -- Partner
    partner_operator VARCHAR(200) NOT NULL,
    partner_country VARCHAR(2) NOT NULL,
    partner_mcc VARCHAR(3),
    -- Mobile Country Code
    partner_mnc VARCHAR(3),
    -- Mobile Network Code
    -- Agreement Type
    agreement_type VARCHAR(20) CHECK (
        agreement_type IN ('BILATERAL', 'UNILATERAL', 'WHOLESALE')
    ),
    -- Services
    voice_enabled BOOLEAN DEFAULT TRUE,
    data_enabled BOOLEAN DEFAULT TRUE,
    sms_enabled BOOLEAN DEFAULT TRUE,
    -- Rates (per minute/MB/SMS)
    voice_rate_outbound DECIMAL(10, 6),
    voice_rate_inbound DECIMAL(10, 6),
    data_rate_per_mb DECIMAL(10, 6),
    sms_rate_outbound DECIMAL(10, 4),
    sms_rate_inbound DECIMAL(10, 4),
    -- Terms
    effective_date DATE NOT NULL,
    expiration_date DATE,
    auto_renew BOOLEAN DEFAULT FALSE,
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'DRAFT',
            'ACTIVE',
            'SUSPENDED',
            'EXPIRED',
            'TERMINATED'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_roaming_agreements_partner ON roaming_agreements(partner_operator);
CREATE INDEX idx_roaming_agreements_status ON roaming_agreements(status);
-- ============================================================================
-- NETWORK CAPACITY
-- ============================================================================
CREATE TABLE network_capacity (
    capacity_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Resource
    resource_type VARCHAR(50) CHECK (
        resource_type IN (
            'TOWER',
            'BASE_STATION',
            'NETWORK_ELEMENT',
            'BANDWIDTH'
        )
    ),
    resource_id UUID NOT NULL,
    -- Capacity Metrics
    total_capacity DECIMAL(15, 2),
    allocated_capacity DECIMAL(15, 2),
    available_capacity DECIMAL(15, 2),
    reserved_capacity DECIMAL(15, 2),
    -- Unit of Measure
    unit_of_measure VARCHAR(20),
    -- GBPS, CONNECTIONS, ERLANG, etc.
    -- Utilization
    utilization_percentage DECIMAL(5, 2),
    peak_utilization_percentage DECIMAL(5, 2),
    -- Thresholds
    warning_threshold_percentage DECIMAL(5, 2) DEFAULT 75,
    critical_threshold_percentage DECIMAL(5, 2) DEFAULT 90,
    -- Time Period
    measurement_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_network_capacity_resource ON network_capacity(resource_type, resource_id);
CREATE INDEX idx_network_capacity_time ON network_capacity(measurement_time);
-- ============================================================================
-- BANDWIDTH ALLOCATIONS
-- ============================================================================
CREATE TABLE bandwidth_allocations (
    allocation_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Resource
    tower_id UUID REFERENCES cell_towers(tower_id),
    base_station_id UUID REFERENCES base_stations(base_station_id),
    -- Allocation
    allocated_bandwidth_mhz DECIMAL(10, 2) NOT NULL,
    frequency_band VARCHAR(50),
    -- Purpose
    allocation_purpose VARCHAR(50) CHECK (
        allocation_purpose IN (
            'COMMERCIAL',
            'EMERGENCY',
            'GOVERNMENT',
            'IOT',
            'TESTING'
        )
    ),
    -- Subscriber/Account
    subscriber_id UUID REFERENCES subscribers(subscriber_id),
    account_id UUID REFERENCES accounts(account_id),
    -- Pricing
    price_per_mhz DECIMAL(10, 2),
    total_price DECIMAL(12, 2),
    -- Time Period
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'PENDING',
            'ACTIVE',
            'COMPLETED',
            'CANCELLED'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_bandwidth_allocations_tower ON bandwidth_allocations(tower_id);
CREATE INDEX idx_bandwidth_allocations_time ON bandwidth_allocations(start_time, end_time);
CREATE INDEX idx_bandwidth_allocations_status ON bandwidth_allocations(status);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_cell_towers_updated_at BEFORE
UPDATE ON cell_towers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_base_stations_updated_at BEFORE
UPDATE ON base_stations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_network_elements_updated_at BEFORE
UPDATE ON network_elements FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_coverage_areas_updated_at BEFORE
UPDATE ON coverage_areas FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_roaming_agreements_updated_at BEFORE
UPDATE ON roaming_agreements FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active towers with capacity
CREATE VIEW v_tower_status AS
SELECT ct.tower_id,
    ct.tower_code,
    ct.tower_name,
    ct.city,
    ct.state,
    ct.status,
    ct.technology,
    ct.max_capacity_gbps,
    COUNT(DISTINCT bs.base_station_id) AS base_station_count,
    AVG(nc.utilization_percentage) AS avg_utilization
FROM cell_towers ct
    LEFT JOIN base_stations bs ON ct.tower_id = bs.tower_id
    LEFT JOIN network_capacity nc ON ct.tower_id = nc.resource_id
    AND nc.resource_type = 'TOWER'
WHERE ct.status = 'ACTIVE'
GROUP BY ct.tower_id,
    ct.tower_code,
    ct.tower_name,
    ct.city,
    ct.state,
    ct.status,
    ct.technology,
    ct.max_capacity_gbps;
-- Coverage summary by area
CREATE VIEW v_coverage_summary AS
SELECT coverage_type,
    technology,
    COUNT(*) AS area_count,
    SUM(estimated_population) AS total_population,
    SUM(estimated_subscribers) AS total_subscribers
FROM coverage_areas
GROUP BY coverage_type,
    technology
ORDER BY coverage_type,
    technology;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE cell_towers IS 'Cell tower infrastructure with geospatial data';
COMMENT ON TABLE base_stations IS 'Base station equipment on towers';
COMMENT ON TABLE network_elements IS 'Core network elements (MSC, MME, etc.)';
COMMENT ON TABLE coverage_areas IS 'Geographic coverage areas with signal strength';
COMMENT ON TABLE roaming_agreements IS 'Roaming agreements with partner operators';
COMMENT ON TABLE network_capacity IS 'Network capacity tracking and utilization';
COMMENT ON TABLE bandwidth_allocations IS 'Bandwidth allocation and bidding records';