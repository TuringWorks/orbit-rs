-- ============================================================================
-- Manufacturing & IoT ML Industry Example - Orbit-RS
-- ============================================================================
-- Use Case: Predictive Maintenance, Quality Control, Supply Chain Optimization,
--           Equipment Monitoring, Anomaly Detection
-- Protocols: PostgreSQL (SQL), Vector Search, Time Series
-- ============================================================================

-- ----------------------------------------------------------------------------
-- 1. PREDICTIVE MAINTENANCE - EQUIPMENT FAILURE PREDICTION
-- ----------------------------------------------------------------------------

-- Create equipment/machines table
CREATE TABLE IF NOT EXISTS equipment (
    equipment_id SERIAL PRIMARY KEY,
    equipment_name VARCHAR(100) NOT NULL,
    equipment_type VARCHAR(50), -- 'CNC Machine', 'Robot Arm', 'Conveyor', 'Press'
    manufacturer VARCHAR(100),
    installation_date DATE,
    last_maintenance_date DATE,
    maintenance_interval_days INTEGER DEFAULT 90,
    location VARCHAR(100),
    status VARCHAR(20) DEFAULT 'OPERATIONAL', -- 'OPERATIONAL', 'MAINTENANCE', 'FAILED'
    failure_probability FLOAT
);

-- Insert sample equipment
INSERT INTO equipment (equipment_name, equipment_type, manufacturer, installation_date, 
                      last_maintenance_date, location) VALUES
('CNC-001', 'CNC Machine', 'Haas Automation', '2020-01-15', '2024-10-01', 'Production Floor A'),
('ROBOT-001', 'Robot Arm', 'FANUC', '2021-06-20', '2024-11-15', 'Assembly Line 1'),
('CONV-001', 'Conveyor Belt', 'Dorner', '2019-03-10', '2024-09-20', 'Warehouse'),
('PRESS-001', 'Hydraulic Press', 'Schuler', '2018-11-05', '2024-08-15', 'Production Floor B');

-- Create sensor data time series table
CREATE TABLE IF NOT EXISTS sensor_readings (
    reading_id SERIAL PRIMARY KEY,
    equipment_id INTEGER REFERENCES equipment(equipment_id),
    timestamp BIGINT NOT NULL, -- Unix timestamp in milliseconds
    sensor_type VARCHAR(50), -- 'temperature', 'vibration', 'pressure', 'current'
    value FLOAT NOT NULL,
    unit VARCHAR(20),
    is_anomaly BOOLEAN DEFAULT FALSE
);

-- Insert sample sensor data (simulating IoT stream)
INSERT INTO sensor_readings (equipment_id, timestamp, sensor_type, value, unit)
SELECT 
    1, -- CNC-001
    extract(epoch from CURRENT_TIMESTAMP - (n || ' minutes')::INTERVAL)::bigint * 1000,
    'temperature',
    65 + (random() * 10), -- Normal: 65-75°C
    'celsius'
FROM generate_series(1, 100) AS n;

INSERT INTO sensor_readings (equipment_id, timestamp, sensor_type, value, unit)
SELECT 
    1,
    extract(epoch from CURRENT_TIMESTAMP - (n || ' minutes')::INTERVAL)::bigint * 1000,
    'vibration',
    2.5 + (random() * 1.5), -- Normal: 2.5-4.0 mm/s
    'mm/s'
FROM generate_series(1, 100) AS n;

-- Detect anomalies in sensor readings using statistical methods
WITH sensor_stats AS (
    SELECT 
        equipment_id,
        sensor_type,
        AVG(value) AS mean_value,
        STDDEV(value) AS stddev_value
    FROM sensor_readings
    WHERE timestamp > extract(epoch from CURRENT_TIMESTAMP - INTERVAL '24 hours')::bigint * 1000
    GROUP BY equipment_id, sensor_type
)
UPDATE sensor_readings sr
SET is_anomaly = TRUE
FROM sensor_stats ss
WHERE sr.equipment_id = ss.equipment_id
  AND sr.sensor_type = ss.sensor_type
  AND ABS(sr.value - ss.mean_value) > 3 * ss.stddev_value; -- 3-sigma rule

-- Query anomalous readings
SELECT 
    e.equipment_name,
    sr.sensor_type,
    sr.value,
    sr.unit,
    to_timestamp(sr.timestamp / 1000) AS reading_time
