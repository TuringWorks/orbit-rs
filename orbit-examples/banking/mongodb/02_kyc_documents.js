/**
 * Banking Use Case: Know Your Customer (KYC) & Identity Management
 * Purpose: Manage comprehensive customer identity profiles, enhanced due diligence (EDD) documents,
 * and sanction screening results using MongoDB's flexible document model.
 * 
 * Features demonstrated:
 * - Rich embedded documents for identity proofs (Passport, National ID)
 * - Array fields for tracking verification history
 * - Complex querying for risk profiling
 */

// 1. Create a collection for KYC Profiles with validation
db.createCollection("kyc_profiles", {
    validator: {
        $jsonSchema: {
            bsonType: "object",
            required: ["customer_id", "full_name", "risk_level", "status"],
            properties: {
                risk_level: {
                    enum: ["LOW", "MEDIUM", "HIGH", "CRITICAL"]
                },
                status: {
                    enum: ["PENDING", "VERIFIED", "REJECTED", "UNDER_REVIEW"]
                }
            }
        }
    }
});

// 2. Insert high-net-worth individual profile with multiple identity documents
db.kyc_profiles.insertOne({
    "customer_id": "CUST_998877",
    "full_name": "Alexander Sterling",
    "dob": "1980-05-15",
    "nationality": "GB",
    "pep_status": false, // Politically Exposed Person
    "risk_level": "MEDIUM", // Due to cross-border activity
    "status": "VERIFIED",
    "identity_proofs": [
        {
            "type": "PASSPORT",
            "document_number": "GB99887766",
            "issuing_country": "GB",
            "expiry_date": "2030-01-01",
            "verified_at": "2024-01-15T10:00:00Z",
            "verification_method": "BIOMETRIC_SCAN",
            "metadata": {
                "mrz_code": "P<GBRSTERLING<<ALEXANDER<<<<<<<<<<<<<<<<<<<<<<<",
                "security_features_checked": ["hologram", "microprint", "uv_response"]
            }
        },
        {
            "type": "DRIVER_LICENSE",
            "document_number": "STE998005159",
            "issuing_country": "GB",
            "expiry_date": "2028-05-15",
            "verified_at": "2024-01-15T10:05:00Z"
        }
    ],
    "addresses": [
        {
            "type": "RESIDENTIAL",
            "street": "123 Oxford Street",
            "city": "London",
            "postal_code": "W1D 1LT",
            "country": "GB",
            "current": true,
            "proof_of_address": "UTILITY_BILL_2024_01"
        }
    ],
    "sanctions_screening": {
        "last_checked": "2024-12-01T09:00:00Z",
        "hits": 0,
        "provider": "OrbitalScreening"
    },
    "edd_history": [] // Enhanced Due Diligence
});

// 3. Insert a corporate entity requiring Enhanced Due Diligence (EDD)
db.kyc_profiles.insertOne({
    "customer_id": "CORP_554433",
    "entity_name": "Global Trade Logistics Ltd.",
    "incorporation_country": "SG",
    "risk_level": "HIGH", // High-risk jurisdiction involvement
    "status": "UNDER_REVIEW",
    "beneficial_owners": [
        { "name": "Sarah Chen", "ownership_pct": 60.0, "nationality": "SG" },
        { "name": "Michael Ross", "ownership_pct": 40.0, "nationality": "US" }
    ],
    "uob_docs": [ // Ultimate Beneficial Ownership documents
        {
            "type": "CERTIFICATE_OF_INCORPORATION",
            "doc_id": "SG_ACRA_202022",
            "verified": true
        }
    ],
    "edd_history": [
        {
            "date": "2024-11-20",
            "analyst_id": "INTEL_05",
            "notes": "Flagged for transaction pattern congruent with shell company operations. Requesting additional supplier invoices.",
            "outcome": "ESCALATED"
        }
    ]
});

// 4. Query to find pending reviews for High-Risk customers
// Useful for compliance officer dashboards
db.kyc_profiles.find({
    "risk_level": { $in: ["HIGH", "CRITICAL"] },
    "status": { $in: ["PENDING", "UNDER_REVIEW"] }
});

// 5. Aggregation: Risk Distribution Analysis
// Provide a breakdown of customer base by risk level for auditing
db.kyc_profiles.aggregate([
    {
        $group: {
            _id: "$risk_level",
            count: { $sum: 1 },
            avg_docs_per_user: { $avg: { $size: { $ifNull: ["$identity_proofs", []] } } }
        }
    },
    { $sort: { count: -1 } }
]);

// 6. Update Sanctions Screening result
// Automating the periodic re-screening process
db.kyc_profiles.updateOne(
    { "customer_id": "CUST_998877" },
    {
        $set: {
            "sanctions_screening.last_checked": new Date().toISOString(),
            "sanctions_screening.status": "CLEAN"
        },
        $push: {
            "audit_trail": {
                "action": "AUTOMATED_SCREENING",
                "timestamp": new Date().toISOString(),
                "system": "ScreeningBot_v2"
            }
        }
    }
);
