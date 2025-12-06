-- ============================================================================
-- Telecommunications ML Industry Example - Orbit-RS
-- ============================================================================
-- Use Case: Network Optimization, Churn Prediction, Fraud Detection,
--           Predictive Maintenance, Customer Experience Analytics
-- Protocols: PostgreSQL (SQL), Vector Search, Time Series
-- ============================================================================

-- ----------------------------------------------------------------------------
-- 1. NETWORK PERFORMANCE MONITORING & OPTIMIZATION
-- ----------------------------------------------------------------------------

-- Create cell towers/base stations table
CREATE TABLE IF NOT EXISTS cell_towers (
    tower_id SERIAL PRIMARY KEY,
    tower_name VARCHAR(100) NOT NULL,
    latitude DECIMAL(10, 8),
    longitude DECIMAL(11, 8),
    technology VARCHAR(20), -- '4G', '5G'
    capacity_mbps INTEGER,
    coverage_radius_km DECIMAL(5, 2),
    installation_date DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE' -- 'ACTIVE', 'MAINTENANCE', 'DEGRADED'
);

-- Insert sample cell towers
INSERT INTO cell_towers (tower_name, latitude, longitude, technology, capacity_mbps, coverage_radius_km, installation_date) VALUES
('TOWER-SF-001', 37.7749, -122.4194, '5G', 10000, 2.5, '2022-03-15'),
('TOWER-SF-002', 37.7849, -122.4094, '5G', 10000, 2.5, '2022-06-20'),
('TOWER-SF-003', 37.7649, -122.4294, '4G', 5000, 3.0, '2019-01-10'),
('TOWER-NY-001', 40.7128, -74.0060, '5G', 10000, 2.0, '2023-01-05');

-- Create network performance metrics time series
CREATE TABLE IF NOT EXISTS network_metrics (
    metric_id SERIAL PRIMARY KEY,
    tower_id INTEGER REFERENCES cell_towers(tower_id),
    timestamp BIGINT NOT NULL,
    active_connections INTEGER,
    throughput_mbps DECIMAL(10, 2),
    latency_ms DECIMAL(8, 2),
    packet_loss_pct DECIMAL(5, 4),
    signal_strength_dbm DECIMAL(6, 2),
    cpu_utilization_pct DECIMAL(5, 2),
    memory_utilization_pct DECIMAL(5, 2)
);

-- Insert sample network metrics (last 24 hours)
INSERT INTO network_metrics (tower_id, timestamp, active_connections, throughput_mbps, 
                             latency_ms, packet_loss_pct, signal_strength_dbm, 
                             cpu_utilization_pct, memory_utilization_pct)
SELECT 
    1,
    extract(epoch from CURRENT_TIMESTAMP - (n || ' minutes')::INTERVAL)::bigint * 1000,
    (random() * 500 + 200)::INTEGER,
    (random() * 3000 + 5000)::DECIMAL(10, 2),
    (random() * 20 + 10)::DECIMAL(8, 2),
    (random() * 0.5)::DECIMAL(5, 4),
    (random() * 20 - 80)::DECIMAL(6, 2),
    (random() * 40 + 40)::DECIMAL(5, 2),
    (random() * 30 + 50)::DECIMAL(5, 2)
FROM generate_series(1, 1440) AS n;

-- Detect network anomalies using moving averages
WITH network_stats AS (
    SELECT 
        tower_id,
        timestamp,
        latency_ms,
        packet_loss_pct,
        AVG(latency_ms) OVER (
            PARTITION BY tower_id 
            ORDER BY timestamp 
            ROWS BETWEEN 59 PRECEDING AND CURRENT ROW
        ) AS avg_latency_1h,
        AVG(packet_loss_pct) OVER (
            PARTITION BY tower_id 
            ORDER BY timestamp 
            ROWS BETWEEN 59 PRECEDING AND CURRENT ROW
        ) AS avg_packet_loss_1h
    FROM network_metrics
)
SELECT 
    ct.tower_name,
    to_timestamp(ns.timestamp / 1000) AS event_time,
    ns.latency_ms,
    ns.avg_latency_1h,
    ns.packet_loss_pct,
    CASE 
        WHEN ns.latency_ms > ns.avg_latency_1h * 2 THEN 'HIGH LATENCY ALERT'
        WHEN ns.packet_loss_pct > 1.0 THEN 'PACKET LOSS ALERT'
        ELSE 'NORMAL'
    END AS alert_status
