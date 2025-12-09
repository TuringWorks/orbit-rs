-- Semiconductor MES (PostgreSQL)
-- Manufacturing Execution System: Tracking Lots, Wafers, and Recipes.
CREATE SCHEMA IF NOT EXISTS semiconductor;
SET search_path TO semiconductor,
    public;
-- 1. Products (Device Types)
CREATE TABLE IF NOT EXISTS products (
    product_id SERIAL PRIMARY KEY,
    name VARCHAR(50) NOT NULL,
    -- e.g. "CPU-X1"
    process_flow_id VARCHAR(50) -- "5nm_FinFET_Main"
);
-- 2. Lots (Batches of Wafers)
CREATE TABLE IF NOT EXISTS lots (
    lot_id VARCHAR(50) PRIMARY KEY,
    -- e.g. "L8842.1"
    product_id INTEGER REFERENCES products(product_id),
    priority INTEGER DEFAULT 1,
    current_step VARCHAR(100),
    -- "Litho_Layer_1"
    status VARCHAR(20) DEFAULT 'ACTIVE',
    -- ACTIVE, HOLD, SCRAP
    start_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- 3. Equipment (Tools)
CREATE TABLE IF NOT EXISTS tools (
    tool_id VARCHAR(50) PRIMARY KEY,
    -- "ETCH-01"
    type VARCHAR(20),
    -- LITH, ETCH, DEP
    state VARCHAR(20) DEFAULT 'UP' -- UP, DOWN, PM
);
-- 4. Process History (Move History)
CREATE TABLE IF NOT EXISTS moves (
    move_id SERIAL PRIMARY KEY,
    lot_id VARCHAR(50) REFERENCES lots(lot_id),
    step_id VARCHAR(100),
    tool_id VARCHAR(50) REFERENCES tools(tool_id),
    time_in TIMESTAMP,
    time_out TIMESTAMP,
    recipe_name VARCHAR(100)
);
-- Data Seeding
INSERT INTO products (name)
VALUES ('GPU-AI-100');
INSERT INTO tools (tool_id, type)
VALUES ('LITHO-ASML-01', 'LITH'),
    ('ETCH-LAM-01', 'ETCH');
INSERT INTO lots (lot_id, product_id, current_step)
VALUES ('L1000.1', 1, 'Etch_Gate');