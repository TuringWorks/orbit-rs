-- Legal Core Schema (PostgreSQL)
-- Handles clients, matters, and billing.
CREATE SCHEMA IF NOT EXISTS legal;
SET search_path TO legal,
    public;
-- 1. Clients
CREATE TABLE IF NOT EXISTS clients (
    client_id SERIAL PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    type VARCHAR(20) DEFAULT 'INDIVIDUAL',
    -- INDIVIDUAL, CORPORATION
    contact_email VARCHAR(100),
    created_at DATE DEFAULT CURRENT_DATE
);
-- 2. Matters (Cases)
CREATE TABLE IF NOT EXISTS matters (
    matter_id SERIAL PRIMARY KEY,
    client_id INTEGER REFERENCES clients(client_id),
    title VARCHAR(200) NOT NULL,
    status VARCHAR(20) DEFAULT 'OPEN',
    -- OPEN, CLOSED, PENDING
    practice_area VARCHAR(50),
    -- Litigation, Corporate, IP
    opened_date DATE DEFAULT CURRENT_DATE
);
-- 3. Time Entries (Billing)
CREATE TABLE IF NOT EXISTS time_entries (
    entry_id SERIAL PRIMARY KEY,
    matter_id INTEGER REFERENCES matters(matter_id),
    attorney_id INTEGER,
    hours DECIMAL(5, 2) NOT NULL,
    rate DECIMAL(10, 2) NOT NULL,
    description TEXT,
    date_worked DATE NOT NULL
);
-- Data Seeding
INSERT INTO clients (name, type)
VALUES ('Acme Corp', 'CORPORATION');
INSERT INTO matters (client_id, title, practice_area)
VALUES (1, 'Acme v. Coyote', 'Litigation');
INSERT INTO time_entries (matter_id, hours, rate, description, date_worked)
VALUES (
        1,
        2.5,
        450.00,
        'Drafting complaint',
        CURRENT_DATE
    );