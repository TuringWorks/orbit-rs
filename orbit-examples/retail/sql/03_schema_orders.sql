-- ============================================================================
-- OrbitRS Retail Examples - Orders & Fulfillment Schema
-- ============================================================================
-- Orders, order items, shipments, returns, refunds
-- ============================================================================
-- ============================================================================
-- ORDERS
-- ============================================================================
CREATE TABLE orders (
    order_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_number VARCHAR(50) UNIQUE NOT NULL,
    -- Customer
    customer_id UUID NOT NULL,
    -- References customers table
    email VARCHAR(255) NOT NULL,
    -- Order Type
    order_type VARCHAR(20) DEFAULT 'WEB' CHECK (
        order_type IN (
            'WEB',
            'MOBILE',
            'POS',
            'PHONE',
            'MARKETPLACE'
        )
    ),
    channel VARCHAR(20) CHECK (channel IN ('ONLINE', 'STORE', 'MOBILE_APP')),
    -- Amounts
    subtotal DECIMAL(12, 2) NOT NULL,
    shipping_amount DECIMAL(10, 2) DEFAULT 0,
    tax_amount DECIMAL(10, 2) DEFAULT 0,
    discount_amount DECIMAL(10, 2) DEFAULT 0,
    total_amount DECIMAL(12, 2) NOT NULL,
    -- Payment
    payment_method VARCHAR(30) CHECK (
        payment_method IN (
            'CREDIT_CARD',
            'DEBIT_CARD',
            'PAYPAL',
            'APPLE_PAY',
            'GOOGLE_PAY',
            'GIFT_CARD',
            'STORE_CREDIT',
            'COD',
            'KLARNA',
            'AFTERPAY'
        )
    ),
    payment_status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        payment_status IN (
            'PENDING',
            'AUTHORIZED',
            'PAID',
            'FAILED',
            'REFUNDED',
            'PARTIALLY_REFUNDED'
        )
    ),
    transaction_id VARCHAR(100),
    -- Fulfillment
    fulfillment_type VARCHAR(20) CHECK (
        fulfillment_type IN (
            'SHIP_TO_HOME',
            'PICKUP_IN_STORE',
            'CURBSIDE',
            'SAME_DAY_DELIVERY'
        )
    ),
    fulfillment_status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        fulfillment_status IN (
            'PENDING',
            'PROCESSING',
            'READY_FOR_PICKUP',
            'SHIPPED',
            'DELIVERED',
            'CANCELLED',
            'RETURNED'
        )
    ),
    -- Shipping Address
    shipping_first_name VARCHAR(100),
    shipping_last_name VARCHAR(100),
    shipping_address_1 VARCHAR(255),
    shipping_address_2 VARCHAR(255),
    shipping_city VARCHAR(100),
    shipping_state VARCHAR(50),
    shipping_postal_code VARCHAR(20),
    shipping_country VARCHAR(2) DEFAULT 'US',
    shipping_phone VARCHAR(20),
    -- Billing Address
    billing_first_name VARCHAR(100),
    billing_last_name VARCHAR(100),
    billing_address_1 VARCHAR(255),
    billing_city VARCHAR(100),
    billing_state VARCHAR(50),
    billing_postal_code VARCHAR(20),
    billing_country VARCHAR(2) DEFAULT 'US',
    -- Store (for BOPIS)
    store_id UUID,
    -- References stores table
    pickup_ready_at TIMESTAMP,
    picked_up_at TIMESTAMP,
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'CONFIRMED',
            'PROCESSING',
            'COMPLETED',
            'CANCELLED',
            'ON_HOLD'
        )
    ),
    -- Promotions
    coupon_code VARCHAR(50),
    loyalty_points_used INTEGER DEFAULT 0,
    -- Notes
    customer_notes TEXT,
    internal_notes TEXT,
    -- Dates
    order_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    confirmed_at TIMESTAMP,
    shipped_at TIMESTAMP,
    delivered_at TIMESTAMP,
    cancelled_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_orders_customer ON orders(customer_id);
