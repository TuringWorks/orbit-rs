-- Automotive Core Schema (PostgreSQL)
-- Handles vehicle registration and ownership.
CREATE SCHEMA IF NOT EXISTS automotive;
SET search_path TO automotive,
    public;
-- 1. Owners
CREATE TABLE IF NOT EXISTS owners (
    owner_id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    phone VARCHAR(20),
    address JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
-- 2. Vehicles
CREATE TABLE IF NOT EXISTS vehicles (
    vehicle_id SERIAL PRIMARY KEY,
    vin VARCHAR(17) UNIQUE NOT NULL,
    -- Vehicle Identification Number
    make VARCHAR(50) NOT NULL,
    model VARCHAR(50) NOT NULL,
    year INTEGER NOT NULL,
    owner_id INTEGER REFERENCES owners(owner_id),
    status VARCHAR(20) DEFAULT 'ACTIVE',
    -- ACTIVE, INACTIVE, RECALLED
    registered_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
-- 3. Service History
CREATE TABLE IF NOT EXISTS service_history (
    service_id SERIAL PRIMARY KEY,
    vehicle_id INTEGER REFERENCES vehicles(vehicle_id),
    service_date DATE NOT NULL,
    description TEXT,
    mileage INTEGER CHECK (mileage >= 0),
    cost DECIMAL(10, 2)
);
-- Data Seeding
INSERT INTO owners (name, email, phone)
VALUES ('Alice Driver', 'alice@example.com', '555-0100'),
    ('Bob Fleet', 'bob@example.com', '555-0101');
INSERT INTO vehicles (vin, make, model, year, owner_id)
VALUES (
        '1HGCM82633A004352',
        'OrbitMotors',
        'Model X',
        2024,
        1
    ),
    (
        '2HGCM82633A009999',
        'OrbitMotors',
        'Hauler 3000',
        2023,
        2
    );