-- ============================================================================
-- Logistics & Transportation ML Industry Example - Orbit-RS
-- ============================================================================
-- Use Case: Route Optimization, Delivery Time Prediction, Fleet Management,
--           Demand Forecasting, Warehouse Optimization
-- Protocols: PostgreSQL (SQL), Vector Search, Time Series, Spatial Functions
-- ============================================================================

-- ----------------------------------------------------------------------------
-- 1. ROUTE OPTIMIZATION & DELIVERY TIME PREDICTION
-- ----------------------------------------------------------------------------

-- Create delivery locations table with spatial data
CREATE TABLE IF NOT EXISTS delivery_locations (
    location_id SERIAL PRIMARY KEY,
    location_name VARCHAR(100),
    address VARCHAR(200),
    latitude DECIMAL(10, 8),
    longitude DECIMAL(11, 8),
    location_type VARCHAR(50), -- 'WAREHOUSE', 'CUSTOMER', 'HUB'
    avg_delivery_time_minutes INTEGER,
    location_embedding vector(64) -- Spatial + contextual embedding
);

-- Insert sample locations
INSERT INTO delivery_locations (location_name, address, latitude, longitude, location_type, 
                                avg_delivery_time_minutes, location_embedding) VALUES
('Main Warehouse', '123 Industrial Blvd, San Francisco, CA', 37.7749, -122.4194, 'WAREHOUSE', 0, 
 array_fill(random()::float, ARRAY[64])::vector(64)),
('Distribution Hub North', '456 Hub St, Oakland, CA', 37.8044, -122.2712, 'HUB', 25,
 array_fill(random()::float, ARRAY[64])::vector(64)),
('Customer A', '789 Market St, San Francisco, CA', 37.7849, -122.4094, 'CUSTOMER', 35,
 array_fill(random()::float, ARRAY[64])::vector(64)),
('Customer B', '321 Mission St, San Francisco, CA', 37.7899, -122.3974, 'CUSTOMER', 40,
 array_fill(random()::float, ARRAY[64])::vector(64)),
('Customer C', '555 Broadway, Oakland, CA', 37.8144, -122.2644, 'CUSTOMER', 55,
 array_fill(random()::float, ARRAY[64])::vector(64));

-- Create deliveries table
CREATE TABLE IF NOT EXISTS deliveries (
    delivery_id SERIAL PRIMARY KEY,
    order_id VARCHAR(50) UNIQUE,
    customer_id INTEGER,
    origin_location_id INTEGER REFERENCES delivery_locations(location_id),
    destination_location_id INTEGER REFERENCES delivery_locations(location_id),
    scheduled_time TIMESTAMP,
    actual_delivery_time TIMESTAMP,
    predicted_delivery_time TIMESTAMP,
    distance_km DECIMAL(10, 2),
    estimated_duration_minutes INTEGER,
    actual_duration_minutes INTEGER,
    driver_id INTEGER,
    vehicle_id INTEGER,
    package_weight_kg DECIMAL(8, 2),
    delivery_status VARCHAR(50), -- 'PENDING', 'IN_TRANSIT', 'DELIVERED', 'DELAYED'
    delay_minutes INTEGER,
    traffic_level VARCHAR(20) -- 'LOW', 'MEDIUM', 'HIGH'
);

-- Insert sample deliveries
INSERT INTO deliveries (order_id, customer_id, origin_location_id, destination_location_id,
                       scheduled_time, distance_km, package_weight_kg, delivery_status, traffic_level) VALUES
('ORD-001', 1001, 1, 3, CURRENT_TIMESTAMP + INTERVAL '2 hours', 5.2, 2.5, 'PENDING', 'MEDIUM'),
('ORD-002', 1002, 1, 4, CURRENT_TIMESTAMP + INTERVAL '3 hours', 6.8, 5.0, 'PENDING', 'HIGH'),
('ORD-003', 1003, 2, 5, CURRENT_TIMESTAMP + INTERVAL '1 hour', 3.5, 1.2, 'IN_TRANSIT', 'LOW'),
('ORD-004', 1004, 1, 3, CURRENT_TIMESTAMP - INTERVAL '1 hour', 5.2, 3.0, 'DELIVERED', 'MEDIUM');

