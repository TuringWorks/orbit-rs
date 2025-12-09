# Logistics & Supply Chain

Comprehensive examples demonstrating OrbitRS's multi-protocol capabilities for logistics operations including real-time tracking, fleet management, warehouse operations, and supply chain analytics.

## Why OrbitRS for Logistics?

OrbitRS excels in logistics environments where:
- **Real-time tracking** requires sub-second updates across millions of packages
- **Geospatial queries** enable efficient driver assignment and route optimization
- **Multi-protocol access** allows IoT devices (Redis), WMS systems (PostgreSQL), and analytics (CQL) to share data
- **Time-series storage** handles high-frequency telemetry from fleet vehicles
- **Event sourcing** provides complete audit trails for compliance

## Scenarios

### 1. Core Logistics Schema - PostgreSQL
**File**: [`sql/01_schema_logistics.sql`](sql/01_schema_logistics.sql)

Relational schema for logistics operations:
- **Facilities**: Warehouses, distribution centers, and sort facilities
- **Shipments**: Origin/destination tracking with weight and distance

**Use Case**: WMS integration, order management, facility planning

```sql
-- Query shipments with delays
SELECT s.*, f1.name as origin, f2.name as destination
FROM shipments s
JOIN facilities f1 ON s.origin_id = f1.facility_id
JOIN facilities f2 ON s.destination_id = f2.facility_id
WHERE s.delay_hours > 0;
```

### 2. Real-time Tracking & Operations - Redis
**File**: [`redis/01_tracking.redis`](redis/01_tracking.redis)

High-performance real-time operations using Redis data structures:
- **Fleet Tracking**: Geospatial driver/vehicle locations with GEORADIUS queries
- **Package Status**: Comprehensive tracking hashes with event timelines
- **Route Management**: Ordered stop lists with route metadata
- **Delivery Streams**: Real-time event log with consumer groups
- **Pub/Sub Notifications**: Customer alerts, driver updates, zone broadcasts
- **Warehouse Ops**: Inventory cache, bin locations, pick queues, dock management
- **Order Fulfillment**: Priority queues for picking, packing, shipping stages
- **Carrier Rates**: Cached rate tables with zone pricing
- **Analytics**: Delivery metrics, driver performance, hourly heatmaps
- **Exception Handling**: Failed deliveries, invalid addresses, weather alerts
- **Full-Text Search**: Address lookup (OrbitRS extension)
- **Vector Search**: Route similarity for ETA prediction (OrbitRS extension)

**Use Case**: Driver apps, customer tracking portals, dispatch systems, warehouse operations

```redis
# Track driver location
GEOADD fleet:drivers:nyc -74.0060 40.7128 "driver:D-101"

# Find nearest driver to delivery address
GEORADIUS fleet:drivers:nyc -73.9857 40.7484 5 km WITHDIST ASC COUNT 3

# Stream delivery event
XADD stream:deliveries * event_type "STATUS_CHANGE" tracking_number "TRK-998877" to_status "OUT_FOR_DELIVERY"

# Real-time customer notification
PUBLISH delivery:customer:C-12345 "YOUR_PACKAGE|TRK-998877|10 stops away"
```

### 3. Shipment Events & IoT Time-Series - CQL (Cassandra)
**File**: [`cql/01_shipment_events.cql`](cql/01_shipment_events.cql)

Wide-column time-series storage for high-volume logistics data:
- **Shipment Events**: Complete tracking history with exception handling (90-day retention)
- **Vehicle Telemetry**: GPS, sensors, driver behavior at second-level granularity (7-day raw)
- **Warehouse Sensors**: Temperature, humidity, CO2 for cold chain compliance (30-day)
- **Delivery Metrics**: Per-delivery SLA tracking and customer ratings (1-year)
- **Route History**: Historical routes for ML training and optimization
- **Inventory Movements**: Full audit trail of all stock movements (1-year)
- **Carrier Statistics**: Pre-aggregated daily performance metrics
- **Geofence Events**: Vehicle entry/exit tracking for warehouses and customers
- **Route Embeddings**: Vector storage for ML route optimization (OrbitRS extension)

**Use Case**: Tracking history, fleet analytics, cold chain compliance, ML training

```cql
-- Query tracking history
SELECT event_time, event_type, facility_name, description
FROM shipment_events
WHERE tracking_number = 'TRK-998877'
  AND event_month = '2024-12';

-- Query vehicle telemetry
SELECT telemetry_time, latitude, longitude, speed_kmh
FROM vehicle_telemetry
WHERE vehicle_id = 'V-1001'
  AND telemetry_date = '2024-12-09';

-- Carrier performance
SELECT stat_date, on_time_rate, avg_rating, total_deliveries
FROM carrier_daily_stats
WHERE carrier = 'UPS'
LIMIT 30;
```

