# Robotics Workflow: Navigation Mission

This workflow details the lifecycle of a robotic delivery mission.

## Flow Description

1. **Task Assignment (PostgreSQL -> Redis)**
   - WMS (Warehouse System) assigns `M-5501` to Robot `R-101`.
   - Update `robots` status to 'MISSION'.
   - Push command to Redis Queue `robot:cmd:R-101`.

2. **Execution (Redis Streaming)**
   - Robot polls Redis List, pops "GOTO" command.
   - Robot streams live telemetry to `robot:pose:R-101` (Redis) for fleet dashboard visibility.
   - Middleware checks battery levels; if < 20%, overrides task with "GOTO CHARGER".

3. **Completion & Archival (MongoDB)**
   - Upon task finish, Robot uploads mission summary log to MongoDB `mission_logs`.
   - Includes planned path vs actual path for efficiency analysis.

## Step-by-Step Code Example

### 1. Assign Task (Redis)
```bash
RPUSH robot:cmd:R-101 "MISSION_START id=M-5501 type=DELIVERY target=DockingBay_4"
```

### 2. Robot Reporting (Pseudo-Code)

```python
# Robot onboard script
def on_mission_complete(mission_data):
    # Log to Mongo
    mongo.db.mission_logs.insert_one(mission_data)
    
    # Update State
    redis.set(f"robot:status:{self.id}", "IDLE")
    
    # Notify Fleet Manager
    pg.execute("UPDATE robots SET status='IDLE' WHERE serial_number=%s", (self.id,))
```
