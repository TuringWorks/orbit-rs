-- Robotics Fleet Management (PostgreSQL)
-- Core registry for autonomous mobile robots (AMRs).
CREATE SCHEMA IF NOT EXISTS robotics;
SET search_path TO robotics,
    public;
-- 1. Robot Models
CREATE TABLE IF NOT EXISTS models (
    model_id SERIAL PRIMARY KEY,
    name VARCHAR(50) NOT NULL,
    -- e.g. "Lifter-500", "Picker-100"
    manufacturer VARCHAR(50),
    max_load_kg DECIMAL(6, 2)
);
-- 2. Robots
CREATE TABLE IF NOT EXISTS robots (
    robot_id SERIAL PRIMARY KEY,
    serial_number VARCHAR(50) UNIQUE NOT NULL,
    model_id INTEGER REFERENCES models(model_id),
    firmware_version VARCHAR(20),
    status VARCHAR(20) DEFAULT 'IDLE',
    -- IDLE, MISSION, ERROR, CHARGING
    commissioned_date DATE DEFAULT CURRENT_DATE
);
-- 3. Maintenance Events
CREATE TABLE IF NOT EXISTS maintenance (
    event_id SERIAL PRIMARY KEY,
    robot_id INTEGER REFERENCES robots(robot_id),
    event_type VARCHAR(50),
    -- BATTERY_REPLACEMENT, MOTOR_CALIB
    technician VARCHAR(50),
    date_performed TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
-- Data Seeding
INSERT INTO models (name, manufacturer, max_load_kg)
VALUES ('TitanLift', 'OrbitRobotics', 1000.00);
INSERT INTO robots (serial_number, model_id, firmware_version)
VALUES ('R-101', 1, 'v2.4.1');