FROM network_stats ns
JOIN cell_towers ct ON ns.tower_id = ct.tower_id
WHERE ns.latency_ms > ns.avg_latency_1h * 2 
   OR ns.packet_loss_pct > 1.0
ORDER BY ns.timestamp DESC
LIMIT 20;

ALTER TABLE customer_accounts ADD COLUMN IF NOT EXISTS churned BOOLEAN;
UPDATE customer_accounts
SET churned = (payment_delays > 2) OR (customer_service_calls > 5);
SELECT ML_TRAIN_MODEL(
  'telecom_churn_rf',
  'random_forest',
  ARRAY[
    monthly_charge,
    contract_length_months,
    account_age_months,
    data_usage_gb_monthly,
    voice_minutes_monthly,
    sms_count_monthly,
    customer_service_calls,
    payment_delays,
    roaming_charges
  ],
  churned
) FROM customer_accounts;
SELECT ML_EVALUATE_MODEL(
  'telecom_churn_rf',
  ARRAY[
    monthly_charge,
    contract_length_months,
    account_age_months,
    data_usage_gb_monthly,
    voice_minutes_monthly,
    sms_count_monthly,
    customer_service_calls,
    payment_delays,
    roaming_charges
  ],
  churned
) FROM customer_accounts;
UPDATE customer_accounts
SET churn_probability = ML_PREDICT(
  'telecom_churn_rf',
  ARRAY[
    monthly_charge,
    contract_length_months,
    account_age_months,
    data_usage_gb_monthly,
    voice_minutes_monthly,
    sms_count_monthly,
    customer_service_calls,
    payment_delays,
    roaming_charges
  ]
);

-- Network capacity planning
SELECT 
    ct.tower_name,
    ct.technology,
    ct.capacity_mbps,
    AVG(nm.active_connections) AS avg_connections,
    MAX(nm.active_connections) AS peak_connections,
    AVG(nm.throughput_mbps) AS avg_throughput,
    MAX(nm.throughput_mbps) AS peak_throughput,
    (MAX(nm.throughput_mbps) / ct.capacity_mbps * 100) AS peak_utilization_pct,
    CASE 
        WHEN MAX(nm.throughput_mbps) / ct.capacity_mbps > 0.8 THEN 'UPGRADE NEEDED'
        WHEN MAX(nm.throughput_mbps) / ct.capacity_mbps > 0.6 THEN 'MONITOR CLOSELY'
        ELSE 'SUFFICIENT CAPACITY'
    END AS capacity_status
FROM cell_towers ct
JOIN network_metrics nm ON ct.tower_id = nm.tower_id
WHERE nm.timestamp > extract(epoch from CURRENT_TIMESTAMP - INTERVAL '24 hours')::bigint * 1000
GROUP BY ct.tower_id, ct.tower_name, ct.technology, ct.capacity_mbps
ORDER BY peak_utilization_pct DESC;

-- ----------------------------------------------------------------------------
-- 2. CUSTOMER CHURN PREDICTION
-- ----------------------------------------------------------------------------

-- Create customer accounts table
CREATE TABLE IF NOT EXISTS customer_accounts (
    customer_id SERIAL PRIMARY KEY,
    account_number VARCHAR(50) UNIQUE,
    customer_name VARCHAR(100),
    plan_type VARCHAR(50), -- 'Prepaid', 'Postpaid'
    monthly_charge DECIMAL(10, 2),
    contract_length_months INTEGER,
    account_age_months INTEGER,
    data_usage_gb_monthly DECIMAL(10, 2),
    voice_minutes_monthly INTEGER,
    sms_count_monthly INTEGER,
    customer_service_calls INTEGER,
    payment_delays INTEGER,
    roaming_charges DECIMAL(10, 2),
    churn_probability FLOAT,
    is_churned BOOLEAN DEFAULT FALSE
);

-- Insert sample customer data
INSERT INTO customer_accounts (account_number, customer_name, plan_type, monthly_charge, 
                               contract_length_months, account_age_months, data_usage_gb_monthly,
                               voice_minutes_monthly, sms_count_monthly, customer_service_calls,
                               payment_delays, roaming_charges) VALUES