-- Calculate distance between locations using Haversine formula
CREATE OR REPLACE FUNCTION calculate_distance_km(
    lat1 DECIMAL, lon1 DECIMAL,
    lat2 DECIMAL, lon2 DECIMAL
) RETURNS DECIMAL AS $$
DECLARE
    earth_radius_km CONSTANT DECIMAL := 6371.0;
    dlat DECIMAL;
    dlon DECIMAL;
    a DECIMAL;
    c DECIMAL;
BEGIN
    dlat := radians(lat2 - lat1);
    dlon := radians(lon2 - lon1);
    
    a := sin(dlat/2) * sin(dlat/2) + 
         cos(radians(lat1)) * cos(radians(lat2)) * 
         sin(dlon/2) * sin(dlon/2);
    c := 2 * atan2(sqrt(a), sqrt(1-a));
    
    RETURN earth_radius_km * c;
END;
$$ LANGUAGE plpgsql;

-- ML-based delivery time prediction
CREATE OR REPLACE FUNCTION predict_delivery_time(
    p_distance_km DECIMAL,
    p_traffic_level VARCHAR,
    p_package_weight_kg DECIMAL,
    p_time_of_day INTEGER
) RETURNS INTEGER AS $$
DECLARE
    base_time_minutes INTEGER;
    traffic_multiplier DECIMAL;
    weight_factor DECIMAL;
    time_factor DECIMAL;
BEGIN
    -- Base time: 10 minutes per km
    base_time_minutes := (p_distance_km * 10)::INTEGER;
    
    -- Traffic adjustment
    traffic_multiplier := CASE p_traffic_level
        WHEN 'LOW' THEN 1.0
        WHEN 'MEDIUM' THEN 1.3
        WHEN 'HIGH' THEN 1.7
        ELSE 1.0
    END;
    
    -- Weight factor (heavier packages take longer)
    weight_factor := 1.0 + (p_package_weight_kg * 0.02);
    
    -- Time of day factor (rush hours)
    time_factor := CASE 
        WHEN p_time_of_day BETWEEN 7 AND 9 THEN 1.3
        WHEN p_time_of_day BETWEEN 17 AND 19 THEN 1.4
        ELSE 1.0
    END;
    
    RETURN (base_time_minutes * traffic_multiplier * weight_factor * time_factor)::INTEGER;
END;
$$ LANGUAGE plpgsql;

-- Update predicted delivery times
UPDATE deliveries d
SET 
    estimated_duration_minutes = predict_delivery_time(
        d.distance_km,
        d.traffic_level,
        d.package_weight_kg,
        EXTRACT(HOUR FROM d.scheduled_time)::INTEGER
    ),
    predicted_delivery_time = d.scheduled_time + 
        (predict_delivery_time(
            d.distance_km,
            d.traffic_level,
            d.package_weight_kg,
            EXTRACT(HOUR FROM d.scheduled_time)::INTEGER
        ) || ' minutes')::INTERVAL
WHERE d.delivery_status IN ('PENDING', 'IN_TRANSIT');

-- Find optimal delivery routes using nearest neighbor
WITH route_optimization AS (
    SELECT 
        d.delivery_id,
        d.order_id,
        d.destination_location_id,
        dl_dest.location_name,
        dl_dest.latitude,
        dl_dest.longitude,
        d.scheduled_time,
        d.estimated_duration_minutes,
        ROW_NUMBER() OVER (ORDER BY d.scheduled_time) AS delivery_sequence
    FROM deliveries d
    JOIN delivery_locations dl_dest ON d.destination_location_id = dl_dest.location_id
    WHERE d.delivery_status = 'PENDING'
)
SELECT 
    delivery_id,
    order_id,
    location_name,
    delivery_sequence,
    scheduled_time,
    estimated_duration_minutes
FROM route_optimization
ORDER BY delivery_sequence;

