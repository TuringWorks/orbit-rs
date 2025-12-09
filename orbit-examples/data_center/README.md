# Data Center & Orbital Operations

This directory contains Orbit-RS examples for managing Data Center infrastructure, covering both traditional terrestrial facilities and next-generation **Orbital Data Centers** (server racks deployed in space).

## Overview

Modern data centers require millisecond-level telemetry, robust asset tracking, and complex network topology management. Orbital deployments add constraints around power, thermal radiation, and intermittent connectivity.

### Scenarios Covered

1.  **Physical Asset Management** (SQL)
    - Tracking Servers, PDUs, and Racks.
    - Orbital specifics: Satellite buses, solar arrays, radiators.

2.  **Environmental Telemetry** (CQL)
    - High-volume sensor ingest: Temperature, Power Draw, Battery Cycles.
    - Radiation monitoring for orbital hardware.

3.  **Real-Time Alerts & Ops** (Redis)
    - Active alarm dashboard.
    - Technician dispatch queues.
    - Ground station uplink/downlink status.

4.  **Network Topology** (Cypher)
    - Modeling physical cabling (Tor -> Spine -> Core).
    - Inter-Satellite Links (ISL) using laser communications.

## Directory Structure

- `sql/`: Asset inventory and maintenance logs.
- `cql/`: Time-series schema for environmental sensors.
- `redis/`: Operations dashboard and alerting.
- `cypher/`: Network graph and routing topology.
- `workflows/`: Standard Operating Procedures (SOPs).

## Running the Examples

Each subdirectory contains specific instructions and files that can be run against an Orbit-RS instance.

```bash
# Example: Run SQL schema
orbit-client run -f data_center/sql/01_asset_management.sql
```
