-- ============================================================================
-- OrbitRS Retail Examples - Inventory & Warehouse Schema
-- ============================================================================
-- Inventory, warehouses, stock transfers, bin locations
-- ============================================================================
-- ============================================================================
-- WAREHOUSES
-- ============================================================================
CREATE TABLE warehouses (
    warehouse_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    warehouse_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    -- Type
    warehouse_type VARCHAR(30) CHECK (
        warehouse_type IN (
            'DISTRIBUTION_CENTER',
            'FULFILLMENT_CENTER',
            'RETAIL_STORE',
            'DROPSHIP',
            'THIRD_PARTY'
        )
    ),
    -- Location
    address_1 VARCHAR(255),
    address_2 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(50),
    postal_code VARCHAR(20),
    country VARCHAR(2) DEFAULT 'US',
    -- Geospatial
    location GEOGRAPHY(POINT, 4326),
    latitude DECIMAL(10, 8),
    longitude DECIMAL(11, 8),
    -- Capacity
    total_capacity_sqft DECIMAL(12, 2),
    available_capacity_sqft DECIMAL(12, 2),
    max_sku_count INTEGER,
    -- Operations
    is_active BOOLEAN DEFAULT TRUE,
    accepts_inbound BOOLEAN DEFAULT TRUE,
    accepts_outbound BOOLEAN DEFAULT TRUE,
    -- Contact
    manager_name VARCHAR(200),
    phone VARCHAR(20),
    email VARCHAR(255),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_warehouses_code ON warehouses(warehouse_code);
CREATE INDEX idx_warehouses_type ON warehouses(warehouse_type);
CREATE INDEX idx_warehouses_location ON warehouses USING GIST(location);
-- ============================================================================
-- INVENTORY
-- ============================================================================
CREATE TABLE inventory (
    inventory_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Product
    product_id UUID NOT NULL,
    -- References products table
    variant_id UUID,
    -- References product_variants table
    sku VARCHAR(100) NOT NULL,
    -- Warehouse
    warehouse_id UUID NOT NULL REFERENCES warehouses(warehouse_id),
    -- Quantities
    quantity_on_hand INTEGER DEFAULT 0 CHECK (quantity_on_hand >= 0),
    quantity_available INTEGER DEFAULT 0 CHECK (quantity_available >= 0),
    quantity_reserved INTEGER DEFAULT 0 CHECK (quantity_reserved >= 0),
    quantity_incoming INTEGER DEFAULT 0,
    -- Bin Location
    bin_location VARCHAR(50),
    aisle VARCHAR(20),
    shelf VARCHAR(20),
    -- Reorder
    reorder_point INTEGER DEFAULT 10,
    reorder_quantity INTEGER DEFAULT 50,
    -- Safety Stock
    safety_stock INTEGER DEFAULT 5,
    -- Last Activity
    last_counted_at TIMESTAMP,
    last_received_at TIMESTAMP,
    last_shipped_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(sku, warehouse_id)
);
CREATE INDEX idx_inventory_product ON inventory(product_id);
CREATE INDEX idx_inventory_sku ON inventory(sku);
CREATE INDEX idx_inventory_warehouse ON inventory(warehouse_id);
CREATE INDEX idx_inventory_available ON inventory(quantity_available);
-- ============================================================================
-- STOCK TRANSFERS
-- ============================================================================
CREATE TABLE stock_transfers (
    transfer_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    transfer_number VARCHAR(50) UNIQUE NOT NULL,
    -- Warehouses
    from_warehouse_id UUID NOT NULL REFERENCES warehouses(warehouse_id),
    to_warehouse_id UUID NOT NULL REFERENCES warehouses(warehouse_id),
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'APPROVED',
            'IN_TRANSIT',
            'RECEIVED',
            'CANCELLED'
        )
    ),
    -- Reason
    transfer_reason VARCHAR(50) CHECK (
        transfer_reason IN (
            'REBALANCING',
            'REPLENISHMENT',
            'RETURN',
            'DAMAGED',
            'OTHER'
        )
    ),
    notes TEXT,
    -- Shipping
    carrier VARCHAR(50),
    tracking_number VARCHAR(100),
    -- Dates
    requested_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    approved_at TIMESTAMP,
    shipped_at TIMESTAMP,
    received_at TIMESTAMP,
    -- Approvals
    requested_by VARCHAR(100),
    approved_by VARCHAR(100),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_transfers_from_warehouse ON stock_transfers(from_warehouse_id);
