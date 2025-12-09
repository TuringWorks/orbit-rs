-- ============================================================================
-- OrbitRS Logistics Examples - Shipment Events & IoT Time-Series (CQL)
-- ============================================================================
-- Wide-column time-series storage optimized for:
-- - High-volume shipment tracking events
-- - Vehicle telemetry from fleet IoT devices
-- - Warehouse sensor data
-- - Route history and analytics
-- ============================================================================

CREATE KEYSPACE IF NOT EXISTS logistics_events
WITH replication = {'class': 'NetworkTopologyStrategy', 'dc1': 3}
AND durable_writes = true;

USE logistics_events;

-- ============================================================================
-- SHIPMENT TRACKING EVENTS
-- ============================================================================
-- Partition by tracking number + month for efficient history queries
-- Supports millions of events per day across all shipments

CREATE TABLE shipment_events (
    tracking_number text,
    event_month text,  -- Format: '2024-12'
    event_time timestamp,
    event_id uuid,
    -- Event details
    event_type text,  -- CREATED, PICKED_UP, IN_TRANSIT, ARRIVED, DEPARTED, OUT_FOR_DELIVERY, DELIVERED, EXCEPTION
    event_code text,  -- Standardized code like 'OFD', 'DEL', 'EXC'
    -- Location
    facility_id text,
    facility_name text,
    city text,
    state text,
    country text,
    location_lat double,
    location_lon double,
    -- Details
    description text,
    handler_id text,
    vehicle_id text,
    signature text,
    proof_of_delivery text,
    -- Exception handling
    exception_code text,
    exception_reason text,
    resolution_required boolean,
    PRIMARY KEY ((tracking_number, event_month), event_time, event_id)
) WITH CLUSTERING ORDER BY (event_time DESC, event_id ASC)
  AND default_time_to_live = 7776000  -- 90 days retention
  AND compaction = {'class': 'TimeWindowCompactionStrategy', 'compaction_window_size': 1, 'compaction_window_unit': 'DAYS'}
  AND compression = {'sstable_compression': 'LZ4Compressor'};

-- Insert shipment events
INSERT INTO shipment_events (
    tracking_number, event_month, event_time, event_id,
    event_type, event_code, facility_id, facility_name,
    city, state, country, description
) VALUES (
    'TRK-998877', '2024-12', '2024-12-08 10:00:00', uuid(),
    'CREATED', 'CRE', 'LAX-DC-001', 'Los Angeles Distribution Center',
    'Los Angeles', 'CA', 'US', 'Shipping label created'
);

INSERT INTO shipment_events (
    tracking_number, event_month, event_time, event_id,
    event_type, event_code, facility_id, facility_name,
    city, state, country, handler_id, description
) VALUES (
    'TRK-998877', '2024-12', '2024-12-08 14:30:00', uuid(),
    'PICKED_UP', 'PKU', 'LAX-DC-001', 'Los Angeles Distribution Center',
    'Los Angeles', 'CA', 'US', 'H-1001', 'Package picked up from sender'
);

INSERT INTO shipment_events (
    tracking_number, event_month, event_time, event_id,
    event_type, event_code, facility_id, facility_name,
    city, state, country, vehicle_id, description
) VALUES (
    'TRK-998877', '2024-12', '2024-12-09 08:00:00', uuid(),
    'OUT_FOR_DELIVERY', 'OFD', 'NYC-SORT-001', 'New York Sort Facility',
    'New York', 'NY', 'US', 'V-1001', 'Out for delivery'
);

-- Query: Get tracking history
SELECT event_time, event_type, facility_name, city, description
FROM shipment_events
WHERE tracking_number = 'TRK-998877'
  AND event_month = '2024-12'
ORDER BY event_time DESC;

-- ============================================================================
-- VEHICLE TELEMETRY (Fleet IoT)
-- ============================================================================
-- High-frequency GPS and sensor data from delivery vehicles

