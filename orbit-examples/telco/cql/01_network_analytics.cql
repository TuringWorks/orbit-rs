-- =============================================================================
-- OrbitRS Telco Example: Network Analytics with CQL
-- =============================================================================
-- Demonstrates time-series data modeling for telecom network monitoring
-- using Cassandra Query Language (CQL) with OrbitRS.

-- =============================================================================
-- KEYSPACE SETUP
-- =============================================================================

-- Create keyspace for network analytics
CREATE KEYSPACE IF NOT EXISTS telco_network
WITH REPLICATION = {
    'class': 'SimpleStrategy',
    'replication_factor': 3
};

USE telco_network;

-- =============================================================================
-- NETWORK METRICS TABLES
-- =============================================================================

-- Cell tower metrics - time-series data partitioned by tower and date
CREATE TABLE IF NOT EXISTS cell_tower_metrics (
    tower_id TEXT,
    metric_date DATE,
    metric_time TIMESTAMP,
    signal_strength_dbm DOUBLE,
    bandwidth_mbps DOUBLE,
    active_connections INT,
    dropped_calls INT,
    handover_success_rate DOUBLE,
    latency_ms DOUBLE,
    packet_loss_rate DOUBLE,
    power_consumption_kw DOUBLE,
    temperature_celsius DOUBLE,
    PRIMARY KEY ((tower_id, metric_date), metric_time)
) WITH CLUSTERING ORDER BY (metric_time DESC)
AND default_time_to_live = 7776000  -- 90 days retention
AND compaction = {
    'class': 'TimeWindowCompactionStrategy',
    'compaction_window_size': 1,
    'compaction_window_unit': 'DAYS'
};

-- Network node status for real-time monitoring
CREATE TABLE IF NOT EXISTS network_nodes (
    region TEXT,
    node_type TEXT,
    node_id TEXT,
    status TEXT,
    last_heartbeat TIMESTAMP,
    ip_address INET,
    firmware_version TEXT,
    uptime_seconds BIGINT,
    cpu_usage_percent DOUBLE,
    memory_usage_percent DOUBLE,
    disk_usage_percent DOUBLE,
    PRIMARY KEY ((region, node_type), node_id)
);

-- CDR (Call Detail Records) for billing and analytics
CREATE TABLE IF NOT EXISTS call_detail_records (
    subscriber_id TEXT,
    call_month TEXT,
    call_timestamp TIMESTAMP,
    call_id UUID,
    call_type TEXT,
    calling_number TEXT,
    called_number TEXT,
    duration_seconds INT,
    data_bytes BIGINT,
    originating_tower TEXT,
    terminating_tower TEXT,
    call_status TEXT,
    roaming BOOLEAN,
    PRIMARY KEY ((subscriber_id, call_month), call_timestamp, call_id)
) WITH CLUSTERING ORDER BY (call_timestamp DESC);

-- Subscriber data usage aggregates
CREATE TABLE IF NOT EXISTS subscriber_usage_daily (
    subscriber_id TEXT,
    usage_date DATE,
    voice_minutes INT,
    sms_count INT,
    data_bytes_upload BIGINT,
    data_bytes_download BIGINT,
    mms_count INT,
    roaming_data_bytes BIGINT,
    international_minutes INT,
    PRIMARY KEY (subscriber_id, usage_date)
) WITH CLUSTERING ORDER BY (usage_date DESC);

-- Network alerts and incidents
CREATE TABLE IF NOT EXISTS network_alerts (
    alert_date DATE,
    severity TEXT,
    alert_time TIMESTAMP,
    alert_id UUID,
    tower_id TEXT,
    alert_type TEXT,
    description TEXT,
    affected_subscribers INT,
    resolution_status TEXT,
    acknowledged_by TEXT,
    resolved_at TIMESTAMP,
    PRIMARY KEY ((alert_date, severity), alert_time, alert_id)
) WITH CLUSTERING ORDER BY (alert_time DESC);

-- =============================================================================
-- MATERIALIZED VIEWS
-- =============================================================================

-- View for querying alerts by tower
CREATE MATERIALIZED VIEW IF NOT EXISTS alerts_by_tower AS
SELECT * FROM network_alerts
WHERE tower_id IS NOT NULL
AND alert_date IS NOT NULL
AND severity IS NOT NULL
AND alert_time IS NOT NULL
AND alert_id IS NOT NULL
PRIMARY KEY ((tower_id), alert_date, severity, alert_time, alert_id);

-- =============================================================================
-- SAMPLE DATA INSERTION
-- =============================================================================

-- Insert cell tower metrics
INSERT INTO cell_tower_metrics (
    tower_id, metric_date, metric_time,
    signal_strength_dbm, bandwidth_mbps, active_connections,
    dropped_calls, handover_success_rate, latency_ms,
    packet_loss_rate, power_consumption_kw, temperature_celsius
) VALUES (
    'TOWER-NYC-001', '2024-01-15', '2024-01-15 10:00:00',
    -75.5, 850.0, 1250,
    3, 0.987, 12.5,
    0.001, 4.5, 22.3
);

INSERT INTO cell_tower_metrics (
    tower_id, metric_date, metric_time,
    signal_strength_dbm, bandwidth_mbps, active_connections,
    dropped_calls, handover_success_rate, latency_ms,
    packet_loss_rate, power_consumption_kw, temperature_celsius
) VALUES (
    'TOWER-NYC-001', '2024-01-15', '2024-01-15 10:05:00',
    -74.2, 920.0, 1320,
    1, 0.992, 11.8,
    0.0008, 4.7, 22.5
);