('ACC-001', 'John Smith', 'Postpaid', 89.99, 24, 36, 25.5, 450, 120, 2, 0, 15.50),
('ACC-002', 'Jane Doe', 'Prepaid', 45.00, 0, 8, 5.2, 200, 50, 8, 3, 0.00),
('ACC-003', 'Bob Johnson', 'Postpaid', 129.99, 24, 48, 50.0, 800, 200, 0, 0, 45.00),
('ACC-004', 'Alice Williams', 'Postpaid', 79.99, 12, 6, 15.0, 300, 80, 5, 2, 5.00);

-- ML-based churn prediction model
CREATE OR REPLACE FUNCTION predict_customer_churn(
    p_account_age_months INTEGER,
    p_contract_length INTEGER,
    p_customer_service_calls INTEGER,
    p_payment_delays INTEGER,
    p_data_usage DECIMAL,
    p_monthly_charge DECIMAL
) RETURNS FLOAT AS $$
DECLARE
    churn_score FLOAT := 0.0;
BEGIN
    -- New customer risk
    IF p_account_age_months < 12 THEN churn_score := churn_score + 0.2; END IF;
    
    -- No contract commitment
    IF p_contract_length = 0 THEN churn_score := churn_score + 0.25; END IF;
    
    -- High customer service calls (dissatisfaction)
    IF p_customer_service_calls > 5 THEN churn_score := churn_score + 0.25; END IF;
    
    -- Payment issues
    IF p_payment_delays > 2 THEN churn_score := churn_score + 0.15; END IF;
    
    -- Low engagement
    IF p_data_usage < 10 THEN churn_score := churn_score + 0.1; END IF;
    
    -- High price sensitivity
    IF p_monthly_charge > 100 THEN churn_score := churn_score + 0.05; END IF;
    
    RETURN LEAST(churn_score, 1.0);
END;
$$ LANGUAGE plpgsql;

-- Update churn probabilities
UPDATE customer_accounts
SET churn_probability = predict_customer_churn(
    account_age_months,
    contract_length_months,
    customer_service_calls,
    payment_delays,
    data_usage_gb_monthly,
    monthly_charge
);

-- Identify at-risk customers for retention campaigns
SELECT 
    customer_id,
    account_number,
    customer_name,
    plan_type,
    monthly_charge,
    account_age_months,
    churn_probability,
    CASE 
        WHEN churn_probability > 0.7 THEN 'CRITICAL - Offer 50% discount for 3 months'
        WHEN churn_probability > 0.5 THEN 'HIGH - Offer plan upgrade or loyalty bonus'
        WHEN churn_probability > 0.3 THEN 'MEDIUM - Send satisfaction survey'
        ELSE 'LOW - Standard retention'
    END AS retention_strategy,
    (monthly_charge * 12 * 0.7) AS estimated_lifetime_value
FROM customer_accounts
WHERE churn_probability > 0.3
  AND is_churned = FALSE
ORDER BY churn_probability DESC, monthly_charge DESC;

-- ----------------------------------------------------------------------------
-- 3. FRAUD DETECTION - CALL DETAIL RECORDS (CDR)
-- ----------------------------------------------------------------------------

-- Create call detail records table
CREATE TABLE IF NOT EXISTS call_records (
    cdr_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customer_accounts(customer_id),
    call_timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    call_type VARCHAR(20), -- 'VOICE', 'SMS', 'DATA'
    destination_number VARCHAR(20),
    destination_country VARCHAR(50),
    duration_seconds INTEGER,
    data_mb DECIMAL(10, 2),
    cost DECIMAL(10, 4),
    tower_id INTEGER REFERENCES cell_towers(tower_id),
    fraud_score FLOAT,
    is_fraud BOOLEAN DEFAULT FALSE
);

-- Insert sample call records
INSERT INTO call_records (customer_id, call_timestamp, call_type, destination_number, 
                         destination_country, duration_seconds, cost, tower_id) VALUES
