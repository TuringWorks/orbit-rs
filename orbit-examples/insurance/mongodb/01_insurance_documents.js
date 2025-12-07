// ============================================================================
// OrbitRS Insurance Examples - MongoDB Document Storage
// ============================================================================
// Policy documents, claims photos, inspection reports, customer communications
// ============================================================================

// Connect to insurance database
use insurance;

// ============================================================================
// POLICY DOCUMENTS COLLECTION
// ============================================================================

// Create policy documents collection with schema validation
db.createCollection("policy_documents", {
    validator: {
        $jsonSchema: {
            bsonType: "object",
            required: ["policy_id", "document_type", "file_name", "uploaded_at"],
            properties: {
                policy_id: { bsonType: "string", description: "UUID of the policy" },
                document_type: {
                    enum: ["policy_contract", "declaration", "endorsement", "renewal_notice", "cancellation_notice"],
                    description: "Type of policy document"
                },
                file_name: { bsonType: "string" },
                file_size_bytes: { bsonType: "long" },
                mime_type: { bsonType: "string" },
                file_data: { bsonType: "binData", description: "Binary file data" },
                storage_url: { bsonType: "string", description: "S3/Azure URL if stored externally" },
                uploaded_by: { bsonType: "string" },
                uploaded_at: { bsonType: "date" }
            }
        }
    }
});

// Insert sample policy contract
db.policy_documents.insertOne({
    _id: ObjectId(),
    policy_id: "550e8400-e29b-41d4-a716-446655440000",
    policy_number: "AUTO-2024-001234",
    document_type: "policy_contract",
    file_name: "AUTO-2024-001234-Contract.pdf",
    file_size_bytes: NumberLong(524288), // 512 KB
    mime_type: "application/pdf",
    storage_url: "s3://insurance-docs/policies/2024/AUTO-2024-001234-Contract.pdf",
    metadata: {
        effective_date: ISODate("2024-01-15"),
        expiration_date: ISODate("2025-01-15"),
        state: "CA",
        product_type: "AUTO"
    },
    uploaded_by: "agent-001",
    uploaded_at: ISODate("2024-01-10T10:00:00Z"),
    version: 1,
    supersedes: null
});

// Insert policy declaration page
db.policy_documents.insertOne({
    _id: ObjectId(),
    policy_id: "550e8400-e29b-41d4-a716-446655440000",
    policy_number: "AUTO-2024-001234",
    document_type: "declaration",
    file_name: "AUTO-2024-001234-Declarations.pdf",
    file_size_bytes: NumberLong(102400), // 100 KB
    mime_type: "application/pdf",
    storage_url: "s3://insurance-docs/policies/2024/AUTO-2024-001234-Declarations.pdf",
    metadata: {
        vehicles: [
            { vin: "1HGCM82633A123456", year: 2023, make: "Honda", model: "Accord" }
        ],
        drivers: [
            { name: "John Doe", license: "D1234567", state: "CA" }
        ],
        coverages: {
            liability: "250/500/100",
            collision_deductible: 500,
            comprehensive_deductible: 500
        }
    },
    uploaded_by: "system",
    uploaded_at: ISODate("2024-01-10T10:05:00Z")
});

// ============================================================================
// CLAIMS DOCUMENTATION COLLECTION
// ============================================================================

db.createCollection("claims_documentation", {
    validator: {
        $jsonSchema: {
            bsonType: "object",
            required: ["claim_id", "document_type", "uploaded_at"],
            properties: {
                claim_id: { bsonType: "string" },
                document_type: {
                    enum: ["photo", "video", "police_report", "medical_record", "repair_estimate", "adjuster_report"],
                    description: "Type of claim document"
                }
            }
        }
    }
});

// Insert accident scene photos
db.claims_documentation.insertMany([
    {
        _id: ObjectId(),
        claim_id: "CLM-2024-001",
        claim_number: "2024-AUTO-001234",
        document_type: "photo",
        description: "Front-end damage to insured vehicle",
        file_name: "accident_front_damage_001.jpg",
        file_size_bytes: NumberLong(2097152), // 2 MB
        mime_type: "image/jpeg",
        storage_url: "s3://insurance-claims/2024/CLM-2024-001/photos/front_damage_001.jpg",
        metadata: {
            location: {
                type: "Point",
                coordinates: [-122.4194, 37.7749] // [longitude, latitude] - San Francisco
            },
            timestamp: ISODate("2024-12-05T14:35:00Z"),
            device: "iPhone 15 Pro",
            camera_settings: {
                resolution: "4032x3024",
                iso: 100,
                focal_length: "6.86mm"
            }
        },
        uploaded_by: "customer-001",
        uploaded_at: ISODate("2024-12-06T09:15:00Z")
    },
    {
        _id: ObjectId(),
        claim_id: "CLM-2024-001",
        claim_number: "2024-AUTO-001234",
        document_type: "photo",
        description: "Side damage to insured vehicle",
        file_name: "accident_side_damage_002.jpg",
        file_size_bytes: NumberLong(1835008), // 1.75 MB
        mime_type: "image/jpeg",
        storage_url: "s3://insurance-claims/2024/CLM-2024-001/photos/side_damage_002.jpg",
        metadata: {
            location: {
                type: "Point",
                coordinates: [-122.4194, 37.7749]
            },
            timestamp: ISODate("2024-12-05T14:36:00Z"),
            device: "iPhone 15 Pro"
        },
        uploaded_by: "customer-001",
        uploaded_at: ISODate("2024-12-06T09:16:00Z")
    }
]);

