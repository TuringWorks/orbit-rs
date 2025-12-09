# Healthcare & Life Sciences

Comprehensive examples demonstrating OrbitRS's multi-protocol capabilities for healthcare applications including Electronic Health Records (EHR), Medical IoT telemetry, real-time patient monitoring, and clinical decision support.

## Why OrbitRS for Healthcare?

OrbitRS excels in healthcare environments where:
- **Multi-protocol access** allows different systems (EHR, monitoring, analytics) to use their native protocols
- **HIPAA-compliant data handling** with encryption at rest and audit logging
- **Real-time + historical data** combines streaming vitals with longitudinal patient records
- **Vector search** enables clinical decision support through similar patient cohort analysis

## Scenarios

### 1. Electronic Health Records (EHR) - PostgreSQL
**File**: [`sql/01_schema_ehr.sql`](sql/01_schema_ehr.sql)

Comprehensive relational schema for patient data management:
- **Patients**: Demographics, allergies, emergency contacts with HIPAA audit fields
- **Providers**: Physicians, NPs, PAs with specialty and NPI tracking
- **Appointments**: Scheduling with status workflow and encounter linking
- **Encounters**: Visit documentation with chief complaint and disposition
- **Diagnoses**: ICD-10 coded conditions linked to encounters
- **Prescriptions**: Medication management with NDC codes and refill tracking
- **Vital Signs**: Structured vitals with BMI auto-calculation
- **Lab Results**: LOINC-coded results with reference ranges
- **Audit Log**: HIPAA-compliant access tracking

**Use Case**: Primary EHR system, clinical documentation, billing integration

```sql
-- Query patient with recent encounters
SELECT p.first_name, p.last_name, e.encounter_date, e.chief_complaint
FROM patients p
JOIN encounters e ON p.patient_id = e.patient_id
WHERE p.mrn = 'MRN-001'
ORDER BY e.encounter_date DESC;
```

### 2. Real-Time Patient Vitals Monitoring - Redis
**File**: [`redis/01_vitals_stream.redis`](redis/01_vitals_stream.redis)

High-performance real-time monitoring using Redis data structures:
- **Hash**: Current patient vital snapshots for dashboard display
- **Streams**: Time-series vital signs from bedside monitors (every second)
- **Consumer Groups**: Parallel processing for nurses, alert processors, analytics
- **Pub/Sub**: Critical alert distribution (CODE_BLUE, vital thresholds)
- **Caching**: Patient demographics, active medications, pending labs
- **GeoSpatial**: Patient location tracking during transports
- **HyperLogLog**: ICU occupancy statistics
- **Full-Text Search**: Medication lookup (OrbitRS extension)
- **Vector Search**: Clinical note similarity (OrbitRS extension)

**Use Case**: ICU monitoring, nurse stations, real-time alerting, clinical dashboards

```redis
# Stream vitals from bedside monitor
XADD stream:vitals:mrn_888 * type heart_rate value 78 unit bpm device_id "monitor-icu4a-001"

# Nurse station reads new vitals
XREADGROUP GROUP nurses-station nurse-1 COUNT 5 STREAMS stream:vitals:mrn_888 >

# Critical alert broadcast
PUBLISH alert:critical "CODE_BLUE|Patient MRN-888|ICU-4A Bed-2|Cardiac Arrest"
```

### 3. Lab Results & Clinical Documents - MongoDB
**File**: [`mongodb/01_lab_results.js`](mongodb/01_lab_results.js)

Flexible document storage for complex clinical data:
- Pathology reports with nested observations
- Free-text clinical notes with metadata
- Complex lab panels with multiple components
- Reference ranges and interpretive comments

**Use Case**: Laboratory information systems, pathology, radiology reports

### 4. Medical Device Telemetry - CQL (Cassandra)
**File**: [`cql/01_device_telemetry.cql`](cql/01_device_telemetry.cql)

