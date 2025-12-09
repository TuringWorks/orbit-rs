-- ============================================================================
-- OrbitRS Travel Examples - Flight Booking Schema
-- ============================================================================
-- Airlines, airports, flights, inventory, pricing, bookings, passengers
-- ============================================================================
-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- AIRLINES
-- ============================================================================
CREATE TABLE airlines (
    airline_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    airline_code VARCHAR(3) UNIQUE NOT NULL,
    -- IATA code (e.g., 'AA', 'DL', 'UA')
    icao_code VARCHAR(4) UNIQUE,
    -- ICAO code (e.g., 'AAL', 'DAL', 'UAL')
    name VARCHAR(200) NOT NULL,
    country VARCHAR(100),
    -- Alliance
    alliance VARCHAR(50) CHECK (
        alliance IN ('STAR_ALLIANCE', 'ONEWORLD', 'SKYTEAM', 'NONE')
    ),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    is_low_cost BOOLEAN DEFAULT FALSE,
    -- Contact
    website VARCHAR(500),
    phone VARCHAR(50),
    -- Metadata
    logo_url VARCHAR(500),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_airlines_code ON airlines(airline_code);
CREATE INDEX idx_airlines_alliance ON airlines(alliance);
-- ============================================================================
-- AIRPORTS
-- ============================================================================
CREATE TABLE airports (
    airport_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    airport_code VARCHAR(3) UNIQUE NOT NULL,
    -- IATA code (e.g., 'SFO', 'JFK', 'LAX')
    icao_code VARCHAR(4) UNIQUE,
    -- ICAO code (e.g., 'KSFO', 'KJFK', 'KLAX')
    name VARCHAR(200) NOT NULL,
    -- Location
    city VARCHAR(100) NOT NULL,
    state VARCHAR(100),
    country VARCHAR(100) NOT NULL,
    latitude DECIMAL(10, 7),
    longitude DECIMAL(11, 7),
    elevation_ft INTEGER,
    -- Time Zone
    timezone VARCHAR(50),
    -- e.g., 'America/Los_Angeles'
    utc_offset_hours DECIMAL(4, 2),
    -- Airport Type
    airport_type VARCHAR(30) CHECK (
        airport_type IN (
            'INTERNATIONAL',
            'DOMESTIC',
            'REGIONAL',
            'PRIVATE'
        )
    ),
    -- Facilities
    num_terminals INTEGER,
    num_runways INTEGER,
    has_customs BOOLEAN DEFAULT FALSE,
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_airports_code ON airports(airport_code);
CREATE INDEX idx_airports_city ON airports(city);
CREATE INDEX idx_airports_country ON airports(country);
-- ============================================================================
-- AIRCRAFT
-- ============================================================================
CREATE TABLE aircraft (
    aircraft_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    aircraft_code VARCHAR(10) UNIQUE NOT NULL,
    -- e.g., '737', 'A320', '787'
    manufacturer VARCHAR(100),
    -- Boeing, Airbus, etc.
    model VARCHAR(100),
    -- Capacity
    total_seats INTEGER NOT NULL,
    first_class_seats INTEGER DEFAULT 0,
    business_class_seats INTEGER DEFAULT 0,
    premium_economy_seats INTEGER DEFAULT 0,
    economy_seats INTEGER DEFAULT 0,
    -- Specifications
    max_range_miles INTEGER,
    cruise_speed_mph INTEGER,
    -- Features
    has_wifi BOOLEAN DEFAULT FALSE,
    has_entertainment BOOLEAN DEFAULT FALSE,
    has_power_outlets BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_aircraft_code ON aircraft(aircraft_code);
-- ============================================================================
-- FLIGHTS
-- ============================================================================
CREATE TABLE flights (
    flight_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    flight_number VARCHAR(10) NOT NULL,
    -- e.g., 'AA123', 'DL456'
    airline_id UUID NOT NULL REFERENCES airlines(airline_id),
    aircraft_id UUID REFERENCES aircraft(aircraft_id),
    -- Route
    origin_airport_id UUID NOT NULL REFERENCES airports(airport_id),
    destination_airport_id UUID NOT NULL REFERENCES airports(airport_id),
    -- Schedule
    departure_time TIME NOT NULL,
    arrival_time TIME NOT NULL,
    flight_duration_minutes INTEGER,
    -- Days of Operation (bit flags: Mon=1, Tue=2, Wed=4, Thu=8, Fri=16, Sat=32, Sun=64)
    operates_on_days INTEGER DEFAULT 127,
    -- All days by default
    -- Effective Dates
    effective_from DATE NOT NULL,
    effective_to DATE,
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    is_seasonal BOOLEAN DEFAULT FALSE,
    -- Service Class
    service_class VARCHAR(30) CHECK (
        service_class IN ('DOMESTIC', 'INTERNATIONAL', 'REGIONAL')
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(flight_number, departure_time)
);
CREATE INDEX idx_flights_number ON flights(flight_number);
CREATE INDEX idx_flights_airline ON flights(airline_id);
CREATE INDEX idx_flights_origin ON flights(origin_airport_id);
CREATE INDEX idx_flights_destination ON flights(destination_airport_id);
CREATE INDEX idx_flights_route ON flights(origin_airport_id, destination_airport_id);
-- ============================================================================
-- FLIGHT SEGMENTS (for multi-leg flights)
-- ============================================================================
CREATE TABLE flight_segments (
    segment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    parent_flight_id UUID REFERENCES flights(flight_id),
    segment_number INTEGER NOT NULL,
    flight_id UUID NOT NULL REFERENCES flights(flight_id),
    -- Layover
    layover_duration_minutes INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(parent_flight_id, segment_number)
);
CREATE INDEX idx_flight_segments_parent ON flight_segments(parent_flight_id);
-- ============================================================================
-- FLIGHT INVENTORY (seat availability by date)
-- ============================================================================
CREATE TABLE flight_inventory (
    inventory_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    flight_id UUID NOT NULL REFERENCES flights(flight_id),
    flight_date DATE NOT NULL,
    -- Seat Availability by Class
    first_class_available INTEGER DEFAULT 0,
    business_class_available INTEGER DEFAULT 0,
    premium_economy_available INTEGER DEFAULT 0,
    economy_available INTEGER DEFAULT 0,
    total_available INTEGER DEFAULT 0,
    -- Booking Status
    total_booked INTEGER DEFAULT 0,
    is_oversold BOOLEAN DEFAULT FALSE,
    -- Flight Status
    status VARCHAR(30) DEFAULT 'SCHEDULED' CHECK (
        status IN (
            'SCHEDULED',
            'BOARDING',
            'DEPARTED',
            'ARRIVED',
            'DELAYED',
            'CANCELLED'
        )
    ),
    -- Actual Times (if different from schedule)
    actual_departure_time TIMESTAMP,
    actual_arrival_time TIMESTAMP,
    delay_minutes INTEGER DEFAULT 0,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(flight_id, flight_date)
);
CREATE INDEX idx_flight_inventory_flight ON flight_inventory(flight_id);
CREATE INDEX idx_flight_inventory_date ON flight_inventory(flight_date);
CREATE INDEX idx_flight_inventory_status ON flight_inventory(status);
-- ============================================================================
-- FLIGHT PRICING (dynamic pricing by class and date)
-- ============================================================================
CREATE TABLE flight_pricing (
    pricing_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    flight_id UUID NOT NULL REFERENCES flights(flight_id),
    flight_date DATE NOT NULL,
    -- Fare Class
    fare_class VARCHAR(30) NOT NULL CHECK (
        fare_class IN (
            'FIRST',
            'BUSINESS',
            'PREMIUM_ECONOMY',
            'ECONOMY'
        )
    ),
    -- Pricing
    base_price DECIMAL(10, 2) NOT NULL,
    taxes_fees DECIMAL(10, 2) DEFAULT 0,
    total_price DECIMAL(10, 2) NOT NULL,
    -- Fare Type
    fare_type VARCHAR(30) CHECK (
        fare_type IN ('BASIC', 'STANDARD', 'FLEXIBLE', 'REFUNDABLE')
    ),
    -- Restrictions
    is_refundable BOOLEAN DEFAULT FALSE,
    is_changeable BOOLEAN DEFAULT TRUE,
    change_fee DECIMAL(10, 2) DEFAULT 0,
    -- Baggage
    checked_bags_included INTEGER DEFAULT 0,
    carry_on_included BOOLEAN DEFAULT TRUE,
    -- Validity
    valid_from TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    valid_to TIMESTAMP,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(flight_id, flight_date, fare_class, fare_type)
);
CREATE INDEX idx_flight_pricing_flight ON flight_pricing(flight_id);
CREATE INDEX idx_flight_pricing_date ON flight_pricing(flight_date);
CREATE INDEX idx_flight_pricing_class ON flight_pricing(fare_class);
-- ============================================================================
-- FLIGHT BOOKINGS
-- ============================================================================
CREATE TABLE flight_bookings (
    booking_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    booking_reference VARCHAR(10) UNIQUE NOT NULL,
    -- e.g., 'ABC123'
    -- Flight Details
    flight_id UUID NOT NULL REFERENCES flights(flight_id),
    flight_date DATE NOT NULL,
    inventory_id UUID REFERENCES flight_inventory(inventory_id),
    -- Customer
    customer_id UUID NOT NULL,
    -- Booking Details
    num_passengers INTEGER NOT NULL DEFAULT 1,
    fare_class VARCHAR(30) NOT NULL,
    fare_type VARCHAR(30) NOT NULL,
    -- Pricing
    base_price DECIMAL(10, 2) NOT NULL,
    taxes_fees DECIMAL(10, 2) NOT NULL,
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
            'CHECKED_IN',
            'BOARDED',
            'COMPLETED',
            'CANCELLED'
        )
    ),
    -- Loyalty
    loyalty_points_earned INTEGER DEFAULT 0,
    loyalty_points_redeemed INTEGER DEFAULT 0,
    -- Timestamps
    booked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    cancelled_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_flight_bookings_reference ON flight_bookings(booking_reference);
CREATE INDEX idx_flight_bookings_customer ON flight_bookings(customer_id);
CREATE INDEX idx_flight_bookings_flight ON flight_bookings(flight_id, flight_date);
CREATE INDEX idx_flight_bookings_status ON flight_bookings(booking_status);
-- ============================================================================
-- PASSENGERS
-- ============================================================================
CREATE TABLE passengers (
    passenger_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    booking_id UUID NOT NULL REFERENCES flight_bookings(booking_id) ON DELETE CASCADE,
    -- Personal Information
    title VARCHAR(10),
    -- Mr, Mrs, Ms, Dr
    first_name VARCHAR(100) NOT NULL,
    middle_name VARCHAR(100),
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE,
    gender VARCHAR(10) CHECK (
        gender IN ('MALE', 'FEMALE', 'OTHER', 'PREFER_NOT_TO_SAY')
    ),
    -- Contact
    email VARCHAR(200),
    phone VARCHAR(50),
    -- Travel Documents
    passport_number VARCHAR(50),
    passport_country VARCHAR(100),
    passport_expiry DATE,
    known_traveler_number VARCHAR(50),
    -- TSA PreCheck, Global Entry
    -- Seat Assignment
    seat_number VARCHAR(10),
    -- e.g., '12A', '23F'
    seat_preference VARCHAR(30) CHECK (
        seat_preference IN ('WINDOW', 'AISLE', 'MIDDLE', 'NO_PREFERENCE')
    ),
    -- Special Requests
    meal_preference VARCHAR(50),
    special_assistance TEXT,
    -- Frequent Flyer
    frequent_flyer_number VARCHAR(50),
    -- Check-in
    is_checked_in BOOLEAN DEFAULT FALSE,
    checked_in_at TIMESTAMP,
    boarding_pass_number VARCHAR(50),
    -- Baggage
    checked_bags INTEGER DEFAULT 0,
    carry_on_bags INTEGER DEFAULT 1,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_passengers_booking ON passengers(booking_id);
CREATE INDEX idx_passengers_name ON passengers(last_name, first_name);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = CURRENT_TIMESTAMP;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER update_airlines_updated_at BEFORE
UPDATE ON airlines FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_airports_updated_at BEFORE
UPDATE ON airports FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_flights_updated_at BEFORE
UPDATE ON flights FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_flight_inventory_updated_at BEFORE
UPDATE ON flight_inventory FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_flight_pricing_updated_at BEFORE
UPDATE ON flight_pricing FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_flight_bookings_updated_at BEFORE
UPDATE ON flight_bookings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_passengers_updated_at BEFORE
UPDATE ON passengers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active flights with route information
CREATE VIEW v_active_flights AS
SELECT f.flight_id,
    f.flight_number,
    a.airline_code,
    a.name AS airline_name,
    orig.airport_code AS origin_code,
    orig.city AS origin_city,
    dest.airport_code AS destination_code,
    dest.city AS destination_city,
    f.departure_time,
    f.arrival_time,
    f.flight_duration_minutes,
    ac.aircraft_code,
    ac.total_seats
FROM flights f
    JOIN airlines a ON f.airline_id = a.airline_id
    JOIN airports orig ON f.origin_airport_id = orig.airport_id
    JOIN airports dest ON f.destination_airport_id = dest.airport_id
    LEFT JOIN aircraft ac ON f.aircraft_id = ac.aircraft_id
WHERE f.is_active = TRUE
    AND a.is_active = TRUE
    AND orig.is_active = TRUE
    AND dest.is_active = TRUE
ORDER BY f.flight_number;
-- Flight availability by date
CREATE VIEW v_flight_availability AS
SELECT fi.inventory_id,
    f.flight_number,
    a.airline_code,
    orig.airport_code AS origin,
    dest.airport_code AS destination,
    fi.flight_date,
    f.departure_time,
    f.arrival_time,
    fi.economy_available,
    fi.premium_economy_available,
    fi.business_class_available,
    fi.first_class_available,
    fi.total_available,
    fi.status
FROM flight_inventory fi
    JOIN flights f ON fi.flight_id = f.flight_id
    JOIN airlines a ON f.airline_id = a.airline_id
    JOIN airports orig ON f.origin_airport_id = orig.airport_id
    JOIN airports dest ON f.destination_airport_id = dest.airport_id
WHERE fi.status = 'SCHEDULED'
    AND fi.total_available > 0
ORDER BY fi.flight_date,
    f.departure_time;
-- ============================================================================
-- SAMPLE DATA
-- ============================================================================
-- Insert sample airlines
INSERT INTO airlines (
        airline_code,
        icao_code,
        name,
        country,
        alliance,
        is_low_cost
    )
VALUES (
        'AA',
        'AAL',
        'American Airlines',
        'United States',
        'ONEWORLD',
        FALSE
    ),
    (
        'DL',
        'DAL',
        'Delta Air Lines',
        'United States',
        'SKYTEAM',
        FALSE
    ),
    (
        'UA',
        'UAL',
        'United Airlines',
        'United States',
        'STAR_ALLIANCE',
        FALSE
    ),
    (
        'WN',
        'SWA',
        'Southwest Airlines',
        'United States',
        'NONE',
        TRUE
    ),
    (
        'B6',
        'JBU',
        'JetBlue Airways',
        'United States',
        'NONE',
        TRUE
    ),
    (
        'AS',
        'ASA',
        'Alaska Airlines',
        'United States',
        'ONEWORLD',
        FALSE
    );
-- Insert sample airports
INSERT INTO airports (
        airport_code,
        icao_code,
        name,
        city,
        state,
        country,
        timezone,
        airport_type,
        has_customs
    )
VALUES (
        'SFO',
        'KSFO',
        'San Francisco International Airport',
        'San Francisco',
        'California',
        'United States',
        'America/Los_Angeles',
        'INTERNATIONAL',
        TRUE
    ),
    (
        'LAX',
        'KLAX',
        'Los Angeles International Airport',
        'Los Angeles',
        'California',
        'United States',
        'America/Los_Angeles',
        'INTERNATIONAL',
        TRUE
    ),
    (
        'JFK',
        'KJFK',
        'John F. Kennedy International Airport',
        'New York',
        'New York',
        'United States',
        'America/New_York',
        'INTERNATIONAL',
        TRUE
    ),
    (
        'ORD',
        'KORD',
        'O''Hare International Airport',
        'Chicago',
        'Illinois',
        'United States',
        'America/Chicago',
        'INTERNATIONAL',
        TRUE
    ),
    (
        'DFW',
        'KDFW',
        'Dallas/Fort Worth International Airport',
        'Dallas',
        'Texas',
        'United States',
        'America/Chicago',
        'INTERNATIONAL',
        TRUE
    ),
    (
        'SEA',
        'KSEA',
        'Seattle-Tacoma International Airport',
        'Seattle',
        'Washington',
        'United States',
        'America/Los_Angeles',
        'INTERNATIONAL',
        TRUE
    ),
    (
        'MIA',
        'KMIA',
        'Miami International Airport',
        'Miami',
        'Florida',
        'United States',
        'America/New_York',
        'INTERNATIONAL',
        TRUE
    ),
    (
        'LAS',
        'KLAS',
        'Harry Reid International Airport',
        'Las Vegas',
        'Nevada',
        'United States',
        'America/Los_Angeles',
        'DOMESTIC',
        FALSE
    );
-- Insert sample aircraft
INSERT INTO aircraft (
        aircraft_code,
        manufacturer,
        model,
        total_seats,
        first_class_seats,
        business_class_seats,
        premium_economy_seats,
        economy_seats,
        has_wifi,
        has_entertainment
    )
VALUES (
        '737',
        'Boeing',
        '737-800',
        175,
        12,
        30,
        0,
        133,
        TRUE,
        TRUE
    ),
    (
        'A320',
        'Airbus',
        'A320-200',
        180,
        16,
        36,
        0,
        128,
        TRUE,
        TRUE
    ),
    (
        '787',
        'Boeing',
        '787-9 Dreamliner',
        290,
        30,
        48,
        28,
        184,
        TRUE,
        TRUE
    ),
    (
        'A350',
        'Airbus',
        'A350-900',
        325,
        36,
        56,
        32,
        201,
        TRUE,
        TRUE
    );
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE airlines IS 'Airline carriers and metadata';
COMMENT ON TABLE airports IS 'Airport information with location and timezone data';
COMMENT ON TABLE flights IS 'Flight schedules and routes';
COMMENT ON TABLE flight_inventory IS 'Daily flight inventory and seat availability';
COMMENT ON TABLE flight_pricing IS 'Dynamic pricing by fare class and date';
COMMENT ON TABLE flight_bookings IS 'Customer flight reservations';
COMMENT ON TABLE passengers IS 'Passenger information and travel documents';