(1, CURRENT_TIMESTAMP - INTERVAL '1 hour', 'VOICE', '+1-555-0101', 'USA', 300, 0.15, 1),
(1, CURRENT_TIMESTAMP - INTERVAL '30 minutes', 'SMS', '+1-555-0102', 'USA', 0, 0.05, 1),
(2, CURRENT_TIMESTAMP - INTERVAL '2 hours', 'VOICE', '+234-800-1234', 'Nigeria', 1800, 45.00, 1),
(2, CURRENT_TIMESTAMP - INTERVAL '1 hour 50 minutes', 'VOICE', '+234-800-5678', 'Nigeria', 2400, 60.00, 1),
(3, CURRENT_TIMESTAMP - INTERVAL '3 hours', 'DATA', NULL, 'USA', 0, 2.50, 2);

-- ML-based fraud detection
CREATE OR REPLACE FUNCTION detect_call_fraud(
    p_destination_country VARCHAR,
    p_duration_seconds INTEGER,
    p_cost DECIMAL,
    p_call_type VARCHAR,
    p_customer_avg_cost DECIMAL
) RETURNS FLOAT AS $$
DECLARE
    fraud_score FLOAT := 0.0;
BEGIN
    -- High-risk destination
    IF p_destination_country IN ('Nigeria', 'Somalia', 'Syria') THEN
        fraud_score := fraud_score + 0.4;
    END IF;
    
    -- Unusually long call
    IF p_call_type = 'VOICE' AND p_duration_seconds > 1800 THEN
        fraud_score := fraud_score + 0.2;
    END IF;
    
    -- High cost anomaly
    IF p_cost > p_customer_avg_cost * 10 THEN
        fraud_score := fraud_score + 0.3;
    END IF;
    
    -- Very high cost
    IF p_cost > 50 THEN
        fraud_score := fraud_score + 0.1;
    END IF;
    
    RETURN LEAST(fraud_score, 1.0);
END;
$$ LANGUAGE plpgsql;

-- Update fraud scores
WITH customer_avg_costs AS (
    SELECT customer_id, AVG(cost) AS avg_cost
    FROM call_records
    GROUP BY customer_id
)
UPDATE call_records cr
SET fraud_score = detect_call_fraud(
    cr.destination_country,
    cr.duration_seconds,
    cr.cost,
    cr.call_type,
    COALESCE(cac.avg_cost, 1.0)
)
FROM customer_avg_costs cac
WHERE cr.customer_id = cac.customer_id;

-- Flag fraudulent calls
UPDATE call_records
SET is_fraud = TRUE
WHERE fraud_score > 0.6;

-- Query suspicious call patterns
SELECT 
    ca.customer_id,
    ca.account_number,
    ca.customer_name,
    COUNT(*) AS suspicious_calls,
    SUM(cr.cost) AS total_suspicious_cost,
    array_agg(DISTINCT cr.destination_country) AS countries_called,
    MAX(cr.fraud_score) AS max_fraud_score
FROM call_records cr
JOIN customer_accounts ca ON cr.customer_id = ca.customer_id
WHERE cr.fraud_score > 0.5
GROUP BY ca.customer_id, ca.account_number, ca.customer_name
ORDER BY max_fraud_score DESC, total_suspicious_cost DESC;

-- ----------------------------------------------------------------------------
-- 4. NETWORK EQUIPMENT PREDICTIVE MAINTENANCE
-- ----------------------------------------------------------------------------

-- Create network equipment table
CREATE TABLE IF NOT EXISTS network_equipment (
    equipment_id SERIAL PRIMARY KEY,
    equipment_name VARCHAR(100),
    equipment_type VARCHAR(50), -- 'Router', 'Switch', 'Base Station'
    tower_id INTEGER REFERENCES cell_towers(tower_id),
    manufacturer VARCHAR(100),
    model VARCHAR(100),
    installation_date DATE,
    last_maintenance_date DATE,
    firmware_version VARCHAR(50),
    failure_probability FLOAT
);

-- Insert sample network equipment
INSERT INTO network_equipment (equipment_name, equipment_type, tower_id, manufacturer, 
                               model, installation_date, last_maintenance_date, firmware_version) VALUES
('ROUTER-SF-001', 'Router', 1, 'Cisco', 'ASR 9000', '2022-03-15', '2024-09-01', '7.5.2'),
('SWITCH-SF-001', 'Switch', 1, 'Juniper', 'EX4300', '2022-03-15', '2024-10-15', '18.4R2'),
('BS-SF-001', 'Base Station', 1, 'Ericsson', 'AIR 6488', '2022-03-15', '2024-08-20', '21.Q4'),
('ROUTER-NY-001', 'Router', 4, 'Cisco', 'ASR 9000', '2023-01-05', '2024-11-01', '7.5.2');

