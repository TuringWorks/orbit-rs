-- =============================================================================
-- OrbitRS Insurance Example: CQL for Policy Management
-- =============================================================================
-- Demonstrates Cassandra CQL patterns for insurance policy data:
--   - Wide-column design for policies by customer
--   - Time-series claims data
--   - Policy event sourcing
--
-- To run:
--   cqlsh localhost 9042
--   SOURCE '01_policy_management.cql';
-- =============================================================================

-- Create keyspace for insurance data
CREATE KEYSPACE IF NOT EXISTS insurance
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 3}
AND durable_writes = true;

USE insurance;

-- =============================================================================
-- POLICY TABLES
-- =============================================================================

-- Policies by customer (optimized for customer lookups)
CREATE TABLE IF NOT EXISTS policies_by_customer (
    customer_id UUID,
    policy_id UUID,
    policy_type TEXT,           -- AUTO, HOME, LIFE, PROPERTY
    policy_number TEXT,
    effective_date DATE,
    expiration_date DATE,
    premium_amount DECIMAL,
    coverage_limit DECIMAL,
    deductible DECIMAL,
    status TEXT,                -- ACTIVE, EXPIRED, CANCELLED, PENDING
    created_at TIMESTAMP,
    updated_at TIMESTAMP,
    PRIMARY KEY ((customer_id), policy_id)
) WITH CLUSTERING ORDER BY (policy_id DESC)
AND default_time_to_live = 0
AND gc_grace_seconds = 864000;

-- Policies by type (for analytics and batch processing)
CREATE TABLE IF NOT EXISTS policies_by_type (
    policy_type TEXT,
    region TEXT,
    policy_id UUID,
    customer_id UUID,
    policy_number TEXT,
    effective_date DATE,
    premium_amount DECIMAL,
    coverage_limit DECIMAL,
    status TEXT,
    PRIMARY KEY ((policy_type, region), effective_date, policy_id)
) WITH CLUSTERING ORDER BY (effective_date DESC, policy_id ASC);

-- Policy events (event sourcing pattern)
CREATE TABLE IF NOT EXISTS policy_events (
    policy_id UUID,
    event_time TIMESTAMP,
    event_id TIMEUUID,
    event_type TEXT,            -- CREATED, RENEWED, MODIFIED, CANCELLED, CLAIM_FILED
    event_data TEXT,            -- JSON payload
    actor_id UUID,
    PRIMARY KEY ((policy_id), event_time, event_id)
) WITH CLUSTERING ORDER BY (event_time DESC, event_id DESC)
AND default_time_to_live = 31536000  -- 1 year retention
AND compaction = {'class': 'TimeWindowCompactionStrategy',
                  'compaction_window_size': 7,
                  'compaction_window_unit': 'DAYS'};

-- =============================================================================
-- CLAIMS TABLES
-- =============================================================================

-- Claims by policy (optimized for policy claim history)
CREATE TABLE IF NOT EXISTS claims_by_policy (
    policy_id UUID,
    claim_id UUID,
    claim_number TEXT,
    claim_date TIMESTAMP,
    claim_type TEXT,            -- COLLISION, THEFT, FIRE, WATER, LIABILITY, MEDICAL
    description TEXT,
    claimed_amount DECIMAL,
    approved_amount DECIMAL,
    status TEXT,                -- FILED, UNDER_REVIEW, APPROVED, DENIED, PAID, CLOSED
    adjuster_id UUID,
    created_at TIMESTAMP,
    resolved_at TIMESTAMP,
    PRIMARY KEY ((policy_id), claim_date, claim_id)
) WITH CLUSTERING ORDER BY (claim_date DESC, claim_id DESC);

