// =============================================================================
// OrbitRS Telco Example: Subscriber Management with MongoDB
// =============================================================================
// Demonstrates document-based data modeling for telecom subscriber management
// using MongoDB query language with OrbitRS.

// =============================================================================
// DATABASE AND COLLECTION SETUP
// =============================================================================

// Switch to telco database (auto-creates if doesn't exist)
use telco;

// Create collections with validation schemas
db.createCollection("subscribers", {
    validator: {
        $jsonSchema: {
            bsonType: "object",
            required: ["subscriber_id", "phone_number", "plan", "status"],
            properties: {
                subscriber_id: { bsonType: "string" },
                phone_number: { bsonType: "string", pattern: "^\\+1-[0-9]{3}-[0-9]{4}$" },
                plan: {
                    bsonType: "object",
                    required: ["plan_id", "name", "monthly_price"],
                    properties: {
                        plan_id: { bsonType: "string" },
                        name: { bsonType: "string" },
                        monthly_price: { bsonType: "double" }
                    }
                },
                status: { enum: ["ACTIVE", "SUSPENDED", "CANCELLED", "PENDING"] }
            }
        }
    }
});

db.createCollection("usage_records");
db.createCollection("billing");
db.createCollection("support_tickets");

// =============================================================================
// INDEXES
// =============================================================================

// Subscribers indexes
db.subscribers.createIndex({ "subscriber_id": 1 }, { unique: true });
db.subscribers.createIndex({ "phone_number": 1 }, { unique: true });
db.subscribers.createIndex({ "email": 1 });
db.subscribers.createIndex({ "plan.plan_id": 1 });
db.subscribers.createIndex({ "status": 1 });
db.subscribers.createIndex({ "address.city": 1, "address.state": 1 });
db.subscribers.createIndex({ "loyalty.tier": 1, "loyalty.points": -1 });

// Usage records indexes
db.usage_records.createIndex({ "subscriber_id": 1, "timestamp": -1 });
db.usage_records.createIndex({ "usage_type": 1, "timestamp": -1 });
db.usage_records.createIndex({ "timestamp": 1 }, { expireAfterSeconds: 7776000 }); // 90 days TTL

// Billing indexes
db.billing.createIndex({ "subscriber_id": 1, "billing_period": -1 });
db.billing.createIndex({ "status": 1, "due_date": 1 });

// Support tickets indexes
db.support_tickets.createIndex({ "subscriber_id": 1, "created_at": -1 });
db.support_tickets.createIndex({ "status": 1, "priority": 1 });

// =============================================================================
// SAMPLE DATA INSERTION
// =============================================================================

