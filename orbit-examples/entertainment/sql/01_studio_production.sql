-- Studio Production & Rights Management
-- 1. Productions Table
-- Tracks the lifecycle of a movie or functional series from greenlight to release.
CREATE TABLE productions (
    production_id SERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    type VARCHAR(50) CHECK (
        type IN ('Movie', 'Series', 'Miniseries', 'Documentary')
    ),
    status VARCHAR(50) CHECK (
        status IN (
            'Development',
            'Pre-Production',
            'Filming',
            'Post-Production',
            'Completed',
            'Released',
            'Canceled'
        )
    ),
    start_date DATE,
    release_date DATE,
    target_budget DECIMAL(15, 2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- 2. Budgets & Expenses
-- Detailed tracking of production finances.
CREATE TABLE budgets (
    budget_id SERIAL PRIMARY KEY,
    production_id INT REFERENCES productions(production_id),
    department VARCHAR(100) (
        e.g.,
        'Above the Line',
        'Production',
        'Post-Production'
    ),
    allocated_amount DECIMAL(15, 2),
    spent_amount DECIMAL(15, 2) DEFAULT 0.00,
    currency VARCHAR(3) DEFAULT 'USD'
);
-- 3. Talent Contracts
-- Managing cast and crew agreements.
CREATE TABLE talent_contracts (
    contract_id SERIAL PRIMARY KEY,
    production_id INT REFERENCES productions(production_id),
    talent_name VARCHAR(255) NOT NULL,
    role VARCHAR(100) (e.g., 'Actor', 'Director', 'Writer'),
    agent_contact VARCHAR(255),
    compensation DECIMAL(15, 2),
    start_date DATE,
    end_date DATE,
    terms TEXT -- Legal terms summary
);
-- 4. Distribution Rights
-- Where and when content can be shown.
CREATE TABLE distribution_rights (
    right_id SERIAL PRIMARY KEY,
    production_id INT REFERENCES productions(production_id),
    region_code VARCHAR(10) (e.g., 'US', 'EU', 'APAC'),
    distributor_name VARCHAR(255),
    license_start DATE,
    license_end DATE,
    exclusivity BOOLEAN DEFAULT FALSE
);
-- Seed Data: Sample Production "Galaxy Quest 2"
INSERT INTO productions (title, type, status, start_date, target_budget)
VALUES (
        'Galaxy Quest 2',
        'Movie',
        'Pre-Production',
        '2025-06-01',
        150000000.00
    );
-- Seed Data: Budget for "Galaxy Quest 2"
INSERT INTO budgets (production_id, department, allocated_amount)
VALUES (
        (
            SELECT production_id
            FROM productions
            WHERE title = 'Galaxy Quest 2'
        ),
        'Above the Line',
        50000000.00
    ),
    (
        (
            SELECT production_id
            FROM productions
            WHERE title = 'Galaxy Quest 2'
        ),
        'VFX',
        40000000.00
    );
-- Analysis Query: Budget Utilization
SELECT p.title,
    b.department,
    b.allocated_amount,
    b.spent_amount,
    (b.spent_amount / b.allocated_amount) * 100 as utilization_percentage
FROM productions p
    JOIN budgets b ON p.production_id = b.production_id
WHERE p.status = 'Pre-Production';