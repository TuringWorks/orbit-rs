/**
 * Healthcare Use Case: Clinical Documentation (FHIR)
 * Purpose: Store unstructured or semi-structured clinical data using standard FHIR JSON formats.
 * MongoDB is ideal for FHIR Resources because they are naturally nested JSON documents.
 */

// 1. Clinical Note (FHIR DocumentReference / Composition)
// Doctor's progress note for a visit.
db.clinical_notes.insertOne({
    "resourceType": "DocumentReference",
    "id": "doc-555",
    "status": "current",
    "docStatus": "final",
    "type": {
        "coding": [{
            "system": "http://loinc.org",
            "code": "11506-3",
            "display": "Progress note"
        }]
    },
    "subject": {
        "reference": "Patient/pat-123" // Link to SQL Patient ID
    },
    "context": {
        "encounter": [{
            "reference": "Encounter/enc-999" // Link to SQL Encounter ID
        }]
    },
    "author": [{
        "reference": "Practitioner/dr-smith"
    }],
    "date": "2024-12-10T09:30:00Z",
    "content": [{
        "attachment": {
            "contentType": "text/plain",
            "data": "VGhlIHBhdGllbnQgcmVwb3J0cyBtaWxkIGhlYWRhY2hlcy4uLg==" // Base64 encoded note or plain text
        }
    }],
    // Custom extensions for app-specific logic
    "extension": [{
        "url": "http://hospital.org/tags",
        "valueString": "needs-followup"
    }]
});

// 2. Care Plan
// A complex, evolving document describing how to treat the patient's condition.
db.care_plans.insertOne({
    "resourceType": "CarePlan",
    "id": "cp-101",
    "status": "active",
    "intent": "plan",
    "subject": { "reference": "Patient/pat-123" },
    "period": {
        "start": "2024-12-01",
        "end": "2025-06-01"
    },
    "addresses": [{
        "reference": "Condition/cond-diabetes"
    }],
    "activity": [
        {
            "detail": {
                "kind": "Appointment",
                "code": {
                    "coding": [{ "system": "http://snomed.info/sct", "code": "406529007", "display": "Dietician Checkup" }]
                },
                "status": "scheduled",
                "scheduledString": "Every 2 weeks"
            }
        },
        {
            "detail": {
                "kind": "MedicationRequest",
                "status": "in-progress",
                "description": "Daily Insulin administration"
            }
        }
    ]
});

// 3. Query: Find all notes for a specific patient encounter
db.clinical_notes.find({
    "context.encounter.reference": "Encounter/enc-999"
});