// Insert subscribers with rich profile data
db.subscribers.insertMany([
    {
        subscriber_id: "SUB-10001",
        phone_number: "+1-555-0101",
        email: "john.smith@email.com",
        profile: {
            first_name: "John",
            last_name: "Smith",
            date_of_birth: new Date("1985-03-15"),
            ssn_last_four: "1234",
            preferred_language: "en"
        },
        address: {
            street: "123 Main St",
            city: "New York",
            state: "NY",
            zip: "10001",
            country: "USA"
        },
        plan: {
            plan_id: "PLAN-UNLIMITED-5G",
            name: "Unlimited 5G Premium",
            monthly_price: 89.99,
            data_limit_gb: null, // unlimited
            voice_minutes: null, // unlimited
            sms_limit: null, // unlimited
            international_included: true,
            hotspot_gb: 50,
            streaming_quality: "4K"
        },
        devices: [
            {
                imei: "353456789012345",
                device_type: "SMARTPHONE",
                manufacturer: "Apple",
                model: "iPhone 15 Pro",
                activated_date: new Date("2024-01-15"),
                sim_type: "eSIM",
                status: "ACTIVE"
            }
        ],
        status: "ACTIVE",
        signup_date: new Date("2022-01-15"),
        loyalty: {
            tier: "GOLD",
            points: 15420,
            member_since: new Date("2022-01-15")
        },
        preferences: {
            paperless_billing: true,
            autopay_enabled: true,
            marketing_consent: true,
            sms_notifications: true
        },
        created_at: new Date(),
        updated_at: new Date()
    },
    {
        subscriber_id: "SUB-10002",
        phone_number: "+1-555-0102",
        email: "jane.doe@email.com",
        profile: {
            first_name: "Jane",
            last_name: "Doe",
            date_of_birth: new Date("1990-07-22"),
            preferred_language: "en"
        },
        address: {
            street: "456 Oak Ave",
            city: "Brooklyn",
            state: "NY",
            zip: "11201",
            country: "USA"
        },
        plan: {
            plan_id: "PLAN-FAMILY-SHARE",
            name: "Family Share 5 Lines",
            monthly_price: 149.99,
            data_limit_gb: 100, // shared
            voice_minutes: null,
            sms_limit: null,
            international_included: false,
            hotspot_gb: 25,
            streaming_quality: "HD"
        },
        devices: [
            {
                imei: "353456789012346",
                device_type: "SMARTPHONE",
                manufacturer: "Samsung",
                model: "Galaxy S24 Ultra",
                activated_date: new Date("2024-02-01"),
                sim_type: "Physical SIM",
                status: "ACTIVE"
            },
            {
                imei: "353456789012350",
                device_type: "SMARTWATCH",
                manufacturer: "Samsung",
                model: "Galaxy Watch 6",
                activated_date: new Date("2024-02-01"),
                sim_type: "eSIM",
                status: "ACTIVE"
            }
        ],
        family_members: [
            { subscriber_id: "SUB-10002-F1", name: "Mike Doe", relation: "spouse" },
            { subscriber_id: "SUB-10002-F2", name: "Emma Doe", relation: "child" }
        ],
        status: "ACTIVE",
        signup_date: new Date("2021-06-20"),
        loyalty: {
            tier: "PLATINUM",
            points: 45820,
            member_since: new Date("2021-06-20")
        },
        preferences: {
            paperless_billing: true,
            autopay_enabled: true,
            marketing_consent: false,
            sms_notifications: true
        },
        created_at: new Date(),
        updated_at: new Date()
    },
    {
        subscriber_id: "SUB-10003",
        phone_number: "+1-555-0103",
        email: "bob.johnson@email.com",
        profile: {
            first_name: "Bob",
            last_name: "Johnson",
            date_of_birth: new Date("1978-11-08"),
            preferred_language: "en"
        },
        address: {
            street: "789 Pine Rd",
            city: "Queens",
            state: "NY",
            zip: "11375",
            country: "USA"
        },
        plan: {
            plan_id: "PLAN-BASIC-4G",
            name: "Basic 4G",
            monthly_price: 45.00,
            data_limit_gb: 10,
            voice_minutes: 500,
            sms_limit: 1000,
            international_included: false,
            hotspot_gb: 5,
            streaming_quality: "SD"
        },
        devices: [
            {
                imei: "353456789012347",
                device_type: "SMARTPHONE",
                manufacturer: "Google",
                model: "Pixel 8",
                activated_date: new Date("2023-11-15"),
                sim_type: "Physical SIM",
                status: "ACTIVE"
            }
        ],
        status: "ACTIVE",
        signup_date: new Date("2023-08-10"),
        loyalty: {
            tier: "STANDARD",
            points: 2150,
            member_since: new Date("2023-08-10")
        },
        preferences: {
            paperless_billing: false,
            autopay_enabled: false,
            marketing_consent: true,
            sms_notifications: false
        },
        created_at: new Date(),
        updated_at: new Date()
    }
]);

// Insert usage records
db.usage_records.insertMany([
    {
        subscriber_id: "SUB-10001",
        timestamp: new Date("2024-01-15T10:30:00Z"),
        usage_type: "VOICE",
        details: {
            called_number: "+1-555-0200",
            duration_seconds: 185,
            call_type: "OUTBOUND",
            roaming: false,
            tower_id: "TOWER-NYC-001"
        }
    },
    {
        subscriber_id: "SUB-10001",
        timestamp: new Date("2024-01-15T11:00:00Z"),
        usage_type: "DATA",
        details: {
            bytes_upload: 524288,
            bytes_download: 10485760,
            session_duration_seconds: 1800,
            connection_type: "5G_NR",
            tower_id: "TOWER-NYC-001"
        }
    },
    {
        subscriber_id: "SUB-10001",
        timestamp: new Date("2024-01-15T12:15:00Z"),
        usage_type: "SMS",
        details: {
            recipient: "+1-555-0300",
            message_type: "OUTBOUND",
            character_count: 120
        }
    },
    {
        subscriber_id: "SUB-10002",
        timestamp: new Date("2024-01-15T09:00:00Z"),
        usage_type: "DATA",
        details: {
            bytes_upload: 1048576,
            bytes_download: 52428800,
            session_duration_seconds: 3600,
            connection_type: "5G_NR",
            tower_id: "TOWER-NYC-002",
            streaming_service: "Netflix"
        }
    }
]);