-- Insert network nodes
INSERT INTO network_nodes (
    region, node_type, node_id, status, last_heartbeat,
    ip_address, firmware_version, uptime_seconds,
    cpu_usage_percent, memory_usage_percent, disk_usage_percent
) VALUES (
    'us-east-1', 'core-router', 'CR-NYC-MAIN-01', 'active', toTimestamp(now()),
    '10.1.1.1', '7.2.1', 864000,
    45.2, 62.1, 38.5
);

INSERT INTO network_nodes (
    region, node_type, node_id, status, last_heartbeat,
    ip_address, firmware_version, uptime_seconds,
    cpu_usage_percent, memory_usage_percent, disk_usage_percent
) VALUES (
    'us-east-1', 'edge-node', 'EN-NYC-001', 'active', toTimestamp(now()),
    '10.1.2.1', '5.1.3', 432000,
    32.8, 55.4, 42.1
);

-- Insert call detail records
INSERT INTO call_detail_records (
    subscriber_id, call_month, call_timestamp, call_id,
    call_type, calling_number, called_number, duration_seconds,
    data_bytes, originating_tower, terminating_tower,
    call_status, roaming
) VALUES (
    'SUB-12345', '2024-01', '2024-01-15 10:30:00', uuid(),
    'VOICE', '+1-555-0100', '+1-555-0200', 185,
    0, 'TOWER-NYC-001', 'TOWER-NYC-002',
    'COMPLETED', false
);

-- Insert subscriber usage
INSERT INTO subscriber_usage_daily (
    subscriber_id, usage_date,
    voice_minutes, sms_count, data_bytes_upload, data_bytes_download,
    mms_count, roaming_data_bytes, international_minutes
) VALUES (
    'SUB-12345', '2024-01-15',
    45, 23, 524288000, 2147483648,
    2, 0, 5
);

-- Insert network alert
INSERT INTO network_alerts (
    alert_date, severity, alert_time, alert_id,
    tower_id, alert_type, description,
    affected_subscribers, resolution_status
) VALUES (
    '2024-01-15', 'WARNING', '2024-01-15 09:45:00', uuid(),
    'TOWER-NYC-001', 'HIGH_LOAD', 'Tower experiencing high connection load',
    450, 'OPEN'
);

-- =============================================================================
-- ANALYTICAL QUERIES
-- =============================================================================

-- Query 1: Get latest metrics for a specific tower
SELECT tower_id, metric_time, signal_strength_dbm,
       active_connections, dropped_calls, latency_ms
FROM cell_tower_metrics
WHERE tower_id = 'TOWER-NYC-001'
AND metric_date = '2024-01-15'
LIMIT 10;

-- Query 2: Find all active network nodes in a region
SELECT node_id, status, cpu_usage_percent, memory_usage_percent, uptime_seconds
FROM network_nodes
WHERE region = 'us-east-1' AND node_type = 'core-router';

-- Query 3: Get subscriber's call history for a month
SELECT call_timestamp, call_type, called_number, duration_seconds, call_status
FROM call_detail_records
WHERE subscriber_id = 'SUB-12345' AND call_month = '2024-01'
ORDER BY call_timestamp DESC
LIMIT 50;

-- Query 4: Check subscriber's daily usage
SELECT usage_date, voice_minutes, sms_count,
       data_bytes_download / 1073741824 AS data_gb
FROM subscriber_usage_daily
WHERE subscriber_id = 'SUB-12345'
ORDER BY usage_date DESC
LIMIT 30;

-- Query 5: Get critical alerts for today
SELECT alert_time, tower_id, alert_type, description, affected_subscribers
FROM network_alerts
WHERE alert_date = '2024-01-15' AND severity = 'CRITICAL';

-- Query 6: Aggregated tower metrics (using secondary query)
SELECT tower_id, metric_time,
       AVG(signal_strength_dbm) as avg_signal,
       SUM(dropped_calls) as total_dropped,
       AVG(latency_ms) as avg_latency
FROM cell_tower_metrics
WHERE tower_id = 'TOWER-NYC-001'
AND metric_date = '2024-01-15'
GROUP BY tower_id, metric_time;

-- =============================================================================
-- TTL AND DATA MANAGEMENT
-- =============================================================================

-- Insert with custom TTL (7 days)
INSERT INTO cell_tower_metrics (
    tower_id, metric_date, metric_time, signal_strength_dbm, active_connections
) VALUES (
    'TOWER-NYC-TEMP', '2024-01-15', '2024-01-15 12:00:00', -80.0, 100
) USING TTL 604800;

-- Update with TTL
UPDATE cell_tower_metrics USING TTL 86400
SET signal_strength_dbm = -72.0
WHERE tower_id = 'TOWER-NYC-001'
AND metric_date = '2024-01-15'
AND metric_time = '2024-01-15 10:00:00';

-- =============================================================================
-- BATCH OPERATIONS
-- =============================================================================

-- Batch insert for atomic operations
BEGIN BATCH
    INSERT INTO network_nodes (region, node_type, node_id, status, last_heartbeat)
    VALUES ('us-east-1', 'edge-node', 'EN-NYC-002', 'active', toTimestamp(now()));

    INSERT INTO network_nodes (region, node_type, node_id, status, last_heartbeat)
    VALUES ('us-east-1', 'edge-node', 'EN-NYC-003', 'active', toTimestamp(now()));

    INSERT INTO network_nodes (region, node_type, node_id, status, last_heartbeat)
    VALUES ('us-east-1', 'edge-node', 'EN-NYC-004', 'active', toTimestamp(now()));
APPLY BATCH;

-- Lightweight transactions for conditional updates
UPDATE network_nodes
SET status = 'maintenance', last_heartbeat = toTimestamp(now())
WHERE region = 'us-east-1' AND node_type = 'edge-node' AND node_id = 'EN-NYC-001'
IF status = 'active';
