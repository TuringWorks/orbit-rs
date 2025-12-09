-- ============================================================================
-- OrbitRS Travel Examples - Car Rental Schema
-- ============================================================================
-- Rental companies, locations, vehicles, pricing, bookings
-- ============================================================================
-- Enable UUID extension (if not already enabled)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- CAR RENTAL COMPANIES
-- ============================================================================
CREATE TABLE car_rental_companies (
    company_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_code VARCHAR(10) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    -- Company Type
    company_type VARCHAR(30) CHECK (
        company_type IN ('MAJOR', 'REGIONAL', 'LOCAL', 'PEER_TO_PEER')
    ),
    -- Loyalty Program
    loyalty_program_name VARCHAR(100),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    -- Contact
    website VARCHAR(500),
    phone VARCHAR(50),
    -- Metadata
    logo_url VARCHAR(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_car_rental_companies_code ON car_rental_companies(company_code);
-- ============================================================================
-- RENTAL LOCATIONS
-- ============================================================================
CREATE TABLE rental_locations (
    location_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    company_id UUID NOT NULL REFERENCES car_rental_companies(company_id),
    location_code VARCHAR(20) UNIQUE NOT NULL,
    -- Location Details
    name VARCHAR(200) NOT NULL,
    location_type VARCHAR(30) CHECK (
        location_type IN (
            'AIRPORT',
            'DOWNTOWN',
            'SUBURBAN',
            'TRAIN_STATION'
        )
    ),
    -- Address
    address_line1 VARCHAR(200),
    address_line2 VARCHAR(200),
    city VARCHAR(100) NOT NULL,
    state VARCHAR(100),
    country VARCHAR(100) NOT NULL,
    postal_code VARCHAR(20),
    latitude DECIMAL(10, 7),
    longitude DECIMAL(11, 7),
    -- Airport Association
    airport_code VARCHAR(3),
    is_airport_location BOOLEAN DEFAULT FALSE,
    shuttle_available BOOLEAN DEFAULT FALSE,
    -- Hours
    hours_of_operation VARCHAR(200),
    is_24_hours BOOLEAN DEFAULT FALSE,
    -- Contact
    phone VARCHAR(50),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_rental_locations_company ON rental_locations(company_id);
CREATE INDEX idx_rental_locations_city ON rental_locations(city);
CREATE INDEX idx_rental_locations_airport ON rental_locations(airport_code);
-- ============================================================================
-- VEHICLE TYPES
-- ============================================================================
CREATE TABLE vehicle_types (
    vehicle_type_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    type_code VARCHAR(20) UNIQUE NOT NULL,
    -- ECAR, CCAR, ICAR, SCAR, FCAR, etc.
    -- Vehicle Category
    category VARCHAR(50) NOT NULL CHECK (
        category IN (
            'ECONOMY',
            'COMPACT',
            'MIDSIZE',
            'STANDARD',
            'FULLSIZE',
            'PREMIUM',
            'LUXURY',
            'SUV',
            'MINIVAN',
            'CONVERTIBLE',
            'SPORTS',
            'TRUCK',
            'ELECTRIC',
            'HYBRID'
        )
    ),
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Capacity
    passenger_capacity INTEGER NOT NULL,
    luggage_capacity INTEGER,
    num_doors INTEGER,
    -- Transmission
    transmission VARCHAR(20) CHECK (
        transmission IN ('AUTOMATIC', 'MANUAL')
    ),
    -- Fuel
    fuel_type VARCHAR(30) CHECK (
        fuel_type IN (
            'GASOLINE',
            'DIESEL',
            'ELECTRIC',
            'HYBRID',
            'PLUGIN_HYBRID'
        )
    ),
    -- Features
    has_ac BOOLEAN DEFAULT TRUE,
    has_gps BOOLEAN DEFAULT FALSE,
    has_bluetooth BOOLEAN DEFAULT TRUE,
    has_backup_camera BOOLEAN DEFAULT FALSE,
    -- Example Models
    example_models VARCHAR(500),
    -- e.g., "Toyota Corolla or similar"
    -- Display
    image_url VARCHAR(500),
    display_order INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_vehicle_types_category ON vehicle_types(category);
-- ============================================================================
-- VEHICLE INVENTORY
-- ============================================================================
CREATE TABLE vehicle_inventory (
    inventory_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    location_id UUID NOT NULL REFERENCES rental_locations(location_id),
    vehicle_type_id UUID NOT NULL REFERENCES vehicle_types(vehicle_type_id),
    rental_date DATE NOT NULL,
    -- Availability
    total_vehicles INTEGER NOT NULL,
    available_vehicles INTEGER NOT NULL,
    reserved_vehicles INTEGER DEFAULT 0,
    -- Status
    is_available BOOLEAN DEFAULT TRUE,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(location_id, vehicle_type_id, rental_date)
);
CREATE INDEX idx_vehicle_inventory_location ON vehicle_inventory(location_id);
CREATE INDEX idx_vehicle_inventory_type ON vehicle_inventory(vehicle_type_id);
CREATE INDEX idx_vehicle_inventory_date ON vehicle_inventory(rental_date);
-- ============================================================================
-- CAR PRICING
-- ============================================================================
CREATE TABLE car_pricing (
    pricing_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    location_id UUID NOT NULL REFERENCES rental_locations(location_id),
    vehicle_type_id UUID NOT NULL REFERENCES vehicle_types(vehicle_type_id),
    rental_date DATE NOT NULL,
    -- Pricing
    daily_rate DECIMAL(10, 2) NOT NULL,
    weekly_rate DECIMAL(10, 2),
    weekend_rate DECIMAL(10, 2),
    -- Additional Fees
    airport_fee DECIMAL(10, 2) DEFAULT 0,
    -- Insurance Options
    collision_damage_waiver_daily DECIMAL(10, 2) DEFAULT 15.00,
    liability_insurance_daily DECIMAL(10, 2) DEFAULT 12.00,
    personal_accident_insurance_daily DECIMAL(10, 2) DEFAULT 8.00,
    -- Add-ons
    gps_daily DECIMAL(10, 2) DEFAULT 10.00,
    child_seat_daily DECIMAL(10, 2) DEFAULT 8.00,
    additional_driver_daily DECIMAL(10, 2) DEFAULT 12.00,
    -- Mileage
    included_miles_per_day INTEGER DEFAULT 150,
    extra_mile_charge DECIMAL(10, 4) DEFAULT 0.25,
    unlimited_mileage BOOLEAN DEFAULT FALSE,
    -- Validity
    valid_from TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    valid_to TIMESTAMP,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(location_id, vehicle_type_id, rental_date)
);
CREATE INDEX idx_car_pricing_location ON car_pricing(location_id);
CREATE INDEX idx_car_pricing_type ON car_pricing(vehicle_type_id);
CREATE INDEX idx_car_pricing_date ON car_pricing(rental_date);
-- ============================================================================
-- CAR BOOKINGS
-- ============================================================================
CREATE TABLE car_bookings (
    booking_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    confirmation_number VARCHAR(20) UNIQUE NOT NULL,
    -- Rental Details
    company_id UUID NOT NULL REFERENCES car_rental_companies(company_id),
    vehicle_type_id UUID NOT NULL REFERENCES vehicle_types(vehicle_type_id),
    pickup_location_id UUID NOT NULL REFERENCES rental_locations(location_id),
    dropoff_location_id UUID NOT NULL REFERENCES rental_locations(location_id),
    -- Customer
    customer_id UUID NOT NULL,
    -- Rental Period
    pickup_date DATE NOT NULL,
    pickup_time TIME NOT NULL,
    dropoff_date DATE NOT NULL,
    dropoff_time TIME NOT NULL,
    num_days INTEGER NOT NULL,
    -- Driver Information
    driver_name VARCHAR(200) NOT NULL,
    driver_license_number VARCHAR(50) NOT NULL,
    driver_age INTEGER,
    additional_drivers INTEGER DEFAULT 0,
    -- Pricing
    daily_rate DECIMAL(10, 2) NOT NULL,
    num_days_charged INTEGER NOT NULL,
    subtotal DECIMAL(10, 2) NOT NULL,
    -- Insurance
    has_collision_damage_waiver BOOLEAN DEFAULT FALSE,
    has_liability_insurance BOOLEAN DEFAULT FALSE,
    has_personal_accident_insurance BOOLEAN DEFAULT FALSE,
    insurance_total DECIMAL(10, 2) DEFAULT 0,
    -- Add-ons
    has_gps BOOLEAN DEFAULT FALSE,
    num_child_seats INTEGER DEFAULT 0,
    addons_total DECIMAL(10, 2) DEFAULT 0,
    -- Fees and Taxes
    fees DECIMAL(10, 2) DEFAULT 0,
    taxes DECIMAL(10, 2) NOT NULL,
    total_price DECIMAL(10, 2) NOT NULL,
    -- Payment
    payment_status VARCHAR(30) DEFAULT 'PENDING' CHECK (
        payment_status IN ('PENDING', 'PAID', 'REFUNDED', 'FAILED')
    ),
    payment_method VARCHAR(30),
    -- Booking Status
    booking_status VARCHAR(30) DEFAULT 'CONFIRMED' CHECK (
        booking_status IN (
            'PENDING',
            'CONFIRMED',
            'PICKED_UP',
            'RETURNED',
            'CANCELLED',
            'NO_SHOW'
        )
    ),
    -- Actual Pickup/Return
    actual_pickup_time TIMESTAMP,
    actual_return_time TIMESTAMP,
    -- Mileage
    pickup_mileage INTEGER,
    return_mileage INTEGER,
    total_miles_driven INTEGER,
    -- Fuel
    fuel_level_pickup VARCHAR(20),
    -- FULL, 3/4, 1/2, 1/4, EMPTY
    fuel_level_return VARCHAR(20),
    fuel_charge DECIMAL(10, 2) DEFAULT 0,
    -- Loyalty
    loyalty_points_earned INTEGER DEFAULT 0,
    loyalty_points_redeemed INTEGER DEFAULT 0,
    -- Timestamps
    booked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    cancelled_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_car_bookings_confirmation ON car_bookings(confirmation_number);
CREATE INDEX idx_car_bookings_customer ON car_bookings(customer_id);
CREATE INDEX idx_car_bookings_company ON car_bookings(company_id);
CREATE INDEX idx_car_bookings_pickup_location ON car_bookings(pickup_location_id);
CREATE INDEX idx_car_bookings_dates ON car_bookings(pickup_date, dropoff_date);
CREATE INDEX idx_car_bookings_status ON car_bookings(booking_status);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_car_rental_companies_updated_at BEFORE
UPDATE ON car_rental_companies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_rental_locations_updated_at BEFORE
UPDATE ON rental_locations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_vehicle_inventory_updated_at BEFORE
UPDATE ON vehicle_inventory FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_car_pricing_updated_at BEFORE
UPDATE ON car_pricing FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_car_bookings_updated_at BEFORE
UPDATE ON car_bookings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active rental locations
CREATE VIEW v_active_rental_locations AS
SELECT rl.location_id,
    rl.location_code,
    rl.name,
    rl.city,
    rl.state,
    rl.country,
    crc.name AS company_name,
    rl.location_type,
    rl.is_airport_location,
    rl.airport_code
FROM rental_locations rl
    JOIN car_rental_companies crc ON rl.company_id = crc.company_id
WHERE rl.is_active = TRUE
    AND crc.is_active = TRUE
ORDER BY rl.city,
    rl.name;
-- Vehicle availability by date
CREATE VIEW v_vehicle_availability AS
SELECT rl.location_id,
    rl.name AS location_name,
    rl.city,
    vt.vehicle_type_id,
    vt.category,
    vt.name AS vehicle_name,
    vi.rental_date,
    vi.available_vehicles,
    cp.daily_rate
FROM vehicle_inventory vi
    JOIN rental_locations rl ON vi.location_id = rl.location_id
    JOIN vehicle_types vt ON vi.vehicle_type_id = vt.vehicle_type_id
    LEFT JOIN car_pricing cp ON vi.location_id = cp.location_id
    AND vi.vehicle_type_id = cp.vehicle_type_id
    AND vi.rental_date = cp.rental_date
WHERE vi.is_available = TRUE
    AND vi.available_vehicles > 0
    AND rl.is_active = TRUE
ORDER BY rl.city,
    vi.rental_date,
    vt.category;
-- ============================================================================
-- SAMPLE DATA
-- ============================================================================
-- Insert sample car rental companies
INSERT INTO car_rental_companies (
        company_code,
        name,
        company_type,
        loyalty_program_name
    )
VALUES (
        'HRTZ',
        'Hertz',
        'MAJOR',
        'Hertz Gold Plus Rewards'
    ),
    ('AVIS', 'Avis', 'MAJOR', 'Avis Preferred'),
    ('ENTR', 'Enterprise', 'MAJOR', 'Enterprise Plus'),
    ('BDGT', 'Budget', 'MAJOR', 'Budget Fastbreak'),
    ('NATL', 'National', 'MAJOR', 'Emerald Club');
-- Insert sample vehicle types
INSERT INTO vehicle_types (
        type_code,
        category,
        name,
        passenger_capacity,
        luggage_capacity,
        transmission,
        fuel_type,
        example_models
    )
VALUES (
        'ECAR',
        'ECONOMY',
        'Economy Car',
        5,
        2,
        'AUTOMATIC',
        'GASOLINE',
        'Toyota Yaris, Nissan Versa or similar'
    ),
    (
        'CCAR',
        'COMPACT',
        'Compact Car',
        5,
        2,
        'AUTOMATIC',
        'GASOLINE',
        'Toyota Corolla, Honda Civic or similar'
    ),
    (
        'ICAR',
        'MIDSIZE',
        'Intermediate Car',
        5,
        3,
        'AUTOMATIC',
        'GASOLINE',
        'Toyota Camry, Honda Accord or similar'
    ),
    (
        'SCAR',
        'STANDARD',
        'Standard Car',
        5,
        3,
        'AUTOMATIC',
        'GASOLINE',
        'Chevrolet Malibu, Nissan Altima or similar'
    ),
    (
        'FCAR',
        'FULLSIZE',
        'Full-Size Car',
        5,
        4,
        'AUTOMATIC',
        'GASOLINE',
        'Chevrolet Impala, Toyota Avalon or similar'
    ),
    (
        'PCAR',
        'PREMIUM',
        'Premium Car',
        5,
        4,
        'AUTOMATIC',
        'GASOLINE',
        'Chrysler 300, Nissan Maxima or similar'
    ),
    (
        'SSUV',
        'SUV',
        'Standard SUV',
        5,
        4,
        'AUTOMATIC',
        'GASOLINE',
        'Toyota RAV4, Honda CR-V or similar'
    ),
    (
        'FSUV',
        'SUV',
        'Full-Size SUV',
        7,
        5,
        'AUTOMATIC',
        'GASOLINE',
        'Chevrolet Tahoe, Ford Expedition or similar'
    ),
    (
        'MVAN',
        'MINIVAN',
        'Minivan',
        7,
        4,
        'AUTOMATIC',
        'GASOLINE',
        'Chrysler Pacifica, Honda Odyssey or similar'
    ),
    (
        'ECAR-EV',
        'ELECTRIC',
        'Electric Car',
        5,
        2,
        'AUTOMATIC',
        'ELECTRIC',
        'Tesla Model 3, Nissan Leaf or similar'
    );
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE car_rental_companies IS 'Car rental agencies and brands';
COMMENT ON TABLE rental_locations IS 'Pickup and dropoff locations';
COMMENT ON TABLE vehicle_types IS 'Vehicle categories and specifications';
COMMENT ON TABLE vehicle_inventory IS 'Daily vehicle availability';
COMMENT ON TABLE car_pricing IS 'Rental rates and insurance options';
COMMENT ON TABLE car_bookings IS 'Car rental reservations';