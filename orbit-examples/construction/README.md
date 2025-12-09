# Construction Industry Examples 🏗️

This directory contains end-to-end examples for a **Construction Management Platform** using Orbit-RS.

Key scenarios include project management, blueprint versioning, and job site IoT monitoring.

## 🏗 Architecture

| Component | Protocol | Port | Usage |
|-----------|----------|------|-------|
| **Projects** | PostgreSQL | 5432 | Project metadata, budgets, contractors, schedules (ACID) |
| **Documents** | MongoDB | 27017 | Blueprints (BIM metadata), daily logs, inspection reports |
| **Site IoT** | Redis | 6379 | Real-time sensor data (crane loads, air quality, noise) |

## 🚀 Running the Examples

### 1. Project Management (PostgreSQL)
Create the core schema for projects and budget tracking.

```bash
psql -h localhost -p 5432 -U orbit -d postgres -f sql/01_project_schema.sql
```

### 2. Blueprint & Site Logs (MongoDB)
Store flexible document data for reports and plans.

```bash
mongosh mongodb://localhost:27017 --file mongodb/01_site_docs.js
```

### 3. Site Sensor Monitoring (Redis)
Track real-time safety metrics and equipment status.

```bash
redis-cli -h localhost -p 6379 < redis/01_site_sensors.redis
```

## 📚 Workflows

- **[Site Safety & Incident Reporting](workflows/01_safety_incident.md)**: From IoT trigger to incident report.