FROM sensor_readings sr
JOIN equipment e ON sr.equipment_id = e.equipment_id
WHERE sr.is_anomaly = TRUE
ORDER BY sr.timestamp DESC
LIMIT 20;

-- Calculate equipment health score and failure probability
CREATE OR REPLACE FUNCTION calculate_failure_probability(
    p_days_since_maintenance INTEGER,
    p_maintenance_interval INTEGER,
    p_avg_temperature FLOAT,
    p_avg_vibration FLOAT,
    p_num_anomalies INTEGER
) RETURNS FLOAT AS $$
DECLARE
    failure_prob FLOAT := 0.0;
BEGIN
    -- Maintenance overdue factor
    IF p_days_since_maintenance > p_maintenance_interval THEN
        failure_prob := failure_prob + 0.3 * (p_days_since_maintenance::FLOAT / p_maintenance_interval - 1.0);
    END IF;
    
    -- Temperature factor (assuming normal < 75°C)
    IF p_avg_temperature > 75 THEN
        failure_prob := failure_prob + 0.2 * ((p_avg_temperature - 75) / 25);
    END IF;
    
    -- Vibration factor (assuming normal < 4.0 mm/s)
    IF p_avg_vibration > 4.0 THEN
        failure_prob := failure_prob + 0.2 * ((p_avg_vibration - 4.0) / 2.0);
    END IF;
    
    -- Anomaly factor
    failure_prob := failure_prob + LEAST(0.3, p_num_anomalies * 0.05);
    
    RETURN LEAST(failure_prob, 1.0);
END;
$$ LANGUAGE plpgsql;

-- Update failure probabilities for all equipment
WITH equipment_metrics AS (
    SELECT 
        e.equipment_id,
        CURRENT_DATE - e.last_maintenance_date AS days_since_maintenance,
        e.maintenance_interval_days,
        AVG(CASE WHEN sr.sensor_type = 'temperature' THEN sr.value END) AS avg_temp,
        AVG(CASE WHEN sr.sensor_type = 'vibration' THEN sr.value END) AS avg_vibration,
        SUM(CASE WHEN sr.is_anomaly THEN 1 ELSE 0 END) AS num_anomalies
    FROM equipment e
    LEFT JOIN sensor_readings sr ON e.equipment_id = sr.equipment_id
    WHERE sr.timestamp > extract(epoch from CURRENT_TIMESTAMP - INTERVAL '24 hours')::bigint * 1000
    GROUP BY e.equipment_id, e.last_maintenance_date, e.maintenance_interval_days
)
UPDATE equipment e
SET failure_probability = calculate_failure_probability(
    em.days_since_maintenance,
    em.maintenance_interval_days,
    COALESCE(em.avg_temp, 70),
    COALESCE(em.avg_vibration, 3.0),
    COALESCE(em.num_anomalies, 0)
)
FROM equipment_metrics em
WHERE e.equipment_id = em.equipment_id;

-- Maintenance priority list
SELECT 
    equipment_id,
    equipment_name,
    equipment_type,
    location,
    last_maintenance_date,
    CURRENT_DATE - last_maintenance_date AS days_since_maintenance,
    failure_probability,
    CASE 
        WHEN failure_probability > 0.7 THEN 'CRITICAL - Schedule Immediately'
        WHEN failure_probability > 0.5 THEN 'HIGH - Schedule This Week'
        WHEN failure_probability > 0.3 THEN 'MEDIUM - Schedule This Month'
        ELSE 'LOW - Normal Schedule'
    END AS maintenance_priority
FROM equipment
WHERE status = 'OPERATIONAL'
ORDER BY failure_probability DESC;

-- ----------------------------------------------------------------------------
-- 2. QUALITY CONTROL - DEFECT DETECTION
-- ----------------------------------------------------------------------------

-- Create production batches table
CREATE TABLE IF NOT EXISTS production_batches (
    batch_id SERIAL PRIMARY KEY,
    product_name VARCHAR(100),
    batch_number VARCHAR(50) UNIQUE,
    production_date DATE DEFAULT CURRENT_DATE,
    quantity_produced INTEGER,
    equipment_id INTEGER REFERENCES equipment(equipment_id),
    quality_score FLOAT,
    defect_rate FLOAT,
    status VARCHAR(20) DEFAULT 'IN_PROGRESS' -- 'IN_PROGRESS', 'PASSED', 'FAILED'
);

