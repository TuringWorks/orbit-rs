# Healthcare Workflow: Patient Admission with ML Diagnosis Support

## Overview

End-to-end workflow for patient admission, diagnosis, and treatment planning with ML-powered clinical decision support.

## Workflow Steps

### 1. Patient Check-In
**Protocol**: PostgreSQL  
**Action**: Verify patient identity and insurance

```sql
-- Look up patient by MRN
SELECT patient_id, first_name, last_name, date_of_birth,
       insurance_provider, readmission_risk_score
FROM patients
WHERE mrn = 'MRN-12345' AND status = 'ACTIVE';
```

### 2. Create Appointment
**Protocol**: PostgreSQL  
**Action**: Schedule appointment with provider

```sql
INSERT INTO appointments (
  appointment_id, patient_id, provider_id,
  appointment_type, scheduled_start, scheduled_end,
  chief_complaint, status
) VALUES (
  uuid_generate_v4(), 'patient-uuid', 'provider-uuid',
  'OFFICE_VISIT', '2024-12-07 10:00:00', '2024-12-07 10:30:00',
  'Chest pain and shortness of breath', 'CHECKED_IN'
);
```

### 3. Record Vital Signs
**Protocol**: PostgreSQL + Redis  
**Action**: Capture and stream real-time vitals

```sql
-- Store vital signs
INSERT INTO vital_signs (
  vital_id, patient_id, appointment_id,
  temperature, blood_pressure_systolic, blood_pressure_diastolic,
  heart_rate, oxygen_saturation, measured_at
) VALUES (
  uuid_generate_v4(), 'patient-uuid', 'appt-uuid',
  98.6, 140, 90, 85, 97.5, CURRENT_TIMESTAMP
);
```

```redis
# Real-time vitals monitoring (ICU/Emergency)
HSET vitals:patient-12345 temperature "98.6"
HSET vitals:patient-12345 bp_systolic "140"
HSET vitals:patient-12345 bp_diastolic "90"
HSET vitals:patient-12345 heart_rate "85"
HSET vitals:patient-12345 spo2 "97.5"
EXPIRE vitals:patient-12345 3600

# Alert if abnormal
IF bp_systolic > 140 THEN
  LPUSH alerts:critical "Patient MRN-12345: High BP 140/90"
```

### 4. ML Diagnosis Support
**Protocol**: Redis (ML Cache)  
**Action**: Get ML-powered diagnosis suggestions

```redis
# Get ML diagnosis prediction based on symptoms
GET ml:diagnosis:symptoms:chest_pain_sob

# Response:
{
  "predictions": [
    {"condition": "Coronary Artery Disease", "probability": 0.72, "icd10": "I25.10"},
    {"condition": "Anxiety Disorder", "probability": 0.18, "icd10": "F41.9"},
    {"condition": "GERD", "probability": 0.10, "icd10": "K21.9"}
  ],
  "confidence": 0.89,
  "model": "diagnosis_rf_v2"
}
```

### 5. Order Lab Tests
**Protocol**: PostgreSQL  
**Action**: Order diagnostic tests

```sql
INSERT INTO lab_results (
  lab_result_id, patient_id, provider_id,
  test_name, loinc_code, status, ordered_date
) VALUES
  (uuid_generate_v4(), 'patient-uuid', 'provider-uuid',
   'Troponin I', '10839-9', 'PENDING', CURRENT_TIMESTAMP),
  (uuid_generate_v4(), 'patient-uuid', 'provider-uuid',
   'ECG', '11524-6', 'PENDING', CURRENT_TIMESTAMP);
```

### 6. Retrieve Medical History
**Protocol**: PostgreSQL + Neo4j  
**Action**: Get patient history and related conditions

```sql
-- Get active diagnoses
SELECT icd10_code, description, diagnosed_date, status
FROM diagnoses
WHERE patient_id = 'patient-uuid'
  AND status IN ('ACTIVE', 'CHRONIC')
ORDER BY diagnosed_date DESC;

-- Get current medications
SELECT medication_name, dosage, frequency, prescribed_date
FROM prescriptions
WHERE patient_id = 'patient-uuid'
  AND status = 'ACTIVE';
```

```cypher
// Find related conditions and comorbidities
MATCH (p:Patient {id: 'patient-uuid'})-[:HAS_CONDITION]->(c:Condition)
MATCH (c)-[:RELATED_TO]->(related:Condition)
RETURN c.name, related.name, related.severity
ORDER BY related.severity DESC;
```

