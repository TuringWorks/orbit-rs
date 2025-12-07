-- ============================================================================
-- OrbitRS Retail Examples - Customers & Loyalty Schema
-- ============================================================================
-- Customer profiles, segments, loyalty programs, reward points
-- ============================================================================
-- ============================================================================
-- CUSTOMERS
-- ============================================================================
CREATE TABLE customers (
    customer_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_number VARCHAR(50) UNIQUE NOT NULL,
    -- Personal Info
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    email VARCHAR(255) UNIQUE NOT NULL,
    phone VARCHAR(20),
    date_of_birth DATE,
    -- Account
    password_hash VARCHAR(255),
    email_verified BOOLEAN DEFAULT FALSE,
    phone_verified BOOLEAN DEFAULT FALSE,
    -- Preferences
    marketing_opt_in BOOLEAN DEFAULT FALSE,
    sms_opt_in BOOLEAN DEFAULT FALSE,
    language_preference VARCHAR(10) DEFAULT 'en',
    currency_preference VARCHAR(3) DEFAULT 'USD',
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'INACTIVE',
            'SUSPENDED',
            'DELETED'
        )
    ),
    -- Loyalty Tier
    loyalty_tier VARCHAR(20) DEFAULT 'BRONZE' CHECK (
        loyalty_tier IN (
            'BRONZE',
            'SILVER',
            'GOLD',
            'PLATINUM',
            'VIP'
        )
    ),
    -- Lifetime Value
    total_orders INTEGER DEFAULT 0,
    total_spent DECIMAL(12, 2) DEFAULT 0,
    average_order_value DECIMAL(10, 2) DEFAULT 0,
    -- Last Activity
    last_login_at TIMESTAMP,
    last_order_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_customers_email ON customers(email);
CREATE INDEX idx_customers_phone ON customers(phone);
CREATE INDEX idx_customers_tier ON customers(loyalty_tier);
CREATE INDEX idx_customers_status ON customers(status);
-- ============================================================================
-- CUSTOMER ADDRESSES
-- ============================================================================
CREATE TABLE customer_addresses (
    address_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id) ON DELETE CASCADE,
    address_type VARCHAR(20) CHECK (address_type IN ('SHIPPING', 'BILLING', 'BOTH')),
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    company VARCHAR(200),
    address_1 VARCHAR(255) NOT NULL,
    address_2 VARCHAR(255),
    city VARCHAR(100) NOT NULL,
    state VARCHAR(50) NOT NULL,
    postal_code VARCHAR(20) NOT NULL,
    country VARCHAR(2) DEFAULT 'US',
    phone VARCHAR(20),
    is_default BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_customer_addresses_customer ON customer_addresses(customer_id);
