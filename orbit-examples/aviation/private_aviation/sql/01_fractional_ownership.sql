-- ============================================================================
-- OrbitRS Aviation Examples - Private Aviation (SQL)
-- ============================================================================
-- Fractional Ownership Management
-- ============================================================================
CREATE TABLE owners (
    owner_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    account_type VARCHAR(20) -- INDIVIDUAL, CORPORATE
);
CREATE TABLE contracts (
    contract_id SERIAL PRIMARY KEY,
    owner_id INTEGER REFERENCES owners(owner_id),
    tail_number VARCHAR(10),
    -- Specific tail or 'FLEET'
    aircraft_type VARCHAR(50),
    -- 'Citation Longitude', 'Gulfstream G650'
    share_size DECIMAL(5, 4),
    -- e.g., 0.125 (1/8th share)
    allocated_hours_per_year INTEGER,
    -- e.g., 50
    remaining_hours DECIMAL(6, 2),
    contract_start DATE,
    contract_end DATE
);
CREATE TABLE flight_requests (
    request_id SERIAL PRIMARY KEY,
    owner_id INTEGER REFERENCES owners(owner_id),
    requested_aircraft_type VARCHAR(50),
    departure_airport VARCHAR(4),
    -- 'TEB' (Teterboro)
    arrival_airport VARCHAR(4),
    departure_time TIMESTAMP,
    pax_count INTEGER,
    status VARCHAR(20) DEFAULT 'PENDING' -- PENDING, CONFIRMED, FERRY_REQUIRED
);
-- Seed Data
INSERT INTO owners (name, account_type)
VALUES ('Acme Corp', 'CORPORATE');
INSERT INTO contracts (
        owner_id,
        aircraft_type,
        share_size,
        allocated_hours_per_year,
        remaining_hours
    )
VALUES (1, 'Gulfstream G650', 0.0625, 25, 22.5);
-- Request a flight
INSERT INTO flight_requests (
        owner_id,
        requested_aircraft_type,
        departure_airport,
        arrival_airport,
        departure_time,
        pax_count
    )
VALUES (
        1,
        'Gulfstream G650',
        'TEB',
        'VNY',
        '2024-07-04 09:00:00',
        4
    );