### 7. Drug Interaction Check
**Protocol**: Neo4j  
**Action**: Check for drug interactions before prescribing

```cypher
// Check interactions with current medications
MATCH (p:Patient {id: 'patient-uuid'})-[:TAKES]->(current:Drug)
MATCH (new:Drug {name: 'Aspirin'})
MATCH (current)-[i:INTERACTS_WITH]-(new)
RETURN current.name, new.name, i.severity, i.description;
```

### 8. Create Diagnosis
**Protocol**: PostgreSQL  
**Action**: Record diagnosis with ML confidence

```sql
INSERT INTO diagnoses (
  diagnosis_id, patient_id, provider_id, appointment_id,
  icd10_code, description, diagnosis_type,
  status, diagnosed_date, ml_confidence
) VALUES (
  uuid_generate_v4(), 'patient-uuid', 'provider-uuid', 'appt-uuid',
  'I25.10', 'Coronary Artery Disease', 'PRIMARY',
  'ACTIVE', CURRENT_DATE, 0.89
);
```

### 9. Prescribe Medication
**Protocol**: PostgreSQL  
**Action**: Create prescription after interaction check

```sql
INSERT INTO prescriptions (
  prescription_id, patient_id, provider_id,
  medication_name, ndc_code, dosage, frequency,
  prescribed_date, interaction_checked, status
) VALUES (
  uuid_generate_v4(), 'patient-uuid', 'provider-uuid',
  'Aspirin 81mg', '00536-3111-01', '81mg', 'Once daily',
  CURRENT_DATE, TRUE, 'ACTIVE'
);
```

### 10. Update Readmission Risk
**Protocol**: PostgreSQL + Redis  
**Action**: Calculate ML readmission risk

```redis
# Get ML readmission risk prediction
GET ml:readmission:patient-12345

# Response:
{
  "risk_score": 0.34,
  "risk_level": "MEDIUM",
  "factors": ["Coronary Artery Disease", "Age 65+", "Multiple Medications"],
  "model": "readmission_xgb_v3"
}
```

```sql
UPDATE patients
SET readmission_risk_score = 0.34
WHERE patient_id = 'patient-uuid';
```

### 11. Store Clinical Notes
**Protocol**: MongoDB  
**Action**: Store unstructured clinical documentation

```javascript
db.clinical_notes.insertOne({
  note_id: "note-uuid",
  patient_id: "patient-uuid",
  appointment_id: "appt-uuid",
  provider_id: "provider-uuid",
  note_type: "PROGRESS_NOTE",
  content: "Patient presents with chest pain...",
  diagnosis_codes: ["I25.10"],
  created_at: new Date()
});
```

### 12. HIPAA Audit Log
**Protocol**: PostgreSQL  
**Action**: Log all PHI access

```sql
INSERT INTO audit_log (
  audit_id, user_id, user_role, action, table_name,
  record_id, patient_id, timestamp, ip_address
) VALUES
  (uuid_generate_v4(), 'provider-uuid', 'PHYSICIAN', 'READ', 'patients',
   'patient-uuid', 'patient-uuid', CURRENT_TIMESTAMP, '10.0.1.5'),
  (uuid_generate_v4(), 'provider-uuid', 'PHYSICIAN', 'CREATE', 'diagnoses',
   'diagnosis-uuid', 'patient-uuid', CURRENT_TIMESTAMP, '10.0.1.5');
```

## Performance Metrics

| Step | Protocol | Latency |
|------|----------|---------|
| Patient Lookup | PostgreSQL | <10ms |
| Vitals Recording | PostgreSQL + Redis | <15ms |
| ML Diagnosis | Redis | <50ms |
| Drug Interaction | Neo4j | <30ms |
| Prescription | PostgreSQL | <20ms |
| **Total** | **Multi-protocol** | **<200ms** |

## ML Models Used

1. **Diagnosis Prediction**: Random Forest (89% accuracy)
2. **Readmission Risk**: XGBoost (85% accuracy)
3. **Treatment Recommendations**: Neural Network
4. **Disease Progression**: LSTM

## Compliance

- **HIPAA**: All PHI access logged
- **HL7/FHIR**: Standard data formats
- **Audit Trail**: Complete access history
- **Encryption**: Data encrypted at rest and in transit
- **RBAC**: Role-based access control enforced