CREATE INDEX idx_orders_number ON orders(order_number);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_date ON orders(order_date);
CREATE INDEX idx_orders_email ON orders(email);
-- ============================================================================
-- ORDER ITEMS
-- ============================================================================
CREATE TABLE order_items (
    order_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    order_id UUID NOT NULL REFERENCES orders(order_id) ON DELETE CASCADE,
    -- Product
    product_id UUID NOT NULL,
    -- References products table
    variant_id UUID,
    -- References product_variants table
    sku VARCHAR(100) NOT NULL,
    product_name VARCHAR(500) NOT NULL,
    -- Variant Details
    variant_attributes JSONB,
    -- {"size": "M", "color": "Blue"}
    -- Quantity & Pricing
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    unit_price DECIMAL(10, 2) NOT NULL,
    discount_amount DECIMAL(10, 2) DEFAULT 0,
    tax_amount DECIMAL(10, 2) DEFAULT 0,
    total_amount DECIMAL(10, 2) NOT NULL,
    -- Fulfillment
    fulfillment_status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        fulfillment_status IN (
            'PENDING',
            'ALLOCATED',
            'PICKED',
            'PACKED',
            'SHIPPED',
            'DELIVERED',
            'CANCELLED',
            'RETURNED'
        )
    ),
    warehouse_id UUID,
    -- References warehouses table
    -- Return
    is_returnable BOOLEAN DEFAULT TRUE,
    return_deadline DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_order_items_order ON order_items(order_id);
CREATE INDEX idx_order_items_product ON order_items(product_id);
CREATE INDEX idx_order_items_sku ON order_items(sku);
-- ============================================================================
-- SHIPMENTS
-- ============================================================================
CREATE TABLE shipments (
    shipment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    shipment_number VARCHAR(50) UNIQUE NOT NULL,
    order_id UUID NOT NULL REFERENCES orders(order_id),
    -- Carrier
    carrier VARCHAR(50) CHECK (
        carrier IN (
            'USPS',
            'UPS',
            'FEDEX',
            'DHL',
            'AMAZON_LOGISTICS',
            'LOCAL_COURIER'
        )
    ),
    service_level VARCHAR(50),
    -- Ground, Express, Overnight, etc.
    tracking_number VARCHAR(100),
    tracking_url VARCHAR(500),
    -- Shipping Details
    shipping_cost DECIMAL(10, 2),
    weight_kg DECIMAL(10, 3),
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'LABEL_CREATED',
            'PICKED_UP',
            'IN_TRANSIT',
            'OUT_FOR_DELIVERY',
            'DELIVERED',
            'FAILED',
            'RETURNED'
        )
    ),
    -- Dates
    shipped_at TIMESTAMP,
    estimated_delivery_date DATE,
    delivered_at TIMESTAMP,
    -- Signature
    signature_required BOOLEAN DEFAULT FALSE,
    signed_by VARCHAR(200),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_shipments_order ON shipments(order_id);