SELECT ML_TRAIN_MODEL(
  'delivery_eta_lr',
  'linear_regression',
  ARRAY[
    distance_km,
    CASE traffic_level WHEN 'LOW' THEN 1 WHEN 'MEDIUM' THEN 2 ELSE 3 END,
    package_weight_kg,
    EXTRACT(HOUR FROM scheduled_time)::INTEGER
  ],
  actual_duration_minutes
) FROM deliveries
WHERE actual_duration_minutes IS NOT NULL;
UPDATE deliveries
SET estimated_duration_minutes = ML_PREDICT(
  'delivery_eta_lr',
  ARRAY[
    distance_km,
    CASE traffic_level WHEN 'LOW' THEN 1 WHEN 'MEDIUM' THEN 2 ELSE 3 END,
    package_weight_kg,
    EXTRACT(HOUR FROM scheduled_time)::INTEGER
  ]
);

-- ----------------------------------------------------------------------------
-- 2. FLEET MANAGEMENT & VEHICLE TRACKING
-- ----------------------------------------------------------------------------

-- Create vehicles table
CREATE TABLE IF NOT EXISTS vehicles (
    vehicle_id SERIAL PRIMARY KEY,
    vehicle_number VARCHAR(50) UNIQUE,
    vehicle_type VARCHAR(50), -- 'VAN', 'TRUCK', 'MOTORCYCLE'
    capacity_kg DECIMAL(10, 2),
    fuel_type VARCHAR(20), -- 'GASOLINE', 'DIESEL', 'ELECTRIC'
    current_location_lat DECIMAL(10, 8),
    current_location_lon DECIMAL(11, 8),
    status VARCHAR(20) DEFAULT 'AVAILABLE', -- 'AVAILABLE', 'IN_USE', 'MAINTENANCE'
    odometer_km INTEGER,
    last_maintenance_km INTEGER,
    maintenance_due_km INTEGER,
    fuel_efficiency_kmpl DECIMAL(5, 2)
);

-- Insert sample vehicles
INSERT INTO vehicles (vehicle_number, vehicle_type, capacity_kg, fuel_type, 
                     current_location_lat, current_location_lon, odometer_km, 
                     last_maintenance_km, maintenance_due_km, fuel_efficiency_kmpl) VALUES
('VAN-001', 'VAN', 1000, 'GASOLINE', 37.7749, -122.4194, 45000, 40000, 50000, 12.5),
('TRUCK-001', 'TRUCK', 5000, 'DIESEL', 37.8044, -122.2712, 120000, 115000, 125000, 8.0),
('VAN-002', 'VAN', 1000, 'ELECTRIC', 37.7849, -122.4094, 25000, 20000, 30000, 0), -- Electric
('MOTO-001', 'MOTORCYCLE', 50, 'GASOLINE', 37.7899, -122.3974, 15000, 10000, 20000, 35.0);

-- Create vehicle telemetry time series
CREATE TABLE IF NOT EXISTS vehicle_telemetry (
    telemetry_id SERIAL PRIMARY KEY,
    vehicle_id INTEGER REFERENCES vehicles(vehicle_id),
    timestamp BIGINT NOT NULL,
    latitude DECIMAL(10, 8),
    longitude DECIMAL(11, 8),
    speed_kmh DECIMAL(6, 2),
    fuel_level_pct DECIMAL(5, 2),
    engine_temperature_celsius DECIMAL(5, 2),
    odometer_reading_km INTEGER,
    is_idle BOOLEAN DEFAULT FALSE
);

-- Insert sample telemetry data (last 24 hours)
INSERT INTO vehicle_telemetry (vehicle_id, timestamp, latitude, longitude, speed_kmh, 
                               fuel_level_pct, engine_temperature_celsius, odometer_reading_km, is_idle)
SELECT 
    1,
    extract(epoch from CURRENT_TIMESTAMP - (n || ' minutes')::INTERVAL)::bigint * 1000,
    37.7749 + (random() * 0.1 - 0.05),
    -122.4194 + (random() * 0.1 - 0.05),
    (random() * 60)::DECIMAL(6, 2),
    (100 - n * 0.05)::DECIMAL(5, 2),
    (random() * 20 + 80)::DECIMAL(5, 2),
    45000 + (n * 0.5)::INTEGER,
    (random() > 0.8)
