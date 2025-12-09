-- ============================================================================
-- OrbitRS Humanoid Robot Manufacturing - Parts Genealogy (SQL)
-- ============================================================================
-- Full traceability of every serialized component
-- ============================================================================
CREATE TABLE part_catalog (
    part_number VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100),
    -- 'Left Hand Actuator Gen3', 'Vision Module'
    supplier_id INTEGER,
    cost DECIMAL(10, 2),
    is_critical_component BOOLEAN DEFAULT FALSE
);
CREATE TABLE robot_assemblies (
    robot_serial VARCHAR(50) PRIMARY KEY,
    -- 'RBT-9000-X1'
    model_version VARCHAR(20),
    -- 'Optimus-Prime-v1'
    production_date DATE,
    status VARCHAR(20) DEFAULT 'ASSEMBLY',
    -- ASSEMBLY, TESTING, CALIBRATION, READY
    firmware_version VARCHAR(20)
);
CREATE TABLE genealogy_trace (
    trace_id SERIAL PRIMARY KEY,
    robot_serial VARCHAR(50) REFERENCES robot_assemblies(robot_serial),
    part_number VARCHAR(50) REFERENCES part_catalog(part_number),
    component_serial VARCHAR(100) UNIQUE,
    -- The specific serial of the installed part
    installed_at TIMESTAMP,
    installed_by_technician_id INTEGER,
    torque_value_nm DECIMAL(5, 2) -- e.g. Bolt torque during install
);
-- Seed Data
INSERT INTO part_catalog (part_number, name, cost, is_critical_component)
VALUES (
        'ACT-L-G3',
        'Left Hand Actuator Gen3',
        2500.00,
        TRUE
    ),
    ('GPU-AI-X', 'AI Inference Core', 12000.00, TRUE);
INSERT INTO robot_assemblies (robot_serial, model_version, production_date)
VALUES ('RBT-9000-001', 'Gen-1', CURRENT_DATE);
-- Install Actuator
INSERT INTO genealogy_trace (
        robot_serial,
        part_number,
        component_serial,
        installed_at,
        torque_value_nm
    )
VALUES (
        'RBT-9000-001',
        'ACT-L-G3',
        'SN-ACT-998877',
        CURRENT_TIMESTAMP,
        45.5
    );
-- Query: Full Bill of Materials for a specific robot instance
SELECT p.name,
    g.component_serial,
    g.installed_at
FROM genealogy_trace g
    JOIN part_catalog p ON g.part_number = p.part_number
WHERE g.robot_serial = 'RBT-9000-001';