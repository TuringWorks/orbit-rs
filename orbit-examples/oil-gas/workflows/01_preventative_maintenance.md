# Oil & Gas Workflow: Preventative Maintenance

This workflow demonstrates how IoT data triggers maintenance actions to prevent equipment failure.

## Flow Description

1. **IoT Monitoring (CQL)**
   - Sensors on "Drill Pump A" send vibration data to Cassandra.
   - Example: Vibration spikes from 2mm/s to 8mm/s.

2. **Anomaly Detection (Streaming/ML)**
   - Analysis service detects trend breach.
   - Trigger alert: "Potential bearing failure imminent".

3. **Work Order Generation (PostgreSQL)**
   - System automatically queries Equipment Registry to find site and model.
   - Inserts record into `maintenance_logs` (scheduled).
   - Updates `equipment` status to 'NEEDS_SERVICE'.

4. **Resource Allocation (Graph - Optional)**
   - Query Graph to find nearest available technician or spare part in warehouse network.

## Step-by-Step Code Example

### 1. Detect Anomaly (Pseudo-code)

```python
# Read last hour of data from CQL
rows = session.execute(
    "SELECT vibration FROM sensor_readings WHERE sensor_id='DP-1001' AND datebucket=%s ORDER BY ts DESC LIMIT 60", 
    (today,)
)
avg_vib = calculate_avg(rows)

if avg_vib > 7.0:
    create_work_order("DP-1001", "High Vibration - Bearing Check")
```

### 2. Create Work Order (SQL)

```sql
INSERT INTO maintenance_logs (equipment_id, service_date, notes)
SELECT equipment_id, CURRENT_DATE + 1, 'Auto-generated: High Vibration Alert'
FROM equipment WHERE serial_number = 'DP-1001';
```
