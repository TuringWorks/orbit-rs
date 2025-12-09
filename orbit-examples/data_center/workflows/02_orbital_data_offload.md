# Workflow: Orbital Data Offload

This workflow manages the complexity of offloading data from an Orbital Data Center (satellite) to a Ground Station.

## Overview

1.  **Pass Prediction** (SQL/Python)
    - Calculate when `SAT-101` will be within range of `GS-London`.
    - Update `sites` location details if needed.

2.  **Connection Establishment** (Redis)
    - As signal is acquired, set `uplink:sat-101` key.
    - If `uplink` is lost, pause transfer.

3.  **Data Dump** (CQL)
    - Bulk read of `sensor_readings` and `orbital_health` from the satellite's local storage (simulated) to the Ground Station's OrbitDB cluster.

4.  **Routing via ISL** (Cypher)
    - If Ground Station is not visible, route traffic to a neighbor satellite that IS visible.
    - `MATCH p=shortestPath((s:Satellite)-[:IS_LINKED*]->(g:GroundStation)) RETURN p`

## Step-by-Step Execution

### Step 1: Check Connectivity
Redis check.
```bash
GET uplink:sat-101
# Returns "CONNECTED"
```

### Step 2: Sync Data
Transfer buffered logs.
```sql
-- (Logical op) COPY local_readings TO remote_ground_station
```

### Step 3: Handle Loss of Signal (LOS)
When `uplink` key expires in Redis, switch to Inter-Satellite Link (ISL).
```cypher
MATCH (source:Satellite {id: 'SAT-101'})-[:IS_LINKED]->(neighbor:Satellite)
WHERE exists((neighbor)-[:UPLINK]->(:GroundStation))
RETURN neighbor.id
```
