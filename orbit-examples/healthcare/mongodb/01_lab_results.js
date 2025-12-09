// ============================================================================
// OrbitRS Healthcare Examples - Lab Results (MongoDB)
// ============================================================================
// Storing complex, nested lab reports
// ============================================================================

db = db.getSiblingDB('hospital_labs');

db.lab_results.insertMany([
    {
        patient_id: "PT-999000",
        order_id: "ORD-2024-555",
        test_type: "Comprehensive Metabolic Panel",
        collected_at: new Date(),
        status: "FINAL",
        results: [
            { name: "Glucose", value: 95, unit: "mg/dL", ref_range: "70-99", flag: "NORMAL" },
            { name: "Calcium", value: 9.8, unit: "mg/dL", ref_range: "8.5-10.2", flag: "NORMAL" },
            { name: "Sodium", value: 140, unit: "mmol/L", ref_range: "134-144", flag: "NORMAL" }
        ],
        pathologist_notes: "All values within normal limits."
    }
]);

// Query: Find patients with abnormal glucose
db.lab_results.find({
    "results": {
        $elemMatch: {
            name: "Glucose",
            value: { $gt: 100 }
        }
    }
});