CREATE INDEX idx_transfers_to_warehouse ON stock_transfers(to_warehouse_id);
CREATE INDEX idx_transfers_status ON stock_transfers(status);
-- ============================================================================
-- STOCK TRANSFER ITEMS
-- ============================================================================
CREATE TABLE stock_transfer_items (
    transfer_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    transfer_id UUID NOT NULL REFERENCES stock_transfers(transfer_id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    variant_id UUID,
    sku VARCHAR(100) NOT NULL,
    quantity_requested INTEGER NOT NULL CHECK (quantity_requested > 0),
    quantity_shipped INTEGER DEFAULT 0,
    quantity_received INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_transfer_items_transfer ON stock_transfer_items(transfer_id);
CREATE INDEX idx_transfer_items_sku ON stock_transfer_items(sku);
-- ============================================================================
-- STOCK ADJUSTMENTS
-- ============================================================================
CREATE TABLE stock_adjustments (
    adjustment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    adjustment_number VARCHAR(50) UNIQUE NOT NULL,
    warehouse_id UUID NOT NULL REFERENCES warehouses(warehouse_id),
    product_id UUID NOT NULL,
    variant_id UUID,
    sku VARCHAR(100) NOT NULL,
    -- Adjustment
    adjustment_type VARCHAR(30) CHECK (
        adjustment_type IN (
            'CYCLE_COUNT',
            'DAMAGE',
            'LOSS',
            'FOUND',
            'CORRECTION',
            'RETURN'
        )
    ),
    quantity_before INTEGER NOT NULL,
    quantity_change INTEGER NOT NULL,
    -- Can be negative
    quantity_after INTEGER NOT NULL,
    -- Reason
    reason TEXT,
    -- Audit
    adjusted_by VARCHAR(100),
    adjusted_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_adjustments_warehouse ON stock_adjustments(warehouse_id);
CREATE INDEX idx_adjustments_sku ON stock_adjustments(sku);
CREATE INDEX idx_adjustments_type ON stock_adjustments(adjustment_type);
-- ============================================================================
-- BIN LOCATIONS
-- ============================================================================
CREATE TABLE bin_locations (
    bin_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    warehouse_id UUID NOT NULL REFERENCES warehouses(warehouse_id),
    -- Location Code
    bin_code VARCHAR(50) NOT NULL,
    aisle VARCHAR(20),
    shelf VARCHAR(20),
    level VARCHAR(20),
    -- Type
    bin_type VARCHAR(30) CHECK (
        bin_type IN (
            'PICKING',
            'RESERVE',
            'RECEIVING',
            'PACKING',
            'RETURNS',
            'DAMAGED'
        )
    ),
    -- Capacity
    max_capacity INTEGER,
    current_occupancy INTEGER DEFAULT 0,
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(warehouse_id, bin_code)
);
CREATE INDEX idx_bins_warehouse ON bin_locations(warehouse_id);
CREATE INDEX idx_bins_type ON bin_locations(bin_type);
-- ============================================================================
-- PURCHASE ORDERS
-- ============================================================================
CREATE TABLE purchase_orders (
    po_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    po_number VARCHAR(50) UNIQUE NOT NULL,
    -- Supplier
    supplier_id UUID,
    -- References suppliers table
    supplier_name VARCHAR(200),
    -- Warehouse
    warehouse_id UUID NOT NULL REFERENCES warehouses(warehouse_id),
    -- Amounts
    subtotal DECIMAL(12, 2),
    tax_amount DECIMAL(10, 2),
    shipping_amount DECIMAL(10, 2),
    total_amount DECIMAL(12, 2),
    -- Status
    status VARCHAR(20) DEFAULT 'DRAFT' CHECK (
        status IN (
            'DRAFT',
            'SUBMITTED',
            'CONFIRMED',
            'SHIPPED',
            'RECEIVED',
            'CANCELLED'
        )
    ),
    -- Dates
    order_date DATE DEFAULT CURRENT_DATE,
    expected_delivery_date DATE,
    received_date DATE,
    -- Notes
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_po_warehouse ON purchase_orders(warehouse_id);
CREATE INDEX idx_po_status ON purchase_orders(status);
-- ============================================================================
-- PURCHASE ORDER ITEMS
-- ============================================================================
CREATE TABLE purchase_order_items (
    po_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    po_id UUID NOT NULL REFERENCES purchase_orders(po_id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    variant_id UUID,
    sku VARCHAR(100) NOT NULL,
    quantity_ordered INTEGER NOT NULL CHECK (quantity_ordered > 0),
    quantity_received INTEGER DEFAULT 0,
    unit_cost DECIMAL(10, 2),
    total_cost DECIMAL(12, 2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_po_items_po ON purchase_order_items(po_id);
CREATE INDEX idx_po_items_sku ON purchase_order_items(sku);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_warehouses_updated_at BEFORE
UPDATE ON warehouses FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_inventory_updated_at BEFORE
UPDATE ON inventory FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_transfers_updated_at BEFORE
UPDATE ON stock_transfers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_bins_updated_at BEFORE
UPDATE ON bin_locations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_po_updated_at BEFORE
UPDATE ON purchase_orders FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Inventory summary by warehouse
CREATE VIEW v_inventory_by_warehouse AS
SELECT w.warehouse_id,
    w.warehouse_code,
    w.name AS warehouse_name,
    COUNT(DISTINCT i.sku) AS sku_count,
    SUM(i.quantity_on_hand) AS total_units,
    SUM(i.quantity_available) AS available_units,
    SUM(i.quantity_reserved) AS reserved_units
FROM warehouses w
    LEFT JOIN inventory i ON w.warehouse_id = i.warehouse_id
GROUP BY w.warehouse_id,
    w.warehouse_code,
    w.name;
-- Low stock items
CREATE VIEW v_low_stock_items AS
SELECT i.inventory_id,
    i.sku,
    w.warehouse_code,
    i.quantity_available,
    i.reorder_point,
    i.reorder_quantity
FROM inventory i
    JOIN warehouses w ON i.warehouse_id = w.warehouse_id
WHERE i.quantity_available <= i.reorder_point
    AND w.is_active = TRUE;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE warehouses IS 'Warehouse and fulfillment center locations';
COMMENT ON TABLE inventory IS 'Product inventory levels by warehouse';
COMMENT ON TABLE stock_transfers IS 'Inter-warehouse stock transfers';
COMMENT ON TABLE stock_adjustments IS 'Inventory adjustments and cycle counts';
COMMENT ON TABLE bin_locations IS 'Warehouse bin and shelf locations';
COMMENT ON TABLE purchase_orders IS 'Purchase orders from suppliers';