-- Create quality inspection records
CREATE TABLE IF NOT EXISTS quality_inspections (
    inspection_id SERIAL PRIMARY KEY,
    batch_id INTEGER REFERENCES production_batches(batch_id),
    inspection_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    inspector_id INTEGER,
    sample_size INTEGER,
    defects_found INTEGER,
    defect_types TEXT[], -- Array of defect types
    inspection_notes TEXT,
    image_embedding vector(512) -- Visual inspection embedding from CNN
);

-- Insert sample production batches
INSERT INTO production_batches (product_name, batch_number, quantity_produced, equipment_id) VALUES
('Widget A', 'BATCH-2024-001', 1000, 1),
('Widget B', 'BATCH-2024-002', 1500, 2),
('Widget A', 'BATCH-2024-003', 1200, 1),
('Widget C', 'BATCH-2024-004', 800, 4);

-- Insert sample quality inspections
INSERT INTO quality_inspections (batch_id, inspector_id, sample_size, defects_found, 
                                defect_types, image_embedding) VALUES
(1, 101, 100, 2, ARRAY['scratch', 'misalignment'], 
 array_fill(random()::float, ARRAY[512])::vector(512)),
(2, 102, 150, 8, ARRAY['crack', 'discoloration', 'dimension_error'],
 array_fill(random()::float, ARRAY[512])::vector(512)),
(3, 101, 120, 1, ARRAY['minor_scratch'],
 array_fill(random()::float, ARRAY[512])::vector(512)),
(4, 103, 80, 15, ARRAY['crack', 'structural_defect'],
 array_fill(random()::float, ARRAY[512])::vector(512));

-- Calculate defect rates and quality scores
UPDATE production_batches pb
SET 
    defect_rate = (
        SELECT SUM(qi.defects_found)::FLOAT / SUM(qi.sample_size)
        FROM quality_inspections qi
        WHERE qi.batch_id = pb.batch_id
    ),
    quality_score = 1.0 - (
        SELECT SUM(qi.defects_found)::FLOAT / SUM(qi.sample_size)
        FROM quality_inspections qi
        WHERE qi.batch_id = pb.batch_id
    );

-- Update batch status based on quality threshold
UPDATE production_batches
SET status = CASE 
    WHEN defect_rate < 0.02 THEN 'PASSED'
    ELSE 'FAILED'
END
WHERE status = 'IN_PROGRESS';

-- Find similar defect patterns using visual embeddings
SELECT 
    qi1.inspection_id,
    qi1.batch_id,
    qi1.defect_types,
    qi2.inspection_id AS similar_inspection_id,
    qi2.batch_id AS similar_batch_id,
    qi2.defect_types AS similar_defect_types,
    1 - (qi1.image_embedding <=> qi2.image_embedding) AS similarity
FROM quality_inspections qi1
CROSS JOIN quality_inspections qi2
WHERE qi1.inspection_id = 1 AND qi2.inspection_id != 1
ORDER BY qi1.image_embedding <=> qi2.image_embedding
LIMIT 5;

-- Quality trends by equipment
SELECT 
    e.equipment_name,
    COUNT(pb.batch_id) AS total_batches,
    AVG(pb.quality_score) AS avg_quality_score,
    AVG(pb.defect_rate) AS avg_defect_rate,
    SUM(CASE WHEN pb.status = 'PASSED' THEN 1 ELSE 0 END)::FLOAT / COUNT(*) * 100 AS pass_rate
FROM equipment e
JOIN production_batches pb ON e.equipment_id = pb.equipment_id
GROUP BY e.equipment_id, e.equipment_name
ORDER BY avg_quality_score DESC;

-- ----------------------------------------------------------------------------
-- 3. SUPPLY CHAIN OPTIMIZATION
-- ----------------------------------------------------------------------------

-- Create suppliers table
CREATE TABLE IF NOT EXISTS suppliers (
    supplier_id SERIAL PRIMARY KEY,
    supplier_name VARCHAR(100) NOT NULL,
    country VARCHAR(50),
    lead_time_days INTEGER,
    reliability_score FLOAT, -- 0.0 to 1.0
    cost_index FLOAT, -- Relative cost (1.0 = baseline)
    quality_rating FLOAT -- 0.0 to 5.0
);

