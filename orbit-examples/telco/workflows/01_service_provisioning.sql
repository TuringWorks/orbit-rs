-- =============================================================================
-- OrbitRS Telco Example: Service Provisioning Workflow
-- =============================================================================
-- Demonstrates PostgreSQL patterns for telco service provisioning:
--   - Service activation workflow
--   - Network element provisioning
--   - SIM/eSIM management
--   - Order orchestration
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_service_provisioning.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS telco_workflow;

-- =============================================================================
-- WORKFLOW STATE MACHINE
-- =============================================================================

CREATE TABLE IF NOT EXISTS telco_workflow.provisioning_states (
    state_code VARCHAR(30) PRIMARY KEY,
    state_name VARCHAR(50) NOT NULL,
    state_order INTEGER,
    is_terminal BOOLEAN DEFAULT false,
    allowed_transitions TEXT[],
    max_retry_count INTEGER DEFAULT 3,
    timeout_minutes INTEGER DEFAULT 30,
    description TEXT
);

INSERT INTO telco_workflow.provisioning_states (state_code, state_name, state_order, is_terminal, allowed_transitions, description)
VALUES
    ('SUBMITTED', 'Submitted', 1, false, ARRAY['VALIDATING', 'REJECTED'], 'Order submitted for processing'),
    ('VALIDATING', 'Validating', 2, false, ARRAY['VALIDATED', 'REJECTED'], 'Order being validated'),
    ('VALIDATED', 'Validated', 3, false, ARRAY['PROVISIONING', 'CANCELLED'], 'Order validated successfully'),
    ('PROVISIONING', 'Provisioning', 4, false, ARRAY['TESTING', 'FAILED', 'PENDING_MANUAL'], 'Network provisioning in progress'),
    ('PENDING_MANUAL', 'Pending Manual', 5, false, ARRAY['PROVISIONING', 'FAILED', 'CANCELLED'], 'Requires manual intervention'),
    ('TESTING', 'Testing', 6, false, ARRAY['COMPLETED', 'FAILED', 'PROVISIONING'], 'Service testing in progress'),
    ('COMPLETED', 'Completed', 7, true, ARRAY[], 'Provisioning completed successfully'),
    ('FAILED', 'Failed', 8, true, ARRAY['SUBMITTED'], 'Provisioning failed'),
    ('CANCELLED', 'Cancelled', 9, true, ARRAY[], 'Order cancelled'),
    ('REJECTED', 'Rejected', 10, true, ARRAY[], 'Order rejected during validation')
ON CONFLICT (state_code) DO NOTHING;