CREATE TABLE vehicle_telemetry (
    vehicle_id text,
    telemetry_date date,
    telemetry_time timestamp,
    -- Location
    latitude double,
    longitude double,
    altitude double,
    heading int,  -- 0-359 degrees
    speed_kmh double,
    -- Vehicle sensors
    odometer_km double,
    fuel_level_pct int,
    engine_temp_c double,
    battery_voltage double,
    -- Status
    ignition_on boolean,
    door_status text,  -- CLOSED, OPEN, REAR_OPEN
    -- Driver behavior
    harsh_braking boolean,
    harsh_acceleration boolean,
    speeding boolean,
    idling boolean,
    PRIMARY KEY ((vehicle_id, telemetry_date), telemetry_time)
) WITH CLUSTERING ORDER BY (telemetry_time DESC)
  AND default_time_to_live = 604800  -- 7 days for raw telemetry
  AND compaction = {'class': 'TimeWindowCompactionStrategy', 'compaction_window_size': 1, 'compaction_window_unit': 'HOURS'}
  AND compression = {'sstable_compression': 'LZ4Compressor'};

-- Insert telemetry
INSERT INTO vehicle_telemetry (
    vehicle_id, telemetry_date, telemetry_time,
    latitude, longitude, speed_kmh, heading,
    fuel_level_pct, engine_temp_c, ignition_on, door_status
) VALUES (
    'V-1001', '2024-12-09', '2024-12-09 10:00:00',
    40.7128, -74.0060, 35.5, 90,
    75, 85.5, true, 'CLOSED'
);

INSERT INTO vehicle_telemetry (
    vehicle_id, telemetry_date, telemetry_time,
    latitude, longitude, speed_kmh, heading,
    fuel_level_pct, engine_temp_c, ignition_on, door_status,
    harsh_braking
) VALUES (
    'V-1001', '2024-12-09', '2024-12-09 10:01:00',
    40.7135, -74.0055, 0.0, 90,
    75, 86.0, true, 'REAR_OPEN',
    true
);

-- Query: Last hour of telemetry for a vehicle
SELECT telemetry_time, latitude, longitude, speed_kmh, fuel_level_pct
FROM vehicle_telemetry
WHERE vehicle_id = 'V-1001'
  AND telemetry_date = '2024-12-09'
  AND telemetry_time >= '2024-12-09 09:00:00'
LIMIT 100;

-- ============================================================================
-- WAREHOUSE SENSOR DATA
-- ============================================================================
-- Temperature, humidity, and environmental monitoring for cold chain

CREATE TABLE warehouse_sensors (
    warehouse_id text,
    zone_id text,  -- COLD_STORAGE, AMBIENT, DOCK_1, etc.
    sensor_date date,
    reading_time timestamp,
    sensor_id text,
    -- Readings
    temperature_c double,
    humidity_pct double,
    co2_ppm int,
    -- Alerts
    temp_alert boolean,
    humidity_alert boolean,
    -- Equipment
    hvac_status text,
    door_open_duration_sec int,
    PRIMARY KEY ((warehouse_id, zone_id, sensor_date), reading_time, sensor_id)
) WITH CLUSTERING ORDER BY (reading_time DESC, sensor_id ASC)
  AND default_time_to_live = 2592000  -- 30 days
  AND compaction = {'class': 'TimeWindowCompactionStrategy', 'compaction_window_size': 1, 'compaction_window_unit': 'HOURS'};

-- Insert sensor reading
INSERT INTO warehouse_sensors (
    warehouse_id, zone_id, sensor_date, reading_time, sensor_id,
    temperature_c, humidity_pct, temp_alert, hvac_status
) VALUES (
    'WH-NYC-001', 'COLD_STORAGE', '2024-12-09', '2024-12-09 10:00:00', 'TEMP-001',
    -18.5, 45.0, false, 'RUNNING'
);

-- Query: Check cold storage temperatures
SELECT reading_time, sensor_id, temperature_c, humidity_pct, temp_alert
FROM warehouse_sensors
WHERE warehouse_id = 'WH-NYC-001'
  AND zone_id = 'COLD_STORAGE'
  AND sensor_date = '2024-12-09';

-- ============================================================================
-- DELIVERY PERFORMANCE
-- ============================================================================
-- Per-delivery metrics for SLA tracking

CREATE TABLE delivery_metrics (
    carrier text,
    delivery_date date,
    tracking_number text,
    -- Timing
    promised_delivery_time timestamp,
    actual_delivery_time timestamp,
    delivery_duration_min int,
    -- Performance
    on_time boolean,
    delay_minutes int,
    delay_reason text,
    -- Attempt tracking
    delivery_attempts int,
    first_attempt_time timestamp,
    -- Quality
    damage_reported boolean,
    customer_rating int,  -- 1-5
    -- Cost
    delivery_cost decimal,
    surcharges decimal,
    PRIMARY KEY ((carrier, delivery_date), tracking_number)
) WITH CLUSTERING ORDER BY (tracking_number ASC)
  AND default_time_to_live = 31536000;  -- 1 year

