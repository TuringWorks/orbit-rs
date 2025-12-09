-- ============================================================================
-- OrbitRS Car Rental Examples - Core System (SQL)
-- ============================================================================
-- Management of Fleet, Locations, Customers, and Rentals
-- ============================================================================
CREATE TABLE locations (
    location_id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    airport_code VARCHAR(3),
    -- e.g., 'SFO', 'LAX'
    address VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(50),
    capacity INTEGER
);
CREATE TABLE car_classes (
    class_id SERIAL PRIMARY KEY,
    name VARCHAR(50) NOT NULL,
    -- Economy, Compact, SUV, Luxury
    base_daily_rate DECIMAL(10, 2),
    deposit_amount DECIMAL(10, 2),
    features TEXT -- JSON or CSV string of features
);
CREATE TABLE fleet (
    vehicle_id SERIAL PRIMARY KEY,
    class_id INTEGER REFERENCES car_classes(class_id),
    current_location_id INTEGER REFERENCES locations(location_id),
    make VARCHAR(50),
    model VARCHAR(50),
    year INTEGER,
    license_plate VARCHAR(20) UNIQUE,
    vin VARCHAR(20) UNIQUE,
    color VARCHAR(20),
    mileage INTEGER,
    status VARCHAR(20) DEFAULT 'AVAILABLE',
    -- AVAILABLE, RENTED, MAINTENANCE, TRANSIT
    last_service_date DATE
);
CREATE TABLE customers (
    customer_id SERIAL PRIMARY KEY,
    first_name VARCHAR(50),
    last_name VARCHAR(50),
    drivers_license VARCHAR(50) UNIQUE,
    email VARCHAR(100) UNIQUE,
    loyalty_tier VARCHAR(20) DEFAULT 'BRONZE' -- BRONZE, SILVER, GOLD
);
CREATE TABLE rentals (
    rental_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customers(customer_id),
    vehicle_id INTEGER REFERENCES fleet(vehicle_id),
    pickup_location_id INTEGER REFERENCES locations(location_id),
    dropoff_location_id INTEGER REFERENCES locations(location_id),
    pickup_date TIMESTAMP NOT NULL,
    return_date TIMESTAMP NOT NULL,
    actual_return_date TIMESTAMP,
    total_cost DECIMAL(10, 2),
    status VARCHAR(20) DEFAULT 'BOOKED' -- BOOKED, ACTIVE, COMPLETED, CANCELLED
);
-- Seed Data
INSERT INTO locations (name, airport_code, city)
VALUES ('SFO Airport', 'SFO', 'San Francisco'),
    ('LAX Airport', 'LAX', 'Los Angeles'),
    ('Downtown SF', NULL, 'San Francisco');
INSERT INTO car_classes (name, base_daily_rate, deposit_amount)
VALUES ('Economy', 45.00, 200.00),
    ('Midsize SUV', 85.00, 300.00),
    ('Luxury Sedan', 150.00, 500.00);
INSERT INTO fleet (
        class_id,
        current_location_id,
        make,
        model,
        year,
        status
    )
VALUES (1, 1, 'Toyota', 'Corolla', 2024, 'AVAILABLE'),
    (1, 1, 'Honda', 'Civic', 2024, 'RENTED'),
    (2, 2, 'Ford', 'Explorer', 2023, 'AVAILABLE'),
    (3, 1, 'BMW', '5 Series', 2024, 'MAINTENANCE');
INSERT INTO customers (first_name, last_name, email)
VALUES ('John', 'Doe', 'john.doe@example.com'),
    ('Jane', 'Smith', 'jane.smith@example.com');
-- Rental Transaction: Book a car
INSERT INTO rentals (
        customer_id,
        vehicle_id,
        pickup_location_id,
        dropoff_location_id,
        pickup_date,
        return_date,
        total_cost,
        status
    )
VALUES (
        1,
        1,
        1,
        1,
        '2024-06-01 10:00:00',
        '2024-06-05 10:00:00',
        180.00,
        'BOOKED'
    );
-- Query: Fleet Utilization by Class
SELECT cc.name as car_class,
    COUNT(f.vehicle_id) as total_fleet,
    SUM(
        CASE
            WHEN f.status = 'RENTED' THEN 1
            ELSE 0
        END
    ) as rented_count,
    ROUND(
        CAST(
            SUM(
                CASE
                    WHEN f.status = 'RENTED' THEN 1
                    ELSE 0
                END
            ) AS DECIMAL
        ) / COUNT(f.vehicle_id) * 100,
        1
    ) as utilization_pct
FROM car_classes cc
    LEFT JOIN fleet f ON cc.class_id = f.class_id
GROUP BY cc.name;
-- Query: Overdue Returns
SELECT r.rental_id,
    c.last_name,
    f.make,
    f.model,
    r.return_date
FROM rentals r
    JOIN customers c ON r.customer_id = c.customer_id
    JOIN fleet f ON r.vehicle_id = f.vehicle_id
WHERE r.status = 'ACTIVE'
    AND r.return_date < CURRENT_TIMESTAMP;