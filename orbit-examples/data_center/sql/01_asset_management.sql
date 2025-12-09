-- Data Center Asset Management (Terrestrial & Orbital)
-- 1. Sites
-- Tracks physical locations (Datacenters) and orbital shells.
CREATE TABLE sites (
    site_id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    type VARCHAR(50) CHECK (
        type IN ('Terrestrial', 'Orbital_Shell', 'Ground_Station')
    ),
    location_details JSONB,
    -- Coordinates for terrestrial, Orbit param (LEO/GEO) for orbital
    capacity_mw DECIMAL(10, 2),
    -- Power capacity in Megawatts
    status VARCHAR(50) DEFAULT 'Active'
);
-- 2. Racks & Satellites
-- The container for compute resources.
CREATE TABLE racks (
    rack_id SERIAL PRIMARY KEY,
    site_id INT REFERENCES sites(site_id),
    asset_tag VARCHAR(50) UNIQUE NOT NULL,
    type VARCHAR(50) CHECK (
        type IN (
            'Standard_42U',
            'HPC_Liquid_Cooled',
            'Satellite_Bus'
        )
    ),
    power_limit_kw DECIMAL(5, 2),
    thermal_limit_btu INT,
    installation_date DATE
);
-- 3. Hardware Inventory
-- Individual servers, switches, or orbital payloads.
CREATE TABLE hardware (
    hardware_id SERIAL PRIMARY KEY,
    rack_id INT REFERENCES racks(rack_id),
    manufacturer VARCHAR(100),
    model VARCHAR(100),
    serial_number VARCHAR(100) UNIQUE,
    type VARCHAR(50) CHECK (
        type IN (
            'Server',
            'Switch',
            'Router',
            'PDU',
            'Compute_Module',
            'Radio_Transceiver'
        )
    ),
    u_position INT,
    -- Vertical position in rack (NULL for satellite components)
    state VARCHAR(50) CHECK (
        state IN (
            'Provisioning',
            'Active',
            'Maintenance',
            'Decommissioned',
            'Failed'
        )
    ),
    specs JSONB -- CPU, RAM, Disk details
);
-- 4. Maintenance Logs
CREATE TABLE maintenance_logs (
    log_id SERIAL PRIMARY KEY,
    hardware_id INT REFERENCES hardware(hardware_id),
    technician_id VARCHAR(50),
    -- Or 'AI_Agent' for automated orbital repairs
    action_type VARCHAR(100),
    description TEXT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- Seed Data: Terrestrial Site
INSERT INTO sites (name, type, location_details, capacity_mw)
VALUES (
        'US-East-1',
        'Terrestrial',
        '{"city": "Ashburn", "state": "VA", "lat": 39.04, "long": -77.48}',
        50.0
    );
-- Seed Data: Orbital Shell
INSERT INTO sites (name, type, location_details, capacity_mw)
VALUES (
        'Orbital-Shell-Alpha',
        'Orbital_Shell',
        '{"orbit": "LEO", "altitude_km": 550, "inclination": 53}',
        2.5
    );
-- Seed Data: Orbital Satellite (Treated as a Rack container)
INSERT INTO racks (site_id, asset_tag, type, power_limit_kw)
VALUES (
        (
            SELECT site_id
            FROM sites
            WHERE name = 'Orbital-Shell-Alpha'
        ),
        'SAT-101',
        'Satellite_Bus',
        1.2
    );
-- Seed Data: Compute Module on Satellite
INSERT INTO hardware (rack_id, manufacturer, model, type, state, specs)
VALUES (
        (
            SELECT rack_id
            FROM racks
            WHERE asset_tag = 'SAT-101'
        ),
        'NVIDIA',
        'Jetson-Space-Hardened',
        'Compute_Module',
        'Active',
        '{"cores": 12, "ram_gb": 32, "rad_hard": true}'
    );