// Insert repair estimate
db.claims_documentation.insertOne({
    _id: ObjectId(),
    claim_id: "CLM-2024-001",
    claim_number: "2024-AUTO-001234",
    document_type: "repair_estimate",
    description: "Initial repair estimate from certified body shop",
    file_name: "repair_estimate_ABC_Auto_Body.pdf",
    file_size_bytes: NumberLong(524288),
    mime_type: "application/pdf",
    storage_url: "s3://insurance-claims/2024/CLM-2024-001/estimates/ABC_Auto_Body.pdf",
    metadata: {
        shop_name: "ABC Auto Body",
        shop_phone: "415-555-0123",
        estimate_date: ISODate("2024-12-06T00:00:00Z"),
        estimated_cost: 4850.00,
        repair_time_days: 5,
        line_items: [
            { description: "Replace front bumper", labor: 350.00, parts: 850.00 },
            { description: "Repair left fender", labor: 450.00, parts: 320.00 },
            { description: "Paint work", labor: 800.00, parts: 580.00 },
            { description: "Headlight replacement", labor: 150.00, parts: 450.00 }
        ]
    },
    uploaded_by: "adjuster-001",
    uploaded_at: ISODate("2024-12-06T15:30:00Z")
});

// ============================================================================
// INSPECTION REPORTS COLLECTION
// ============================================================================

db.createCollection("inspection_reports");

// Insert home inspection report
db.inspection_reports.insertOne({
    _id: ObjectId(),
    property_id: "prop-001",
    policy_id: "550e8400-e29b-41d4-a716-446655440001",
    inspection_type: "INITIAL",
    inspection_date: ISODate("2024-01-05T00:00:00Z"),
    inspector: {
        name: "Mike Johnson",
        company: "Professional Home Inspections Inc.",
        license: "HI-12345",
        phone: "415-555-0199"
    },
    property: {
        address: "123 Main St, San Francisco, CA 94102",
        type: "SINGLE_FAMILY",
        year_built: 1985,
        square_footage: 2500
    },
    findings: {
        overall_condition: "GOOD",
        roof: {
            condition: "GOOD",
            type: "ASPHALT_SHINGLE",
            age_years: 5,
            estimated_remaining_life: 15,
            notes: "Recent replacement, in excellent condition"
        },
        electrical: {
            condition: "FAIR",
            panel_type: "100A",
            updated: false,
            notes: "Original 1985 wiring, recommend upgrade to 200A panel"
        },
        plumbing: {
            condition: "GOOD",
            updated: true,
            notes: "Copper pipes replaced in 2020"
        },
        hvac: {
            condition: "GOOD",
            heating_type: "FORCED_AIR_GAS",
            cooling_type: "CENTRAL_AC",
            age_years: 3
        },
        foundation: {
            condition: "EXCELLENT",
            type: "SLAB",
            notes: "No cracks or settling observed"
        }
    },
    safety_features: {
        smoke_detectors: true,
        carbon_monoxide_detectors: true,
        fire_extinguisher: true,
        security_system: true,
        monitored: true
    },
    issues_found: [
        {
            severity: "MINOR",
            area: "ELECTRICAL",
            description: "Recommend panel upgrade",
            estimated_cost: 2500.00,
            priority: "LOW"
        },
        {
            severity: "MINOR",
            area: "EXTERIOR",
            description: "Gutter cleaning needed",
            estimated_cost: 150.00,
            priority: "MEDIUM"
        }
    ],
    photos: [
        {
            description: "Roof condition",
            file_name: "roof_overview.jpg",
            storage_url: "s3://insurance-inspections/2024/prop-001/roof_overview.jpg"
        },
        {
            description: "Electrical panel",
            file_name: "electrical_panel.jpg",
            storage_url: "s3://insurance-inspections/2024/prop-001/electrical_panel.jpg"
        }
    ],
    report_document: {
        file_name: "Inspection_Report_123_Main_St.pdf",
        storage_url: "s3://insurance-inspections/2024/prop-001/full_report.pdf",
        file_size_bytes: NumberLong(3145728) // 3 MB
    },
    created_at: ISODate("2024-01-05T18:00:00Z")
});

