-- ============================================================================
-- OrbitRS Field Service Examples - Work Order System (SQL)
-- ============================================================================
-- Dispatch, Jobs, and Invoicing
-- ============================================================================
CREATE TABLE technicians (
    tech_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    email VARCHAR(100),
    phone VARCHAR(20),
    hourly_rate DECIMAL(6, 2),
    home_zip_code VARCHAR(10),
    is_active BOOLEAN DEFAULT TRUE
);
CREATE TABLE customers (
    customer_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    address VARCHAR(255),
    city VARCHAR(100),
    zip_code VARCHAR(10),
    phone VARCHAR(20)
);
CREATE TABLE job_types (
    job_type_id SERIAL PRIMARY KEY,
    name VARCHAR(50),
    -- e.g., 'HVAC Install', 'Leak Repair'
    estimated_hours DECIMAL(4, 2),
    base_charge DECIMAL(8, 2)
);
CREATE TABLE work_orders (
    order_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customers(customer_id),
    job_type_id INTEGER REFERENCES job_types(job_type_id),
    assigned_tech_id INTEGER REFERENCES technicians(tech_id),
    status VARCHAR(20) DEFAULT 'SCHEDULED',
    -- SCHEDULED, EN_ROUTE, IN_PROGRESS, COMPLETED, CANCELLED
    scheduled_start TIMESTAMP,
    scheduled_end TIMESTAMP,
    actual_start TIMESTAMP,
    actual_end TIMESTAMP,
    notes TEXT,
    total_invoice_amount DECIMAL(10, 2)
);
-- Seed Data
INSERT INTO technicians (name, hourly_rate, home_zip_code)
VALUES ('Mario Plumber', 120.00, '90210'),
    ('Luigi Green', 115.00, '90211');
INSERT INTO customers (name, address, city, zip_code)
VALUES (
        'Princess Toadstool',
        '1 Castle Dr',
        'Mushroom Kingdom',
        '90210'
    );
INSERT INTO job_types (name, estimated_hours, base_charge)
VALUES ('Pipe Leak Repair', 2.0, 150.00),
    ('Water Heater Install', 4.0, 800.00);
-- Schedule a Job
INSERT INTO work_orders (
        customer_id,
        job_type_id,
        assigned_tech_id,
        scheduled_start,
        scheduled_end,
        status
    )
VALUES (
        1,
        1,
        1,
        '2024-06-01 09:00:00',
        '2024-06-01 11:00:00',
        'SCHEDULED'
    );
-- Query: Tech Schedule for Today
SELECT t.name as technician,
    c.name as customer,
    c.address,
    jt.name as task,
    wo.scheduled_start,
    wo.status
FROM work_orders wo
    JOIN technicians t ON wo.assigned_tech_id = t.tech_id
    JOIN customers c ON wo.customer_id = c.customer_id
    JOIN job_types jt ON wo.job_type_id = jt.job_type_id
WHERE DATE(wo.scheduled_start) = '2024-06-01'
ORDER BY wo.scheduled_start;