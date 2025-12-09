# Oil & Gas Exploration and Refinement Examples 🛢️

This directory contains end-to-end examples for an **Upstream & Downstream Energy Platform** using Orbit-RS.

Key scenarios include seismic exploration data, refinery process monitoring, and pipeline network management.

## 🏗 Architecture

| Component | Protocol | Port | Usage |
|-----------|----------|------|-------|
| **Assets** | PostgreSQL | 5432 | Rigs, wells, equipment inventory, maintenance schedules (ACID) |
| **Refinery IoT** | CQL (Cassandra) | 9042 | High-frequency sensor readings (Pressure, Temp, Flow) |
| **Pipelines** | Cypher (Bolt) | 7687 | Network topology (Pipelines, Valves, Stations) for routing |
| **Exploration** | MongoDB | 27017 | Seismic survey logs, geological core samples (Flexible) |

## 🚀 Running the Examples

### 1. Asset Management (PostgreSQL)
Core schema for wells and drilling rigs.

```bash
psql -h localhost -p 5432 -U orbit -d postgres -f sql/01_assets.sql
```

### 2. Refinery Sensor Data (CQL)
Ingest massive streams of sensor data from cracking units.

```bash
cqlsh localhost 9042 -f cql/01_refinery_iot.cql
```

### 3. Pipeline Network (Cypher)
Analyze flow paths and valve dependencies.

```bash
cypher-shell -a bolt://localhost:7687 -u orbit -p orbit -f cypher/01_pipeline_graph.cypher
```

### 4. Seismic Data (MongoDB)
Store complex geological survey data.

```bash
mongosh mongodb://localhost:27017 --file mongodb/01_seismic_data.js
```

## 📚 Workflows

- **[Predictive Maintenance](workflows/01_preventative_maintenance.md)**: Using IoT trends to schedule maintenance before failure.