CREATE INDEX idx_shipments_tracking ON shipments(tracking_number);
CREATE INDEX idx_shipments_status ON shipments(status);
-- ============================================================================
-- SHIPMENT ITEMS
-- ============================================================================
CREATE TABLE shipment_items (
    shipment_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    shipment_id UUID NOT NULL REFERENCES shipments(shipment_id) ON DELETE CASCADE,
    order_item_id UUID NOT NULL REFERENCES order_items(order_item_id),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_shipment_items_shipment ON shipment_items(shipment_id);
CREATE INDEX idx_shipment_items_order_item ON shipment_items(order_item_id);
-- ============================================================================
-- RETURNS
-- ============================================================================
CREATE TABLE returns (
    return_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    return_number VARCHAR(50) UNIQUE NOT NULL,
    order_id UUID NOT NULL REFERENCES orders(order_id),
    -- Return Details
    return_reason VARCHAR(50) CHECK (
        return_reason IN (
            'WRONG_SIZE',
            'WRONG_COLOR',
            'DEFECTIVE',
            'NOT_AS_DESCRIBED',
            'CHANGED_MIND',
            'DAMAGED',
            'LATE_DELIVERY',
            'OTHER'
        )
    ),
    return_comments TEXT,
    -- Refund
    refund_amount DECIMAL(10, 2),
    refund_method VARCHAR(30) CHECK (
        refund_method IN (
            'ORIGINAL_PAYMENT',
            'STORE_CREDIT',
            'GIFT_CARD',
            'EXCHANGE'
        )
    ),
    -- Status
    status VARCHAR(20) DEFAULT 'REQUESTED' CHECK (
        status IN (
            'REQUESTED',
            'APPROVED',
            'REJECTED',
            'RECEIVED',
            'INSPECTED',
            'REFUNDED',
            'COMPLETED'
        )
    ),
    -- Return Shipping
    return_tracking_number VARCHAR(100),
    return_label_url VARCHAR(500),
    -- Dates
    requested_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    approved_at TIMESTAMP,
    received_at TIMESTAMP,
    refunded_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_returns_order ON returns(order_id);
CREATE INDEX idx_returns_status ON returns(status);
-- ============================================================================
-- RETURN ITEMS
-- ============================================================================
CREATE TABLE return_items (
    return_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    return_id UUID NOT NULL REFERENCES returns(return_id) ON DELETE CASCADE,
    order_item_id UUID NOT NULL REFERENCES order_items(order_item_id),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    refund_amount DECIMAL(10, 2),
    -- Inspection
    condition VARCHAR(20) CHECK (
        condition IN (
            'NEW',
            'LIKE_NEW',
            'GOOD',
            'FAIR',
            'DAMAGED',
            'DEFECTIVE'
        )
    ),
    restockable BOOLEAN,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_return_items_return ON return_items(return_id);
CREATE INDEX idx_return_items_order_item ON return_items(order_item_id);
-- ============================================================================
-- SHOPPING CART (Persistent)
-- ============================================================================
CREATE TABLE shopping_carts (
    cart_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    customer_id UUID,
    session_id VARCHAR(100),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'ABANDONED',
            'CONVERTED',
            'EXPIRED'
        )
    ),
    -- Totals
    subtotal DECIMAL(10, 2) DEFAULT 0,
    -- Dates
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    converted_to_order_id UUID REFERENCES orders(order_id)
);
CREATE INDEX idx_carts_customer ON shopping_carts(customer_id);
CREATE INDEX idx_carts_session ON shopping_carts(session_id);
CREATE INDEX idx_carts_status ON shopping_carts(status);
-- ============================================================================
-- CART ITEMS
-- ============================================================================
CREATE TABLE cart_items (
    cart_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    cart_id UUID NOT NULL REFERENCES shopping_carts(cart_id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    variant_id UUID,
    sku VARCHAR(100) NOT NULL,
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    unit_price DECIMAL(10, 2) NOT NULL,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_cart_items_cart ON cart_items(cart_id);
CREATE INDEX idx_cart_items_product ON cart_items(product_id);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_orders_updated_at BEFORE
UPDATE ON orders FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_shipments_updated_at BEFORE
UPDATE ON shipments FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_returns_updated_at BEFORE
UPDATE ON returns FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_carts_updated_at BEFORE
UPDATE ON shopping_carts FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_cart_items_updated_at BEFORE
UPDATE ON cart_items FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Order summary with items
CREATE VIEW v_order_summary AS
SELECT o.order_id,
    o.order_number,
    o.customer_id,
    o.email,
    o.total_amount,
    o.status,
    o.fulfillment_status,
    o.order_date,
    COUNT(oi.order_item_id) AS item_count,
    SUM(oi.quantity) AS total_quantity
FROM orders o
    LEFT JOIN order_items oi ON o.order_id = oi.order_id
GROUP BY o.order_id,
    o.order_number,
    o.customer_id,
    o.email,
    o.total_amount,
    o.status,
    o.fulfillment_status,
    o.order_date;
-- Pending shipments
CREATE VIEW v_pending_shipments AS
SELECT s.shipment_id,
    s.shipment_number,
    o.order_number,
    o.customer_id,
    s.carrier,
    s.tracking_number,
    s.status,
    s.shipped_at,
    s.estimated_delivery_date
FROM shipments s
    JOIN orders o ON s.order_id = o.order_id
WHERE s.status IN (
        'PENDING',
        'LABEL_CREATED',
        'IN_TRANSIT',
        'OUT_FOR_DELIVERY'
    );
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE orders IS 'Customer orders with payment and fulfillment details';
COMMENT ON TABLE order_items IS 'Line items in orders';
COMMENT ON TABLE shipments IS 'Shipment tracking and carrier information';
COMMENT ON TABLE returns IS 'Product returns and refunds';
COMMENT ON TABLE shopping_carts IS 'Persistent shopping carts';