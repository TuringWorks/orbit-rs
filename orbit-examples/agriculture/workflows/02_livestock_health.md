# Agriculture Workflow: Livestock Grazing & Health

This workflow integrates IoT tracking with health records to manage efficient grazing and rapid disease response.

## Flow Description

1. **Tracking (Redis Geo)**
   - Smart collars send GPS coordinates every 5 minutes.
   - App checks geofence (Grazing Zone A). If animal leaves -> Alert.

2. **Health Monitoring (Redis Streams)**
   - Collar sends biometrics: Temp, Heart Rate, Activity.
   - Sudden drop in activity + high temp = Potential Illness.

3. **Vet Intervention (PostgreSQL)**
   - Farm manager schedules vet visit.
   - Vet diagnoses infection, prescribes antibiotics.
   - Record stored in SQL `vet_records`.

4. **Quarantine & Traceability (Cypher)**
   - If contagious (e.g., Foot & Mouth), query Graph to find:
     - All animals "penned with" the sick animal.
     - Lineage check for hereditary issues.

## Step-by-Step Code Example

### 1. Identify Sick Animal (Pseudo-code)

```python
# Check vitals in Redis
vitals = redis.hgetall("animal:vitals:TAG-002")
if vitals['temp_c'] > 40.0:
    trigger_alert("FEVER_DETECTED", "TAG-002")
```

### 2. Isolate & Log (SQL)

```sql
UPDATE animals SET status = 'SICK', location = 'Quarantine Pen 1' 
WHERE tag_id = 'TAG-002';

INSERT INTO vet_records (animal_id, visit_date, notes)
VALUES ((SELECT animal_id FROM animals WHERE tag_id='TAG-002'), CURRENT_DATE, 'High fever detected via IoT');
```
