# Automotive Industry Examples 🚗

This directory contains end-to-end examples for a **Connected Vehicle Platform** using Orbit-RS.

Key scenarios include vehicle telemetry ingestion, fleet management, and supply chain tracking.

## 🏗 Architecture

| Component | Protocol | Port | Usage |
|-----------|----------|------|-------|
| **Registry** | PostgreSQL | 5432 | Vehicle registration, owner data, service history (ACID) |
| **Telemetry** | CQL (Cassandra) | 9042 | High-velocity sensor data (speed, fuel, diagnostics) |
| **Fleet** | Redis | 6379 | Real-time vehicle status, geofencing alerts (Pub/Sub) |
| **Supply Chain** | AQL (ArangoDB) | 8529 | Parts dependency graph (Recall management) |

## 🚀 Running the Examples

### 1. Vehicle Registry (PostgreSQL)
Create the core schema for vehicles and owners.

```bash
psql -h localhost -p 5432 -U orbit -d postgres -f sql/01_registry_schema.sql
```

### 2. Telemetry Ingestion (CQL)
Set up tables for storing massive amounts of sensor data.

```bash
cqlsh localhost 9042 -f cql/01_telemetry_schema.cql
```

### 3. Fleet Status (Redis)
Manage real-time vehicle states.

```bash
redis-cli -h localhost -p 6379 < redis/01_fleet_status.redis
```

### 4. Parts Supply Chain (AQL)
Track part dependencies to manage recalls.

```bash
arangosh --server.endpoint tcp://localhost:8529 --javascript.execute aql/01_parts_graph.aql
```

## 📚 Workflows

- **[Telemetry Pipeline & Alerts](workflows/01_telemetry_pipeline.md)**: From sensor reading to dashboard visualization.