// ============================================================================
// CUSTOMER COMMUNICATIONS COLLECTION
// ============================================================================

db.createCollection("customer_communications");

// Insert email correspondence
db.customer_communications.insertMany([
    {
        _id: ObjectId(),
        customer_id: "550e8400-e29b-41d4-a716-446655440000",
        policy_id: "550e8400-e29b-41d4-a716-446655440000",
        communication_type: "EMAIL",
        direction: "OUTBOUND",
        subject: "Your Auto Insurance Policy is Ready",
        from: "policies@insurance.com",
        to: "john.doe@email.com",
        body: "Dear John,\n\nYour auto insurance policy AUTO-2024-001234 is now active...",
        sent_at: ISODate("2024-01-15T10:00:00Z"),
        status: "DELIVERED",
        attachments: [
            {
                file_name: "Policy_Documents.pdf",
                storage_url: "s3://insurance-emails/2024/attachments/policy_docs_001.pdf"
            }
        ]
    },
    {
        _id: ObjectId(),
        customer_id: "550e8400-e29b-41d4-a716-446655440000",
        claim_id: "CLM-2024-001",
        communication_type: "SMS",
        direction: "OUTBOUND",
        to: "+14155550123",
        body: "Your claim CLM-2024-001 has been approved for $4,500. Payment will be processed within 3 business days.",
        sent_at: ISODate("2024-12-06T19:00:00Z"),
        status: "DELIVERED"
    },
    {
        _id: ObjectId(),
        customer_id: "550e8400-e29b-41d4-a716-446655440000",
        policy_id: "550e8400-e29b-41d4-a716-446655440000",
        communication_type: "CALL_NOTE",
        direction: "INBOUND",
        agent_id: "agent-001",
        subject: "Question about coverage",
        notes: "Customer called to ask about rental car coverage. Explained that rental reimbursement is included with $50/day limit for up to 30 days.",
        duration_seconds: 420,
        call_timestamp: ISODate("2024-11-20T14:30:00Z"),
        created_at: ISODate("2024-11-20T14:37:00Z")
    }
]);

// ============================================================================
// QUERIES
// ============================================================================

// Find all documents for a claim
db.claims_documentation.find({ claim_id: "CLM-2024-001" });

// Find all photos for a claim
db.claims_documentation.find({
    claim_id: "CLM-2024-001",
    document_type: "photo"
});

// Find policy documents by policy number
db.policy_documents.find({ policy_number: "AUTO-2024-001234" });

// Find recent communications for a customer
db.customer_communications.find({
    customer_id: "550e8400-e29b-41d4-a716-446655440000"
}).sort({ sent_at: -1 }).limit(10);

// Find inspection reports with issues
db.inspection_reports.find({
    "issues_found": { $exists: true, $ne: [] }
});

// Geospatial query - find claims near a location
db.claims_documentation.createIndex({ "metadata.location": "2dsphere" });

db.claims_documentation.find({
    "metadata.location": {
        $near: {
            $geometry: {
                type: "Point",
                coordinates: [-122.4194, 37.7749]
            },
            $maxDistance: 5000 // 5km
        }
    }
});

// ============================================================================
// AGGREGATIONS
// ============================================================================

// Count documents by type for a claim
db.claims_documentation.aggregate([
    { $match: { claim_id: "CLM-2024-001" } },
    {
        $group: {
            _id: "$document_type",
            count: { $sum: 1 },
            total_size: { $sum: "$file_size_bytes" }
        }
    }
]);

// Get total storage used by customer
db.policy_documents.aggregate([
    { $match: { policy_id: "550e8400-e29b-41d4-a716-446655440000" } },
    {
        $group: {
            _id: null,
            total_documents: { $sum: 1 },
            total_size_bytes: { $sum: "$file_size_bytes" }
        }
    }
]);

// ============================================================================
// INDEXES
// ============================================================================

// Create indexes for performance
db.policy_documents.createIndex({ policy_id: 1 });
db.policy_documents.createIndex({ policy_number: 1 });
db.policy_documents.createIndex({ document_type: 1 });
db.policy_documents.createIndex({ uploaded_at: -1 });

db.claims_documentation.createIndex({ claim_id: 1 });
db.claims_documentation.createIndex({ claim_number: 1 });
db.claims_documentation.createIndex({ document_type: 1 });
db.claims_documentation.createIndex({ uploaded_at: -1 });

db.inspection_reports.createIndex({ property_id: 1 });
db.inspection_reports.createIndex({ policy_id: 1 });
db.inspection_reports.createIndex({ inspection_date: -1 });

db.customer_communications.createIndex({ customer_id: 1 });
db.customer_communications.createIndex({ policy_id: 1 });
db.customer_communications.createIndex({ claim_id: 1 });
db.customer_communications.createIndex({ sent_at: -1 });

print("MongoDB insurance collections created successfully!");
