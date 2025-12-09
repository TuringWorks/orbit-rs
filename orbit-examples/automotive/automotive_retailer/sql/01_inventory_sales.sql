-- ============================================================================
-- OrbitRS Car Dealership Examples - Sales System (SQL)
-- ============================================================================
-- Inventory, Sales Transactions, and Finance
-- ============================================================================
CREATE TABLE vehicles (
    vin VARCHAR(17) PRIMARY KEY,
    make VARCHAR(50) NOT NULL,
    model VARCHAR(50) NOT NULL,
    year INTEGER NOT NULL,
    color VARCHAR(30),
    trim_level VARCHAR(50),
    status VARCHAR(20) DEFAULT 'IN_STOCK',
    -- IN_STOCK, PENDING_SALE, SOLD, TRADE_IN
    purchase_cost DECIMAL(10, 2),
    -- Dealer cost
    list_price DECIMAL(10, 2),
    -- Sticker price
    arrival_date DATE DEFAULT CURRENT_DATE,
    days_in_inventory INTEGER GENERATED ALWAYS AS (CURRENT_DATE - arrival_date) STORED
);
CREATE TABLE sales_staff (
    staff_id SERIAL PRIMARY KEY,
    first_name VARCHAR(50),
    last_name VARCHAR(50),
    role VARCHAR(30) -- SALES_REP, FINANCE_MANAGER
);
CREATE TABLE customers (
    customer_id SERIAL PRIMARY KEY,
    first_name VARCHAR(50),
    last_name VARCHAR(50),
    email VARCHAR(100),
    phone VARCHAR(20)
);
CREATE TABLE sales_orders (
    order_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customers(customer_id),
    vin VARCHAR(17) REFERENCES vehicles(vin),
    sales_rep_id INTEGER REFERENCES sales_staff(staff_id),
    finance_manager_id INTEGER REFERENCES sales_staff(staff_id),
    sale_date DATE DEFAULT CURRENT_DATE,
    sale_price DECIMAL(10, 2),
    trade_in_value DECIMAL(10, 2) DEFAULT 0.00,
    taxes_fees DECIMAL(10, 2),
    total_amount DECIMAL(10, 2),
    finance_type VARCHAR(20),
    -- CASH, FINANCE, LEASE
    finance_term_months INTEGER,
    interest_rate DECIMAL(4, 2)
);
-- Seed Data
INSERT INTO vehicles (
        vin,
        make,
        model,
        year,
        trim_level,
        status,
        list_price
    )
VALUES (
        '1HGCM82633A001234',
        'Honda',
        'Accord',
        2024,
        'Touring',
        'IN_STOCK',
        38500.00
    ),
    (
        '1G1YY22U055123456',
        'Chevrolet',
        'Corvette',
        2023,
        'Stingray',
        'PENDING_SALE',
        75000.00
    ),
    (
        'JN1AZ4EH1DM123456',
        'Nissan',
        'Rogue',
        2024,
        'Platinum',
        'SOLD',
        36000.00
    );
INSERT INTO sales_staff (first_name, last_name, role)
VALUES ('David', 'Salesman', 'SALES_REP'),
    ('Sarah', 'Finance', 'FINANCE_MANAGER');
INSERT INTO customers (first_name, last_name, email)
VALUES ('Michael', 'Buyer', 'mike@example.com');
-- Record a Sale
INSERT INTO sales_orders (
        customer_id,
        vin,
        sales_rep_id,
        finance_manager_id,
        sale_price,
        taxes_fees,
        total_amount,
        finance_type
    )
VALUES (
        1,
        'JN1AZ4EH1DM123456',
        1,
        2,
        35000.00,
        3500.00,
        38500.00,
        'CASH'
    );
-- Query: Inventory Aging Report (Cars > 60 days old need discounts)
SELECT vin,
    make,
    model,
    year,
    days_in_inventory,
    list_price
FROM vehicles
WHERE status = 'IN_STOCK'
    AND arrival_date < CURRENT_DATE - INTERVAL '60 days'
ORDER BY days_in_inventory DESC;
-- Query: Monthly Sales Commission Report
SELECT s.first_name,
    s.last_name,
    COUNT(so.order_id) as units_sold,
    SUM(so.sale_price) as total_volume,
    SUM(so.sale_price) * 0.02 as estimated_commission -- 2% flat comission
FROM sales_staff s
    JOIN sales_orders so ON s.staff_id = so.sales_rep_id
WHERE so.sale_date >= DATE_TRUNC('month', CURRENT_DATE)
GROUP BY s.staff_id;