-- Create equipment health metrics
CREATE TABLE IF NOT EXISTS equipment_health (
    health_id SERIAL PRIMARY KEY,
    equipment_id INTEGER REFERENCES network_equipment(equipment_id),
    timestamp BIGINT NOT NULL,
    temperature_celsius DECIMAL(5, 2),
    cpu_utilization_pct DECIMAL(5, 2),
    memory_utilization_pct DECIMAL(5, 2),
    error_count INTEGER,
    packet_drops INTEGER,
    uptime_hours INTEGER
);

-- Insert sample equipment health data
INSERT INTO equipment_health (equipment_id, timestamp, temperature_celsius, cpu_utilization_pct,
                              memory_utilization_pct, error_count, packet_drops, uptime_hours)
SELECT 
    1,
    extract(epoch from CURRENT_TIMESTAMP - (n || ' hours')::INTERVAL)::bigint * 1000,
    (random() * 20 + 40)::DECIMAL(5, 2),
    (random() * 40 + 40)::DECIMAL(5, 2),
    (random() * 30 + 50)::DECIMAL(5, 2),
    (random() * 10)::INTEGER,
    (random() * 100)::INTEGER,
    (720 - n)::INTEGER
FROM generate_series(1, 168) AS n; -- 1 week of hourly data

-- Calculate equipment failure probability
CREATE OR REPLACE FUNCTION calculate_equipment_failure_risk(
    p_days_since_maintenance INTEGER,
    p_avg_temperature DECIMAL,
    p_avg_cpu_utilization DECIMAL,
    p_total_errors INTEGER,
    p_firmware_age_days INTEGER
) RETURNS FLOAT AS $$
DECLARE
    failure_prob FLOAT := 0.0;
BEGIN
    -- Maintenance overdue
    IF p_days_since_maintenance > 180 THEN
        failure_prob := failure_prob + 0.25;
    END IF;
    
    -- High temperature
    IF p_avg_temperature > 70 THEN
        failure_prob := failure_prob + 0.2;
    END IF;
    
    -- High CPU utilization
    IF p_avg_cpu_utilization > 80 THEN
        failure_prob := failure_prob + 0.15;
    END IF;
    
    -- Error accumulation
    IF p_total_errors > 100 THEN
        failure_prob := failure_prob + 0.2;
    END IF;
    
    -- Outdated firmware
    IF p_firmware_age_days > 365 THEN
        failure_prob := failure_prob + 0.2;
    END IF;
    
    RETURN LEAST(failure_prob, 1.0);
END;
$$ LANGUAGE plpgsql;

-- Update failure probabilities
WITH equipment_metrics AS (
    SELECT 
        ne.equipment_id,
        CURRENT_DATE - ne.last_maintenance_date AS days_since_maintenance,
        AVG(eh.temperature_celsius) AS avg_temp,
        AVG(eh.cpu_utilization_pct) AS avg_cpu,
        SUM(eh.error_count) AS total_errors,
        CURRENT_DATE - ne.installation_date AS firmware_age_days
    FROM network_equipment ne
    LEFT JOIN equipment_health eh ON ne.equipment_id = eh.equipment_id
    WHERE eh.timestamp > extract(epoch from CURRENT_TIMESTAMP - INTERVAL '7 days')::bigint * 1000
    GROUP BY ne.equipment_id, ne.last_maintenance_date, ne.installation_date
)
UPDATE network_equipment ne
SET failure_probability = calculate_equipment_failure_risk(
    em.days_since_maintenance,
    COALESCE(em.avg_temp, 50),
    COALESCE(em.avg_cpu, 50),
    COALESCE(em.total_errors, 0),
    em.firmware_age_days
)
FROM equipment_metrics em
WHERE ne.equipment_id = em.equipment_id;