// Insert billing records
db.billing.insertMany([
    {
        subscriber_id: "SUB-10001",
        billing_period: "2024-01",
        invoice_id: "INV-2024-001-10001",
        statement_date: new Date("2024-02-01"),
        due_date: new Date("2024-02-15"),
        status: "PAID",
        charges: {
            base_plan: 89.99,
            overage_data: 0,
            overage_voice: 0,
            international: 12.50,
            device_payment: 41.66,
            insurance: 15.00,
            taxes_fees: 14.82
        },
        total_amount: 173.97,
        amount_paid: 173.97,
        payment_date: new Date("2024-02-10"),
        payment_method: "AUTOPAY_CARD"
    },
    {
        subscriber_id: "SUB-10002",
        billing_period: "2024-01",
        invoice_id: "INV-2024-001-10002",
        statement_date: new Date("2024-02-01"),
        due_date: new Date("2024-02-15"),
        status: "PENDING",
        charges: {
            base_plan: 149.99,
            overage_data: 25.00,
            overage_voice: 0,
            international: 0,
            device_payment: 0,
            insurance: 25.00,
            taxes_fees: 18.75
        },
        total_amount: 218.74,
        amount_paid: 0,
        payment_date: null,
        payment_method: null
    }
]);

// Insert support tickets
db.support_tickets.insertMany([
    {
        ticket_id: "TKT-2024-001",
        subscriber_id: "SUB-10001",
        created_at: new Date("2024-01-14T14:30:00Z"),
        category: "BILLING",
        priority: "MEDIUM",
        status: "RESOLVED",
        subject: "Question about international charges",
        description: "I see international charges on my bill but I didn't make any international calls.",
        resolution: "Charges were for international SMS sent via messaging app. Explained plan details.",
        assigned_to: "agent-smith",
        resolved_at: new Date("2024-01-14T16:45:00Z"),
        satisfaction_rating: 5
    },
    {
        ticket_id: "TKT-2024-002",
        subscriber_id: "SUB-10003",
        created_at: new Date("2024-01-15T09:00:00Z"),
        category: "NETWORK",
        priority: "HIGH",
        status: "OPEN",
        subject: "Poor signal at home",
        description: "Getting very weak signal at my home address. Calls keep dropping.",
        resolution: null,
        assigned_to: "tech-team",
        resolved_at: null,
        notes: [
            { timestamp: new Date("2024-01-15T10:00:00Z"), agent: "tech-team", note: "Assigned to network engineering for investigation" },
            { timestamp: new Date("2024-01-15T14:00:00Z"), agent: "network-eng", note: "Tower TOWER-NYC-003 shows normal operation. Scheduling site visit." }
        ]
    }
]);

// =============================================================================
// AGGREGATION QUERIES
// =============================================================================

// Query 1: Subscriber summary with usage statistics
db.subscribers.aggregate([
    { $match: { status: "ACTIVE" } },
    {
        $lookup: {
            from: "usage_records",
            localField: "subscriber_id",
            foreignField: "subscriber_id",
            as: "usage"
        }
    },
    {
        $project: {
            subscriber_id: 1,
            name: { $concat: ["$profile.first_name", " ", "$profile.last_name"] },
            plan_name: "$plan.name",
            monthly_price: "$plan.monthly_price",
            device_count: { $size: "$devices" },
            usage_count: { $size: "$usage" },
            loyalty_tier: "$loyalty.tier"
        }
    }
]);

// Query 2: Revenue by plan type
db.billing.aggregate([
    { $match: { status: { $in: ["PAID", "PENDING"] } } },
    {
        $lookup: {
            from: "subscribers",
            localField: "subscriber_id",
            foreignField: "subscriber_id",
            as: "subscriber"
        }
    },
    { $unwind: "$subscriber" },
    {
        $group: {
            _id: "$subscriber.plan.plan_id",
            plan_name: { $first: "$subscriber.plan.name" },
            total_revenue: { $sum: "$total_amount" },
            paid_revenue: { $sum: "$amount_paid" },
            subscriber_count: { $addToSet: "$subscriber_id" }
        }
    },
    {
        $project: {
            plan_id: "$_id",
            plan_name: 1,
            total_revenue: { $round: ["$total_revenue", 2] },
            paid_revenue: { $round: ["$paid_revenue", 2] },
            subscriber_count: { $size: "$subscriber_count" },
            avg_revenue_per_subscriber: {
                $round: [{ $divide: ["$total_revenue", { $size: "$subscriber_count" }] }, 2]
            }
        }
    },
    { $sort: { total_revenue: -1 } }
]);

// Query 3: Data usage analysis by time of day
db.usage_records.aggregate([
    { $match: { usage_type: "DATA" } },
    {
        $project: {
            subscriber_id: 1,
            hour: { $hour: "$timestamp" },
            bytes_total: { $add: ["$details.bytes_upload", "$details.bytes_download"] },
            connection_type: "$details.connection_type"
        }
    },
    {
        $group: {
            _id: { hour: "$hour", connection_type: "$connection_type" },
            total_bytes: { $sum: "$bytes_total" },
            session_count: { $sum: 1 }
        }
    },
    {
        $project: {
            hour: "$_id.hour",
            connection_type: "$_id.connection_type",
            total_gb: { $round: [{ $divide: ["$total_bytes", 1073741824] }, 2] },
            session_count: 1
        }
    },
    { $sort: { hour: 1 } }
]);

