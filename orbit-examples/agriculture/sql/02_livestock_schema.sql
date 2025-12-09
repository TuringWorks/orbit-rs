-- Livestock Management (PostgreSQL)
-- Core registries for animals, health records, and grazing schedules.
CREATE SCHEMA IF NOT EXISTS agriculture;
SET search_path TO agriculture,
    public;
-- 1. Herds
CREATE TABLE IF NOT EXISTS herds (
    herd_id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    -- e.g., "North Pasture Cattle"
    type VARCHAR(50) NOT NULL,
    -- CATTLE, SHEEP, GOATS
    location VARCHAR(100)
);
-- 2. Animals
CREATE TABLE IF NOT EXISTS animals (
    animal_id SERIAL PRIMARY KEY,
    tag_id VARCHAR(50) UNIQUE NOT NULL,
    -- RFID/Ear Tag
    herd_id INTEGER REFERENCES herds(herd_id),
    breed VARCHAR(50),
    birth_date DATE,
    gender VARCHAR(10),
    -- M, F
    weight_kg DECIMAL(6, 2),
    status VARCHAR(20) DEFAULT 'HEALTHY' -- HEALTHY, SICK, SOLD
);
-- 3. Veterinary Records
CREATE TABLE IF NOT EXISTS vet_records (
    record_id SERIAL PRIMARY KEY,
    animal_id INTEGER REFERENCES animals(animal_id),
    visit_date DATE NOT NULL,
    vet_name VARCHAR(100),
    diagnosis TEXT,
    treatment TEXT,
    cost DECIMAL(10, 2)
);
-- Data Seeding
INSERT INTO herds (name, type)
VALUES ('Angus Beef Herd A', 'CATTLE');
INSERT INTO animals (tag_id, herd_id, breed, gender, weight_kg)
VALUES ('TAG-001', 1, 'Angus', 'F', 650.50),
    ('TAG-002', 1, 'Angus', 'M', 720.00);