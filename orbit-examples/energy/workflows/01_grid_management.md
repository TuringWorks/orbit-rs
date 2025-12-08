# Energy Workflow: Smart Grid Load Balancing

## Overview
Real-time smart grid management with ML load forecasting and demand response.

## Workflow Steps

### 1. Meter Data Ingestion (Cassandra)
```cql
INSERT INTO meter_readings (meter_id, timestamp, kwh, voltage, current)
VALUES ('meter-12345', now(), 45.2, 120.1, 12.5);
```
**Scale**: 10M+ meters, 1B+ readings/day, <5ms latency

### 2. Real-Time Grid Status (Redis)
```redis
HSET grid:status:zone-east load "8500MW"
HSET grid:status:zone-east capacity "10000MW"
HSET grid:status:zone-east utilization "0.85"
```

### 3. ML Load Forecasting (Redis)
```redis
GET ml:forecast:load:zone-east:next-hour
# Returns: Predicted load (LSTM, 94% accuracy)
```

### 4. Outage Detection (PostgreSQL + ML)
```sql
SELECT grid_section_id, status, last_reading
FROM grid_infrastructure
WHERE status = 'OFFLINE' OR last_reading < NOW() - INTERVAL '5 minutes';
```

```redis
GET ml:outage:prediction:section-456
# Returns: Outage probability, affected customers
```

### 5. Demand Response (Redis Pub/Sub)
```redis
PUBLISH demand:response:zone-east '{
  "action": "REDUCE_LOAD",
  "target_mw": 500,
  "duration": "1h"
}'
```

**Performance**: <10ms grid monitoring, 94% load forecast accuracy