-- Create raw materials inventory
CREATE TABLE IF NOT EXISTS raw_materials (
    material_id SERIAL PRIMARY KEY,
    material_name VARCHAR(100) NOT NULL,
    current_stock INTEGER,
    reorder_point INTEGER,
    unit_cost DECIMAL(10, 2),
    supplier_id INTEGER REFERENCES suppliers(supplier_id),
    last_order_date DATE
);

-- Insert sample suppliers
INSERT INTO suppliers (supplier_name, country, lead_time_days, reliability_score, cost_index, quality_rating) VALUES
('Acme Materials', 'USA', 7, 0.95, 1.0, 4.5),
('Global Supply Co', 'China', 30, 0.85, 0.7, 4.0),
('Euro Parts Ltd', 'Germany', 14, 0.92, 1.15, 4.8),
('Local Supplier', 'USA', 3, 0.98, 1.25, 4.2);

-- Insert sample raw materials
INSERT INTO raw_materials (material_name, current_stock, reorder_point, unit_cost, supplier_id, last_order_date) VALUES
('Steel Sheet', 500, 200, 25.50, 1, CURRENT_DATE - INTERVAL '15 days'),
('Aluminum Rod', 150, 100, 18.75, 2, CURRENT_DATE - INTERVAL '45 days'),
('Copper Wire', 800, 300, 12.30, 3, CURRENT_DATE - INTERVAL '20 days'),
('Plastic Resin', 50, 150, 8.90, 4, CURRENT_DATE - INTERVAL '5 days');

-- ML-based supplier selection optimization
CREATE OR REPLACE FUNCTION calculate_supplier_score(
    p_reliability FLOAT,
    p_cost_index FLOAT,
    p_quality_rating FLOAT,
    p_lead_time_days INTEGER,
    p_urgency_level VARCHAR
) RETURNS FLOAT AS $$
DECLARE
    score FLOAT := 0.0;
    reliability_weight FLOAT := 0.3;
    cost_weight FLOAT := 0.25;
    quality_weight FLOAT := 0.25;
    lead_time_weight FLOAT := 0.2;
BEGIN
    -- Adjust weights based on urgency
    IF p_urgency_level = 'HIGH' THEN
        lead_time_weight := 0.4;
        reliability_weight := 0.4;
        cost_weight := 0.1;
        quality_weight := 0.1;
    END IF;
    
    -- Calculate weighted score
    score := (p_reliability * reliability_weight) +
             ((2.0 - p_cost_index) / 2.0 * cost_weight) + -- Lower cost is better
             (p_quality_rating / 5.0 * quality_weight) +
             ((1.0 - (p_lead_time_days::FLOAT / 30.0)) * lead_time_weight); -- Shorter lead time is better
    
    RETURN score;
END;
$$ LANGUAGE plpgsql;

-- Identify materials needing reorder and recommend suppliers
SELECT 
    rm.material_id,
    rm.material_name,
    rm.current_stock,
    rm.reorder_point,
    s.supplier_name,
    s.lead_time_days,
    s.cost_index,
    s.quality_rating,
    calculate_supplier_score(
        s.reliability_score,
        s.cost_index,
        s.quality_rating,
        s.lead_time_days,
        CASE 
            WHEN rm.current_stock < rm.reorder_point * 0.5 THEN 'HIGH'
            ELSE 'NORMAL'
        END
    ) AS supplier_score,
    CASE 
        WHEN rm.current_stock < rm.reorder_point * 0.5 THEN 'URGENT'
        WHEN rm.current_stock < rm.reorder_point THEN 'REORDER'
        ELSE 'SUFFICIENT'
    END AS stock_status
FROM raw_materials rm
JOIN suppliers s ON rm.supplier_id = s.supplier_id
WHERE rm.current_stock <= rm.reorder_point
ORDER BY stock_status, supplier_score DESC;

-- ----------------------------------------------------------------------------
-- 4. PRODUCTION LINE OPTIMIZATION
-- ----------------------------------------------------------------------------

