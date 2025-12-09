-- Oil & Gas Asset Management (PostgreSQL)
-- Handles wells, rigs, and maintenance schedules.
CREATE SCHEMA IF NOT EXISTS oil_gas;
SET search_path TO oil_gas,
    public;
-- 1. Sites (Fields / Refineries)
CREATE TABLE IF NOT EXISTS sites (
    site_id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    type VARCHAR(50),
    -- OFFSHORE_RIG, ONSHORE_WELL, REFINERY
    location_coords POINT,
    status VARCHAR(20) DEFAULT 'ACTIVE'
);
-- 2. Equipment
CREATE TABLE IF NOT EXISTS equipment (
    equipment_id SERIAL PRIMARY KEY,
    site_id INTEGER REFERENCES sites(site_id),
    name VARCHAR(100) NOT NULL,
    serial_number VARCHAR(100) UNIQUE,
    install_date DATE,
    last_service_date DATE
);
-- 3. Maintenance Logs
CREATE TABLE IF NOT EXISTS maintenance_logs (
    log_id SERIAL PRIMARY KEY,
    equipment_id INTEGER REFERENCES equipment(equipment_id),
    service_date DATE NOT NULL,
    technician VARCHAR(100),
    notes TEXT,
    cost DECIMAL(10, 2)
);
-- Data Seeding
INSERT INTO sites (name, type, status)
VALUES ('Alpha Rig', 'OFFSHORE_RIG', 'ACTIVE'),
    ('Texas Refinery 1', 'REFINERY', 'ACTIVE');
INSERT INTO equipment (site_id, name, serial_number)
VALUES (1, 'Drill Pump A', 'DP-1001'),
    (2, 'Cracking Unit 4', 'CU-4004');