FROM generate_series(1, 1440) AS n;

-- Detect vehicle anomalies
WITH vehicle_stats AS (
    SELECT 
        vehicle_id,
        AVG(speed_kmh) AS avg_speed,
        MAX(speed_kmh) AS max_speed,
        AVG(engine_temperature_celsius) AS avg_temp,
        MAX(engine_temperature_celsius) AS max_temp,
        SUM(CASE WHEN is_idle THEN 1 ELSE 0 END)::FLOAT / COUNT(*) * 100 AS idle_time_pct
    FROM vehicle_telemetry
    WHERE timestamp > extract(epoch from CURRENT_TIMESTAMP - INTERVAL '24 hours')::bigint * 1000
    GROUP BY vehicle_id
)
SELECT 
    v.vehicle_number,
    v.vehicle_type,
    vs.avg_speed,
    vs.max_speed,
    vs.avg_temp,
    vs.max_temp,
    vs.idle_time_pct,
    CASE 
        WHEN vs.max_temp > 110 THEN 'OVERHEATING ALERT'
        WHEN vs.max_speed > 100 THEN 'SPEEDING ALERT'
        WHEN vs.idle_time_pct > 30 THEN 'EXCESSIVE IDLING'
        ELSE 'NORMAL'
    END AS alert_status
FROM vehicles v
JOIN vehicle_stats vs ON v.vehicle_id = vs.vehicle_id
WHERE vs.max_temp > 110 OR vs.max_speed > 100 OR vs.idle_time_pct > 30;

-- Vehicle maintenance prediction
SELECT 
    vehicle_id,
    vehicle_number,
    vehicle_type,
    odometer_km,
    maintenance_due_km,
    maintenance_due_km - odometer_km AS km_until_maintenance,
    CASE 
        WHEN odometer_km >= maintenance_due_km THEN 'OVERDUE - Schedule Immediately'
        WHEN maintenance_due_km - odometer_km < 1000 THEN 'DUE SOON - Schedule This Week'
        WHEN maintenance_due_km - odometer_km < 3000 THEN 'UPCOMING - Plan Ahead'
        ELSE 'NOT DUE'
    END AS maintenance_status
FROM vehicles
WHERE odometer_km >= maintenance_due_km - 3000
ORDER BY km_until_maintenance ASC;

-- ----------------------------------------------------------------------------
-- 3. DEMAND FORECASTING - TIME SERIES
-- ----------------------------------------------------------------------------

-- Create delivery demand history
CREATE TABLE IF NOT EXISTS delivery_demand (
    demand_id SERIAL PRIMARY KEY,
    demand_date DATE NOT NULL,
    hour_of_day INTEGER,
    num_deliveries INTEGER,
    total_packages INTEGER,
    total_weight_kg DECIMAL(10, 2),
    avg_distance_km DECIMAL(8, 2),
    day_of_week INTEGER, -- 0=Sunday, 6=Saturday
    is_holiday BOOLEAN DEFAULT FALSE,
    weather_condition VARCHAR(50) -- 'CLEAR', 'RAIN', 'SNOW'
);

-- Insert sample demand data (last 90 days)
INSERT INTO delivery_demand (demand_date, hour_of_day, num_deliveries, total_packages, 
                             total_weight_kg, avg_distance_km, day_of_week, weather_condition)
SELECT 
    CURRENT_DATE - (n || ' days')::INTERVAL,
    h,
    (random() * 50 + 20)::INTEGER,
    (random() * 100 + 50)::INTEGER,
    (random() * 500 + 200)::DECIMAL(10, 2),
    (random() * 10 + 5)::DECIMAL(8, 2),
    EXTRACT(DOW FROM CURRENT_DATE - (n || ' days')::INTERVAL)::INTEGER,
    CASE 
        WHEN random() > 0.8 THEN 'RAIN'
        WHEN random() > 0.95 THEN 'SNOW'
        ELSE 'CLEAR'
    END
