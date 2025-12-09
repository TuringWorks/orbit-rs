# Workflow: Predictive Maintenance

This workflow illustrates how high-frequency sensor data triggers real-time alerts and eventual physical maintenance.

## Overview

1.  **Telemetry Ingest** (CQL)
    - Thousands of temperature sensors stream data into Cassandra.
    - `sensor_readings` table absorbs the write load.

2.  **Anomaly Detection** (Application Layer)
    - An analytic job queries recent CQL data.
    - If `temperature > 45C` for > 5 mins, flag as critical.

3.  **Alerting** (Redis)
    - Publish alert to `alarms:site:us-east-1` channel.
    - Create a dispatch ticket in `dispatch:queue`.

4.  **Impact Analysis** (Cypher)
    - Query the graph to see which services are running on the affected rack.
    - `MATCH (r:Rack {id: 'rack-055'})<-[:HOSTED_ON]-(svc:Service) RETURN svc`

5.  **Resolution** (SQL)
    - Technician replaces the fan.
    - Log entry created in `maintenance_logs` (SQL).

## Step-by-Step Execution

### Step 1: Simulate Overheat
Insert high temp reading into CQL.
```sql
INSERT INTO sensor_readings (site_id, asset_id, sensor_type, value) VALUES (..., 'rack-055', 'temperature', 48.0);
```

### Step 2: Trigger Alert
Redis publish command.
```redis
PUBLISH alarms:site:us-east-1 "CRITICAL: Rack-055 Overheat"
```

### Step 3: Find Neighbors
Cypher query to see if heat might affect adjacent racks.
```cypher
MATCH (r1:Rack {asset_tag: 'rack-055'})-[:NEXT_TO]-(r2:Rack) RETURN r2
```
