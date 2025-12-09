-- Construction Core Schema (PostgreSQL)
-- Handles project management, budgets, and scheduling.
CREATE SCHEMA IF NOT EXISTS construction;
SET search_path TO construction,
    public;
-- 1. Projects
CREATE TABLE IF NOT EXISTS projects (
    project_id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    location VARCHAR(200),
    budget DECIMAL(15, 2),
    start_date DATE,
    end_date DATE,
    status VARCHAR(20) DEFAULT 'PLANNING' -- PLANNING, ACTIVE, COMPLETED, ON_HOLD
);
-- 2. Contractors
CREATE TABLE IF NOT EXISTS contractors (
    contractor_id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    specialty VARCHAR(50),
    -- Electrical, Plumbing, HVAC
    rating DECIMAL(3, 2)
);
-- 3. Contracts
CREATE TABLE IF NOT EXISTS contracts (
    contract_id SERIAL PRIMARY KEY,
    project_id INTEGER REFERENCES projects(project_id),
    contractor_id INTEGER REFERENCES contractors(contractor_id),
    value DECIMAL(12, 2),
    signed_date DATE
);
-- Data Seeding
INSERT INTO projects (name, location, budget, status)
VALUES (
        'Skyline Tower',
        '123 Market St, SF',
        50000000.00,
        'ACTIVE'
    ),
    (
        'River Bridge Rehab',
        'Highway 101, Mile 40',
        12000000.00,
        'PLANNING'
    );
INSERT INTO contractors (name, specialty)
VALUES ('Sparky Electric', 'Electrical'),
    ('PipeWorks Inc', 'Plumbing');