-- Insert delivery metric
INSERT INTO delivery_metrics (
    carrier, delivery_date, tracking_number,
    promised_delivery_time, actual_delivery_time, delivery_duration_min,
    on_time, delivery_attempts, customer_rating, delivery_cost
) VALUES (
    'UPS', '2024-12-09', 'TRK-998877',
    '2024-12-09 17:00:00', '2024-12-09 14:30:00', 45,
    true, 1, 5, 12.99
);

-- Query: Carrier performance for the day
SELECT tracking_number, on_time, delay_minutes, customer_rating
FROM delivery_metrics
WHERE carrier = 'UPS'
  AND delivery_date = '2024-12-09';

-- ============================================================================
-- ROUTE HISTORY
-- ============================================================================
-- Historical routes for ML training and optimization

CREATE TABLE route_history (
    zone text,
    route_date date,
    route_id text,
    -- Route details
    driver_id text,
    vehicle_id text,
    start_time timestamp,
    end_time timestamp,
    -- Stops
    planned_stops int,
    completed_stops int,
    failed_stops int,
    -- Distance and time
    planned_distance_km double,
    actual_distance_km double,
    planned_duration_min int,
    actual_duration_min int,
    -- Efficiency
    stops_per_hour double,
    fuel_consumed_liters double,
    cost_per_stop decimal,
    -- Conditions
    weather_conditions text,
    traffic_level text,  -- LIGHT, MODERATE, HEAVY
    PRIMARY KEY ((zone, route_date), route_id)
);

-- Insert route history
INSERT INTO route_history (
    zone, route_date, route_id,
    driver_id, vehicle_id, start_time, end_time,
    planned_stops, completed_stops, failed_stops,
    planned_distance_km, actual_distance_km,
    planned_duration_min, actual_duration_min,
    stops_per_hour, weather_conditions, traffic_level
) VALUES (
    'NYC-MANHATTAN', '2024-12-09', 'R-2024-001',
    'D-101', 'V-1001', '2024-12-09 08:00:00', '2024-12-09 16:30:00',
    45, 43, 2,
    85.5, 92.3,
    480, 510,
    5.1, 'CLEAR', 'MODERATE'
);

-- Query: Route efficiency by zone
SELECT route_id, completed_stops, actual_duration_min, stops_per_hour
FROM route_history
WHERE zone = 'NYC-MANHATTAN'
  AND route_date = '2024-12-09';

-- ============================================================================
-- INVENTORY MOVEMENTS
-- ============================================================================
-- Track all inventory movements for audit and analytics

CREATE TABLE inventory_movements (
    warehouse_id text,
    movement_date date,
    movement_time timestamp,
    movement_id uuid,
    -- Item
    sku text,
    quantity int,
    -- Movement type
    movement_type text,  -- RECEIVE, PICK, PACK, SHIP, TRANSFER, ADJUST, RETURN
    -- Locations
    from_location text,
    to_location text,
    -- Reference
    order_id text,
    shipment_id text,
    po_number text,
    -- Handler
    handler_id text,
    -- Quality
    condition text,  -- NEW, DAMAGED, EXPIRED
    lot_number text,
    expiry_date date,
    PRIMARY KEY ((warehouse_id, movement_date), movement_time, movement_id)
) WITH CLUSTERING ORDER BY (movement_time DESC, movement_id ASC)
  AND default_time_to_live = 31536000;  -- 1 year

-- Insert inventory movement
INSERT INTO inventory_movements (
    warehouse_id, movement_date, movement_time, movement_id,
    sku, quantity, movement_type,
    from_location, to_location,
    order_id, handler_id, condition
) VALUES (
    'WH-NYC-001', '2024-12-09', '2024-12-09 10:30:00', uuid(),
    'SKU-001', 5, 'PICK',
    'A-01-02-03', 'PACK-STATION-01',
    'ORD-12345', 'H-1001', 'NEW'
);