-- ============================================================================
-- CUSTOMER SEGMENTS
-- ============================================================================
CREATE TABLE customer_segments (
    segment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    segment_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Segment Type
    segment_type VARCHAR(30) CHECK (
        segment_type IN (
            'BEHAVIORAL',
            'DEMOGRAPHIC',
            'GEOGRAPHIC',
            'PSYCHOGRAPHIC',
            'VALUE_BASED'
        )
    ),
    -- Criteria (stored as JSONB for flexibility)
    criteria JSONB,
    -- Auto-update
    is_dynamic BOOLEAN DEFAULT TRUE,
    last_updated_at TIMESTAMP,
    -- Stats
    member_count INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_segments_type ON customer_segments(segment_type);
-- ============================================================================
-- CUSTOMER SEGMENT MEMBERS
-- ============================================================================
CREATE TABLE customer_segment_members (
    membership_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    segment_id UUID NOT NULL REFERENCES customer_segments(segment_id) ON DELETE CASCADE,
    customer_id UUID NOT NULL REFERENCES customers(customer_id) ON DELETE CASCADE,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(segment_id, customer_id)
);
CREATE INDEX idx_segment_members_segment ON customer_segment_members(segment_id);
CREATE INDEX idx_segment_members_customer ON customer_segment_members(customer_id);
-- ============================================================================
-- LOYALTY PROGRAMS
-- ============================================================================
CREATE TABLE loyalty_programs (
    program_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    program_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Tiers
    tiers JSONB,
    -- [{"name": "Bronze", "min_points": 0}, ...]
    -- Points
    points_per_dollar DECIMAL(5, 2) DEFAULT 1.00,
    points_expiry_days INTEGER,
    -- NULL = never expires
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    -- Dates
    start_date DATE,
    end_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- ============================================================================
-- LOYALTY POINTS
-- ============================================================================
CREATE TABLE loyalty_points (
    points_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id) ON DELETE CASCADE,
    program_id UUID NOT NULL REFERENCES loyalty_programs(program_id),
    -- Transaction
    transaction_type VARCHAR(30) CHECK (
        transaction_type IN (
            'EARNED',
            'REDEEMED',
            'EXPIRED',
            'ADJUSTED',
            'BONUS'
        )
    ),
    points_amount INTEGER NOT NULL,
    -- Can be negative for redemptions
    -- Reference
    order_id UUID,
    -- References orders table
    description TEXT,
    -- Balance
    balance_after INTEGER,
    -- Expiry
    expires_at DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_loyalty_points_customer ON loyalty_points(customer_id);
CREATE INDEX idx_loyalty_points_program ON loyalty_points(program_id);
CREATE INDEX idx_loyalty_points_expires ON loyalty_points(expires_at);
-- ============================================================================
-- REWARD REDEMPTIONS
-- ============================================================================
CREATE TABLE reward_redemptions (
    redemption_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    -- Reward
    reward_type VARCHAR(30) CHECK (
        reward_type IN (
            'DISCOUNT',
            'FREE_SHIPPING',
            'FREE_PRODUCT',
            'GIFT_CARD',
            'CASH_BACK'
        )
    ),
    points_redeemed INTEGER NOT NULL,
    reward_value DECIMAL(10, 2),
    -- Usage
    order_id UUID,
    -- References orders table
    used_at TIMESTAMP,
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'USED',
            'EXPIRED',
            'CANCELLED'
        )
    ),
    -- Expiry
    expires_at DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_redemptions_customer ON reward_redemptions(customer_id);
CREATE INDEX idx_redemptions_status ON reward_redemptions(status);
-- ============================================================================
-- WISHLISTS
-- ============================================================================
CREATE TABLE wishlists (
    wishlist_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID NOT NULL REFERENCES customers(customer_id) ON DELETE CASCADE,
    name VARCHAR(200) DEFAULT 'My Wishlist',
    is_public BOOLEAN DEFAULT FALSE,
    is_default BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_wishlists_customer ON wishlists(customer_id);
-- ============================================================================
-- WISHLIST ITEMS
-- ============================================================================
CREATE TABLE wishlist_items (
    wishlist_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    wishlist_id UUID NOT NULL REFERENCES wishlists(wishlist_id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    variant_id UUID,
    sku VARCHAR(100) NOT NULL,
    quantity INTEGER DEFAULT 1,
    priority INTEGER DEFAULT 0,
    notes TEXT,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_wishlist_items_wishlist ON wishlist_items(wishlist_id);
CREATE INDEX idx_wishlist_items_product ON wishlist_items(product_id);
-- ============================================================================
-- CUSTOMER REVIEWS
-- ============================================================================
CREATE TABLE customer_reviews (
    review_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    product_id UUID NOT NULL,
    customer_id UUID NOT NULL REFERENCES customers(customer_id),
    order_id UUID,
    -- References orders table
    -- Rating
    rating INTEGER NOT NULL CHECK (
        rating BETWEEN 1 AND 5
    ),
    title VARCHAR(200),
    review_text TEXT,
    -- Verification
    is_verified_purchase BOOLEAN DEFAULT FALSE,
    -- Helpfulness
    helpful_count INTEGER DEFAULT 0,
    not_helpful_count INTEGER DEFAULT 0,
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'APPROVED',
            'REJECTED',
            'FLAGGED'
        )
    ),
    -- Moderation
    moderated_by VARCHAR(100),
    moderated_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_reviews_product ON customer_reviews(product_id);
CREATE INDEX idx_reviews_customer ON customer_reviews(customer_id);
CREATE INDEX idx_reviews_status ON customer_reviews(status);
CREATE INDEX idx_reviews_rating ON customer_reviews(rating);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_customers_updated_at BEFORE
UPDATE ON customers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_customer_addresses_updated_at BEFORE
UPDATE ON customer_addresses FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_segments_updated_at BEFORE
UPDATE ON customer_segments FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_loyalty_programs_updated_at BEFORE
UPDATE ON loyalty_programs FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_wishlists_updated_at BEFORE
UPDATE ON wishlists FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_reviews_updated_at BEFORE
UPDATE ON customer_reviews FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Customer loyalty summary
CREATE VIEW v_customer_loyalty AS
SELECT c.customer_id,
    c.email,
    c.loyalty_tier,
    c.total_orders,
    c.total_spent,
    COALESCE(
        SUM(
            CASE
                WHEN lp.transaction_type = 'EARNED' THEN lp.points_amount
                ELSE 0
            END
        ),
        0
    ) AS total_points_earned,
    COALESCE(
        SUM(
            CASE
                WHEN lp.transaction_type = 'REDEEMED' THEN ABS(lp.points_amount)
                ELSE 0
            END
        ),
        0
    ) AS total_points_redeemed,
    COALESCE(MAX(lp.balance_after), 0) AS current_points_balance
FROM customers c
    LEFT JOIN loyalty_points lp ON c.customer_id = lp.customer_id
GROUP BY c.customer_id,
    c.email,
    c.loyalty_tier,
    c.total_orders,
    c.total_spent;
-- Top customers by spend
CREATE VIEW v_top_customers AS
SELECT customer_id,
    email,
    first_name,
    last_name,
    total_orders,
    total_spent,
    average_order_value,
    loyalty_tier
FROM customers
WHERE status = 'ACTIVE'
ORDER BY total_spent DESC
LIMIT 100;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE customers IS 'Customer profiles and account information';
COMMENT ON TABLE customer_segments IS 'Customer segmentation for marketing';
COMMENT ON TABLE loyalty_programs IS 'Loyalty and rewards programs';
COMMENT ON TABLE loyalty_points IS 'Customer loyalty points transactions';
COMMENT ON TABLE wishlists IS 'Customer wishlists';
COMMENT ON TABLE customer_reviews IS 'Product reviews and ratings';