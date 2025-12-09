-- ============================================================================
-- OrbitRS Car Sharing Examples - Membership & Bookings (SQL)
-- ============================================================================
-- Managing Users, Plans, and Short-term Reservations
-- ============================================================================
CREATE TABLE membership_plans (
    plan_id SERIAL PRIMARY KEY,
    name VARCHAR(50),
    -- 'Basic', 'Premium', 'Student'
    monthly_fee DECIMAL(10, 2),
    hourly_rate_discount_percent DECIMAL(4, 2) DEFAULT 0.00
);
CREATE TABLE members (
    member_id SERIAL PRIMARY KEY,
    email VARCHAR(100) UNIQUE,
    full_name VARCHAR(100),
    drivers_license VARCHAR(50) UNIQUE,
    plan_id INTEGER REFERENCES membership_plans(plan_id),
    join_date DATE DEFAULT CURRENT_DATE,
    account_balance DECIMAL(10, 2) DEFAULT 0.00,
    status VARCHAR(20) DEFAULT 'ACTIVE' -- ACTIVE, SUSPENDED, PENDING_VERIFICATION
);
CREATE TABLE stations (
    station_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    address VARCHAR(255),
    geo_lat DECIMAL(9, 6),
    geo_long DECIMAL(9, 6),
    total_parking_spots INTEGER
);
CREATE TABLE shared_vehicles (
    vehicle_id VARCHAR(20) PRIMARY KEY,
    -- e.g., 'CAR-1001'
    vin VARCHAR(17) UNIQUE,
    make VARCHAR(50),
    model VARCHAR(50),
    home_station_id INTEGER REFERENCES stations(station_id),
    hourly_rate DECIMAL(6, 2),
    fuel_level_percent INTEGER,
    is_available BOOLEAN DEFAULT TRUE
);
CREATE TABLE bookings (
    booking_id SERIAL PRIMARY KEY,
    member_id INTEGER REFERENCES members(member_id),
    vehicle_id VARCHAR(20) REFERENCES shared_vehicles(vehicle_id),
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    status VARCHAR(20) DEFAULT 'SCHEDULED',
    -- SCHEDULED, ACTIVE, COMPLETED, CANCELLED
    estimated_cost DECIMAL(10, 2),
    final_cost DECIMAL(10, 2),
    -- Constraint to prevent double booking the same car
    EXCLUDE USING GIST (
        vehicle_id WITH =,
        tsrange(start_time, end_time) WITH &&
    )
);
-- Seed Data
INSERT INTO membership_plans (name, monthly_fee, hourly_rate_discount_percent)
VALUES ('Basic', 0.00, 0.00),
    ('Frequent Driver', 15.00, 20.00);
INSERT INTO stations (name, address, total_parking_spots)
VALUES ('Downtown Metro', '100 Main St', 10),
    ('University Campus', '500 College Ave', 5);
INSERT INTO shared_vehicles (
        vehicle_id,
        make,
        model,
        home_station_id,
        hourly_rate
    )
VALUES ('CAR-555', 'Toyota', 'Prius', 1, 12.00),
    ('CAR-777', 'Mini', 'Cooper', 1, 14.50),
    ('CAR-888', 'Ford', 'Transit Van', 2, 18.00);
INSERT INTO members (full_name, email, plan_id)
VALUES ('Alice Sharer', 'alice@share.com', 2);
-- Create a Booking (2 hours)
INSERT INTO bookings (
        member_id,
        vehicle_id,
        start_time,
        end_time,
        estimated_cost
    )
VALUES (
        1,
        'CAR-555',
        '2024-07-01 14:00:00',
        '2024-07-01 16:00:00',
        24.00 * 0.8
    );
-- 20% discount applied
-- Query: Active bookings happening right NOW
SELECT b.booking_id,
    m.full_name,
    v.make,
    v.model,
    b.end_time
FROM bookings b
    JOIN members m ON b.member_id = m.member_id
    JOIN shared_vehicles v ON b.vehicle_id = v.vehicle_id
WHERE b.status = 'ACTIVE'
    AND CURRENT_TIMESTAMP BETWEEN b.start_time AND b.end_time;