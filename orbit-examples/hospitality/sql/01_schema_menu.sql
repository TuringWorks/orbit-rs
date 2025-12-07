-- ============================================================================
-- OrbitRS Hospitality Examples - Menu & Products Schema
-- ============================================================================
-- Menu categories, items, modifiers, recipes, ingredients
-- ============================================================================
-- Enable UUID extension
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- MENU CATEGORIES
-- ============================================================================
CREATE TABLE menu_categories (
    category_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    category_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Category Type
    category_type VARCHAR(30) CHECK (
        category_type IN (
            'BEVERAGE',
            'FOOD',
            'PASTRY',
            'MERCHANDISE'
        )
    ),
    -- Display
    display_order INTEGER DEFAULT 0,
    image_url VARCHAR(500),
    icon VARCHAR(50),
    -- Availability
    is_active BOOLEAN DEFAULT TRUE,
    available_all_day BOOLEAN DEFAULT TRUE,
    available_start_time TIME,
    available_end_time TIME,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_menu_categories_type ON menu_categories(category_type);
-- ============================================================================
-- MENU ITEMS
-- ============================================================================
CREATE TABLE menu_items (
    item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    item_code VARCHAR(50) UNIQUE NOT NULL,
    category_id UUID NOT NULL REFERENCES menu_categories(category_id),
    -- Basic Info
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Item Type
    item_type VARCHAR(30) CHECK (
        item_type IN (
            'ESPRESSO',
            'BREWED_COFFEE',
            'TEA',
            'COLD_BREW',
            'FRAPPUCCINO',
            'SANDWICH',
            'SALAD',
            'PASTRY',
            'BREAKFAST',
            'LUNCH',
            'MERCHANDISE'
        )
    ),
    -- Pricing (base price, sizes have different prices)
    base_price DECIMAL(10, 2) NOT NULL,
    cost DECIMAL(10, 2),
    -- Sizes
    has_sizes BOOLEAN DEFAULT FALSE,
    default_size VARCHAR(20),
    -- Customization
    allows_modifiers BOOLEAN DEFAULT TRUE,
    -- Nutritional Info
    calories INTEGER,
    caffeine_mg INTEGER,
    -- Allergens
    contains_dairy BOOLEAN DEFAULT FALSE,
    contains_nuts BOOLEAN DEFAULT FALSE,
    contains_gluten BOOLEAN DEFAULT FALSE,
    is_vegan BOOLEAN DEFAULT FALSE,
    is_vegetarian BOOLEAN DEFAULT FALSE,
    -- Availability
    is_active BOOLEAN DEFAULT TRUE,
    is_seasonal BOOLEAN DEFAULT FALSE,
    available_start_date DATE,
    available_end_date DATE,
    -- Preparation
    prep_time_seconds INTEGER DEFAULT 120,
    requires_barista BOOLEAN DEFAULT FALSE,
    -- Display
    image_url VARCHAR(500),
    display_order INTEGER DEFAULT 0,
    -- Popularity
    popularity_score DECIMAL(5, 2) DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_menu_items_category ON menu_items(category_id);
CREATE INDEX idx_menu_items_type ON menu_items(item_type);
CREATE INDEX idx_menu_items_active ON menu_items(is_active);
-- ============================================================================
-- ITEM SIZES
-- ============================================================================
CREATE TABLE item_sizes (
    size_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    item_id UUID NOT NULL REFERENCES menu_items(item_id) ON DELETE CASCADE,
    -- Size Details
    size_name VARCHAR(50) NOT NULL,
    -- Short, Tall, Grande, Venti
    size_code VARCHAR(20) NOT NULL,
    -- S, M, L, XL
    volume_oz DECIMAL(5, 2),
    -- Pricing
    price DECIMAL(10, 2) NOT NULL,
    -- Nutritional adjustments
    calories_adjustment INTEGER DEFAULT 0,
    -- Display
    display_order INTEGER DEFAULT 0,
    is_default BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(item_id, size_code)
);
CREATE INDEX idx_item_sizes_item ON item_sizes(item_id);
-- ============================================================================
-- MODIFIER GROUPS
-- ============================================================================
CREATE TABLE modifier_groups (
    group_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    group_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    -- Group Type
    group_type VARCHAR(30) CHECK (
        group_type IN (
            'MILK',
            'SYRUP',
            'TOPPING',
            'EXTRA',
            'TEMPERATURE',
            'PREPARATION'
        )
    ),
    -- Selection Rules
    min_selections INTEGER DEFAULT 0,
    max_selections INTEGER DEFAULT 1,
    is_required BOOLEAN DEFAULT FALSE,
    -- Display
    display_order INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- ============================================================================
-- MODIFIERS
-- ============================================================================
CREATE TABLE modifiers (
    modifier_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    group_id UUID NOT NULL REFERENCES modifier_groups(group_id) ON DELETE CASCADE,
    modifier_code VARCHAR(50) UNIQUE NOT NULL,
    -- Modifier Details
    name VARCHAR(200) NOT NULL,
    description TEXT,
    -- Pricing
    price_adjustment DECIMAL(10, 2) DEFAULT 0,
    -- Nutritional Impact
    calories_adjustment INTEGER DEFAULT 0,
    -- Availability
    is_active BOOLEAN DEFAULT TRUE,
    -- Display
    display_order INTEGER DEFAULT 0,
    is_default BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_modifiers_group ON modifiers(group_id);
-- ============================================================================
-- ITEM MODIFIER GROUPS (which modifiers apply to which items)
-- ============================================================================
CREATE TABLE item_modifier_groups (
    item_modifier_group_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    item_id UUID NOT NULL REFERENCES menu_items(item_id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES modifier_groups(group_id) ON DELETE CASCADE,
    -- Override group settings for this item
    min_selections INTEGER,
    max_selections INTEGER,
    is_required BOOLEAN,
    display_order INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(item_id, group_id)
);
CREATE INDEX idx_item_modifier_groups_item ON item_modifier_groups(item_id);
CREATE INDEX idx_item_modifier_groups_group ON item_modifier_groups(group_id);
-- ============================================================================
-- INGREDIENTS
-- ============================================================================
CREATE TABLE ingredients (
    ingredient_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    ingredient_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(200) NOT NULL,
    -- Type
    ingredient_type VARCHAR(30) CHECK (
        ingredient_type IN (
            'COFFEE_BEAN',
            'MILK',
            'SYRUP',
            'SAUCE',
            'POWDER',
            'TOPPING',
            'BREAD',
            'MEAT',
            'CHEESE',
            'VEGETABLE',
            'FRUIT',
            'CONDIMENT'
        )
    ),
    -- Unit of Measure
    unit_of_measure VARCHAR(20),
    -- OZ, ML, G, EACH
    -- Cost
    cost_per_unit DECIMAL(10, 4),
    -- Supplier
    supplier_id UUID,
    -- Allergens
    contains_dairy BOOLEAN DEFAULT FALSE,
    contains_nuts BOOLEAN DEFAULT FALSE,
    contains_gluten BOOLEAN DEFAULT FALSE,
    -- Storage
    requires_refrigeration BOOLEAN DEFAULT FALSE,
    shelf_life_days INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_ingredients_type ON ingredients(ingredient_type);
-- ============================================================================
-- RECIPES
-- ============================================================================
CREATE TABLE recipes (
    recipe_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    item_id UUID NOT NULL REFERENCES menu_items(item_id) ON DELETE CASCADE,
    size_id UUID REFERENCES item_sizes(size_id),
    -- Recipe Details
    recipe_name VARCHAR(200),
    instructions TEXT,
    -- Preparation
    prep_station VARCHAR(50),
    -- ESPRESSO_BAR, COLD_BAR, FOOD_STATION
    prep_time_seconds INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_recipes_item ON recipes(item_id);
-- ============================================================================
-- RECIPE INGREDIENTS
-- ============================================================================
CREATE TABLE recipe_ingredients (
    recipe_ingredient_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    recipe_id UUID NOT NULL REFERENCES recipes(recipe_id) ON DELETE CASCADE,
    ingredient_id UUID NOT NULL REFERENCES ingredients(ingredient_id),
    -- Quantity
    quantity DECIMAL(10, 4) NOT NULL,
    unit_of_measure VARCHAR(20),
    -- Optional
    is_optional BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_recipe_ingredients_recipe ON recipe_ingredients(recipe_id);
CREATE INDEX idx_recipe_ingredients_ingredient ON recipe_ingredients(ingredient_id);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = CURRENT_TIMESTAMP;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER update_menu_categories_updated_at BEFORE
UPDATE ON menu_categories FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_menu_items_updated_at BEFORE
UPDATE ON menu_items FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_ingredients_updated_at BEFORE
UPDATE ON ingredients FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_recipes_updated_at BEFORE
UPDATE ON recipes FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Active menu items with pricing
CREATE VIEW v_active_menu AS
SELECT mi.item_id,
    mi.item_code,
    mi.name,
    mi.description,
    mc.name AS category_name,
    mi.base_price,
    mi.item_type,
    mi.is_seasonal,
    mi.popularity_score
FROM menu_items mi
    JOIN menu_categories mc ON mi.category_id = mc.category_id
WHERE mi.is_active = TRUE
    AND mc.is_active = TRUE
ORDER BY mc.display_order,
    mi.display_order;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE menu_items IS 'Menu items with pricing and customization options';
COMMENT ON TABLE modifiers IS 'Customization options (milk type, syrups, etc.)';
COMMENT ON TABLE recipes IS 'Preparation instructions and ingredient lists';
COMMENT ON TABLE ingredients IS 'Raw ingredients for inventory tracking';