// Query 4: Support ticket metrics
db.support_tickets.aggregate([
    {
        $facet: {
            by_status: [
                { $group: { _id: "$status", count: { $sum: 1 } } }
            ],
            by_category: [
                { $group: { _id: "$category", count: { $sum: 1 } } }
            ],
            by_priority: [
                { $group: { _id: "$priority", count: { $sum: 1 } } }
            ],
            avg_resolution_time: [
                { $match: { status: "RESOLVED" } },
                {
                    $project: {
                        resolution_hours: {
                            $divide: [
                                { $subtract: ["$resolved_at", "$created_at"] },
                                3600000 // milliseconds to hours
                            ]
                        }
                    }
                },
                {
                    $group: {
                        _id: null,
                        avg_hours: { $avg: "$resolution_hours" }
                    }
                }
            ],
            satisfaction: [
                { $match: { satisfaction_rating: { $exists: true } } },
                {
                    $group: {
                        _id: null,
                        avg_rating: { $avg: "$satisfaction_rating" },
                        total_ratings: { $sum: 1 }
                    }
                }
            ]
        }
    }
]);

// Query 5: Churn risk analysis - subscribers with high usage but poor experience
db.subscribers.aggregate([
    { $match: { status: "ACTIVE" } },
    {
        $lookup: {
            from: "support_tickets",
            localField: "subscriber_id",
            foreignField: "subscriber_id",
            pipeline: [
                { $match: { status: { $in: ["OPEN", "IN_PROGRESS"] } } }
            ],
            as: "open_tickets"
        }
    },
    {
        $lookup: {
            from: "billing",
            localField: "subscriber_id",
            foreignField: "subscriber_id",
            pipeline: [
                { $match: { status: "PENDING" } }
            ],
            as: "unpaid_bills"
        }
    },
    {
        $match: {
            $or: [
                { "open_tickets": { $ne: [] } },
                { "unpaid_bills": { $ne: [] } }
            ]
        }
    },
    {
        $project: {
            subscriber_id: 1,
            name: { $concat: ["$profile.first_name", " ", "$profile.last_name"] },
            plan: "$plan.name",
            loyalty_tier: "$loyalty.tier",
            open_ticket_count: { $size: "$open_tickets" },
            unpaid_bill_count: { $size: "$unpaid_bills" },
            churn_risk_score: {
                $add: [
                    { $multiply: [{ $size: "$open_tickets" }, 20] },
                    { $multiply: [{ $size: "$unpaid_bills" }, 30] }
                ]
            }
        }
    },
    { $sort: { churn_risk_score: -1 } }
]);

// =============================================================================
// UPDATE OPERATIONS
// =============================================================================

// Update subscriber's plan
db.subscribers.updateOne(
    { subscriber_id: "SUB-10003" },
    {
        $set: {
            "plan.plan_id": "PLAN-UNLIMITED-5G",
            "plan.name": "Unlimited 5G Premium",
            "plan.monthly_price": 89.99,
            "plan.data_limit_gb": null,
            updated_at: new Date()
        },
        $push: {
            plan_history: {
                previous_plan: "PLAN-BASIC-4G",
                changed_at: new Date(),
                reason: "UPGRADE"
            }
        }
    }
);

// Add loyalty points
db.subscribers.updateOne(
    { subscriber_id: "SUB-10001" },
    {
        $inc: { "loyalty.points": 500 },
        $set: { updated_at: new Date() }
    }
);

// Bulk update - apply promotion to eligible subscribers
db.subscribers.updateMany(
    {
        status: "ACTIVE",
        "loyalty.tier": { $in: ["GOLD", "PLATINUM"] },
        "plan.monthly_price": { $gte: 80 }
    },
    {
        $set: {
            "promotions": {
                code: "LOYALTY2024",
                discount_percent: 10,
                valid_until: new Date("2024-06-30"),
                applied_at: new Date()
            }
        }
    }
);

// =============================================================================
// TEXT SEARCH
// =============================================================================

// Create text index for support tickets
db.support_tickets.createIndex({
    subject: "text",
    description: "text",
    "notes.note": "text"
});

// Search support tickets
db.support_tickets.find({
    $text: { $search: "signal dropping" }
}, {
    score: { $meta: "textScore" }
}).sort({ score: { $meta: "textScore" } });
