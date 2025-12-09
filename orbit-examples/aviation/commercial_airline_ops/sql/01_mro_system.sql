-- ============================================================================
-- OrbitRS Aviation Examples - Commercial Airline Ops (SQL)
-- ============================================================================
-- Maintenance, Repair, and Overhaul (MRO) System
-- ============================================================================
CREATE TABLE aircraft_fleet (
    tail_number VARCHAR(10) PRIMARY KEY,
    -- e.g., 'N12345'
    model VARCHAR(50),
    -- 'Boeing 737-800'
    serial_number VARCHAR(50),
    manufacture_date DATE,
    total_flight_hours DECIMAL(10, 2),
    total_cycles INTEGER,
    last_c_check_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE' -- ACTIVE, MAINTENANCE, AOG (Aircraft On Ground), RETIRED
);
CREATE TABLE maintenance_tasks (
    task_id SERIAL PRIMARY KEY,
    tail_number VARCHAR(10) REFERENCES aircraft_fleet(tail_number),
    task_code VARCHAR(20),
    -- e.g., 'ATA-32-10-01' (Landing Gear)
    description TEXT,
    priority VARCHAR(10),
    -- A, B, C, AOG
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    due_date TIMESTAMP,
    status VARCHAR(20) DEFAULT 'OPEN' -- OPEN, IN_PROGRESS, COMPLETED, DEFERRED
);
CREATE TABLE parts_inventory (
    part_number VARCHAR(50) PRIMARY KEY,
    description VARCHAR(100),
    quantity_on_hand INTEGER,
    min_stock_level INTEGER,
    warehouse_location VARCHAR(20)
);
CREATE TABLE work_orders (
    work_order_id SERIAL PRIMARY KEY,
    task_id INTEGER REFERENCES maintenance_tasks(task_id),
    technician_id INTEGER,
    start_time TIMESTAMP,
    end_time TIMESTAMP,
    sign_off_status VARCHAR(20)
);
-- Seed Data
INSERT INTO aircraft_fleet (tail_number, model, total_flight_hours, status)
VALUES ('N787BA', 'Boeing 787-9', 12500.5, 'ACTIVE'),
    (
        'N737MX',
        'Boeing 737-MAX8',
        3400.0,
        'MAINTENANCE'
    );
INSERT INTO maintenance_tasks (
        tail_number,
        task_code,
        description,
        priority,
        status
    )
VALUES (
        'N737MX',
        'ATA-24-00',
        'Electrical Power Diagnostic',
        'AOG',
        'IN_PROGRESS'
    );
-- Query: Fleet Status Dashboard
SELECT status,
    COUNT(*) as count,
    AVG(total_flight_hours) as avg_hours
FROM aircraft_fleet
GROUP BY status;
-- Query: Critical Open Tasks
SELECT mj.task_code,
    mj.description,
    af.tail_number,
    mj.due_date
FROM maintenance_tasks mj
    JOIN aircraft_fleet af ON mj.tail_number = af.tail_number
WHERE mj.status != 'COMPLETED'
    AND mj.priority = 'AOG';