-- Query: Today's picks for a warehouse
SELECT movement_time, sku, quantity, from_location, order_id
FROM inventory_movements
WHERE warehouse_id = 'WH-NYC-001'
  AND movement_date = '2024-12-09'
  AND movement_type = 'PICK'
ALLOW FILTERING;

-- ============================================================================
-- CARRIER PERFORMANCE AGGREGATES
-- ============================================================================
-- Pre-aggregated carrier stats for dashboards

CREATE TABLE carrier_daily_stats (
    carrier text,
    stat_date date,
    -- Volume
    total_deliveries int,
    on_time_deliveries int,
    late_deliveries int,
    failed_deliveries int,
    -- Performance
    on_time_rate double,
    avg_delay_minutes double,
    avg_rating double,
    -- Cost
    total_cost decimal,
    avg_cost_per_delivery decimal,
    -- SLA
    sla_breaches int,
    PRIMARY KEY (carrier, stat_date)
) WITH CLUSTERING ORDER BY (stat_date DESC);

-- Insert carrier stats
INSERT INTO carrier_daily_stats (
    carrier, stat_date,
    total_deliveries, on_time_deliveries, late_deliveries, failed_deliveries,
    on_time_rate, avg_delay_minutes, avg_rating,
    total_cost, avg_cost_per_delivery, sla_breaches
) VALUES (
    'UPS', '2024-12-09',
    1250, 1175, 65, 10,
    0.94, 12.5, 4.7,
    15625.00, 12.50, 3
);

-- Query: Carrier performance trend
SELECT stat_date, on_time_rate, avg_rating, total_deliveries
FROM carrier_daily_stats
WHERE carrier = 'UPS'
LIMIT 30;

-- ============================================================================
-- ROUTE EMBEDDINGS (OrbitRS Extension)
-- ============================================================================
-- Vector storage for ML-based route optimization

CREATE TABLE route_embeddings (
    zone text,
    route_id text,
    -- Embedding
    embedding vector<float, 64>,
    -- Route features for filtering
    total_stops int,
    total_distance_km double,
    day_of_week int,
    -- Performance metrics
    duration_min int,
    efficiency_score double,
    PRIMARY KEY (zone, route_id)
);

-- Insert route embedding
INSERT INTO route_embeddings (
    zone, route_id,
    total_stops, total_distance_km, day_of_week,
    duration_min, efficiency_score
) VALUES (
    'NYC-MANHATTAN', 'R-2024-001',
    45, 85.5, 1,
    510, 0.85
);

-- ANN query to find similar routes (OrbitRS extension)
-- SELECT route_id, total_stops, duration_min, efficiency_score
-- FROM route_embeddings
-- WHERE zone = 'NYC-MANHATTAN'
-- ORDER BY embedding ANN OF [0.1, 0.2, ...]
-- LIMIT 10;

-- ============================================================================
-- GEOFENCE EVENTS
-- ============================================================================
-- Track when vehicles enter/exit geofences

CREATE TABLE geofence_events (
    vehicle_id text,
    event_date date,
    event_time timestamp,
    event_id uuid,
    -- Geofence
    geofence_id text,
    geofence_name text,
    geofence_type text,  -- WAREHOUSE, CUSTOMER, NO_GO_ZONE, SERVICE_AREA
    -- Event
    event_type text,  -- ENTER, EXIT, DWELL_START, DWELL_END
    -- Location
    latitude double,
    longitude double,
    -- Duration
    dwell_duration_sec int,
    PRIMARY KEY ((vehicle_id, event_date), event_time, event_id)
) WITH CLUSTERING ORDER BY (event_time DESC, event_id ASC)
  AND default_time_to_live = 2592000;  -- 30 days

-- Insert geofence event
INSERT INTO geofence_events (
    vehicle_id, event_date, event_time, event_id,
    geofence_id, geofence_name, geofence_type, event_type,
    latitude, longitude
) VALUES (
    'V-1001', '2024-12-09', '2024-12-09 10:30:00', uuid(),
    'GF-WH-NYC-001', 'NYC Distribution Center', 'WAREHOUSE', 'EXIT',
    40.7128, -74.0060
);

-- Query: Vehicle geofence activity today
SELECT event_time, geofence_name, event_type, dwell_duration_sec
FROM geofence_events
WHERE vehicle_id = 'V-1001'
  AND event_date = '2024-12-09';

