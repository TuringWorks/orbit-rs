-- ============================================================================
-- OrbitRS Retail Examples - Products & Catalog Schema
-- ============================================================================
-- Product catalog, categories, brands, variants, attributes
-- ============================================================================
-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- CATEGORIES
-- ============================================================================
CREATE TABLE categories (
    category_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    category_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Hierarchy
    parent_category_id UUID REFERENCES categories(category_id),
    level INTEGER DEFAULT 1,
    path TEXT,
    -- e.g., "Men/Clothing/Shirts"
    -- Display
    display_order INTEGER DEFAULT 0,
    image_url VARCHAR(500),
    is_visible BOOLEAN DEFAULT TRUE,
    -- SEO
    slug VARCHAR(200) UNIQUE,
    meta_title VARCHAR(200),
    meta_description TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_categories_parent ON categories(parent_category_id);
CREATE INDEX idx_categories_slug ON categories(slug);
-- ============================================================================
-- BRANDS
-- ============================================================================
CREATE TABLE brands (
    brand_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    brand_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Brand Info
    logo_url VARCHAR(500),
    website_url VARCHAR(500),
    country_of_origin VARCHAR(2),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    is_featured BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_brands_name ON brands(name);
-- ============================================================================
-- PRODUCTS
-- ============================================================================
CREATE TABLE products (
    product_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    sku VARCHAR(100) UNIQUE NOT NULL,
    -- Basic Info
    name VARCHAR(500) NOT NULL,
    description TEXT,
    short_description VARCHAR(1000),
    -- Classification
    category_id UUID NOT NULL REFERENCES categories(category_id),
    brand_id UUID REFERENCES brands(brand_id),
    -- Product Type
    product_type VARCHAR(50) CHECK (
        product_type IN (
            'SIMPLE',
            'CONFIGURABLE',
            'BUNDLE',
            'GROUPED',
            'VIRTUAL',
            'DOWNLOADABLE'
        )
    ),
    -- Pricing
    base_price DECIMAL(10, 2) NOT NULL,
    cost DECIMAL(10, 2),
    msrp DECIMAL(10, 2),
    -- Manufacturer's Suggested Retail Price
    -- Inventory
    track_inventory BOOLEAN DEFAULT TRUE,
    stock_status VARCHAR(20) DEFAULT 'IN_STOCK' CHECK (
        stock_status IN (
            'IN_STOCK',
            'OUT_OF_STOCK',
            'BACKORDER',
            'DISCONTINUED'
        )
    ),
    -- Dimensions & Weight
    weight_kg DECIMAL(10, 3),
    length_cm DECIMAL(10, 2),
    width_cm DECIMAL(10, 2),
    height_cm DECIMAL(10, 2),
    -- Status
    status VARCHAR(20) DEFAULT 'DRAFT' CHECK (
        status IN (
            'DRAFT',
            'ACTIVE',
            'INACTIVE',
            'DISCONTINUED',
            'SEASONAL'
        )
    ),
    visibility VARCHAR(20) DEFAULT 'CATALOG_SEARCH' CHECK (
        visibility IN (
            'NOT_VISIBLE',
            'CATALOG',
            'SEARCH',
            'CATALOG_SEARCH'
        )
    ),
    -- Dates
    available_from DATE,
    available_to DATE,
    -- SEO
    slug VARCHAR(200) UNIQUE,
    meta_title VARCHAR(200),
    meta_description TEXT,
    meta_keywords TEXT,
    -- Ratings
    average_rating DECIMAL(3, 2) DEFAULT 0,
    review_count INTEGER DEFAULT 0,
    -- Sales
    view_count INTEGER DEFAULT 0,
    sales_count INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_products_sku ON products(sku);
CREATE INDEX idx_products_category ON products(category_id);
CREATE INDEX idx_products_brand ON products(brand_id);
CREATE INDEX idx_products_status ON products(status);
CREATE INDEX idx_products_slug ON products(slug);
CREATE INDEX idx_products_price ON products(base_price);
-- ============================================================================
-- PRODUCT VARIANTS
-- ============================================================================
CREATE TABLE product_variants (
    variant_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    product_id UUID NOT NULL REFERENCES products(product_id) ON DELETE CASCADE,
    sku VARCHAR(100) UNIQUE NOT NULL,
    -- Variant Attributes (e.g., size, color)
    attributes JSONB NOT NULL,
    -- {"size": "M", "color": "Blue"}
    -- Pricing
    price DECIMAL(10, 2) NOT NULL,
    cost DECIMAL(10, 2),
    -- Inventory
    stock_quantity INTEGER DEFAULT 0,
    -- Images
    image_url VARCHAR(500),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_variants_product ON product_variants(product_id);
CREATE INDEX idx_variants_sku ON product_variants(sku);
CREATE INDEX idx_variants_attributes ON product_variants USING GIN(attributes);
-- ============================================================================
-- PRODUCT ATTRIBUTES
-- ============================================================================
CREATE TABLE product_attributes (
    attribute_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    attribute_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    -- Type
    attribute_type VARCHAR(20) CHECK (
        attribute_type IN (
            'TEXT',
            'SELECT',
            'MULTISELECT',
            'BOOLEAN',
            'NUMBER',
            'DATE'
        )
    ),
    -- Options (for SELECT/MULTISELECT)
    options JSONB,
    -- ["Small", "Medium", "Large"]
    -- Validation
    is_required BOOLEAN DEFAULT FALSE,
    is_filterable BOOLEAN DEFAULT TRUE,
    is_searchable BOOLEAN DEFAULT FALSE,
    -- Display
    display_order INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- ============================================================================
-- PRODUCT ATTRIBUTE VALUES
-- ============================================================================
CREATE TABLE product_attribute_values (
    value_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    product_id UUID NOT NULL REFERENCES products(product_id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES product_attributes(attribute_id),
    value TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(product_id, attribute_id)
);
CREATE INDEX idx_attribute_values_product ON product_attribute_values(product_id);
CREATE INDEX idx_attribute_values_attribute ON product_attribute_values(attribute_id);
-- ============================================================================
-- PRODUCT IMAGES
-- ============================================================================
CREATE TABLE product_images (
    image_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    product_id UUID NOT NULL REFERENCES products(product_id) ON DELETE CASCADE,
    variant_id UUID REFERENCES product_variants(variant_id) ON DELETE CASCADE,
    -- Image Details
    image_url VARCHAR(500) NOT NULL,
    thumbnail_url VARCHAR(500),
    alt_text VARCHAR(500),
    -- Type
    image_type VARCHAR(20) CHECK (
        image_type IN ('MAIN', 'GALLERY', 'SWATCH', 'THUMBNAIL')
    ),
    -- Display
    display_order INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_images_product ON product_images(product_id);
CREATE INDEX idx_images_variant ON product_images(variant_id);
-- ============================================================================
-- PRODUCT COLLECTIONS (Fast Fashion)
-- ============================================================================
CREATE TABLE collections (
    collection_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    collection_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Type
    collection_type VARCHAR(30) CHECK (
        collection_type IN (
            'SEASONAL',
            'CAPSULE',
            'COLLABORATION',
            'LIMITED_EDITION',
            'EVERGREEN'
        )
    ),
    -- Season
    season VARCHAR(20) CHECK (season IN ('SPRING', 'SUMMER', 'FALL', 'WINTER')),
    year INTEGER,
    -- Availability
    launch_date TIMESTAMP,
    end_date TIMESTAMP,
    is_limited_edition BOOLEAN DEFAULT FALSE,
    max_quantity INTEGER,
    -- For limited editions
    -- Display
    image_url VARCHAR(500),
    is_featured BOOLEAN DEFAULT FALSE,
    display_order INTEGER DEFAULT 0,
    -- Status
    status VARCHAR(20) DEFAULT 'DRAFT' CHECK (
        status IN (
            'DRAFT',
            'SCHEDULED',
            'ACTIVE',
            'ENDED',
            'ARCHIVED'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_collections_type ON collections(collection_type);
CREATE INDEX idx_collections_status ON collections(status);
CREATE INDEX idx_collections_launch ON collections(launch_date);
-- ============================================================================
-- COLLECTION PRODUCTS
-- ============================================================================
CREATE TABLE collection_products (
    collection_product_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    collection_id UUID NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES products(product_id) ON DELETE CASCADE,
    display_order INTEGER DEFAULT 0,
    is_featured BOOLEAN DEFAULT FALSE,
    added_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(collection_id, product_id)
);
CREATE INDEX idx_collection_products_collection ON collection_products(collection_id);
CREATE INDEX idx_collection_products_product ON collection_products(product_id);
-- ============================================================================
-- PRODUCT BUNDLES
-- ============================================================================
CREATE TABLE product_bundles (
    bundle_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bundle_product_id UUID NOT NULL REFERENCES products(product_id) ON DELETE CASCADE,
    included_product_id UUID NOT NULL REFERENCES products(product_id),
    quantity INTEGER DEFAULT 1,
    discount_percentage DECIMAL(5, 2) DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(bundle_product_id, included_product_id)
);
CREATE INDEX idx_bundles_bundle_product ON product_bundles(bundle_product_id);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = CURRENT_TIMESTAMP;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER update_categories_updated_at BEFORE
UPDATE ON categories FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_brands_updated_at BEFORE
UPDATE ON brands FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_products_updated_at BEFORE
UPDATE ON products FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_variants_updated_at BEFORE
UPDATE ON product_variants FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_collections_updated_at BEFORE
UPDATE ON collections FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active products with full details
CREATE VIEW v_active_products AS
SELECT p.product_id,
    p.sku,
    p.name,
    p.base_price,
    p.stock_status,
    c.name AS category_name,
    b.name AS brand_name,
    p.average_rating,
    p.review_count,
    p.sales_count
FROM products p
    LEFT JOIN categories c ON p.category_id = c.category_id
    LEFT JOIN brands b ON p.brand_id = b.brand_id
WHERE p.status = 'ACTIVE'
    AND p.visibility IN ('CATALOG', 'SEARCH', 'CATALOG_SEARCH');
-- Product inventory summary
CREATE VIEW v_product_inventory AS
SELECT p.product_id,
    p.sku,
    p.name,
    p.stock_status,
    COALESCE(SUM(pv.stock_quantity), 0) AS total_stock,
    COUNT(pv.variant_id) AS variant_count
FROM products p
    LEFT JOIN product_variants pv ON p.product_id = pv.product_id
    AND pv.is_active = TRUE
GROUP BY p.product_id,
    p.sku,
    p.name,
    p.stock_status;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE products IS 'Product catalog with SKUs, pricing, and metadata';
COMMENT ON TABLE product_variants IS 'Product variants (size, color, etc.)';
COMMENT ON TABLE categories IS 'Product categories with hierarchy';
COMMENT ON TABLE brands IS 'Brand information';
COMMENT ON TABLE collections IS 'Seasonal collections and limited editions';
COMMENT ON TABLE product_bundles IS 'Product bundles and kits';