-- Create production line performance table
CREATE TABLE IF NOT EXISTS production_metrics (
    metric_id SERIAL PRIMARY KEY,
    equipment_id INTEGER REFERENCES equipment(equipment_id),
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    units_produced INTEGER,
    cycle_time_seconds INTEGER,
    downtime_minutes INTEGER,
    oee_score FLOAT, -- Overall Equipment Effectiveness
    quality_rate FLOAT,
    availability_rate FLOAT,
    performance_rate FLOAT
);

-- Insert sample production metrics
INSERT INTO production_metrics (equipment_id, timestamp, units_produced, cycle_time_seconds, 
                               downtime_minutes, quality_rate, availability_rate, performance_rate)
SELECT 
    1,
    CURRENT_TIMESTAMP - (n || ' hours')::INTERVAL,
    (random() * 50 + 80)::INTEGER,
    (random() * 10 + 30)::INTEGER,
    (random() * 15)::INTEGER,
    0.95 + (random() * 0.05),
    0.90 + (random() * 0.10),
    0.85 + (random() * 0.15)
FROM generate_series(1, 24) AS n;

-- Calculate OEE (Overall Equipment Effectiveness)
UPDATE production_metrics
SET oee_score = quality_rate * availability_rate * performance_rate;

-- Production efficiency trends
SELECT 
    e.equipment_name,
    DATE_TRUNC('day', pm.timestamp) AS production_day,
    AVG(pm.oee_score) AS avg_oee,
    SUM(pm.units_produced) AS total_units,
    SUM(pm.downtime_minutes) AS total_downtime,
    AVG(pm.cycle_time_seconds) AS avg_cycle_time
FROM production_metrics pm
JOIN equipment e ON pm.equipment_id = e.equipment_id
GROUP BY e.equipment_name, DATE_TRUNC('day', pm.timestamp)
ORDER BY production_day DESC;

-- Identify bottlenecks
SELECT 
    e.equipment_name,
    e.equipment_type,
    AVG(pm.oee_score) AS avg_oee,
    AVG(pm.cycle_time_seconds) AS avg_cycle_time,
    SUM(pm.downtime_minutes) AS total_downtime,
    CASE 
        WHEN AVG(pm.oee_score) < 0.65 THEN 'CRITICAL BOTTLENECK'
        WHEN AVG(pm.oee_score) < 0.75 THEN 'BOTTLENECK'
        WHEN AVG(pm.oee_score) < 0.85 THEN 'NEEDS IMPROVEMENT'
        ELSE 'OPTIMAL'
    END AS performance_status
FROM equipment e
JOIN production_metrics pm ON e.equipment_id = pm.equipment_id
WHERE pm.timestamp > CURRENT_TIMESTAMP - INTERVAL '7 days'
GROUP BY e.equipment_id, e.equipment_name, e.equipment_type
ORDER BY avg_oee ASC;

-- ----------------------------------------------------------------------------
-- 5. ENERGY CONSUMPTION OPTIMIZATION
-- ----------------------------------------------------------------------------

-- Create energy consumption table
CREATE TABLE IF NOT EXISTS energy_consumption (
    consumption_id SERIAL PRIMARY KEY,
    equipment_id INTEGER REFERENCES equipment(equipment_id),
    timestamp BIGINT NOT NULL,
    power_kw FLOAT NOT NULL,
    energy_kwh FLOAT,
    cost_usd DECIMAL(10, 2),
    is_peak_hours BOOLEAN DEFAULT FALSE
);

-- Insert sample energy data
INSERT INTO energy_consumption (equipment_id, timestamp, power_kw, is_peak_hours)
SELECT 
    1,
    extract(epoch from CURRENT_TIMESTAMP - (n || ' minutes')::INTERVAL)::bigint * 1000,
    15 + (random() * 5), -- 15-20 kW
    EXTRACT(HOUR FROM CURRENT_TIMESTAMP - (n || ' minutes')::INTERVAL) BETWEEN 9 AND 17
FROM generate_series(1, 1440) AS n; -- 24 hours of minute-level data

-- Calculate energy costs (peak vs off-peak pricing)
UPDATE energy_consumption
SET 
    energy_kwh = power_kw / 60.0, -- Per minute to per hour
    cost_usd = CASE 
        WHEN is_peak_hours THEN (power_kw / 60.0) * 0.15 -- $0.15/kWh peak
        ELSE (power_kw / 60.0) * 0.08 -- $0.08/kWh off-peak
    END;

