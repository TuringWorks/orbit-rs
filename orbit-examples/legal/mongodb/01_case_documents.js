// ============================================================================
// OrbitRS Legal Examples - Case Documents (MongoDB)
// ============================================================================
// Storing complex legal case documents with nested evidence and filings
// ============================================================================

db = db.getSiblingDB('legal_firm');

// Insert case documents with nested structure
db.case_documents.insertMany([
    {
        case_id: "CASE-2024-001",
        client: {
            name: "Acme Corporation",
            type: "CORPORATION",
            contact_email: "legal@acme.com"
        },
        matter: {
            title: "Acme Corp v. Coyote Industries",
            practice_area: "Litigation",
            status: "ACTIVE",
            opened_date: new Date("2024-01-15")
        },
        filings: [
            {
                type: "Complaint",
                filed_date: new Date("2024-01-20"),
                document_url: "s3://legal-docs/case-001/complaint.pdf",
                page_count: 45
            },
            {
                type: "Motion to Dismiss",
                filed_date: new Date("2024-02-10"),
                document_url: "s3://legal-docs/case-001/mtd.pdf",
                page_count: 22
            }
        ],
        evidence: [
            {
                exhibit_id: "EX-001",
                description: "Contract signed 2023-06-15",
                type: "Document",
                admitted: true
            },
            {
                exhibit_id: "EX-002",
                description: "Email correspondence July 2023",
                type: "Electronic",
                admitted: false
            }
        ],
        billing_summary: {
            total_hours: 127.5,
            total_amount: 57375.00,
            last_invoice_date: new Date("2024-03-01")
        }
    },
    {
        case_id: "CASE-2024-002",
        client: {
            name: "Jane Smith",
            type: "INDIVIDUAL",
            contact_email: "jane.smith@email.com"
        },
        matter: {
            title: "Smith Estate Planning",
            practice_area: "Estate",
            status: "ACTIVE",
            opened_date: new Date("2024-02-01")
        },
        filings: [],
        evidence: [],
        billing_summary: {
            total_hours: 8.0,
            total_amount: 2400.00,
            last_invoice_date: new Date("2024-02-28")
        }
    }
]);

// Query: Find all active litigation cases
db.case_documents.find({
    "matter.practice_area": "Litigation",
    "matter.status": "ACTIVE"
});

// Query: Find cases with admitted evidence
db.case_documents.find({
    "evidence": {
        $elemMatch: {
            admitted: true
        }
    }
});

// Aggregation: Calculate total billing by practice area
db.case_documents.aggregate([
    {
        $group: {
            _id: "$matter.practice_area",
            total_hours: { $sum: "$billing_summary.total_hours" },
            total_revenue: { $sum: "$billing_summary.total_amount" },
            case_count: { $sum: 1 }
        }
    },
    { $sort: { total_revenue: -1 } }
]);
