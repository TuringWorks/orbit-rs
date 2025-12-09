# Automotive Workflow: Telemetry Pipeline & Alerts

This workflow processes streams of sensor data from connected vehicles.

## Flow Description

1. **Ingestion (CQL)**
   - Vehicle sends telemetry packet (Speed, RPM, Location).
   - High-throughput write to Cassandra/ScyllaDB (`telemetry_raw` table).

2. **Real-Time State (Redis)**
   - Update "Latest Known State" in Redis Hash (`vehicle:status:VIN`).
   - Push location to Redis Geo (`fleet:locations`).

3. **Anomaly Detection (Orbit ML / Python)**
   - Analyze stream for anomalies (e.g., Engine Temp > 110C).
   - If anomaly detected:
     - Publish to Redis Channel `alerts:critical`.
     - Log diagnostic code to CQL `diagnostic_codes`.

4. **Visualization (SQL)**
   - Dashboard queries PostgreSQL for Owner info to send SMS/Email alerts.

## Step-by-Step Code Example

### 1. Ingestion Code (Pseudo-Python)

```python
def ingest_telemetry(packet):
    # 1. Write History (CQL) - Fire and Forget
    session.execute_async(
        "INSERT INTO telemetry_raw (vehicle_id, datebucket, ts, speed) VALUES (%s, %s, %s, %s)",
        (packet.vin, today, now, packet.speed)
    )

    # 2. Update Live State (Redis)
    r.hset(f"vehicle:status:{packet.vin}", mapping={
        "speed": packet.speed, 
        "last_seen": now
    })
    r.geoadd("fleet:locations", (packet.lon, packet.lat, packet.vin))
    
    # 3. Check for Alerts
    if packet.temp > 110:
        trigger_alert(packet.vin, "Overheating")

def trigger_alert(vin, message):
    # Get Owner (Postgres)
    cur.execute("SELECT phone FROM owners o JOIN vehicles v ON v.owner_id = o.owner_id WHERE v.vin = %s", (vin,))
    phone = cur.fetchone()[0]
    send_sms(phone, message)
```
