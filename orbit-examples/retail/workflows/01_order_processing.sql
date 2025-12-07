-- =============================================================================
-- OrbitRS Retail Example: Order Processing Workflow
-- =============================================================================
-- Demonstrates PostgreSQL patterns for order processing workflows:
--   - Order state machine
--   - Inventory management
--   - Payment processing
--   - Fulfillment tracking
--
-- Connect: psql -h localhost -p 5432 -U orbit -d orbit
-- Run: \i 01_order_processing.sql
-- =============================================================================

-- Create schema
CREATE SCHEMA IF NOT EXISTS retail_workflow;

-- =============================================================================
-- ORDER STATE MACHINE
-- =============================================================================

CREATE TABLE IF NOT EXISTS retail_workflow.order_states (
    state_code VARCHAR(30) PRIMARY KEY,
    state_name VARCHAR(50) NOT NULL,
    state_order INTEGER,
    is_terminal BOOLEAN DEFAULT false,
    allowed_transitions TEXT[],
    description TEXT
);

INSERT INTO retail_workflow.order_states (state_code, state_name, state_order, is_terminal, allowed_transitions, description)
VALUES
    ('PENDING', 'Pending', 1, false, ARRAY['CONFIRMED', 'CANCELLED'], 'Order created, awaiting confirmation'),
    ('CONFIRMED', 'Confirmed', 2, false, ARRAY['PROCESSING', 'CANCELLED'], 'Order confirmed, payment pending'),
    ('PROCESSING', 'Processing', 3, false, ARRAY['READY_TO_SHIP', 'ON_HOLD', 'CANCELLED'], 'Order being prepared'),
    ('ON_HOLD', 'On Hold', 4, false, ARRAY['PROCESSING', 'CANCELLED'], 'Order temporarily on hold'),
    ('READY_TO_SHIP', 'Ready to Ship', 5, false, ARRAY['SHIPPED', 'ON_HOLD'], 'Order ready for shipment'),
    ('SHIPPED', 'Shipped', 6, false, ARRAY['DELIVERED', 'RETURNED'], 'Order has been shipped'),
    ('DELIVERED', 'Delivered', 7, true, ARRAY['RETURNED'], 'Order delivered to customer'),
    ('CANCELLED', 'Cancelled', 8, true, ARRAY[], 'Order was cancelled'),
    ('RETURNED', 'Returned', 9, true, ARRAY[], 'Order was returned')
ON CONFLICT (state_code) DO NOTHING;