-- =============================================================================
-- SERVICE ORDER TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS telco_workflow.service_orders (
    order_id VARCHAR(50) PRIMARY KEY,
    order_number VARCHAR(30) UNIQUE NOT NULL,
    order_type VARCHAR(30) NOT NULL, -- NEW_SERVICE, UPGRADE, DOWNGRADE, PORT_IN, DISCONNECT
    customer_id VARCHAR(50) NOT NULL,
    -- Service details
    service_type VARCHAR(30) NOT NULL, -- MOBILE, BROADBAND, TV, LANDLINE, BUNDLE
    plan_code VARCHAR(50),
    plan_name VARCHAR(100),
    -- Contact info
    contact_phone VARCHAR(20),
    contact_email VARCHAR(200),
    -- Address for installation
    service_address TEXT,
    -- Scheduling
    requested_date DATE,
    scheduled_date DATE,
    scheduled_window VARCHAR(20), -- AM, PM, EVENING
    -- Status
    status VARCHAR(30) DEFAULT 'SUBMITTED',
    priority INTEGER DEFAULT 5, -- 1=highest, 10=lowest
    -- Retry tracking
    retry_count INTEGER DEFAULT 0,
    last_error TEXT,
    -- Timestamps
    submitted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    validated_at TIMESTAMP,
    completed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS telco_workflow.order_items (
    item_id VARCHAR(50) PRIMARY KEY,
    order_id VARCHAR(50) REFERENCES telco_workflow.service_orders(order_id),
    item_type VARCHAR(30) NOT NULL, -- SERVICE, DEVICE, SIM, ACCESSORY, INSTALLATION
    item_code VARCHAR(50),
    item_name VARCHAR(200),
    quantity INTEGER DEFAULT 1,
    unit_price DECIMAL(10,2),
    total_price DECIMAL(10,2),
    provisioning_status VARCHAR(30) DEFAULT 'PENDING',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS telco_workflow.order_events (
    event_id VARCHAR(50) PRIMARY KEY,
    order_id VARCHAR(50) REFERENCES telco_workflow.service_orders(order_id),
    event_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    event_type VARCHAR(50) NOT NULL,
    from_status VARCHAR(30),
    to_status VARCHAR(30),
    actor VARCHAR(50), -- System component or user ID
    details TEXT,
    error_code VARCHAR(20),
    error_message TEXT
);

-- =============================================================================
-- NETWORK ELEMENT PROVISIONING
-- =============================================================================

CREATE TABLE IF NOT EXISTS telco_workflow.network_elements (
    element_id VARCHAR(50) PRIMARY KEY,
    element_type VARCHAR(30) NOT NULL, -- HLR, HSS, PCRF, OCS, AAA, DNS, DHCP
    element_name VARCHAR(100),
    hostname VARCHAR(200),
    ip_address VARCHAR(45),
    region VARCHAR(20),
    vendor VARCHAR(50),
    version VARCHAR(30),
    capacity_limit INTEGER,
    current_load INTEGER DEFAULT 0,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS telco_workflow.provisioning_tasks (
    task_id VARCHAR(50) PRIMARY KEY,
    order_id VARCHAR(50) REFERENCES telco_workflow.service_orders(order_id),
    element_id VARCHAR(50) REFERENCES telco_workflow.network_elements(element_id),
    task_type VARCHAR(50) NOT NULL,
    -- HLR_PROFILE_CREATE, HSS_SUBSCRIPTION_ADD, PCRF_POLICY_APPLY,
    -- OCS_ACCOUNT_CREATE, RADIUS_PROFILE_ADD, SIM_ACTIVATE
    task_order INTEGER DEFAULT 1,
    -- Task data
    input_parameters TEXT, -- JSON
    output_parameters TEXT, -- JSON
    -- Status
    status VARCHAR(30) DEFAULT 'PENDING',
    retry_count INTEGER DEFAULT 0,
    -- Timing
    scheduled_at TIMESTAMP,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    timeout_at TIMESTAMP,
    -- Error handling
    error_code VARCHAR(20),
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- SIM/eSIM MANAGEMENT
-- =============================================================================

CREATE TABLE IF NOT EXISTS telco_workflow.sim_inventory (
    sim_id VARCHAR(50) PRIMARY KEY,
    iccid VARCHAR(30) UNIQUE NOT NULL,
    imsi VARCHAR(20),
    ki VARCHAR(64), -- Encrypted
    sim_type VARCHAR(20), -- PHYSICAL, ESIM
    form_factor VARCHAR(20), -- MINI, MICRO, NANO, EMBEDDED
    -- Status
    status VARCHAR(20) DEFAULT 'AVAILABLE', -- AVAILABLE, RESERVED, ASSIGNED, ACTIVE, SUSPENDED, DEACTIVATED
    -- Assignment
    order_id VARCHAR(50),
    customer_id VARCHAR(50),
    msisdn VARCHAR(20),
    -- Timestamps
    manufactured_at DATE,
    assigned_at TIMESTAMP,
    activated_at TIMESTAMP,
    deactivated_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS telco_workflow.esim_profiles (
    profile_id VARCHAR(50) PRIMARY KEY,
    sim_id VARCHAR(50) REFERENCES telco_workflow.sim_inventory(sim_id),
    profile_type VARCHAR(30), -- OPERATIONAL, PROVISIONING, TEST
    smdp_address VARCHAR(200), -- SM-DP+ server
    matching_id VARCHAR(100),
    confirmation_code VARCHAR(50),
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING', -- PENDING, DOWNLOADING, INSTALLED, ACTIVE, DISABLED
    -- Download tracking
    download_initiated_at TIMESTAMP,
    download_completed_at TIMESTAMP,
    install_confirmed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- NUMBER MANAGEMENT
-- =============================================================================

CREATE TABLE IF NOT EXISTS telco_workflow.number_inventory (
    number_id VARCHAR(50) PRIMARY KEY,
    msisdn VARCHAR(20) UNIQUE NOT NULL,
    number_type VARCHAR(20), -- MOBILE, LANDLINE, TOLLFREE, PREMIUM
    rate_center VARCHAR(100),
    region VARCHAR(50),
    -- Status
    status VARCHAR(20) DEFAULT 'AVAILABLE', -- AVAILABLE, RESERVED, ASSIGNED, PORTED_OUT
    -- Assignment
    order_id VARCHAR(50),
    customer_id VARCHAR(50),
    sim_id VARCHAR(50),
    -- Port tracking
    is_ported_in BOOLEAN DEFAULT false,
    port_in_date DATE,
    donor_carrier VARCHAR(100),
    -- Timestamps
    reserved_at TIMESTAMP,
    assigned_at TIMESTAMP,
    released_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- WORKFLOW FUNCTIONS
-- =============================================================================

-- Validate state transition
CREATE OR REPLACE FUNCTION telco_workflow.can_transition(
    current_state VARCHAR,
    new_state VARCHAR
) RETURNS BOOLEAN AS $$
DECLARE
    allowed TEXT[];
BEGIN
    SELECT allowed_transitions INTO allowed
    FROM telco_workflow.provisioning_states
    WHERE state_code = current_state;

    RETURN new_state = ANY(allowed);
END;
$$ LANGUAGE plpgsql;

-- Update order status with event logging
CREATE OR REPLACE FUNCTION telco_workflow.update_order_status(
    p_order_id VARCHAR,
    p_new_status VARCHAR,
    p_actor VARCHAR DEFAULT 'SYSTEM',
    p_details TEXT DEFAULT NULL,
    p_error_code VARCHAR DEFAULT NULL,
    p_error_message TEXT DEFAULT NULL
) RETURNS BOOLEAN AS $$
DECLARE
    v_current_status VARCHAR;
BEGIN
    SELECT status INTO v_current_status
    FROM telco_workflow.service_orders
    WHERE order_id = p_order_id;

    IF NOT telco_workflow.can_transition(v_current_status, p_new_status) THEN
        RAISE EXCEPTION 'Invalid state transition from % to %', v_current_status, p_new_status;
    END IF;

    -- Update order
    UPDATE telco_workflow.service_orders
    SET status = p_new_status,
        updated_at = CURRENT_TIMESTAMP,
        last_error = COALESCE(p_error_message, last_error),
        validated_at = CASE WHEN p_new_status = 'VALIDATED' THEN CURRENT_TIMESTAMP ELSE validated_at END,
        completed_at = CASE WHEN p_new_status = 'COMPLETED' THEN CURRENT_TIMESTAMP ELSE completed_at END
    WHERE order_id = p_order_id;

    -- Log event
    INSERT INTO telco_workflow.order_events (
        event_id, order_id, event_type, from_status, to_status, actor, details, error_code, error_message
    ) VALUES (
        'EVT-' || SUBSTRING(MD5(RANDOM()::TEXT) FROM 1 FOR 12),
        p_order_id, 'STATUS_CHANGE', v_current_status, p_new_status, p_actor, p_details, p_error_code, p_error_message
    );

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Reserve SIM for order
CREATE OR REPLACE FUNCTION telco_workflow.reserve_sim(
    p_order_id VARCHAR,
    p_sim_type VARCHAR DEFAULT 'PHYSICAL'
) RETURNS VARCHAR AS $$
DECLARE
    v_sim_id VARCHAR;
    v_iccid VARCHAR;
BEGIN
    -- Find available SIM
    SELECT sim_id, iccid INTO v_sim_id, v_iccid
    FROM telco_workflow.sim_inventory
    WHERE status = 'AVAILABLE'
    AND sim_type = p_sim_type
    ORDER BY manufactured_at
    LIMIT 1
    FOR UPDATE SKIP LOCKED;

    IF v_sim_id IS NULL THEN
        RAISE EXCEPTION 'No available SIM cards of type %', p_sim_type;
    END IF;

    -- Reserve SIM
    UPDATE telco_workflow.sim_inventory
    SET status = 'RESERVED',
        order_id = p_order_id
    WHERE sim_id = v_sim_id;

    RETURN v_iccid;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Network elements
INSERT INTO telco_workflow.network_elements (element_id, element_type, element_name, hostname, ip_address, region, vendor)
VALUES
    ('NE-HLR-001', 'HLR', 'Primary HLR', 'hlr01.network.local', '10.1.1.10', 'EAST', 'Ericsson'),
    ('NE-HSS-001', 'HSS', 'HSS Cluster 1', 'hss01.network.local', '10.1.2.10', 'EAST', 'Nokia'),
    ('NE-PCRF-001', 'PCRF', 'Policy Server 1', 'pcrf01.network.local', '10.1.3.10', 'EAST', 'Cisco'),
    ('NE-OCS-001', 'OCS', 'Charging System 1', 'ocs01.network.local', '10.1.4.10', 'EAST', 'Huawei'),
    ('NE-AAA-001', 'AAA', 'RADIUS Server 1', 'aaa01.network.local', '10.1.5.10', 'EAST', 'FreeRADIUS')
ON CONFLICT (element_id) DO NOTHING;

-- SIM inventory
INSERT INTO telco_workflow.sim_inventory (sim_id, iccid, imsi, sim_type, form_factor, status, manufactured_at)
VALUES
    ('SIM-001', '89012345678901234567', '310260123456789', 'PHYSICAL', 'NANO', 'AVAILABLE', '2024-01-15'),
    ('SIM-002', '89012345678901234568', '310260123456790', 'PHYSICAL', 'NANO', 'AVAILABLE', '2024-01-15'),
    ('SIM-003', '89012345678901234569', '310260123456791', 'ESIM', 'EMBEDDED', 'AVAILABLE', '2024-02-01'),
    ('SIM-004', '89012345678901234570', '310260123456792', 'PHYSICAL', 'NANO', 'ACTIVE', '2024-01-15'),
    ('SIM-005', '89012345678901234571', '310260123456793', 'ESIM', 'EMBEDDED', 'AVAILABLE', '2024-02-01')
ON CONFLICT (sim_id) DO NOTHING;

-- Number inventory
INSERT INTO telco_workflow.number_inventory (number_id, msisdn, number_type, rate_center, region, status)
VALUES
    ('NUM-001', '+12125551001', 'MOBILE', 'New York', 'NY', 'AVAILABLE'),
    ('NUM-002', '+12125551002', 'MOBILE', 'New York', 'NY', 'AVAILABLE'),
    ('NUM-003', '+13105551001', 'MOBILE', 'Los Angeles', 'CA', 'AVAILABLE'),
    ('NUM-004', '+13125551001', 'MOBILE', 'Chicago', 'IL', 'ASSIGNED'),
    ('NUM-005', '+18005551001', 'TOLLFREE', 'National', 'US', 'AVAILABLE')
ON CONFLICT (number_id) DO NOTHING;

-- Service orders
INSERT INTO telco_workflow.service_orders (order_id, order_number, order_type, customer_id,
    service_type, plan_code, plan_name, contact_email, status, priority)
VALUES
    ('ORD-T-001', 'SO-2024-0001', 'NEW_SERVICE', 'CUST-001',
     'MOBILE', 'PLAN-UNL-5G', 'Unlimited 5G', 'john@email.com', 'PROVISIONING', 3),
    ('ORD-T-002', 'SO-2024-0002', 'PORT_IN', 'CUST-002',
     'MOBILE', 'PLAN-FAM-SHARE', 'Family Share 100GB', 'jane@email.com', 'VALIDATING', 2),
    ('ORD-T-003', 'SO-2024-0003', 'UPGRADE', 'CUST-003',
     'BROADBAND', 'PLAN-FIBER-1G', 'Fiber 1Gbps', 'bob@email.com', 'COMPLETED', 5)
ON CONFLICT (order_id) DO NOTHING;

-- Order items
INSERT INTO telco_workflow.order_items (item_id, order_id, item_type, item_code, item_name, quantity, unit_price, total_price)
VALUES
    ('OI-001', 'ORD-T-001', 'SERVICE', 'PLAN-UNL-5G', 'Unlimited 5G Plan', 1, 79.99, 79.99),
    ('OI-002', 'ORD-T-001', 'DEVICE', 'IPHONE-15-PRO', 'iPhone 15 Pro 256GB', 1, 999.00, 999.00),
    ('OI-003', 'ORD-T-001', 'SIM', 'SIM-NANO', 'Nano SIM Card', 1, 0.00, 0.00),
    ('OI-004', 'ORD-T-002', 'SERVICE', 'PLAN-FAM-SHARE', 'Family Share Plan', 1, 149.99, 149.99)
ON CONFLICT (item_id) DO NOTHING;

-- Provisioning tasks
INSERT INTO telco_workflow.provisioning_tasks (task_id, order_id, element_id, task_type, task_order, status)
VALUES
    ('TASK-001', 'ORD-T-001', 'NE-HLR-001', 'HLR_PROFILE_CREATE', 1, 'COMPLETED'),
    ('TASK-002', 'ORD-T-001', 'NE-HSS-001', 'HSS_SUBSCRIPTION_ADD', 2, 'COMPLETED'),
    ('TASK-003', 'ORD-T-001', 'NE-PCRF-001', 'PCRF_POLICY_APPLY', 3, 'IN_PROGRESS'),
    ('TASK-004', 'ORD-T-001', 'NE-OCS-001', 'OCS_ACCOUNT_CREATE', 4, 'PENDING')
ON CONFLICT (task_id) DO NOTHING;

-- =============================================================================
-- WORKFLOW QUERIES
-- =============================================================================

-- Order pipeline by status
SELECT
    status,
    COUNT(*) as order_count,
    AVG(EXTRACT(EPOCH FROM (CURRENT_TIMESTAMP - submitted_at)) / 3600) as avg_hours_in_status
FROM telco_workflow.service_orders
GROUP BY status
ORDER BY (SELECT state_order FROM telco_workflow.provisioning_states WHERE state_code = status);

-- Provisioning task status
SELECT
    so.order_number,
    ne.element_type,
    pt.task_type,
    pt.status as task_status,
    pt.retry_count,
    pt.error_message
FROM telco_workflow.provisioning_tasks pt
JOIN telco_workflow.service_orders so ON pt.order_id = so.order_id
JOIN telco_workflow.network_elements ne ON pt.element_id = ne.element_id
WHERE so.status = 'PROVISIONING'
ORDER BY so.order_number, pt.task_order;

-- SIM availability
SELECT
    sim_type,
    form_factor,
    status,
    COUNT(*) as count
FROM telco_workflow.sim_inventory
GROUP BY sim_type, form_factor, status
ORDER BY sim_type, form_factor, status;

-- Number inventory by region
SELECT
    region,
    number_type,
    status,
    COUNT(*) as count
FROM telco_workflow.number_inventory
GROUP BY region, number_type, status
ORDER BY region, number_type, status;

-- Order event timeline
SELECT
    oe.event_time,
    so.order_number,
    oe.event_type,
    oe.from_status,
    oe.to_status,
    oe.actor,
    oe.error_code
FROM telco_workflow.order_events oe
JOIN telco_workflow.service_orders so ON oe.order_id = so.order_id
ORDER BY oe.event_time DESC
LIMIT 20;

-- Network element load
SELECT
    element_type,
    element_name,
    current_load,
    capacity_limit,
    ROUND((current_load::DECIMAL / NULLIF(capacity_limit, 0)) * 100, 2) as utilization_percent,
    status
FROM telco_workflow.network_elements
ORDER BY utilization_percent DESC NULLS LAST;