FROM generate_series(1, 90) AS n
CROSS JOIN generate_series(8, 20) AS h; -- Business hours 8 AM to 8 PM

-- Calculate demand trends with moving averages
SELECT 
    demand_date,
    SUM(num_deliveries) AS daily_deliveries,
    AVG(SUM(num_deliveries)) OVER (
        ORDER BY demand_date 
        ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
    ) AS ma_7day,
    AVG(SUM(num_deliveries)) OVER (
        ORDER BY demand_date 
        ROWS BETWEEN 29 PRECEDING AND CURRENT ROW
    ) AS ma_30day
FROM delivery_demand
GROUP BY demand_date
ORDER BY demand_date DESC
LIMIT 30;

-- Predict next week's demand by day of week
WITH historical_patterns AS (
    SELECT 
        day_of_week,
        AVG(num_deliveries) AS avg_deliveries,
        STDDEV(num_deliveries) AS stddev_deliveries,
        AVG(total_weight_kg) AS avg_weight
    FROM delivery_demand
    WHERE demand_date > CURRENT_DATE - INTERVAL '60 days'
    GROUP BY day_of_week
)
SELECT 
    day_of_week,
    CASE day_of_week
        WHEN 0 THEN 'Sunday'
        WHEN 1 THEN 'Monday'
        WHEN 2 THEN 'Tuesday'
        WHEN 3 THEN 'Wednesday'
        WHEN 4 THEN 'Thursday'
        WHEN 5 THEN 'Friday'
        WHEN 6 THEN 'Saturday'
    END AS day_name,
    ROUND(avg_deliveries) AS predicted_deliveries,
    ROUND(avg_weight) AS predicted_weight_kg,
    ROUND(avg_deliveries + stddev_deliveries) AS high_estimate,
    ROUND(avg_deliveries - stddev_deliveries) AS low_estimate
FROM historical_patterns
ORDER BY day_of_week;

-- Peak hour analysis for resource allocation
SELECT 
    hour_of_day,
    AVG(num_deliveries) AS avg_deliveries,
    MAX(num_deliveries) AS peak_deliveries,
    CASE 
        WHEN AVG(num_deliveries) > 40 THEN 'HIGH DEMAND - Add Drivers'
        WHEN AVG(num_deliveries) > 30 THEN 'MODERATE DEMAND'
        ELSE 'LOW DEMAND - Reduce Staff'
    END AS staffing_recommendation
FROM delivery_demand
WHERE demand_date > CURRENT_DATE - INTERVAL '30 days'
GROUP BY hour_of_day
ORDER BY hour_of_day;

-- ----------------------------------------------------------------------------
-- 4. WAREHOUSE OPTIMIZATION
-- ----------------------------------------------------------------------------

-- Create warehouse inventory table
CREATE TABLE IF NOT EXISTS warehouse_inventory (
    inventory_id SERIAL PRIMARY KEY,
    warehouse_id INTEGER,
    product_sku VARCHAR(50),
    product_name VARCHAR(200),
    quantity INTEGER,
    location_zone VARCHAR(10), -- 'A1', 'B2', etc.
    picking_frequency INTEGER, -- Times picked per month
    storage_cost_per_unit DECIMAL(8, 2),
    last_picked_date DATE,
    reorder_point INTEGER,
    optimal_zone VARCHAR(10) -- ML-recommended zone
);

-- Insert sample inventory
INSERT INTO warehouse_inventory (warehouse_id, product_sku, product_name, quantity, location_zone,
                                 picking_frequency, storage_cost_per_unit, last_picked_date, reorder_point) VALUES
(1, 'SKU-001', 'Widget A', 500, 'C3', 150, 0.50, CURRENT_DATE - INTERVAL '2 days', 100),
(1, 'SKU-002', 'Widget B', 1000, 'A1', 300, 0.30, CURRENT_DATE, 200),
(1, 'SKU-003', 'Widget C', 50, 'A2', 280, 0.75, CURRENT_DATE - INTERVAL '1 day', 20),
(1, 'SKU-004', 'Widget D', 2000, 'D5', 10, 0.20, CURRENT_DATE - INTERVAL '45 days', 500);

