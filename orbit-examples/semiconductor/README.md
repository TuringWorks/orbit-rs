# Semiconductor Manufacturing Examples 💾

This directory contains end-to-end examples for a **Semiconductor Fab** using Orbit-RS.

Key scenarios include wafer lot tracking, process control (SPC) data, and yield lineage analysis.

## 🏗 Architecture

| Component | Protocol | Port | Usage |
|-----------|----------|------|-------|
| **MES** | PostgreSQL | 5432 | Lot tracking, recipes, equipment status (ACID) |
| **SPC/FDC** | CQL (Cassandra) | 9042 | Sensor traces, endpoint detection, Fault Detection & Classification |
| **Yield** | AQL (ArangoDB) | 8529 | Lineage graph (Wafer -> Die -> Bin), Root cause analysis |

## 🚀 Running the Examples

### 1. MES - Lot Tracking (PostgreSQL)
Track wafer lots through 500+ process steps.

```bash
psql -h localhost -p 5432 -U orbit -d postgres -f sql/01_mes_tracking.sql
```

### 2. Sensor Data / SPC (CQL)
Store massive sensor traces for process steps.

```bash
cqlsh localhost 9042 -f cql/01_process_sensors.cql
```

### 3. Yield Lineage (AQL)
Trace yield loss back to specific tools or chambers.

```bash
arangosh --server.endpoint tcp://localhost:8529 --javascript.execute aql/01_yield_lineage.aql
```

## 📚 Workflows

- **[Fault Detection & Interdiction](workflows/01_fdc_interdiction.md)**: Halting a tool immediately upon sensor drift.
