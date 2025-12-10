# Workflow: Telehealth Patient Visit

## Overview
A modern patient encounter involving real-time data, structured records, and unstructured clinical notes.

## Workflow Steps

### 1. Patient Check-In (Redis)
**Actor**: Patient App  
**Action**: Patient comes online for the appointment.
-   **System**: Updates Presence and establishes WebSocket for video.
-   **Data**: `HSET presence:pat-123 status "ONLINE" video_url "..."`

### 2. Provider Context Load (SQL + MongoDB)
**Actor**: Doctor's Dashboard  
**Action**: Fetches patient history.
-   **Query 1 (SQL)**: `SELECT * FROM encounters WHERE patient_id = ...` (Recent visits)
-   **Query 2 (MongoDB)**: `db.clinical_notes.find(...)` (Read past progress notes)

### 3. Vitals Monitoring (Redis Stream)
**System**: Wearable Device Integration  
**Action**: As the call starts, the system monitors live vitals to display to the doctor.
-   **Data**: Stream `vitals:pat-123`.
-   **Alert**: If O2 drops, UI flashes red.

### 4. Prescribing Medication (Cypher)
**Actor**: Doctor  
**Action**: Prescribes a new medication during the call.
-   **System**: Runs a safety check against the graph.
    ```cypher
    MATCH (p)-[:TAKES]->(existing), (new)-[:INTERACTS_WITH]->(existing) RETURN ...
    ```
-   **Decision**: If Warning, Doctor confirms or changes drug.

### 5. Documentation (MongoDB)
**Actor**: Doctor  
**Action**: Writes the "SOAP" note (Subjective, Objective, Assessment, Plan).
-   **Data**: Saved as FHIR `DocumentReference` in MongoDB.

### 6. Encounter Close (SQL)
**Action**: The visit ends.
-   **Data**: Update `encounters` table. `UPDATE encounters SET status='COMPLETED', end_time=NOW() ...`
-   **Audit**: Log the completion in `access_logs`.
