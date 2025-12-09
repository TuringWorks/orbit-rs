# Manufacturing & Industrial IoT

Examples for Smart Factories, Supply Chain, and Digital Twins.

## Scenarios

### 1. Product Schema (SQL)
**File**: [`sql/01_schema_products.sql`](sql/01_schema_products.sql)
- Relational schema for product lines and inventory.

### 2. Factory IoT (CQL)
**File**: [`cql/01_factory_iot.cql`](cql/01_factory_iot.cql)
- High-volume ingestion of sensor data (vibration, temperature) from the assembly line.

### 3. Digital Twin / Specs (MongoDB)
**File**: [`mongodb/01_product_specs.js`](mongodb/01_product_specs.js)
- Complex JSON documents for Product Specifications, Bill of Materials (BOM), and QC checks.

### 4. Line Operations (Redis)
**File**: [`redis/01_operations.redis`](redis/01_operations.redis)
- Real-time counters and status flags for production machines.

### 5. Humanoid Robots (End-to-End)
**Directory**: [`humanoid_robots/`](humanoid_robots/)
- **Genealogy**: Parts traceability (SQL).
- **Calibration**: High-freq sensor burn-in (CQL).
- **Birth Record**: As-built QA document (Mongo).
- **AI Training**: Vector search for failure modes (OrbitQL).