-- Maintenance priority list
SELECT 
    ne.equipment_name,
    ne.equipment_type,
    ct.tower_name,
    ne.last_maintenance_date,
    CURRENT_DATE - ne.last_maintenance_date AS days_since_maintenance,
    ne.failure_probability,
    CASE 
        WHEN ne.failure_probability > 0.7 THEN 'CRITICAL - Schedule Immediately'
        WHEN ne.failure_probability > 0.5 THEN 'HIGH - Schedule This Week'
        WHEN ne.failure_probability > 0.3 THEN 'MEDIUM - Schedule This Month'
        ELSE 'LOW - Normal Schedule'
    END AS maintenance_priority
FROM network_equipment ne
JOIN cell_towers ct ON ne.tower_id = ct.tower_id
ORDER BY ne.failure_probability DESC;

-- ----------------------------------------------------------------------------
-- 5. CUSTOMER EXPERIENCE ANALYTICS
-- ----------------------------------------------------------------------------

-- Create customer experience events table
CREATE TABLE IF NOT EXISTS customer_experience_events (
    event_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customer_accounts(customer_id),
    event_timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    event_type VARCHAR(50), -- 'CALL_DROP', 'SLOW_DATA', 'NO_SIGNAL', 'BILLING_ISSUE'
    severity VARCHAR(20), -- 'LOW', 'MEDIUM', 'HIGH', 'CRITICAL'
    tower_id INTEGER REFERENCES cell_towers(tower_id),
    resolution_time_minutes INTEGER,
    customer_satisfaction_score INTEGER, -- 1-5
    description TEXT
);

-- Insert sample customer experience events
INSERT INTO customer_experience_events (customer_id, event_timestamp, event_type, severity, 
                                       tower_id, resolution_time_minutes, customer_satisfaction_score, description) VALUES
(1, CURRENT_TIMESTAMP - INTERVAL '2 days', 'CALL_DROP', 'MEDIUM', 1, 30, 3, 'Call dropped during important conversation'),
(2, CURRENT_TIMESTAMP - INTERVAL '1 day', 'SLOW_DATA', 'HIGH', 1, 120, 2, 'Very slow data speeds, unable to stream video'),
(3, CURRENT_TIMESTAMP - INTERVAL '3 hours', 'NO_SIGNAL', 'CRITICAL', 2, 180, 1, 'Complete loss of signal for 3 hours'),
(4, CURRENT_TIMESTAMP - INTERVAL '5 days', 'BILLING_ISSUE', 'MEDIUM', NULL, 45, 4, 'Incorrect charge on bill, resolved quickly');

-- Calculate customer experience score
WITH customer_experience_metrics AS (
    SELECT 
        ca.customer_id,
        ca.account_number,
        ca.customer_name,
        COUNT(cee.event_id) AS total_issues,
        AVG(cee.customer_satisfaction_score) AS avg_satisfaction,
        SUM(CASE WHEN cee.severity = 'CRITICAL' THEN 1 ELSE 0 END) AS critical_issues,
        AVG(cee.resolution_time_minutes) AS avg_resolution_time
    FROM customer_accounts ca
    LEFT JOIN customer_experience_events cee ON ca.customer_id = cee.customer_id
    WHERE cee.event_timestamp > CURRENT_TIMESTAMP - INTERVAL '30 days'
    GROUP BY ca.customer_id, ca.account_number, ca.customer_name
)
SELECT 
    customer_id,
    account_number,
    customer_name,
    total_issues,
    avg_satisfaction,
    critical_issues,
    avg_resolution_time,
    CASE 
        WHEN avg_satisfaction < 2.5 OR critical_issues > 2 THEN 'POOR - Immediate Action Required'
        WHEN avg_satisfaction < 3.5 OR critical_issues > 0 THEN 'FAIR - Needs Improvement'
        WHEN avg_satisfaction < 4.5 THEN 'GOOD - Satisfactory'
        ELSE 'EXCELLENT - Highly Satisfied'
    END AS experience_rating
FROM customer_experience_metrics
WHERE total_issues > 0
ORDER BY avg_satisfaction ASC, critical_issues DESC;

