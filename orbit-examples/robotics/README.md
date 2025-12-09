# Robotics Industry Examples 🤖

This directory contains end-to-end examples for a **Robotics Fleet Management** system using Orbit-RS.

Key scenarios include warehouse robots (AMRs), telemetry streaming, and mission logging.

## 🏗 Architecture

| Component | Protocol | Port | Usage |
|-----------|----------|------|-------|
| **Fleet** | PostgreSQL | 5432 | Robot registry, firmware versions, battery life stats (ACID) |
| **Telemetry** | Redis | 6379 | Real-time LiDAR stream, localized pose (x,y,theta), battery level |
| **Logs** | MongoDB | 27017 | Mission logs, error dumps, SLAM maps (Binary/Grid) |

## 🚀 Running the Examples

### 1. Fleet Management (PostgreSQL)
Register robots and manage maintenance schedules.

```bash
psql -h localhost -p 5432 -U orbit -d postgres -f sql/01_fleet_registry.sql
```

### 2. Live Telemetry (Redis)
Stream real-time pose and sensor data.

```bash
redis-cli -h localhost -p 6379 < redis/01_telemetry_stream.redis
```

### 3. Mission Logs (MongoDB)
Store complex mission execution logs.

```bash
mongosh mongodb://localhost:27017 --file mongodb/01_mission_logs.js
```

## 📚 Workflows

- **[Autonomous Navigation Mission](workflows/01_navigation_mission.md)**: From task assignment to execution log.
