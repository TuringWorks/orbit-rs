-- ============================================================================
-- OrbitRS Travel Examples - Vacation Packages & Core Booking Schema
-- ============================================================================
-- Customers, packages, bookings, payments, loyalty programs
-- ============================================================================
-- Enable UUID extension (if not already enabled)
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- CUSTOMERS
-- ============================================================================
CREATE TABLE customers (
    customer_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_number VARCHAR(20) UNIQUE NOT NULL,
    -- Personal Information
    title VARCHAR(10),
    first_name VARCHAR(100) NOT NULL,
    middle_name VARCHAR(100),
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE,
    gender VARCHAR(10) CHECK (
        gender IN ('MALE', 'FEMALE', 'OTHER', 'PREFER_NOT_TO_SAY')
    ),
    -- Contact
    email VARCHAR(200) UNIQUE NOT NULL,
    phone VARCHAR(50),
    mobile_phone VARCHAR(50),
    -- Address
    address_line1 VARCHAR(200),
    address_line2 VARCHAR(200),
    city VARCHAR(100),
    state VARCHAR(100),
    country VARCHAR(100),
    postal_code VARCHAR(20),
    -- Travel Preferences
    preferred_airline VARCHAR(3),
    preferred_hotel_chain VARCHAR(10),
    preferred_car_company VARCHAR(10),
    seat_preference VARCHAR(30),
    meal_preference VARCHAR(50),
    -- Documents
    passport_number VARCHAR(50),
    passport_country VARCHAR(100),
    passport_expiry DATE,
    known_traveler_number VARCHAR(50),
    -- Account Status
    account_status VARCHAR(30) DEFAULT 'ACTIVE' CHECK (
        account_status IN ('ACTIVE', 'SUSPENDED', 'CLOSED')
    ),
    email_verified BOOLEAN DEFAULT FALSE,
    phone_verified BOOLEAN DEFAULT FALSE,
    -- Loyalty
    loyalty_tier VARCHAR(30) DEFAULT 'BRONZE' CHECK (
        loyalty_tier IN (
            'BRONZE',
            'SILVER',
            'GOLD',
            'PLATINUM',
            'DIAMOND'
        )
    ),
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    last_login_at TIMESTAMP
);
CREATE INDEX idx_customers_email ON customers(email);
CREATE INDEX idx_customers_number ON customers(customer_number);
CREATE INDEX idx_customers_name ON customers(last_name, first_name);
CREATE INDEX idx_customers_loyalty_tier ON customers(loyalty_tier);
-- ============================================================================
-- VACATION PACKAGES
-- ============================================================================
CREATE TABLE vacation_packages (
    package_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    package_code VARCHAR(20) UNIQUE NOT NULL,
    -- Package Details
    name VARCHAR(200) NOT NULL,
    description TEXT,
    destination VARCHAR(100) NOT NULL,
    -- Package Type
    package_type VARCHAR(30) CHECK (
        package_type IN (
            'FLIGHT_HOTEL',
            'FLIGHT_HOTEL_CAR',
            'ALL_INCLUSIVE',
            'CRUISE',
            'TOUR',
            'ADVENTURE',
            'BEACH',
            'CITY_BREAK'
        )
    ),
    -- Duration
    num_nights INTEGER NOT NULL,
    num_days INTEGER NOT NULL,
    -- Components Included
    includes_flight BOOLEAN DEFAULT TRUE,
    includes_hotel BOOLEAN DEFAULT TRUE,
    includes_car BOOLEAN DEFAULT FALSE,
    includes_activities BOOLEAN DEFAULT FALSE,
    includes_meals BOOLEAN DEFAULT FALSE,
    -- Pricing
    base_price_per_person DECIMAL(10, 2) NOT NULL,
    single_supplement DECIMAL(10, 2) DEFAULT 0,
    child_discount_percent DECIMAL(5, 2) DEFAULT 0,
    -- Availability
    available_from DATE,
    available_to DATE,
    min_travelers INTEGER DEFAULT 1,
    max_travelers INTEGER,
    -- Booking Window
    min_advance_booking_days INTEGER DEFAULT 7,
    max_advance_booking_days INTEGER DEFAULT 365,
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    is_featured BOOLEAN DEFAULT FALSE,
    popularity_score DECIMAL(5, 2) DEFAULT 0,
    -- Media
    image_url VARCHAR(500),
    thumbnail_url VARCHAR(500),
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_vacation_packages_code ON vacation_packages(package_code);
CREATE INDEX idx_vacation_packages_destination ON vacation_packages(destination);
CREATE INDEX idx_vacation_packages_type ON vacation_packages(package_type);
CREATE INDEX idx_vacation_packages_active ON vacation_packages(is_active);
-- ============================================================================
-- PACKAGE COMPONENTS
-- ============================================================================
CREATE TABLE package_components (
    component_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    package_id UUID NOT NULL REFERENCES vacation_packages(package_id) ON DELETE CASCADE,
    -- Component Type
    component_type VARCHAR(30) CHECK (
        component_type IN (
            'FLIGHT',
            'HOTEL',
            'CAR',
            'ACTIVITY',
            'TRANSFER',
            'MEAL'
        )
    ),
    component_name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Sequence
    day_number INTEGER,
    sequence_order INTEGER,
    -- Details (flexible JSON-like storage)
    details TEXT,
    -- Could store JSON
    -- Pricing
    included_in_base_price BOOLEAN DEFAULT TRUE,
    additional_cost DECIMAL(10, 2) DEFAULT 0,
    -- Status
    is_optional BOOLEAN DEFAULT FALSE,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_package_components_package ON package_components(package_id);
CREATE INDEX idx_package_components_type ON package_components(component_type);
-- ============================================================================
-- BOOKINGS (Master booking record)
-- ============================================================================
CREATE TABLE bookings (
    booking_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    booking_reference VARCHAR(20) UNIQUE NOT NULL,
    -- Customer
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    -- Booking Type
    booking_type VARCHAR(30) CHECK (
        booking_type IN (
            'FLIGHT_ONLY',
            'HOTEL_ONLY',
            'CAR_ONLY',
            'PACKAGE',
            'MULTI_COMPONENT'
        )
    ),
    -- Package (if applicable)
    package_id UUID REFERENCES vacation_packages(package_id),
    -- Travelers
    num_adults INTEGER NOT NULL DEFAULT 1,
    num_children INTEGER DEFAULT 0,
    num_infants INTEGER DEFAULT 0,
    -- Pricing
    subtotal DECIMAL(10, 2) NOT NULL,
    taxes_fees DECIMAL(10, 2) NOT NULL,
    discounts DECIMAL(10, 2) DEFAULT 0,
    total_price DECIMAL(10, 2) NOT NULL,
    -- Payment
    payment_status VARCHAR(30) DEFAULT 'PENDING' CHECK (
        payment_status IN (
            'PENDING',
            'PARTIAL',
            'PAID',
            'REFUNDED',
            'FAILED'
        )
    ),
    amount_paid DECIMAL(10, 2) DEFAULT 0,
    amount_due DECIMAL(10, 2),
    -- Booking Status
    booking_status VARCHAR(30) DEFAULT 'PENDING' CHECK (
        booking_status IN (
            'PENDING',
            'CONFIRMED',
            'IN_PROGRESS',
            'COMPLETED',
            'CANCELLED'
        )
    ),
    -- Loyalty
    loyalty_points_earned INTEGER DEFAULT 0,
    loyalty_points_redeemed INTEGER DEFAULT 0,
    -- Timestamps
    booked_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    travel_start_date DATE,
    travel_end_date DATE,
    cancelled_at TIMESTAMP,
    cancellation_reason TEXT,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_bookings_reference ON bookings(booking_reference);
CREATE INDEX idx_bookings_customer ON bookings(customer_id);
CREATE INDEX idx_bookings_type ON bookings(booking_type);
CREATE INDEX idx_bookings_status ON bookings(booking_status);
CREATE INDEX idx_bookings_travel_dates ON bookings(travel_start_date, travel_end_date);
-- ============================================================================
-- BOOKING COMPONENTS (links to specific flight/hotel/car bookings)
-- ============================================================================
CREATE TABLE booking_components (
    component_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    booking_id UUID NOT NULL REFERENCES bookings(booking_id) ON DELETE CASCADE,
    -- Component Type
    component_type VARCHAR(30) CHECK (
        component_type IN (
            'FLIGHT',
            'HOTEL',
            'CAR',
            'ACTIVITY',
            'INSURANCE'
        )
    ),
    -- Reference to specific booking
    flight_booking_id UUID,
    -- REFERENCES flight_bookings(booking_id)
    hotel_booking_id UUID,
    -- REFERENCES hotel_bookings(booking_id)
    car_booking_id UUID,
    -- REFERENCES car_bookings(booking_id)
    -- Component Details
    component_name VARCHAR(200),
    component_price DECIMAL(10, 2),
    -- Status
    component_status VARCHAR(30) DEFAULT 'CONFIRMED',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_booking_components_booking ON booking_components(booking_id);
CREATE INDEX idx_booking_components_type ON booking_components(component_type);
-- ============================================================================
-- PAYMENTS
-- ============================================================================
CREATE TABLE payments (
    payment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    booking_id UUID NOT NULL REFERENCES bookings(booking_id),
    -- Payment Details
    payment_reference VARCHAR(50) UNIQUE NOT NULL,
    payment_method VARCHAR(30) CHECK (
        payment_method IN (
            'CREDIT_CARD',
            'DEBIT_CARD',
            'PAYPAL',
            'BANK_TRANSFER',
            'POINTS'
        )
    ),
    -- Amount
    amount DECIMAL(10, 2) NOT NULL,
    currency VARCHAR(3) DEFAULT 'USD',
    -- Card Details (encrypted in production)
    card_last_four VARCHAR(4),
    card_type VARCHAR(20),
    -- Visa, Mastercard, Amex
    -- Payment Status
    payment_status VARCHAR(30) DEFAULT 'PENDING' CHECK (
        payment_status IN (
            'PENDING',
            'PROCESSING',
            'COMPLETED',
            'FAILED',
            'REFUNDED'
        )
    ),
    -- Transaction
    transaction_id VARCHAR(100),
    processor_response TEXT,
    -- Refund
    refund_amount DECIMAL(10, 2) DEFAULT 0,
    refunded_at TIMESTAMP,
    -- Timestamps
    processed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_payments_booking ON payments(booking_id);
CREATE INDEX idx_payments_reference ON payments(payment_reference);
CREATE INDEX idx_payments_status ON payments(payment_status);
-- ============================================================================
-- LOYALTY PROGRAMS
-- ============================================================================
CREATE TABLE loyalty_programs (
    program_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    program_code VARCHAR(20) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Tiers
    tier_names TEXT [],
    -- ['Bronze', 'Silver', 'Gold', 'Platinum']
    -- Points
    points_per_dollar DECIMAL(5, 2) DEFAULT 1.00,
    -- Redemption
    points_value_cents DECIMAL(5, 2) DEFAULT 1.00,
    -- 100 points = $1.00
    min_redemption_points INTEGER DEFAULT 1000,
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- ============================================================================
-- LOYALTY ACCOUNTS
-- ============================================================================
CREATE TABLE loyalty_accounts (
    account_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    program_id UUID NOT NULL REFERENCES loyalty_programs(program_id),
    -- Account Details
    membership_number VARCHAR(50) UNIQUE NOT NULL,
    current_tier VARCHAR(30) DEFAULT 'BRONZE',
    -- Points
    points_balance INTEGER DEFAULT 0,
    lifetime_points INTEGER DEFAULT 0,
    points_expiring_soon INTEGER DEFAULT 0,
    next_expiry_date DATE,
    -- Tier Progress
    tier_qualifying_points INTEGER DEFAULT 0,
    tier_qualifying_dollars DECIMAL(10, 2) DEFAULT 0,
    tier_year_start DATE,
    tier_year_end DATE,
    -- Status
    account_status VARCHAR(30) DEFAULT 'ACTIVE' CHECK (
        account_status IN ('ACTIVE', 'SUSPENDED', 'CLOSED')
    ),
    -- Timestamps
    enrolled_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(customer_id, program_id)
);
CREATE INDEX idx_loyalty_accounts_customer ON loyalty_accounts(customer_id);
CREATE INDEX idx_loyalty_accounts_membership ON loyalty_accounts(membership_number);
-- ============================================================================
-- LOYALTY TRANSACTIONS
-- ============================================================================
CREATE TABLE loyalty_transactions (
    transaction_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_id UUID NOT NULL REFERENCES loyalty_accounts(account_id),
    booking_id UUID REFERENCES bookings(booking_id),
    -- Transaction Type
    transaction_type VARCHAR(30) CHECK (
        transaction_type IN (
            'EARN',
            'REDEEM',
            'EXPIRE',
            'BONUS',
            'ADJUSTMENT'
        )
    ),
    -- Points
    points_amount INTEGER NOT NULL,
    points_balance_after INTEGER NOT NULL,
    -- Description
    description TEXT,
    -- Expiry
    expires_at DATE,
    -- Timestamps
    transaction_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_loyalty_transactions_account ON loyalty_transactions(account_id);
CREATE INDEX idx_loyalty_transactions_booking ON loyalty_transactions(booking_id);
CREATE INDEX idx_loyalty_transactions_date ON loyalty_transactions(transaction_date);
-- ============================================================================
-- BOOKING HISTORY (for analytics)
-- ============================================================================
CREATE TABLE booking_history (
    history_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    booking_id UUID NOT NULL REFERENCES bookings(booking_id),
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    -- Snapshot of booking at time of change
    booking_status VARCHAR(30),
    payment_status VARCHAR(30),
    total_price DECIMAL(10, 2),
    -- Change Details
    change_type VARCHAR(30) CHECK (
        change_type IN (
            'CREATED',
            'MODIFIED',
            'CANCELLED',
            'COMPLETED',
            'PAYMENT'
        )
    ),
    change_description TEXT,
    changed_by VARCHAR(100),
    -- User ID or system
    -- Timestamp
    changed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_booking_history_booking ON booking_history(booking_id);
CREATE INDEX idx_booking_history_customer ON booking_history(customer_id);
CREATE INDEX idx_booking_history_date ON booking_history(changed_at);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_customers_updated_at BEFORE
UPDATE ON customers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_vacation_packages_updated_at BEFORE
UPDATE ON vacation_packages FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_bookings_updated_at BEFORE
UPDATE ON bookings FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_loyalty_accounts_updated_at BEFORE
UPDATE ON loyalty_accounts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active vacation packages
CREATE VIEW v_active_packages AS
SELECT vp.package_id,
    vp.package_code,
    vp.name,
    vp.destination,
    vp.package_type,
    vp.num_nights,
    vp.num_days,
    vp.base_price_per_person,
    vp.is_featured,
    vp.popularity_score
FROM vacation_packages vp
WHERE vp.is_active = TRUE
    AND (
        vp.available_to IS NULL
        OR vp.available_to >= CURRENT_DATE
    )
ORDER BY vp.is_featured DESC,
    vp.popularity_score DESC;
-- Customer booking summary
CREATE VIEW v_customer_bookings AS
SELECT c.customer_id,
    c.first_name,
    c.last_name,
    c.email,
    COUNT(b.booking_id) AS total_bookings,
    SUM(
        CASE
            WHEN b.booking_status = 'COMPLETED' THEN 1
            ELSE 0
        END
    ) AS completed_bookings,
    SUM(b.total_price) AS total_spent,
    MAX(b.booked_at) AS last_booking_date
FROM customers c
    LEFT JOIN bookings b ON c.customer_id = b.customer_id
GROUP BY c.customer_id,
    c.first_name,
    c.last_name,
    c.email;
-- ============================================================================
-- SAMPLE DATA
-- ============================================================================
-- Insert sample loyalty program
INSERT INTO loyalty_programs (
        program_code,
        name,
        description,
        tier_names,
        points_per_dollar
    )
VALUES (
        'TRAVEL_REWARDS',
        'Travel Rewards Program',
        'Earn points on every booking',
        ARRAY ['Bronze', 'Silver', 'Gold', 'Platinum', 'Diamond'],
        1.00
    );
-- Insert sample vacation packages
INSERT INTO vacation_packages (
        package_code,
        name,
        description,
        destination,
        package_type,
        num_nights,
        num_days,
        includes_flight,
        includes_hotel,
        includes_car,
        base_price_per_person,
        is_featured
    )
VALUES (
        'PKG-HAWAII-001',
        'Hawaiian Paradise',
        'Experience the beauty of Hawaii with flights, hotel, and car rental',
        'Honolulu, Hawaii',
        'FLIGHT_HOTEL_CAR',
        7,
        8,
        TRUE,
        TRUE,
        TRUE,
        1299.00,
        TRUE
    ),
    (
        'PKG-PARIS-001',
        'Paris City Break',
        'Romantic getaway to Paris with flights and luxury hotel',
        'Paris, France',
        'FLIGHT_HOTEL',
        5,
        6,
        TRUE,
        TRUE,
        FALSE,
        1599.00,
        TRUE
    ),
    (
        'PKG-CANCUN-001',
        'Cancun All-Inclusive',
        'All-inclusive beach resort in Cancun',
        'Cancun, Mexico',
        'ALL_INCLUSIVE',
        7,
        8,
        TRUE,
        TRUE,
        FALSE,
        1899.00,
        TRUE
    );
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE customers IS 'Customer profiles and preferences';
COMMENT ON TABLE vacation_packages IS 'Pre-packaged travel deals';
COMMENT ON TABLE package_components IS 'Components included in vacation packages';
COMMENT ON TABLE bookings IS 'Master booking records for all travel types';
COMMENT ON TABLE booking_components IS 'Individual components of a booking';
COMMENT ON TABLE payments IS 'Payment transactions and processing';
COMMENT ON TABLE loyalty_programs IS 'Loyalty reward program definitions';
COMMENT ON TABLE loyalty_accounts IS 'Customer loyalty accounts and points';
COMMENT ON TABLE loyalty_transactions IS 'Points earning and redemption history';
COMMENT ON TABLE booking_history IS 'Audit trail of booking changes';