-- Network quality by location
SELECT 
    ct.tower_name,
    ct.technology,
    COUNT(cee.event_id) AS total_incidents,
    SUM(CASE WHEN cee.event_type = 'CALL_DROP' THEN 1 ELSE 0 END) AS call_drops,
    SUM(CASE WHEN cee.event_type = 'SLOW_DATA' THEN 1 ELSE 0 END) AS slow_data_incidents,
    SUM(CASE WHEN cee.event_type = 'NO_SIGNAL' THEN 1 ELSE 0 END) AS no_signal_incidents,
    AVG(cee.customer_satisfaction_score) AS avg_satisfaction,
    CASE 
        WHEN COUNT(cee.event_id) > 10 THEN 'PROBLEM AREA - Investigate'
        WHEN COUNT(cee.event_id) > 5 THEN 'MONITOR CLOSELY'
        ELSE 'NORMAL'
    END AS area_status
FROM cell_towers ct
LEFT JOIN customer_experience_events cee ON ct.tower_id = cee.tower_id
WHERE cee.event_timestamp > CURRENT_TIMESTAMP - INTERVAL '30 days'
GROUP BY ct.tower_id, ct.tower_name, ct.technology
ORDER BY total_incidents DESC;

-- ----------------------------------------------------------------------------
-- 6. DATA USAGE PREDICTION & PLAN RECOMMENDATIONS
-- ----------------------------------------------------------------------------

-- Create data usage history
CREATE TABLE IF NOT EXISTS data_usage_history (
    usage_id SERIAL PRIMARY KEY,
    customer_id INTEGER REFERENCES customer_accounts(customer_id),
    usage_date DATE,
    data_used_gb DECIMAL(10, 2),
    peak_hour_usage_gb DECIMAL(10, 2),
    video_streaming_gb DECIMAL(10, 2),
    social_media_gb DECIMAL(10, 2),
    other_gb DECIMAL(10, 2)
);

-- Insert sample data usage history (last 30 days)
INSERT INTO data_usage_history (customer_id, usage_date, data_used_gb, peak_hour_usage_gb,
                                video_streaming_gb, social_media_gb, other_gb)
SELECT 
    1,
    CURRENT_DATE - (n || ' days')::INTERVAL,
    (random() * 2 + 0.5)::DECIMAL(10, 2),
    (random() * 0.5)::DECIMAL(10, 2),
    (random() * 1.0)::DECIMAL(10, 2),
    (random() * 0.5)::DECIMAL(10, 2),
    (random() * 0.5)::DECIMAL(10, 2)
FROM generate_series(1, 30) AS n;

-- Predict next month's data usage using trend analysis
WITH usage_trends AS (
    SELECT 
        customer_id,
        AVG(data_used_gb) AS avg_daily_usage,
        STDDEV(data_used_gb) AS usage_volatility,
        MAX(data_used_gb) AS peak_daily_usage,
        SUM(data_used_gb) AS total_monthly_usage
    FROM data_usage_history
    WHERE usage_date > CURRENT_DATE - INTERVAL '30 days'
    GROUP BY customer_id
)
SELECT 
    ca.customer_id,
    ca.account_number,
    ca.customer_name,
    ca.plan_type,
    ca.data_usage_gb_monthly AS current_plan_data,
    ut.total_monthly_usage AS actual_usage_last_month,
    (ut.avg_daily_usage * 30) AS predicted_next_month_usage,
    CASE 
        WHEN ut.avg_daily_usage * 30 > ca.data_usage_gb_monthly * 1.2 THEN 'RECOMMEND UPGRADE'
        WHEN ut.avg_daily_usage * 30 < ca.data_usage_gb_monthly * 0.5 THEN 'RECOMMEND DOWNGRADE'
        ELSE 'CURRENT PLAN SUITABLE'
    END AS plan_recommendation,
    CASE 
        WHEN ut.avg_daily_usage * 30 > ca.data_usage_gb_monthly * 1.2 
        THEN CEIL((ut.avg_daily_usage * 30 - ca.data_usage_gb_monthly) / 5) * 5
        ELSE 0
    END AS additional_gb_needed
FROM customer_accounts ca
JOIN usage_trends ut ON ca.customer_id = ut.customer_id
ORDER BY predicted_next_month_usage DESC;

-- ============================================================================
-- SUMMARY: Telecommunications ML Use Cases Demonstrated
-- ============================================================================
-- 1. Network performance monitoring with anomaly detection
-- 2. Customer churn prediction with retention strategies
-- 3. Fraud detection in call detail records
-- 4. Network equipment predictive maintenance
-- 5. Customer experience analytics and quality monitoring
-- 6. Data usage prediction and plan recommendations
-- ============================================================================
