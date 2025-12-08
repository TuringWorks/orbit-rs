-- ============================================================================
-- OrbitRS Manufacturing Examples - Products & BOM Schema
-- ============================================================================
-- Products, components, Bill of Materials (BOM)
-- ============================================================================
-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- PRODUCT CATEGORIES
-- ============================================================================
CREATE TABLE product_categories (
    category_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    category_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Hierarchy
    parent_category_id UUID REFERENCES product_categories(category_id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_product_categories_parent ON product_categories(parent_category_id);
-- ============================================================================
-- PRODUCTS (Finished Goods)
-- ============================================================================
CREATE TABLE products (
    product_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    product_code VARCHAR(100) UNIQUE NOT NULL,
    category_id UUID REFERENCES product_categories(category_id),
    -- Product Details
    name VARCHAR(300) NOT NULL,
    description TEXT,
    model_number VARCHAR(100),
    -- Product Type
    product_type VARCHAR(30) CHECK (
        product_type IN (
            'SMARTPHONE',
            'TABLET',
            'LAPTOP',
            'DESKTOP',
            'WEARABLE',
            'ACCESSORY',
            'COMPONENT',
            'ASSEMBLY'
        )
    ),
    -- Specifications
    specifications JSONB,
    -- {"screen_size": "6.1", "storage": "256GB", "color": "Black"}
    -- Manufacturing
    standard_cost DECIMAL(12, 4),
    target_price DECIMAL(12, 2),
    lead_time_days INTEGER DEFAULT 30,
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    is_discontinued BOOLEAN DEFAULT FALSE,
    -- Lifecycle
    introduced_date DATE,
    discontinued_date DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_products_category ON products(category_id);
CREATE INDEX idx_products_type ON products(product_type);
CREATE INDEX idx_products_active ON products(is_active);
-- ============================================================================
-- COMPONENTS (Parts, Raw Materials)
-- ============================================================================
CREATE TABLE components (
    component_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    component_code VARCHAR(100) UNIQUE NOT NULL,
    -- Component Details
    name VARCHAR(300) NOT NULL,
    description TEXT,
    manufacturer_part_number VARCHAR(100),
    -- Component Type
    component_type VARCHAR(30) CHECK (
        component_type IN (
            'PCB',
            'CHIP',
            'DISPLAY',
            'BATTERY',
            'CAMERA',
            'SENSOR',
            'CONNECTOR',
            'CASE',
            'SCREW',
            'ADHESIVE',
            'PACKAGING'
        )
    ),
    -- Specifications
    specifications JSONB,
    -- Unit of Measure
    unit_of_measure VARCHAR(20) DEFAULT 'EACH',
    -- EACH, KG, M, L
    -- Costing
    standard_cost DECIMAL(12, 4),
    -- Lead Time
    lead_time_days INTEGER DEFAULT 14,
    -- Quality
    requires_inspection BOOLEAN DEFAULT FALSE,
    shelf_life_days INTEGER,
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_components_type ON components(component_type);
CREATE INDEX idx_components_active ON components(is_active);
-- ============================================================================
-- SUPPLIERS
-- ============================================================================
CREATE TABLE suppliers (
    supplier_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    supplier_code VARCHAR(50) UNIQUE NOT NULL,
    -- Supplier Details
    name VARCHAR(300) NOT NULL,
    -- Type
    supplier_type VARCHAR(30) CHECK (
        supplier_type IN (
            'TIER_1',
            'TIER_2',
            'TIER_3',
            'OEM',
            'ODM'
        )
    ),
    -- Contact
    contact_name VARCHAR(200),
    email VARCHAR(255),
    phone VARCHAR(20),
    -- Address
    address_1 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(50),
    country VARCHAR(2),
    -- Performance
    quality_rating DECIMAL(3, 2),
    -- 0.00 to 5.00
    delivery_rating DECIMAL(3, 2),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    is_approved BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_suppliers_type ON suppliers(supplier_type);
CREATE INDEX idx_suppliers_active ON suppliers(is_active);
-- ============================================================================
-- COMPONENT SUPPLIERS (Many-to-Many)
-- ============================================================================
CREATE TABLE component_suppliers (
    component_supplier_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    component_id UUID NOT NULL REFERENCES components(component_id),
    supplier_id UUID NOT NULL REFERENCES suppliers(supplier_id),
    -- Supplier-specific details
    supplier_part_number VARCHAR(100),
    unit_cost DECIMAL(12, 4),
    lead_time_days INTEGER,
    min_order_quantity INTEGER DEFAULT 1,
    -- Priority
    is_preferred BOOLEAN DEFAULT FALSE,
    priority INTEGER DEFAULT 1,
    -- 1 = primary, 2 = secondary, etc.
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(component_id, supplier_id)
);
CREATE INDEX idx_component_suppliers_component ON component_suppliers(component_id);
CREATE INDEX idx_component_suppliers_supplier ON component_suppliers(supplier_id);
-- ============================================================================
-- BOM HEADERS (Bill of Materials)
-- ============================================================================
CREATE TABLE bom_headers (
    bom_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bom_number VARCHAR(50) UNIQUE NOT NULL,
    product_id UUID NOT NULL REFERENCES products(product_id),
    -- BOM Details
    bom_name VARCHAR(200),
    revision VARCHAR(20) DEFAULT '1.0',
    -- Status
    status VARCHAR(20) DEFAULT 'DRAFT' CHECK (
        status IN (
            'DRAFT',
            'ACTIVE',
            'SUPERSEDED',
            'OBSOLETE'
        )
    ),
    -- Dates
    effective_date DATE,
    expiration_date DATE,
    -- Notes
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_bom_headers_product ON bom_headers(product_id);
CREATE INDEX idx_bom_headers_status ON bom_headers(status);
-- ============================================================================
-- BOM ITEMS (Components in BOM)
-- ============================================================================
CREATE TABLE bom_items (
    bom_item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bom_id UUID NOT NULL REFERENCES bom_headers(bom_id) ON DELETE CASCADE,
    -- Component
    component_id UUID NOT NULL REFERENCES components(component_id),
    -- Quantity
    quantity DECIMAL(12, 4) NOT NULL,
    unit_of_measure VARCHAR(20),
    -- Assembly Details
    assembly_sequence INTEGER,
    assembly_station VARCHAR(50),
    -- Substitutes
    is_optional BOOLEAN DEFAULT FALSE,
    substitute_component_id UUID REFERENCES components(component_id),
    -- Scrap/Waste
    scrap_factor DECIMAL(5, 4) DEFAULT 0,
    -- e.g., 0.02 = 2% scrap
    -- Reference Designator (for PCBs)
    reference_designator VARCHAR(50),
    -- Notes
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_bom_items_bom ON bom_items(bom_id);
CREATE INDEX idx_bom_items_component ON bom_items(component_id);
-- ============================================================================
-- PRODUCT VARIANTS
-- ============================================================================
CREATE TABLE product_variants (
    variant_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    product_id UUID NOT NULL REFERENCES products(product_id),
    -- Variant Details
    variant_code VARCHAR(100) UNIQUE NOT NULL,
    variant_name VARCHAR(200),
    -- Attributes (e.g., color, storage, memory)
    attributes JSONB,
    -- {"color": "Black", "storage": "256GB", "memory": "8GB"}
    -- BOM
    bom_id UUID REFERENCES bom_headers(bom_id),
    -- Pricing
    variant_cost DECIMAL(12, 4),
    variant_price DECIMAL(12, 2),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_product_variants_product ON product_variants(product_id);
CREATE INDEX idx_product_variants_bom ON product_variants(bom_id);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = CURRENT_TIMESTAMP;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER update_products_updated_at BEFORE
UPDATE ON products FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_components_updated_at BEFORE
UPDATE ON components FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_suppliers_updated_at BEFORE
UPDATE ON suppliers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_bom_headers_updated_at BEFORE
UPDATE ON bom_headers FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active products with BOM
CREATE VIEW v_active_products AS
SELECT p.product_id,
    p.product_code,
    p.name,
    p.product_type,
    p.standard_cost,
    p.target_price,
    bh.bom_id,
    bh.bom_number,
    bh.revision
FROM products p
    LEFT JOIN bom_headers bh ON p.product_id = bh.product_id
    AND bh.status = 'ACTIVE'
WHERE p.is_active = TRUE;
-- BOM explosion (single-level)
CREATE VIEW v_bom_explosion AS
SELECT bh.bom_id,
    bh.bom_number,
    p.product_code,
    p.name AS product_name,
    bi.component_id,
    c.component_code,
    c.name AS component_name,
    bi.quantity,
    bi.unit_of_measure,
    c.standard_cost,
    (bi.quantity * c.standard_cost) AS extended_cost
FROM bom_headers bh
    JOIN products p ON bh.product_id = p.product_id
    JOIN bom_items bi ON bh.bom_id = bi.bom_id
    JOIN components c ON bi.component_id = c.component_id
WHERE bh.status = 'ACTIVE';
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE products IS 'Finished goods and assemblies';
COMMENT ON TABLE components IS 'Parts, raw materials, and sub-assemblies';
COMMENT ON TABLE bom_headers IS 'Bill of Materials headers';
COMMENT ON TABLE bom_items IS 'Components in each BOM';
COMMENT ON TABLE suppliers IS 'Component and material suppliers';