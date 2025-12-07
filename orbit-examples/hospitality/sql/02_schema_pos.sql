-- ============================================================================
-- OrbitRS Hospitality Examples - Point-of-Sale Schema
-- ============================================================================
-- Orders, transactions, payments, tips
-- ============================================================================
-- ============================================================================
-- ORDERS
-- ============================================================================
CREATE TABLE orders (
    order_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_number VARCHAR(50) UNIQUE NOT NULL,
    -- Store
    store_id UUID NOT NULL,
    -- References stores table
    -- Customer
    customer_id UUID,
    -- References customers table (NULL for guest orders)
    customer_name VARCHAR(200),
    -- Order Type
    order_type VARCHAR(20) NOT NULL CHECK (
        order_type IN (
            'IN_STORE',
            'MOBILE',
            'DRIVE_THRU',
            'DELIVERY',
            'CURBSIDE'
        )
    ),
    order_source VARCHAR(20) CHECK (
        order_source IN (
            'POS',
            'MOBILE_APP',
            'WEB',
            'KIOSK',
            'PHONE'
        )
    ),
    -- Fulfillment
    fulfillment_status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        fulfillment_status IN (
            'PENDING',
            'PREPARING',
            'READY',
            'COMPLETED',
            'CANCELLED'
        )
    ),
    -- Amounts
    subtotal DECIMAL(10, 2) NOT NULL,
    tax_amount DECIMAL(10, 2) DEFAULT 0,
    tip_amount DECIMAL(10, 2) DEFAULT 0,
    discount_amount DECIMAL(10, 2) DEFAULT 0,
    total_amount DECIMAL(10, 2) NOT NULL,
    -- Payment
    payment_status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        payment_status IN (
            'PENDING',
            'PAID',
            'REFUNDED',
            'FAILED'
        )
    ),
    payment_method VARCHAR(30) CHECK (
        payment_method IN (
            'CREDIT_CARD',
            'DEBIT_CARD',
            'MOBILE_PAY',
            'GIFT_CARD',
            'CASH',
            'LOYALTY_POINTS'
        )
    ),
    -- Loyalty
    loyalty_points_earned INTEGER DEFAULT 0,
    loyalty_points_redeemed INTEGER DEFAULT 0,
    -- Staff
    cashier_id UUID,
    -- References staff table
    barista_id UUID,
    -- Timing
    ordered_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    promised_at TIMESTAMP,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    picked_up_at TIMESTAMP,
    -- Special Instructions
    special_instructions TEXT,
    -- Status
    status VARCHAR(20) DEFAULT 'OPEN' CHECK (
        status IN (
            'OPEN',
            'COMPLETED',
            'CANCELLED',
            'REFUNDED'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_orders_store ON orders(store_id);
CREATE INDEX idx_orders_customer ON orders(customer_id);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_type ON orders(order_type);
CREATE INDEX idx_orders_ordered_at ON orders(ordered_at);
-- ============================================================================
-- ORDER ITEMS
-- ============================================================================
CREATE TABLE order_items (
    order_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(order_id) ON DELETE CASCADE,
    -- Menu Item
    item_id UUID NOT NULL REFERENCES menu_items(item_id),
    size_id UUID REFERENCES item_sizes(size_id),
    -- Item Details (snapshot at time of order)
    item_name VARCHAR(200) NOT NULL,
    size_name VARCHAR(50),
    -- Quantity
    quantity INTEGER NOT NULL DEFAULT 1 CHECK (quantity > 0),
    -- Pricing
    unit_price DECIMAL(10, 2) NOT NULL,
    total_price DECIMAL(10, 2) NOT NULL,
    -- Preparation
    prep_status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        prep_status IN (
            'PENDING',
            'PREPARING',
            'READY',
            'SERVED'
        )
    ),
    prep_station VARCHAR(50),
    -- Special Instructions
    special_instructions TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_order_items_order ON order_items(order_id);
CREATE INDEX idx_order_items_item ON order_items(item_id);
CREATE INDEX idx_order_items_prep_status ON order_items(prep_status);
-- ============================================================================
-- ORDER ITEM MODIFIERS
-- ============================================================================
CREATE TABLE order_item_modifiers (
    order_item_modifier_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_item_id UUID NOT NULL REFERENCES order_items(order_item_id) ON DELETE CASCADE,
    -- Modifier
    modifier_id UUID NOT NULL REFERENCES modifiers(modifier_id),
    modifier_name VARCHAR(200) NOT NULL,
    -- Pricing
    price_adjustment DECIMAL(10, 2) DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_order_item_modifiers_order_item ON order_item_modifiers(order_item_id);
-- ============================================================================
-- TRANSACTIONS
-- ============================================================================
CREATE TABLE transactions (
    transaction_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    transaction_number VARCHAR(50) UNIQUE NOT NULL,
    order_id UUID NOT NULL REFERENCES orders(order_id),
    -- Transaction Type
    transaction_type VARCHAR(20) CHECK (
        transaction_type IN ('SALE', 'REFUND', 'VOID')
    ),
    -- Amount
    amount DECIMAL(10, 2) NOT NULL,
    -- Payment Method
    payment_method VARCHAR(30) NOT NULL,
    -- Card Details (masked)
    card_last_four VARCHAR(4),
    card_brand VARCHAR(20),
    -- Processing
    processor VARCHAR(50),
    processor_transaction_id VARCHAR(100),
    authorization_code VARCHAR(50),
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'AUTHORIZED',
            'CAPTURED',
            'DECLINED',
            'REFUNDED',
            'VOIDED'
        )
    ),
    -- Dates
    transaction_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_transactions_order ON transactions(order_id);
CREATE INDEX idx_transactions_status ON transactions(status);
CREATE INDEX idx_transactions_date ON transactions(transaction_date);
-- ============================================================================
-- TIPS
-- ============================================================================
CREATE TABLE tips (
    tip_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(order_id),
    -- Tip Amount
    tip_amount DECIMAL(10, 2) NOT NULL,
    tip_percentage DECIMAL(5, 2),
    -- Tip Type
    tip_type VARCHAR(20) CHECK (
        tip_type IN (
            'PERCENTAGE',
            'FIXED_AMOUNT',
            'CUSTOM'
        )
    ),
    -- Distribution
    staff_id UUID,
    -- If tip goes to specific staff member
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_tips_order ON tips(order_id);
CREATE INDEX idx_tips_staff ON tips(staff_id);
-- ============================================================================
-- DISCOUNTS
-- ============================================================================
CREATE TABLE order_discounts (
    discount_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(order_id) ON DELETE CASCADE,
    -- Discount Type
    discount_type VARCHAR(30) CHECK (
        discount_type IN (
            'PERCENTAGE',
            'FIXED_AMOUNT',
            'BOGO',
            'LOYALTY_REWARD',
            'EMPLOYEE',
            'PROMOTION'
        )
    ),
    -- Discount Details
    discount_code VARCHAR(50),
    discount_name VARCHAR(200),
    discount_amount DECIMAL(10, 2) NOT NULL,
    -- Approval
    requires_approval BOOLEAN DEFAULT FALSE,
    approved_by UUID,
    -- Staff member who approved
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_order_discounts_order ON order_discounts(order_id);
-- ============================================================================
-- REFUNDS
-- ============================================================================
CREATE TABLE refunds (
    refund_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(order_id),
    transaction_id UUID REFERENCES transactions(transaction_id),
    -- Refund Details
    refund_amount DECIMAL(10, 2) NOT NULL,
    refund_reason VARCHAR(50) CHECK (
        refund_reason IN (
            'WRONG_ORDER',
            'QUALITY_ISSUE',
            'CUSTOMER_REQUEST',
            'MISTAKE',
            'OTHER'
        )
    ),
    refund_notes TEXT,
    -- Approval
    approved_by UUID NOT NULL,
    -- Staff member
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'APPROVED',
            'PROCESSED',
            'DECLINED'
        )
    ),
    refund_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_refunds_order ON refunds(order_id);
CREATE INDEX idx_refunds_status ON refunds(status);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_orders_updated_at BEFORE
UPDATE ON orders FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Today's orders summary
CREATE VIEW v_todays_orders AS
SELECT o.order_id,
    o.order_number,
    o.order_type,
    o.total_amount,
    o.status,
    o.fulfillment_status,
    o.ordered_at,
    COUNT(oi.order_item_id) AS item_count
FROM orders o
    LEFT JOIN order_items oi ON o.order_id = oi.order_id
WHERE DATE(o.ordered_at) = CURRENT_DATE
GROUP BY o.order_id,
    o.order_number,
    o.order_type,
    o.total_amount,
    o.status,
    o.fulfillment_status,
    o.ordered_at;
-- Orders in queue (preparing)
CREATE VIEW v_order_queue AS
SELECT o.order_id,
    o.order_number,
    o.order_type,
    o.ordered_at,
    o.promised_at,
    oi.order_item_id,
    oi.item_name,
    oi.size_name,
    oi.quantity,
    oi.prep_status,
    oi.prep_station
FROM orders o
    JOIN order_items oi ON o.order_id = oi.order_id
WHERE o.fulfillment_status IN ('PENDING', 'PREPARING')
    AND oi.prep_status IN ('PENDING', 'PREPARING')
ORDER BY o.ordered_at;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE orders IS 'Customer orders from all channels (POS, mobile, etc.)';
COMMENT ON TABLE order_items IS 'Line items in orders with customizations';
COMMENT ON TABLE transactions IS 'Payment transactions';
COMMENT ON TABLE tips IS 'Customer tips';