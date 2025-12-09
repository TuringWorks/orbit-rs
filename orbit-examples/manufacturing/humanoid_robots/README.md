# Humanoid Robot Manufacturing

End-to-end examples for the production, calibration, and lifecycle management of **Humanoid Robots**.

## Scenarios

### 1. Parts Genealogy (SQL)
**File**: [`sql/01_parts_genealogy.sql`](sql/01_parts_genealogy.sql)
- Traceability of serialized components (actuators, GPUs) from supplier to specific robot joint.

### 2. Sensor Calibration (CQL)
**File**: [`cql/02_sensor_calibration.cql`](cql/02_sensor_calibration.cql)
- Ingesting terabytes of high-frequency (1kHz+) sensor data during joint burn-in and calibration tests.

### 3. Digital Birth Record (MongoDB)
**File**: [`mongodb/03_digital_birth_record.js`](mongodb/03_digital_birth_record.js)
- The "As-Built" document containing final QA scores, factory configuration snapshot, and shipping manifests.

### 4. AI Training Similarity (OrbitQL)
**File**: [`orbitql/04_training_episodes.orbitql`](orbitql/04_training_episodes.orbitql)
- Using Vector Search (`ANN`) to find similar training episodes or failure modes to improve walking policies.
