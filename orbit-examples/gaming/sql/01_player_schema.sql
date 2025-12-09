-- Gaming Core Schema (PostgreSQL)
-- Handles critical player data, inventory, and currency.
CREATE SCHEMA IF NOT EXISTS gaming;
SET search_path TO gaming,
    public;
-- 1. Players Table
CREATE TABLE IF NOT EXISTS players (
    player_id SERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    last_login TIMESTAMP WITH TIME ZONE,
    premium_status BOOLEAN DEFAULT FALSE
);
-- 2. Currencies (Gold, Gems)
CREATE TABLE IF NOT EXISTS currencies (
    player_id INTEGER REFERENCES players(player_id),
    gold BIGINT DEFAULT 0 CHECK (gold >= 0),
    gems INTEGER DEFAULT 0 CHECK (gems >= 0),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (player_id)
);
-- 3. Inventory Items
CREATE TABLE IF NOT EXISTS inventory (
    inventory_id SERIAL PRIMARY KEY,
    player_id INTEGER REFERENCES players(player_id),
    item_id VARCHAR(50) NOT NULL,
    -- e.g., 'sword_001', 'potion_heal'
    quantity INTEGER DEFAULT 1 CHECK (quantity > 0),
    metadata JSONB,
    -- Store enchantments, durability, etc.
    acquired_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(player_id, item_id)
);
-- Data Seeding
INSERT INTO players (username, email, premium_status)
VALUES ('Slayer99', 'slayer@example.com', true),
    ('HealerPro', 'healer@example.com', false),
    ('TankMaster', 'tank@example.com', true) ON CONFLICT DO NOTHING;
INSERT INTO currencies (player_id, gold, gems)
SELECT player_id,
    1000,
    50
FROM players
WHERE username = 'Slayer99' ON CONFLICT DO NOTHING;
INSERT INTO inventory (player_id, item_id, quantity, metadata)
SELECT player_id,
    'sword_legendary',
    1,
    '{"durability": 100, "enchantments": ["fire"]}'::jsonb
FROM players
WHERE username = 'Slayer99' ON CONFLICT DO NOTHING;