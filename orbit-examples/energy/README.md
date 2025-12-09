# Energy & Utilities

Examples for Smart Grids, Renewables, and Asset Management.

## Scenarios

### 1. Asset Registry (SQL)
**File**: [`sql/01_schema_energy.sql`](sql/01_schema_energy.sql)
- Registry of substations, transformers, and meters.

### 2. Smart Meter Data (CQL)
**File**: [`cql/01_meter_readings.cql`](cql/01_meter_readings.cql)
- Time-series storage for AMI (Advanced Metering Infrastructure) kilowatt-hour readings.

### 3. Grid State & Balancing (Redis)
**File**: [`redis/01_grid_state.redis`](redis/01_grid_state.redis)
- Real-time load balancing, demand response signals, and solar array telemetry cache.