-- ML-based optimal zone recommendation (ABC analysis)
UPDATE warehouse_inventory
SET optimal_zone = CASE 
    WHEN picking_frequency > 200 THEN 'A1' -- High frequency, easy access
    WHEN picking_frequency > 100 THEN 'B2' -- Medium frequency
    WHEN picking_frequency > 50 THEN 'C3'  -- Low frequency
    ELSE 'D5' -- Very low frequency, deep storage
END;

-- Identify misplaced inventory
SELECT 
    product_sku,
    product_name,
    quantity,
    location_zone AS current_zone,
    optimal_zone AS recommended_zone,
    picking_frequency,
    CASE 
        WHEN location_zone != optimal_zone THEN 'RELOCATE'
        ELSE 'OPTIMAL'
    END AS action_required,
    picking_frequency * 2 AS estimated_time_savings_minutes_per_month
FROM warehouse_inventory
WHERE location_zone != optimal_zone
ORDER BY picking_frequency DESC;

-- Inventory reorder recommendations
SELECT 
    product_sku,
    product_name,
    quantity AS current_stock,
    reorder_point,
    picking_frequency,
    CASE 
        WHEN quantity < reorder_point * 0.5 THEN 'URGENT REORDER'
        WHEN quantity < reorder_point THEN 'REORDER NOW'
        WHEN quantity < reorder_point * 1.5 THEN 'REORDER SOON'
        ELSE 'SUFFICIENT'
    END AS reorder_status,
    CEIL((reorder_point * 2 - quantity) / 100.0) * 100 AS suggested_order_quantity
FROM warehouse_inventory
WHERE quantity <= reorder_point * 1.5
ORDER BY 
    CASE 
        WHEN quantity < reorder_point * 0.5 THEN 1
        WHEN quantity < reorder_point THEN 2
        ELSE 3
    END;

-- ----------------------------------------------------------------------------
-- 5. DRIVER PERFORMANCE ANALYTICS
-- ----------------------------------------------------------------------------

-- Create drivers table
CREATE TABLE IF NOT EXISTS drivers (
    driver_id SERIAL PRIMARY KEY,
    driver_name VARCHAR(100),
    license_number VARCHAR(50),
    hire_date DATE,
    total_deliveries INTEGER DEFAULT 0,
    on_time_deliveries INTEGER DEFAULT 0,
    average_rating DECIMAL(3, 2),
    total_distance_km INTEGER DEFAULT 0,
    safety_score DECIMAL(5, 2),
    efficiency_score DECIMAL(5, 2)
);

-- Insert sample drivers
INSERT INTO drivers (driver_name, license_number, hire_date, total_deliveries, 
                    on_time_deliveries, average_rating, total_distance_km) VALUES
('John Driver', 'DL-12345', '2022-01-15', 1500, 1425, 4.8, 25000),
('Jane Wheeler', 'DL-23456', '2021-06-20', 2200, 2090, 4.9, 35000),
('Bob Courier', 'DL-34567', '2023-03-10', 800, 720, 4.5, 12000),
('Alice Swift', 'DL-45678', '2020-11-05', 3000, 2850, 4.7, 48000);

-- Calculate driver performance metrics
UPDATE drivers
SET 
    efficiency_score = (on_time_deliveries::FLOAT / NULLIF(total_deliveries, 0) * 100),
    safety_score = CASE 
        WHEN total_distance_km > 0 THEN 
            100 - (total_deliveries::FLOAT / total_distance_km * 1000) -- Fewer incidents per km
        ELSE 100
    END;

