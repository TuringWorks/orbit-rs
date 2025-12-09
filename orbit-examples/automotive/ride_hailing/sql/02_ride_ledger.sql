-- ============================================================================
-- OrbitRS Ride Hailing Examples - Ride Ledger (SQL)
-- ============================================================================
-- Transactional system for Users, Drivers, and Trip Records
-- ============================================================================
CREATE TABLE users (
    user_id SERIAL PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    full_name VARCHAR(100),
    phone VARCHAR(20),
    rating DECIMAL(3, 2) DEFAULT 5.00,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE drivers (
    driver_id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES users(user_id),
    -- Drivers are also users
    license_number VARCHAR(50) UNIQUE,
    status VARCHAR(20) DEFAULT 'OFFLINE',
    -- OFFLINE, ONLINE, ON_TRIP
    total_trips INTEGER DEFAULT 0,
    rating DECIMAL(3, 2) DEFAULT 5.00,
    joined_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE trips (
    trip_id SERIAL PRIMARY KEY,
    rider_id INTEGER REFERENCES users(user_id),
    driver_id INTEGER REFERENCES drivers(driver_id),
    vehicle_id VARCHAR(50),
    -- Reference to MongoDB vehicle_id
    pickup_address VARCHAR(255),
    dropoff_address VARCHAR(255),
    status VARCHAR(20) DEFAULT 'REQUESTED',
    -- REQUESTED, MATCHED, IN_PROGRESS, COMPLETED, CANCELLED
    fare_amount DECIMAL(10, 2),
    tip_amount DECIMAL(10, 2) DEFAULT 0.00,
    currency VARCHAR(3) DEFAULT 'USD',
    requested_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    started_at TIMESTAMP,
    completed_at TIMESTAMP
);
-- Indexes for performance
CREATE INDEX idx_trips_rider ON trips(rider_id);
CREATE INDEX idx_trips_driver ON trips(driver_id);
CREATE INDEX idx_trips_status ON trips(status);
-- Seed Data
INSERT INTO users (email, full_name, phone, rating)
VALUES (
        'alice@example.com',
        'Alice Rider',
        '+15550101',
        4.8
    ),
    (
        'bob@example.com',
        'Bob Driver',
        '+15550102',
        4.9
    ),
    (
        'charlie@example.com',
        'Charlie Rider',
        '+15550103',
        4.5
    );
INSERT INTO drivers (user_id, license_number, status, rating)
VALUES (2, 'DL-CA-998877', 'ONLINE', 4.95);
-- Simulate a completed trip
INSERT INTO trips (
        rider_id,
        driver_id,
        vehicle_id,
        pickup_address,
        dropoff_address,
        status,
        fare_amount,
        tip_amount,
        started_at,
        completed_at
    )
VALUES (
        1,
        1,
        'VH-1002',
        '123 Market St',
        'SF MOMA',
        'COMPLETED',
        15.50,
        3.00,
        NOW() - INTERVAL '30 minutes',
        NOW() - INTERVAL '10 minutes'
    );
-- Simulate an active trip
INSERT INTO trips (
        rider_id,
        driver_id,
        vehicle_id,
        pickup_address,
        dropoff_address,
        status,
        requested_at,
        started_at
    )
VALUES (
        3,
        1,
        'VH-1001',
        'Ferry Building',
        'Coit Tower',
        'IN_PROGRESS',
        NOW() - INTERVAL '15 minutes',
        NOW() - INTERVAL '5 minutes'
    );
-- Query: Trip History for User 1 (Alice)
SELECT t.trip_id,
    t.pickup_address,
    t.dropoff_address,
    t.fare_amount + t.tip_amount as total_cost,
    t.completed_at
FROM trips t
WHERE t.rider_id = 1
    AND t.status = 'COMPLETED'
ORDER BY t.completed_at DESC;
-- Query: Driver Earnings Report (Today)
SELECT d.full_name as driver_name,
    COUNT(t.trip_id) as trips_count,
    SUM(t.fare_amount) as total_fare,
    SUM(t.tip_amount) as total_tips
FROM trips t
    JOIN drivers dr ON t.driver_id = dr.driver_id
    JOIN users d ON dr.user_id = d.user_id
WHERE t.status = 'COMPLETED'
    AND t.completed_at >= CURRENT_DATE
GROUP BY d.full_name;