-- Claims by status (for claims processing workflows)
CREATE TABLE IF NOT EXISTS claims_by_status (
    status TEXT,
    claim_date DATE,
    claim_id UUID,
    policy_id UUID,
    claim_number TEXT,
    claim_type TEXT,
    claimed_amount DECIMAL,
    priority INT,               -- 1=HIGH, 2=MEDIUM, 3=LOW
    PRIMARY KEY ((status), claim_date, priority, claim_id)
) WITH CLUSTERING ORDER BY (claim_date DESC, priority ASC, claim_id ASC);

-- Claims time-series metrics (for dashboards)
CREATE TABLE IF NOT EXISTS claims_metrics (
    metric_date DATE,
    hour INT,
    metric_type TEXT,           -- FILED, APPROVED, DENIED, PAID
    policy_type TEXT,
    region TEXT,
    count COUNTER,
    PRIMARY KEY ((metric_date), hour, metric_type, policy_type, region)
) WITH CLUSTERING ORDER BY (hour ASC, metric_type ASC, policy_type ASC, region ASC);

-- =============================================================================
-- CUSTOMER TABLES
-- =============================================================================

-- Customer profiles
CREATE TABLE IF NOT EXISTS customers (
    customer_id UUID,
    email TEXT,
    first_name TEXT,
    last_name TEXT,
    date_of_birth DATE,
    phone TEXT,
    address_street TEXT,
    address_city TEXT,
    address_state TEXT,
    address_zip TEXT,
    risk_score INT,
    customer_since DATE,
    total_premium DECIMAL,
    total_claims DECIMAL,
    PRIMARY KEY ((customer_id))
);

-- Customer by email (for login/lookup)
CREATE TABLE IF NOT EXISTS customers_by_email (
    email TEXT,
    customer_id UUID,
    first_name TEXT,
    last_name TEXT,
    PRIMARY KEY ((email))
);

-- =============================================================================
-- RISK ASSESSMENT TABLES
-- =============================================================================

-- Auto insurance risk factors
CREATE TABLE IF NOT EXISTS auto_risk_factors (
    customer_id UUID,
    vehicle_vin TEXT,
    assessment_date DATE,
    driver_score INT,           -- 0-100
    vehicle_age_years INT,
    annual_mileage INT,
    accident_history_3yr INT,
    violation_history_3yr INT,
    coverage_gap_days INT,
    risk_tier TEXT,             -- PREFERRED, STANDARD, HIGH_RISK
    premium_modifier DECIMAL,
    PRIMARY KEY ((customer_id), vehicle_vin, assessment_date)
) WITH CLUSTERING ORDER BY (vehicle_vin ASC, assessment_date DESC);

-- Property risk assessments
CREATE TABLE IF NOT EXISTS property_risk_factors (
    property_id UUID,
    assessment_date DATE,
    property_type TEXT,         -- HOUSE, CONDO, APARTMENT, COMMERCIAL
    year_built INT,
    square_footage INT,
    construction_type TEXT,
    roof_type TEXT,
    security_features SET<TEXT>,
    natural_disaster_zone TEXT,
    fire_risk_score INT,
    flood_risk_score INT,
    earthquake_risk_score INT,
    overall_risk_tier TEXT,
    PRIMARY KEY ((property_id), assessment_date)
) WITH CLUSTERING ORDER BY (assessment_date DESC);

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Sample customers
INSERT INTO customers (customer_id, email, first_name, last_name, date_of_birth, phone,
    address_city, address_state, address_zip, risk_score, customer_since, total_premium, total_claims)
VALUES (uuid(), 'john.smith@email.com', 'John', 'Smith', '1985-03-15', '+1-555-0101',
    'New York', 'NY', '10001', 75, '2020-01-15', 4500.00, 1200.00);

INSERT INTO customers (customer_id, email, first_name, last_name, date_of_birth, phone,
    address_city, address_state, address_zip, risk_score, customer_since, total_premium, total_claims)
VALUES (uuid(), 'jane.doe@email.com', 'Jane', 'Doe', '1990-07-22', '+1-555-0102',
    'Los Angeles', 'CA', '90001', 85, '2019-06-01', 6200.00, 0.00);