Wide-column time-series storage for high-frequency medical IoT:
- **CGM Readings**: Continuous glucose monitor data (every 5 minutes, 90-day retention)
- **Vital Signs Time-Series**: ICU monitors at 1-second intervals (7-day retention)
- **Infusion Pump Data**: Medication delivery tracking with alarm states
- **Wearable Devices**: Fitbit, Apple Watch, Garmin health data (1-year retention)
- **Medication Dispenser Events**: Compliance tracking with missed dose alerts
- **Lab Results History**: Longitudinal trending (HbA1c over time)
- **Daily Aggregates**: Pre-computed statistics for dashboards
- **Patient Embeddings**: Vector storage for similar patient analysis (OrbitRS extension)

**Use Case**: Medical device integration, remote patient monitoring, clinical research

```cql
-- Query CGM readings for today
SELECT reading_time, glucose_mgdl, trend_arrow
FROM cgm_readings
WHERE device_id = 11111111-1111-1111-1111-111111111111
  AND reading_date = '2024-12-09';

-- HbA1c trend over time
SELECT result_date, result_value
FROM lab_results_history
WHERE patient_mrn = 'MRN-888'
  AND test_code = '4548-4'
ORDER BY result_date DESC;
```

## Multi-Protocol Integration Pattern

A typical healthcare deployment uses multiple protocols simultaneously:

```
                    ┌─────────────────────────────────────────────────────────┐
                    │                      OrbitRS                            │
                    │                                                         │
  ┌────────────-─┐  │   ┌────────────┐   ┌────────────┐   ┌────────────┐      │
  │ EHR System   │◄─┼──►│ PostgreSQL │   │   Redis    │   │    CQL     │      │
  │ (Epic/Cerner)│  │   │   :5432    │   │   :6379    │   │   :9042    │      │
  └────────────-─┘  │   └─────┬──────┘   └─────┬──────┘   └─────┬──────┘      │
                    │         │                │                │             │
  ┌─────────────┐   │         │                │                │             │
  │  Monitoring │◄──┼─────────┼────────────────┘                │             │
  │  Dashboard  │   │         │                                 │             │
  └─────────────┘   │         │                                 │             │
                    │         │                                 │             │
  ┌─────────────┐   │         │    ┌────────────────────────────┘             │
  │ IoT Gateway │◄──┼─────────┼────┘                                          │
  │ (Devices)   │   │         │                                               │
  └─────────────┘   │         ▼                                               │
                    │   ┌──────────────────────────────────────────────┐      │
                    │   │           Unified Storage Layer              │      │
                    │   │  (RocksDB + Tiered Storage + Replication)    │      │
                    │   └──────────────────────────────────────────────┘      │
                    └─────────────────────────────────────────────────────────┘
```

## Compliance Considerations

These examples include patterns for:
- **HIPAA**: Audit logging, PHI encryption, access controls
- **HL7 FHIR**: Compatible data models for interoperability
- **21 CFR Part 11**: Electronic records and signatures
- **HITECH**: Breach notification and meaningful use

## Getting Started

1. Start OrbitRS with healthcare configuration:
```bash
cargo run --bin orbit-server -- --config config/healthcare.toml
```

2. Load the SQL schema:
```bash
psql -h localhost -p 5432 -f sql/01_schema_ehr.sql
```

3. Run Redis vitals streaming example:
```bash
redis-cli -p 6379 < redis/01_vitals_stream.redis
```

4. Load CQL device telemetry schema:
```bash
cqlsh localhost 9042 -f cql/01_device_telemetry.cql
```

## Related Documentation

- [PostgreSQL Protocol](../../docs/content/protocols/postgresql.md)
- [Redis Protocol](../../docs/content/protocols/redis.md)
- [CQL Protocol](../../docs/content/protocols/cql.md)
- [Vector Search](../../docs/content/features/vector-search.md)
- [Full-Text Search](../../docs/content/features/full-text-search.md)