### 4. Waybills & Manifests - MongoDB
**File**: [`mongodb/01_shipments.js`](mongodb/01_shipments.js)

Flexible document storage for complex logistics documents:
- Waybills with itemized manifests
- Customs clearance documents
- Multi-leg shipment tracking
- HS codes and compliance data

**Use Case**: International shipping, customs brokerage, freight forwarding

### 5. ML Predictions - Python + PostgreSQL
**Files**: [`python/01_run_ml_examples.py`](python/01_run_ml_examples.py), [`sql/02_ml_examples.sql`](sql/02_ml_examples.sql)

Machine learning integration for logistics optimization:
- Delay prediction
- Route optimization
- Demand forecasting

**Use Case**: Predictive analytics, dynamic routing, capacity planning

## Multi-Protocol Integration Pattern

A typical logistics deployment uses multiple protocols for different workloads:

```
                    ┌─────────────────────────────────────────────────────────┐
                    │                      OrbitRS                            │
                    │                                                         │
  ┌─────────────┐   │   ┌────────────┐   ┌────────────┐   ┌────────────┐    │
  │    WMS      │◄──┼──►│ PostgreSQL │   │   Redis    │   │    CQL     │    │
  │   System    │   │   │   :5432    │   │   :6379    │   │   :9042    │    │
  └─────────────┘   │   └─────┬──────┘   └─────┬──────┘   └─────┬──────┘    │
                    │         │                │                │           │
  ┌─────────────┐   │         │                │                │           │
  │  Driver App │◄──┼─────────┼────────────────┘                │           │
  │  (Mobile)   │   │         │  (real-time tracking)           │           │
  └─────────────┘   │         │                                 │           │
                    │         │                                 │           │
  ┌─────────────┐   │         │    ┌────────────────────────────┘           │
  │ IoT Gateway │◄──┼─────────┼────┘  (fleet telemetry)                     │
  │  (Vehicles) │   │         │                                             │
  └─────────────┘   │         ▼                                             │
                    │   ┌──────────────────────────────────────────────┐    │
                    │   │           Unified Storage Layer              │    │
                    │   │  (RocksDB + Time-Series Optimization)       │    │
                    │   └──────────────────────────────────────────────┘    │
                    └─────────────────────────────────────────────────────────┘
```

## Data Flow Example

```
1. Customer places order → PostgreSQL (order record)
                        → Redis (fulfillment queue)

2. Warehouse picks order → Redis (inventory decrement)
                        → CQL (inventory movement event)

3. Package scanned       → Redis (status update, pub/sub)
                        → CQL (shipment event)

4. Driver picks up       → Redis (geospatial update)
                        → CQL (vehicle telemetry stream)

5. En route             → Redis (real-time location)
                        → Redis Streams (tracking updates)
                        → Pub/Sub (customer notifications)

6. Delivered            → CQL (delivery metrics)
                        → PostgreSQL (order completion)
                        → Redis (analytics counters)
```

## Performance Characteristics

| Operation | Protocol | Expected Latency |
|-----------|----------|-----------------|
| Package status lookup | Redis | < 1ms |
| Nearest driver query | Redis GEORADIUS | < 5ms |
| Tracking event write | CQL | < 10ms |
| Route optimization | Redis + PostgreSQL | < 100ms |
| Tracking history | CQL | < 50ms |
| Telemetry write (batch) | CQL | < 20ms |

## Volume Handling

| Data Type | Daily Volume | Retention |
|-----------|-------------|-----------|
| Package status updates | 10M+ | 30 days |
| Vehicle telemetry | 100M+ | 7 days (raw), 1 year (aggregated) |
| Tracking events | 50M+ | 90 days |
| Warehouse sensors | 10M+ | 30 days |

## Getting Started

1. Start OrbitRS with logistics configuration:
```bash
cargo run --bin orbit-server -- --config config/logistics.toml
```

2. Load the SQL schema:
```bash
psql -h localhost -p 5432 -f sql/01_schema_logistics.sql
```

3. Run Redis tracking examples:
```bash
redis-cli -p 6379 < redis/01_tracking.redis
```

4. Load CQL event schema:
```bash
cqlsh localhost 9042 -f cql/01_shipment_events.cql
```

5. Load MongoDB manifests:
```bash
mongosh --port 27017 < mongodb/01_shipments.js
```

## Related Documentation

- [PostgreSQL Protocol](../../docs/content/protocols/postgresql.md)
- [Redis Protocol](../../docs/content/protocols/redis.md)
- [CQL Protocol](../../docs/content/protocols/cql.md)
- [MongoDB Protocol](../../docs/content/protocols/mongodb.md)
- [Geospatial Queries](../../docs/content/features/geospatial.md)
- [Time-Series Data](../../docs/content/features/time-series.md)
