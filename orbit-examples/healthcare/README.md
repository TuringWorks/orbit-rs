# Healthcare Industry Examples

This directory contains examples demonstrating Orbit-RS in the Healthcare & Life Sciences sector, covering Electronic Health Records (EHR), FHIR interoperability, IoMT, and Medical Knowledge Graphs.

## Scenarios

### 1. Electronic Health Records (EHR) (SQL)
- **File**: `sql/02_ehr_core.sql`
- **Description**: Relational schema for managing core patient data, encounters, and providers.
- **Features**: HIPAA-compliant audit trails, strict typing for demographics.

### 2. Clinical Notes & FHIR (MongoDB)
- **File**: `mongodb/02_fhir_clinical_notes.js`
- **Description**: Storing unstructured clinical data using HL7 FHIR standard resource formats.
- **Features**: nested `DocumentReference` and `CarePlan` documents.

### 3. IoMT Vital Signs (Redis)
- **File**: `redis/02_iomt_vitals.redis`
- **Description**: Real-time ingestion of high-velocity data from medical devices.
- **Features**: Streams for raw data, TimeSeries for historical trending, and sliding window alerts.

### 4. Drug Interactions Graph (Cypher)
- **File**: `cypher/02_drug_interactions_graph.cypher`
- **Description**: Knowledge graph for checking prescriptions against known drug-drug interactions and patient allergies.
- **Features**: Traversal queries for clinical decision support (CDS).

## Workflows

### 02_telehealth_visit
- **File**: `workflows/02_telehealth_visit.md`
- **Description**: End-to-end flow of a Telehealth consultation, integrating real-time video context, EHR lookup, and new prescriptions.