-- Driver performance ranking
SELECT 
    driver_id,
    driver_name,
    total_deliveries,
    on_time_deliveries,
    (on_time_deliveries::FLOAT / total_deliveries * 100) AS on_time_pct,
    average_rating,
    efficiency_score,
    safety_score,
    (efficiency_score * 0.4 + safety_score * 0.3 + average_rating * 20 * 0.3) AS overall_score,
    CASE 
        WHEN (efficiency_score * 0.4 + safety_score * 0.3 + average_rating * 20 * 0.3) > 90 THEN 'EXCELLENT'
        WHEN (efficiency_score * 0.4 + safety_score * 0.3 + average_rating * 20 * 0.3) > 80 THEN 'GOOD'
        WHEN (efficiency_score * 0.4 + safety_score * 0.3 + average_rating * 20 * 0.3) > 70 THEN 'AVERAGE'
        ELSE 'NEEDS IMPROVEMENT'
    END AS performance_tier
FROM drivers
ORDER BY overall_score DESC;

-- ----------------------------------------------------------------------------
-- 6. DELIVERY COST OPTIMIZATION
-- ----------------------------------------------------------------------------

-- Create delivery costs table
CREATE TABLE IF NOT EXISTS delivery_costs (
    cost_id SERIAL PRIMARY KEY,
    delivery_id INTEGER REFERENCES deliveries(delivery_id),
    fuel_cost DECIMAL(10, 2),
    driver_cost DECIMAL(10, 2),
    vehicle_maintenance_cost DECIMAL(10, 2),
    toll_cost DECIMAL(10, 2),
    total_cost DECIMAL(10, 2),
    revenue DECIMAL(10, 2),
    profit_margin DECIMAL(10, 2)
);

-- Calculate delivery costs
INSERT INTO delivery_costs (delivery_id, fuel_cost, driver_cost, vehicle_maintenance_cost, 
                           toll_cost, revenue)
SELECT 
    d.delivery_id,
    (d.distance_km * 0.15)::DECIMAL(10, 2) AS fuel_cost,
    (d.estimated_duration_minutes * 0.30)::DECIMAL(10, 2) AS driver_cost,
    (d.distance_km * 0.05)::DECIMAL(10, 2) AS maintenance_cost,
    (CASE WHEN d.distance_km > 10 THEN 5.00 ELSE 0.00 END)::DECIMAL(10, 2) AS toll_cost,
    (d.package_weight_kg * 5.00 + d.distance_km * 2.00)::DECIMAL(10, 2) AS revenue
FROM deliveries d
WHERE d.delivery_id NOT IN (SELECT delivery_id FROM delivery_costs);

-- Update total costs and profit margins
UPDATE delivery_costs
SET 
    total_cost = fuel_cost + driver_cost + vehicle_maintenance_cost + toll_cost,
    profit_margin = revenue - (fuel_cost + driver_cost + vehicle_maintenance_cost + toll_cost);

-- Cost analysis and optimization opportunities
SELECT 
    d.delivery_id,
    d.order_id,
    d.distance_km,
    dc.fuel_cost,
    dc.driver_cost,
    dc.vehicle_maintenance_cost,
    dc.total_cost,
    dc.revenue,
    dc.profit_margin,
    (dc.profit_margin / dc.revenue * 100) AS profit_margin_pct,
    CASE 
        WHEN dc.profit_margin < 0 THEN 'UNPROFITABLE - Review Pricing'
        WHEN dc.profit_margin / dc.revenue < 0.15 THEN 'LOW MARGIN - Optimize Route'
        WHEN dc.profit_margin / dc.revenue < 0.30 THEN 'ACCEPTABLE'
        ELSE 'HIGH MARGIN'
    END AS profitability_status
FROM deliveries d
JOIN delivery_costs dc ON d.delivery_id = dc.delivery_id
ORDER BY profit_margin_pct ASC;

-- ============================================================================
-- SUMMARY: Logistics & Transportation ML Use Cases Demonstrated
-- ============================================================================
-- 1. Route optimization with delivery time prediction
-- 2. Fleet management with vehicle tracking and anomaly detection
-- 3. Demand forecasting using time series analysis
-- 4. Warehouse optimization with ABC analysis
-- 5. Driver performance analytics and ranking
-- 6. Delivery cost optimization and profitability analysis
-- ============================================================================