-- =============================================================================
-- ORDER TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS retail_workflow.orders (
    order_id VARCHAR(50) PRIMARY KEY,
    order_number VARCHAR(30) UNIQUE NOT NULL,
    customer_id VARCHAR(50) NOT NULL,
    customer_email VARCHAR(200),
    -- Order details
    order_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    status VARCHAR(30) DEFAULT 'PENDING',
    subtotal DECIMAL(12,2) NOT NULL,
    tax_amount DECIMAL(10,2) DEFAULT 0,
    shipping_amount DECIMAL(10,2) DEFAULT 0,
    discount_amount DECIMAL(10,2) DEFAULT 0,
    total_amount DECIMAL(12,2) NOT NULL,
    currency VARCHAR(3) DEFAULT 'USD',
    -- Shipping info
    shipping_method VARCHAR(50),
    shipping_address TEXT,
    -- Payment info
    payment_method VARCHAR(50),
    payment_status VARCHAR(30) DEFAULT 'PENDING',
    -- Timestamps
    confirmed_at TIMESTAMP,
    shipped_at TIMESTAMP,
    delivered_at TIMESTAMP,
    cancelled_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS retail_workflow.order_items (
    item_id VARCHAR(50) PRIMARY KEY,
    order_id VARCHAR(50) REFERENCES retail_workflow.orders(order_id),
    product_id VARCHAR(50) NOT NULL,
    product_name VARCHAR(200) NOT NULL,
    sku VARCHAR(50),
    quantity INTEGER NOT NULL,
    unit_price DECIMAL(10,2) NOT NULL,
    total_price DECIMAL(12,2) NOT NULL,
    status VARCHAR(30) DEFAULT 'PENDING',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS retail_workflow.order_events (
    event_id VARCHAR(50) PRIMARY KEY,
    order_id VARCHAR(50) REFERENCES retail_workflow.orders(order_id),
    event_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    event_type VARCHAR(50) NOT NULL,
    from_status VARCHAR(30),
    to_status VARCHAR(30),
    actor_id VARCHAR(50),
    actor_type VARCHAR(30), -- SYSTEM, CUSTOMER, STAFF
    details TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- INVENTORY TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS retail_workflow.inventory (
    inventory_id VARCHAR(50) PRIMARY KEY,
    product_id VARCHAR(50) NOT NULL,
    warehouse_id VARCHAR(50) NOT NULL,
    quantity_on_hand INTEGER DEFAULT 0,
    quantity_reserved INTEGER DEFAULT 0,
    quantity_available INTEGER GENERATED ALWAYS AS (quantity_on_hand - quantity_reserved) STORED,
    reorder_point INTEGER DEFAULT 10,
    reorder_quantity INTEGER DEFAULT 50,
    last_restock_date DATE,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (product_id, warehouse_id)
);

CREATE TABLE IF NOT EXISTS retail_workflow.inventory_reservations (
    reservation_id VARCHAR(50) PRIMARY KEY,
    order_id VARCHAR(50) REFERENCES retail_workflow.orders(order_id),
    product_id VARCHAR(50) NOT NULL,
    warehouse_id VARCHAR(50) NOT NULL,
    quantity INTEGER NOT NULL,
    status VARCHAR(30) DEFAULT 'RESERVED', -- RESERVED, RELEASED, FULFILLED
    reserved_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP,
    released_at TIMESTAMP
);

-- =============================================================================
-- PAYMENT TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS retail_workflow.payments (
    payment_id VARCHAR(50) PRIMARY KEY,
    order_id VARCHAR(50) REFERENCES retail_workflow.orders(order_id),
    payment_method VARCHAR(50) NOT NULL,
    amount DECIMAL(12,2) NOT NULL,
    currency VARCHAR(3) DEFAULT 'USD',
    status VARCHAR(30) DEFAULT 'PENDING',
    -- Payment processor info
    processor VARCHAR(50),
    transaction_id VARCHAR(100),
    authorization_code VARCHAR(50),
    -- Timestamps
    initiated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP,
    failed_at TIMESTAMP,
    failure_reason TEXT
);

-- =============================================================================
-- FULFILLMENT TABLES
-- =============================================================================

CREATE TABLE IF NOT EXISTS retail_workflow.shipments (
    shipment_id VARCHAR(50) PRIMARY KEY,
    order_id VARCHAR(50) REFERENCES retail_workflow.orders(order_id),
    carrier VARCHAR(50) NOT NULL,
    tracking_number VARCHAR(100),
    shipping_method VARCHAR(50),
    status VARCHAR(30) DEFAULT 'PENDING',
    -- Package info
    weight_lbs DECIMAL(8,2),
    dimensions VARCHAR(50), -- LxWxH
    -- Timestamps
    label_created_at TIMESTAMP,
    picked_up_at TIMESTAMP,
    in_transit_at TIMESTAMP,
    delivered_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS retail_workflow.shipment_tracking (
    tracking_id VARCHAR(50) PRIMARY KEY,
    shipment_id VARCHAR(50) REFERENCES retail_workflow.shipments(shipment_id),
    event_time TIMESTAMP NOT NULL,
    location VARCHAR(200),
    status VARCHAR(50),
    description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- =============================================================================
-- WORKFLOW FUNCTIONS
-- =============================================================================

-- Validate state transition
CREATE OR REPLACE FUNCTION retail_workflow.can_transition(
    current_state VARCHAR,
    new_state VARCHAR
) RETURNS BOOLEAN AS $$
DECLARE
    allowed TEXT[];
BEGIN
    SELECT allowed_transitions INTO allowed
    FROM retail_workflow.order_states
    WHERE state_code = current_state;

    RETURN new_state = ANY(allowed);
END;
$$ LANGUAGE plpgsql;

-- Process order state change
CREATE OR REPLACE FUNCTION retail_workflow.change_order_status(
    p_order_id VARCHAR,
    p_new_status VARCHAR,
    p_actor_id VARCHAR DEFAULT 'SYSTEM',
    p_actor_type VARCHAR DEFAULT 'SYSTEM',
    p_details TEXT DEFAULT NULL
) RETURNS BOOLEAN AS $$
DECLARE
    v_current_status VARCHAR;
BEGIN
    -- Get current status
    SELECT status INTO v_current_status
    FROM retail_workflow.orders
    WHERE order_id = p_order_id;

    -- Validate transition
    IF NOT retail_workflow.can_transition(v_current_status, p_new_status) THEN
        RAISE EXCEPTION 'Invalid state transition from % to %', v_current_status, p_new_status;
    END IF;

    -- Update order status
    UPDATE retail_workflow.orders
    SET status = p_new_status,
        updated_at = CURRENT_TIMESTAMP,
        confirmed_at = CASE WHEN p_new_status = 'CONFIRMED' THEN CURRENT_TIMESTAMP ELSE confirmed_at END,
        shipped_at = CASE WHEN p_new_status = 'SHIPPED' THEN CURRENT_TIMESTAMP ELSE shipped_at END,
        delivered_at = CASE WHEN p_new_status = 'DELIVERED' THEN CURRENT_TIMESTAMP ELSE delivered_at END,
        cancelled_at = CASE WHEN p_new_status = 'CANCELLED' THEN CURRENT_TIMESTAMP ELSE cancelled_at END
    WHERE order_id = p_order_id;

    -- Log event
    INSERT INTO retail_workflow.order_events (
        event_id, order_id, event_type, from_status, to_status, actor_id, actor_type, details
    ) VALUES (
        'EVT-' || SUBSTRING(MD5(RANDOM()::TEXT) FROM 1 FOR 12),
        p_order_id, 'STATUS_CHANGE', v_current_status, p_new_status, p_actor_id, p_actor_type, p_details
    );

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- =============================================================================
-- INSERT SAMPLE DATA
-- =============================================================================

-- Sample orders
INSERT INTO retail_workflow.orders (order_id, order_number, customer_id, customer_email,
    status, subtotal, tax_amount, shipping_amount, total_amount, shipping_method, payment_method)
VALUES
    ('ORD-001', 'ORD-2024-0001', 'CUST-001', 'john@email.com',
     'PROCESSING', 299.97, 24.00, 9.99, 333.96, 'STANDARD', 'CREDIT_CARD'),
    ('ORD-002', 'ORD-2024-0002', 'CUST-002', 'jane@email.com',
     'SHIPPED', 149.99, 12.00, 0.00, 161.99, 'EXPRESS', 'PAYPAL'),
    ('ORD-003', 'ORD-2024-0003', 'CUST-003', 'bob@email.com',
     'PENDING', 89.99, 7.20, 5.99, 103.18, 'ECONOMY', 'CREDIT_CARD')
ON CONFLICT (order_id) DO NOTHING;

-- Sample order items
INSERT INTO retail_workflow.order_items (item_id, order_id, product_id, product_name, sku, quantity, unit_price, total_price)
VALUES
    ('ITEM-001', 'ORD-001', 'PROD-001', 'Wireless Headphones', 'WH-001', 1, 149.99, 149.99),
    ('ITEM-002', 'ORD-001', 'PROD-002', 'Phone Case', 'PC-001', 2, 24.99, 49.98),
    ('ITEM-003', 'ORD-001', 'PROD-003', 'USB Cable 3-Pack', 'UC-003', 1, 99.99, 99.99),
    ('ITEM-004', 'ORD-002', 'PROD-004', 'Bluetooth Speaker', 'BS-001', 1, 149.99, 149.99)
ON CONFLICT (item_id) DO NOTHING;

-- Sample inventory
INSERT INTO retail_workflow.inventory (inventory_id, product_id, warehouse_id, quantity_on_hand, quantity_reserved, reorder_point)
VALUES
    ('INV-001', 'PROD-001', 'WH-EAST', 100, 5, 20),
    ('INV-002', 'PROD-002', 'WH-EAST', 250, 10, 50),
    ('INV-003', 'PROD-003', 'WH-EAST', 150, 2, 25),
    ('INV-004', 'PROD-004', 'WH-WEST', 75, 8, 15)
ON CONFLICT (inventory_id) DO NOTHING;

-- Sample payments
INSERT INTO retail_workflow.payments (payment_id, order_id, payment_method, amount, status, processor, transaction_id, completed_at)
VALUES
    ('PAY-001', 'ORD-001', 'CREDIT_CARD', 333.96, 'COMPLETED', 'STRIPE', 'txn_abc123', CURRENT_TIMESTAMP - INTERVAL '2 days'),
    ('PAY-002', 'ORD-002', 'PAYPAL', 161.99, 'COMPLETED', 'PAYPAL', 'PAYID-xyz789', CURRENT_TIMESTAMP - INTERVAL '3 days')
ON CONFLICT (payment_id) DO NOTHING;

-- Sample shipment
INSERT INTO retail_workflow.shipments (shipment_id, order_id, carrier, tracking_number, shipping_method, status, picked_up_at)
VALUES
    ('SHIP-001', 'ORD-002', 'UPS', '1Z999AA10123456784', 'GROUND', 'IN_TRANSIT', CURRENT_TIMESTAMP - INTERVAL '1 day')
ON CONFLICT (shipment_id) DO NOTHING;

-- =============================================================================
-- WORKFLOW QUERIES
-- =============================================================================

-- Order pipeline summary
SELECT
    status,
    COUNT(*) as order_count,
    SUM(total_amount) as total_value,
    AVG(total_amount) as avg_order_value
FROM retail_workflow.orders
GROUP BY status
ORDER BY (SELECT state_order FROM retail_workflow.order_states WHERE state_code = status);

-- Orders by fulfillment stage
SELECT
    o.order_number,
    o.status as order_status,
    o.total_amount,
    p.status as payment_status,
    s.status as shipment_status,
    s.tracking_number
FROM retail_workflow.orders o
LEFT JOIN retail_workflow.payments p ON o.order_id = p.order_id
LEFT JOIN retail_workflow.shipments s ON o.order_id = s.order_id
ORDER BY o.order_date DESC;

-- Inventory availability check
SELECT
    i.product_id,
    i.warehouse_id,
    i.quantity_on_hand,
    i.quantity_reserved,
    i.quantity_available,
    CASE
        WHEN i.quantity_available <= 0 THEN 'OUT_OF_STOCK'
        WHEN i.quantity_available <= i.reorder_point THEN 'LOW_STOCK'
        ELSE 'IN_STOCK'
    END as stock_status
FROM retail_workflow.inventory i
ORDER BY i.quantity_available;

-- Order event timeline
SELECT
    e.event_time,
    o.order_number,
    e.event_type,
    e.from_status,
    e.to_status,
    e.actor_type,
    e.details
FROM retail_workflow.order_events e
JOIN retail_workflow.orders o ON e.order_id = o.order_id
ORDER BY e.event_time DESC;