-- Sample policies (using specific UUIDs for referential examples)
INSERT INTO policies_by_customer (customer_id, policy_id, policy_type, policy_number,
    effective_date, expiration_date, premium_amount, coverage_limit, deductible, status, created_at)
VALUES (
    550e8400-e29b-41d4-a716-446655440000,
    550e8400-e29b-41d4-a716-446655440001,
    'AUTO', 'POL-AUTO-2024-001',
    '2024-01-01', '2025-01-01',
    1200.00, 100000.00, 500.00,
    'ACTIVE', toTimestamp(now())
);

INSERT INTO policies_by_customer (customer_id, policy_id, policy_type, policy_number,
    effective_date, expiration_date, premium_amount, coverage_limit, deductible, status, created_at)
VALUES (
    550e8400-e29b-41d4-a716-446655440000,
    550e8400-e29b-41d4-a716-446655440002,
    'HOME', 'POL-HOME-2024-001',
    '2024-01-01', '2025-01-01',
    1800.00, 500000.00, 1000.00,
    'ACTIVE', toTimestamp(now())
);

-- Sample claims
INSERT INTO claims_by_policy (policy_id, claim_id, claim_number, claim_date, claim_type,
    description, claimed_amount, status, created_at)
VALUES (
    550e8400-e29b-41d4-a716-446655440001,
    550e8400-e29b-41d4-a716-446655440010,
    'CLM-2024-0001',
    '2024-06-15 14:30:00',
    'COLLISION',
    'Rear-end collision at intersection',
    3500.00,
    'UNDER_REVIEW',
    toTimestamp(now())
);

-- Policy event
INSERT INTO policy_events (policy_id, event_time, event_id, event_type, event_data, actor_id)
VALUES (
    550e8400-e29b-41d4-a716-446655440001,
    toTimestamp(now()),
    now(),
    'CLAIM_FILED',
    '{"claim_id": "550e8400-e29b-41d4-a716-446655440010", "amount": 3500.00}',
    550e8400-e29b-41d4-a716-446655440099
);

-- Update claims metrics counters
UPDATE claims_metrics SET count = count + 1
WHERE metric_date = '2024-06-15' AND hour = 14 AND metric_type = 'FILED'
AND policy_type = 'AUTO' AND region = 'NY';

-- =============================================================================
-- QUERY PATTERNS
-- =============================================================================

-- Get all policies for a customer
SELECT * FROM policies_by_customer
WHERE customer_id = 550e8400-e29b-41d4-a716-446655440000;

-- Get active auto policies in a region
SELECT * FROM policies_by_type
WHERE policy_type = 'AUTO' AND region = 'NY'
AND effective_date >= '2024-01-01';

-- Get claim history for a policy
SELECT claim_number, claim_date, claim_type, claimed_amount, status
FROM claims_by_policy
WHERE policy_id = 550e8400-e29b-41d4-a716-446655440001
LIMIT 10;

-- Get all pending claims for processing
SELECT * FROM claims_by_status
WHERE status = 'UNDER_REVIEW'
AND claim_date >= '2024-06-01';

-- Get claims metrics for dashboard
SELECT hour, metric_type, policy_type, count
FROM claims_metrics
WHERE metric_date = '2024-06-15';

-- Get policy event history
SELECT event_time, event_type, event_data
FROM policy_events
WHERE policy_id = 550e8400-e29b-41d4-a716-446655440001
LIMIT 20;

-- =============================================================================
-- MATERIALIZED VIEWS (if supported)
-- =============================================================================

-- Active policies view
-- CREATE MATERIALIZED VIEW IF NOT EXISTS active_policies AS
--     SELECT policy_id, customer_id, policy_type, policy_number, premium_amount
--     FROM policies_by_customer
--     WHERE status = 'ACTIVE' AND customer_id IS NOT NULL AND policy_id IS NOT NULL
--     PRIMARY KEY ((policy_type), premium_amount, policy_id);
