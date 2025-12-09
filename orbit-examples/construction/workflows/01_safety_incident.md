# Construction Workflow: Site Safety & Incident Reporting

This workflow manages safety incidents starting from real-time detection to compliance reporting.

## Flow Description

1. **Detection (Redis)**
   - IoT sensor (e.g., noise, crane load, or wearable fall detector) sends data to Redis Stream/PubSub.
   - Example: Fall detected at Zone B.

2. **Immediate Alert (Redis -> Worker App)**
   - Redis Pub/Sub channel `safety:alerts` triggers push notification to Site Safety Officer.

3. **Incident Creation (PostgreSQL)**
   - Safety Officer confirms incident.
   - Creates record in SQL `incidents` table (linked to Project).

4. **Report & Evidence (MongoDB)**
   - Officer uploads photos and writes detailed statement.
   - Stored in MongoDB `incident_reports` (flexible schema for various incident types).

## Step-by-Step Code Example

### 1. Alert Trigger (Redis)
```bash
PUBLISH safety:alerts "FALL_DETECTED: Worker_ID_442 at Zone B"
```

### 2. logging (Pseudo-Code)

```javascript
// Worker service consumes alert
redis.subscribe("safety:alerts", (message) => {
    // 1. Create SQL Record
    const incident_id = await sql.query(
        "INSERT INTO incidents (project_id, type, status) VALUES ($1, 'FALL', 'OPEN') RETURNING id", 
        [1]
    );

    // 2. Create Mongo Report Doc
    await mongo.collection("incident_reports").insertOne({
        incident_sql_id: incident_id,
        timestamp: new Date(),
        details: message,
        witnesses: [],
        media_urls: []
    });
});
```