-- Energy consumption analysis
SELECT 
    e.equipment_name,
    SUM(ec.energy_kwh) AS total_energy_kwh,
    SUM(ec.cost_usd) AS total_cost_usd,
    AVG(ec.power_kw) AS avg_power_kw,
    MAX(ec.power_kw) AS peak_power_kw
FROM energy_consumption ec
JOIN equipment e ON ec.equipment_id = e.equipment_id
WHERE ec.timestamp > extract(epoch from CURRENT_TIMESTAMP - INTERVAL '24 hours')::bigint * 1000
GROUP BY e.equipment_id, e.equipment_name
ORDER BY total_cost_usd DESC;

-- Identify opportunities for load shifting (peak to off-peak)
WITH hourly_consumption AS (
    SELECT 
        equipment_id,
        EXTRACT(HOUR FROM to_timestamp(timestamp / 1000)) AS hour_of_day,
        AVG(power_kw) AS avg_power,
        SUM(cost_usd) AS total_cost,
        AVG(CASE WHEN is_peak_hours THEN 1 ELSE 0 END) AS peak_ratio
    FROM energy_consumption
    WHERE timestamp > extract(epoch from CURRENT_TIMESTAMP - INTERVAL '7 days')::bigint * 1000
    GROUP BY equipment_id, EXTRACT(HOUR FROM to_timestamp(timestamp / 1000))
)
SELECT 
    e.equipment_name,
    hc.hour_of_day,
    hc.avg_power,
    hc.total_cost,
    CASE 
        WHEN hc.peak_ratio > 0.5 AND hc.avg_power > 15 THEN 'HIGH SAVINGS POTENTIAL'
        WHEN hc.peak_ratio > 0.5 THEN 'MODERATE SAVINGS POTENTIAL'
        ELSE 'LOW SAVINGS POTENTIAL'
    END AS load_shift_opportunity
FROM hourly_consumption hc
JOIN equipment e ON hc.equipment_id = e.equipment_id
WHERE hc.peak_ratio > 0.5
ORDER BY hc.total_cost DESC;

-- ----------------------------------------------------------------------------
-- 6. PREDICTIVE QUALITY - CORRELATION ANALYSIS
-- ----------------------------------------------------------------------------

-- Correlate sensor readings with quality outcomes
WITH sensor_aggregates AS (
    SELECT 
        sr.equipment_id,
        DATE(to_timestamp(sr.timestamp / 1000)) AS reading_date,
        AVG(CASE WHEN sr.sensor_type = 'temperature' THEN sr.value END) AS avg_temp,
        AVG(CASE WHEN sr.sensor_type = 'vibration' THEN sr.value END) AS avg_vibration,
        MAX(CASE WHEN sr.sensor_type = 'temperature' THEN sr.value END) AS max_temp,
        MAX(CASE WHEN sr.sensor_type = 'vibration' THEN sr.value END) AS max_vibration
    FROM sensor_readings sr
    GROUP BY sr.equipment_id, DATE(to_timestamp(sr.timestamp / 1000))
)
SELECT 
    e.equipment_name,
    sa.reading_date,
    sa.avg_temp,
    sa.avg_vibration,
    pb.quality_score,
    pb.defect_rate,
    CASE 
        WHEN sa.avg_temp > 75 AND pb.defect_rate > 0.03 THEN 'High temp correlates with defects'
        WHEN sa.avg_vibration > 4.0 AND pb.defect_rate > 0.03 THEN 'High vibration correlates with defects'
        ELSE 'Normal operation'
    END AS quality_insight
FROM sensor_aggregates sa
JOIN equipment e ON sa.equipment_id = e.equipment_id
LEFT JOIN production_batches pb ON sa.equipment_id = pb.equipment_id 
    AND sa.reading_date = pb.production_date
WHERE pb.quality_score IS NOT NULL
ORDER BY sa.reading_date DESC;

-- ============================================================================
-- SUMMARY: Manufacturing & IoT ML Use Cases Demonstrated
-- ============================================================================
-- 1. Predictive maintenance with sensor data and failure probability
-- 2. Quality control with defect detection and visual embeddings
-- 3. Supply chain optimization with ML-based supplier selection
-- 4. Production line optimization with OEE and bottleneck detection
-- 5. Energy consumption optimization with peak/off-peak analysis
-- 6. Predictive quality with sensor-quality correlation analysis
-- ============================================================================
