-- ============================================================================
-- OrbitRS Travel Examples - Hotel Reservation Schema
-- ============================================================================
-- Hotel chains, properties, rooms, inventory, pricing, bookings, amenities
-- ============================================================================
-- Enable UUID extension (if not already enabled)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- HOTEL CHAINS
-- ============================================================================
CREATE TABLE hotel_chains (
    chain_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    chain_code VARCHAR(10) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    -- Chain Type
    chain_type VARCHAR(30) CHECK (
        chain_type IN (
            'LUXURY',
            'UPSCALE',
            'MIDSCALE',
            'ECONOMY',
            'BOUTIQUE',
            'RESORT'
        )
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
CREATE INDEX idx_hotel_chains_code ON hotel_chains(chain_code);
-- ============================================================================
-- HOTELS
-- ============================================================================
CREATE TABLE hotels (
    hotel_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    hotel_code VARCHAR(20) UNIQUE NOT NULL,
    chain_id UUID REFERENCES hotel_chains(chain_id),
    -- Basic Info
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Location
    address_line1 VARCHAR(200),
    address_line2 VARCHAR(200),
    city VARCHAR(100) NOT NULL,
    state VARCHAR(100),
    country VARCHAR(100) NOT NULL,
    postal_code VARCHAR(20),
    latitude DECIMAL(10, 7),
    longitude DECIMAL(11, 7),
    -- Property Details
    star_rating DECIMAL(2, 1) CHECK (
        star_rating BETWEEN 1 AND 5
    ),
    total_rooms INTEGER NOT NULL,
    num_floors INTEGER,
    year_built INTEGER,
    year_renovated INTEGER,
    -- Check-in/out
    check_in_time TIME DEFAULT '15:00:00',
    check_out_time TIME DEFAULT '11:00:00',
    -- Contact
    phone VARCHAR(50),
    email VARCHAR(200),
    website VARCHAR(500),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    -- Ratings
    average_rating DECIMAL(3, 2) DEFAULT 0,
    total_reviews INTEGER DEFAULT 0,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_hotels_code ON hotels(hotel_code);
CREATE INDEX idx_hotels_chain ON hotels(chain_id);
CREATE INDEX idx_hotels_city ON hotels(city);
CREATE INDEX idx_hotels_rating ON hotels(star_rating);
-- ============================================================================
-- HOTEL AMENITIES
-- ============================================================================
CREATE TABLE hotel_amenities (
    amenity_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    hotel_id UUID NOT NULL REFERENCES hotels(hotel_id) ON DELETE CASCADE,
    -- Amenity Type
    amenity_type VARCHAR(50) CHECK (
        amenity_type IN (
            'POOL',
            'GYM',
            'SPA',
            'RESTAURANT',
            'BAR',
            'PARKING',
            'WIFI',
            'BUSINESS_CENTER',
            'CONCIERGE',
            'ROOM_SERVICE',
            'AIRPORT_SHUTTLE',
            'PET_FRIENDLY',
            'LAUNDRY',
            'VALET'
        )
    ),
    name VARCHAR(100) NOT NULL,
    description TEXT,
    -- Pricing
    is_complimentary BOOLEAN DEFAULT TRUE,
    additional_fee DECIMAL(10, 2) DEFAULT 0,
    -- Availability
    is_available BOOLEAN DEFAULT TRUE,
    hours_of_operation VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_hotel_amenities_hotel ON hotel_amenities(hotel_id);
CREATE INDEX idx_hotel_amenities_type ON hotel_amenities(amenity_type);
-- ============================================================================
-- ROOM TYPES
-- ============================================================================
CREATE TABLE room_types (
    room_type_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    hotel_id UUID NOT NULL REFERENCES hotels(hotel_id) ON DELETE CASCADE,
    room_type_code VARCHAR(20) NOT NULL,
    -- Room Details
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Capacity
    max_occupancy INTEGER NOT NULL,
    num_beds INTEGER DEFAULT 1,
    bed_type VARCHAR(50),
    -- King, Queen, Double, Twin
    -- Size
    size_sqft INTEGER,
    -- Features
    has_view BOOLEAN DEFAULT FALSE,
    view_type VARCHAR(50),
    -- Ocean, City, Garden, Mountain
    has_balcony BOOLEAN DEFAULT FALSE,
    has_kitchen BOOLEAN DEFAULT FALSE,
    has_living_room BOOLEAN DEFAULT FALSE,
    -- Amenities
    has_wifi BOOLEAN DEFAULT TRUE,
    has_tv BOOLEAN DEFAULT TRUE,
    has_minibar BOOLEAN DEFAULT FALSE,
    has_safe BOOLEAN DEFAULT TRUE,
    has_coffee_maker BOOLEAN DEFAULT TRUE,
    -- Accessibility
    is_accessible BOOLEAN DEFAULT FALSE,
    -- Smoking
    is_smoking_allowed BOOLEAN DEFAULT FALSE,
    -- Pricing
    base_price DECIMAL(10, 2) NOT NULL,
    -- Display
    image_url VARCHAR(500),
    display_order INTEGER DEFAULT 0,
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(hotel_id, room_type_code)
);
CREATE INDEX idx_room_types_hotel ON room_types(hotel_id);
CREATE INDEX idx_room_types_active ON room_types(is_active);
-- ============================================================================
-- ROOM INVENTORY (availability by date)
-- ============================================================================
CREATE TABLE room_inventory (
    inventory_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    room_type_id UUID NOT NULL REFERENCES room_types(room_type_id),
    stay_date DATE NOT NULL,
    -- Availability
    total_rooms INTEGER NOT NULL,
    available_rooms INTEGER NOT NULL,
    booked_rooms INTEGER DEFAULT 0,
    blocked_rooms INTEGER DEFAULT 0,
    -- For maintenance, etc.
    -- Status
    is_available BOOLEAN DEFAULT TRUE,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(room_type_id, stay_date)
);
CREATE INDEX idx_room_inventory_room_type ON room_inventory(room_type_id);
CREATE INDEX idx_room_inventory_date ON room_inventory(stay_date);
CREATE INDEX idx_room_inventory_available ON room_inventory(is_available);
-- ============================================================================
-- HOTEL PRICING (dynamic pricing by date)
-- ============================================================================
CREATE TABLE hotel_pricing (
    pricing_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    room_type_id UUID NOT NULL REFERENCES room_types(room_type_id),
    stay_date DATE NOT NULL,
    -- Pricing
    base_rate DECIMAL(10, 2) NOT NULL,
    weekend_rate DECIMAL(10, 2),
    -- Rate Type
    rate_type VARCHAR(30) CHECK (
        rate_type IN (
            'STANDARD',
            'ADVANCE_PURCHASE',
            'MEMBER',
            'CORPORATE',
            'GOVERNMENT'
        )
    ),
    -- Restrictions
    min_nights INTEGER DEFAULT 1,
    max_nights INTEGER,
    is_refundable BOOLEAN DEFAULT TRUE,
    cancellation_deadline_hours INTEGER DEFAULT 24,
    -- Validity
    valid_from TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    valid_to TIMESTAMP,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(room_type_id, stay_date, rate_type)
);
CREATE INDEX idx_hotel_pricing_room_type ON hotel_pricing(room_type_id);
CREATE INDEX idx_hotel_pricing_date ON hotel_pricing(stay_date);
-- ============================================================================
-- HOTEL BOOKINGS
-- ============================================================================
CREATE TABLE hotel_bookings (
    booking_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    confirmation_number VARCHAR(20) UNIQUE NOT NULL,
    -- Hotel Details
    hotel_id UUID NOT NULL REFERENCES hotels(hotel_id),
    room_type_id UUID NOT NULL REFERENCES room_types(room_type_id),
    -- Customer
    customer_id UUID NOT NULL,
    -- Stay Details
    check_in_date DATE NOT NULL,
    check_out_date DATE NOT NULL,
    num_nights INTEGER NOT NULL,
    num_adults INTEGER NOT NULL DEFAULT 1,
    num_children INTEGER DEFAULT 0,
    -- Pricing
    nightly_rate DECIMAL(10, 2) NOT NULL,
    subtotal DECIMAL(10, 2) NOT NULL,
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
            'CHECKED_OUT',
            'CANCELLED',
            'NO_SHOW'
        )
    ),
    -- Special Requests
    special_requests TEXT,
    early_check_in BOOLEAN DEFAULT FALSE,
    late_check_out BOOLEAN DEFAULT FALSE,
    -- Room Assignment
    room_number VARCHAR(10),
    -- Loyalty
    loyalty_points_earned INTEGER DEFAULT 0,
    loyalty_points_redeemed INTEGER DEFAULT 0,
    -- Timestamps
    booked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    checked_in_at TIMESTAMP,
    checked_out_at TIMESTAMP,
    cancelled_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_hotel_bookings_confirmation ON hotel_bookings(confirmation_number);
CREATE INDEX idx_hotel_bookings_customer ON hotel_bookings(customer_id);
CREATE INDEX idx_hotel_bookings_hotel ON hotel_bookings(hotel_id);
CREATE INDEX idx_hotel_bookings_dates ON hotel_bookings(check_in_date, check_out_date);
CREATE INDEX idx_hotel_bookings_status ON hotel_bookings(booking_status);
-- ============================================================================
-- HOTEL POLICIES
-- ============================================================================
CREATE TABLE hotel_policies (
    policy_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    hotel_id UUID NOT NULL REFERENCES hotels(hotel_id) ON DELETE CASCADE,
    -- Policy Type
    policy_type VARCHAR(50) CHECK (
        policy_type IN (
            'CANCELLATION',
            'DEPOSIT',
            'PAYMENT',
            'PET',
            'SMOKING',
            'AGE_REQUIREMENT'
        )
    ),
    policy_name VARCHAR(200) NOT NULL,
    policy_text TEXT NOT NULL,
    -- Enforcement
    is_mandatory BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_hotel_policies_hotel ON hotel_policies(hotel_id);
CREATE INDEX idx_hotel_policies_type ON hotel_policies(policy_type);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_hotel_chains_updated_at BEFORE
UPDATE ON hotel_chains FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_hotels_updated_at BEFORE
UPDATE ON hotels FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_room_types_updated_at BEFORE
UPDATE ON room_types FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_room_inventory_updated_at BEFORE
UPDATE ON room_inventory FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_hotel_pricing_updated_at BEFORE
UPDATE ON hotel_pricing FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_hotel_bookings_updated_at BEFORE
UPDATE ON hotel_bookings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_hotel_policies_updated_at BEFORE
UPDATE ON hotel_policies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active hotels with details
CREATE VIEW v_active_hotels AS
SELECT h.hotel_id,
    h.hotel_code,
    h.name,
    h.city,
    h.state,
    h.country,
    hc.name AS chain_name,
    h.star_rating,
    h.total_rooms,
    h.average_rating,
    h.total_reviews
FROM hotels h
    LEFT JOIN hotel_chains hc ON h.chain_id = hc.chain_id
WHERE h.is_active = TRUE
ORDER BY h.city,
    h.name;
-- Room availability by date
CREATE VIEW v_room_availability AS
SELECT h.hotel_id,
    h.name AS hotel_name,
    h.city,
    rt.room_type_id,
    rt.name AS room_type,
    ri.stay_date,
    ri.available_rooms,
    hp.base_rate,
    hp.rate_type
FROM room_inventory ri
    JOIN room_types rt ON ri.room_type_id = rt.room_type_id
    JOIN hotels h ON rt.hotel_id = h.hotel_id
    LEFT JOIN hotel_pricing hp ON rt.room_type_id = hp.room_type_id
    AND ri.stay_date = hp.stay_date
    AND hp.rate_type = 'STANDARD'
WHERE ri.is_available = TRUE
    AND ri.available_rooms > 0
    AND h.is_active = TRUE
ORDER BY h.city,
    h.name,
    ri.stay_date;
-- ============================================================================
-- SAMPLE DATA
-- ============================================================================
-- Insert sample hotel chains
INSERT INTO hotel_chains (
        chain_code,
        name,
        chain_type,
        loyalty_program_name
    )
VALUES (
        'MAR',
        'Marriott International',
        'UPSCALE',
        'Marriott Bonvoy'
    ),
    (
        'HLT',
        'Hilton Hotels',
        'UPSCALE',
        'Hilton Honors'
    ),
    (
        'IHG',
        'InterContinental Hotels Group',
        'UPSCALE',
        'IHG One Rewards'
    ),
    (
        'HYT',
        'Hyatt Hotels',
        'LUXURY',
        'World of Hyatt'
    ),
    (
        'ACC',
        'Accor Hotels',
        'MIDSCALE',
        'ALL - Accor Live Limitless'
    );
-- Insert sample hotels
INSERT INTO hotels (
        hotel_code,
        chain_id,
        name,
        city,
        state,
        country,
        star_rating,
        total_rooms,
        phone
    )
VALUES (
        'MAR-SFO-001',
        (
            SELECT chain_id
            FROM hotel_chains
            WHERE chain_code = 'MAR'
        ),
        'San Francisco Marriott Marquis',
        'San Francisco',
        'California',
        'United States',
        4.5,
        1500,
        '+1-415-896-1600'
    ),
    (
        'HLT-NYC-001',
        (
            SELECT chain_id
            FROM hotel_chains
            WHERE chain_code = 'HLT'
        ),
        'New York Hilton Midtown',
        'New York',
        'New York',
        'United States',
        4.0,
        1878,
        '+1-212-586-7000'
    ),
    (
        'HYT-CHI-001',
        (
            SELECT chain_id
            FROM hotel_chains
            WHERE chain_code = 'HYT'
        ),
        'Hyatt Regency Chicago',
        'Chicago',
        'Illinois',
        'United States',
        4.5,
        2019,
        '+1-312-565-1234'
    ),
    (
        'IHG-MIA-001',
        (
            SELECT chain_id
            FROM hotel_chains
            WHERE chain_code = 'IHG'
        ),
        'InterContinental Miami',
        'Miami',
        'Florida',
        'United States',
        5.0,
        641,
        '+1-305-577-1000'
    );
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE hotel_chains IS 'Hotel brands and chains';
COMMENT ON TABLE hotels IS 'Hotel properties with location and amenities';
COMMENT ON TABLE room_types IS 'Room categories and features';
COMMENT ON TABLE room_inventory IS 'Daily room availability';
COMMENT ON TABLE hotel_pricing IS 'Dynamic room pricing by date';
COMMENT ON TABLE hotel_bookings IS 'Hotel reservations and confirmations';
COMMENT ON TABLE hotel_amenities IS 'Hotel facilities and services';
COMMENT ON TABLE hotel_policies IS 'Hotel policies and restrictions';