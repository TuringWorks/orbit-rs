// ============================================================================
// OrbitRS Field Service Examples - Service Reports (MongoDB)
// ============================================================================
// Storing detailed field service reports with parts, labor, and signatures
// ============================================================================

db = db.getSiblingDB('field_service');

// Insert work orders with nested service details
db.work_orders.insertMany([
    {
        work_order_id: "WO-2024-001234",
        customer: {
            account_id: "CUST-5678",
            name: "Northern Manufacturing Inc",
            address: {
                street: "1234 Industrial Blvd",
                city: "Detroit",
                state: "MI",
                zip: "48201"
            },
            contact: {
                name: "Mike Johnson",
                phone: "+1-313-555-0100",
                email: "mjohnson@northern-mfg.com"
            }
        },
        equipment: {
            asset_id: "EQ-CNC-001",
            type: "CNC Machine",
            manufacturer: "Haas",
            model: "VF-4SS",
            serial_number: "SN-HAAS-2021-4567",
            warranty_status: "In Warranty"
        },
        service_type: "Preventive Maintenance",
        priority: "Medium",
        status: "Completed",
        scheduled_date: new Date("2024-03-15T09:00:00Z"),
        technician: {
            tech_id: "TECH-101",
            name: "Sarah Chen",
            certifications: ["Haas Certified", "Fanuc CNC"]
        },
        service_details: {
            arrival_time: new Date("2024-03-15T09:15:00Z"),
            completion_time: new Date("2024-03-15T12:30:00Z"),
            symptoms_reported: "Spindle vibration at high RPM",
            diagnosis: "Spindle bearing showing wear",
            work_performed: [
                "Inspected spindle assembly",
                "Replaced spindle bearings",
                "Calibrated axis alignment",
                "Updated firmware to v4.2.1"
            ],
            parts_used: [
                {
                    part_number: "HAAS-BRG-001",
                    description: "Spindle Bearing Set",
                    quantity: 1,
                    unit_cost: 450.00
                },
                {
                    part_number: "HAAS-LUB-005",
                    description: "Spindle Lubricant 1L",
                    quantity: 2,
                    unit_cost: 35.00
                }
            ],
            labor_hours: 3.25,
            labor_rate: 125.00,
            travel_time_hours: 0.5
        },
        resolution: {
            status: "Resolved",
            notes: "Spindle running smoothly. Recommended follow-up in 6 months.",
            customer_signature: true,
            signed_at: new Date("2024-03-15T12:35:00Z")
        },
        billing: {
            parts_total: 520.00,
            labor_total: 406.25,
            travel_fee: 50.00,
            tax: 78.10,
            grand_total: 1054.35,
            invoice_number: "INV-2024-001234"
        }
    },
    {
        work_order_id: "WO-2024-001235",
        customer: {
            account_id: "CUST-9012",
            name: "Midwest HVAC Systems",
            address: {
                street: "5678 Commerce Dr",
                city: "Chicago",
                state: "IL",
                zip: "60607"
            },
            contact: {
                name: "Lisa Martinez",
                phone: "+1-312-555-0200"
            }
        },
        equipment: {
            asset_id: "EQ-HVAC-042",
            type: "Rooftop HVAC Unit",
            manufacturer: "Carrier",
            model: "48TC",
            serial_number: "SN-CARR-2019-8901",
            warranty_status: "Expired"
        },
        service_type: "Emergency Repair",
        priority: "Critical",
        status: "In Progress",
        scheduled_date: new Date(),
        technician: {
            tech_id: "TECH-205",
            name: "James Wilson",
            certifications: ["EPA 608", "Carrier Factory Certified"]
        },
        service_details: {
            arrival_time: new Date(),
            symptoms_reported: "Unit not cooling, compressor cycling",
            diagnosis: null,
            work_performed: []
        }
    }
]);

// Query: Find all open emergency work orders
db.work_orders.find({
    service_type: "Emergency Repair",
    status: { $in: ["Open", "In Progress"] }
});

// Query: Find work orders for equipment out of warranty
db.work_orders.find({
    "equipment.warranty_status": "Expired"
}, {
    work_order_id: 1,
    "customer.name": 1,
    "equipment.type": 1,
    "equipment.model": 1,
    service_type: 1
});

// Aggregation: Technician performance metrics
db.work_orders.aggregate([
    { $match: { status: "Completed" } },
    {
        $group: {
            _id: "$technician.tech_id",
            technician_name: { $first: "$technician.name" },
            jobs_completed: { $sum: 1 },
            total_labor_hours: { $sum: "$service_details.labor_hours" },
            total_revenue: { $sum: "$billing.grand_total" },
            avg_job_duration_hours: { $avg: "$service_details.labor_hours" }
        }
    },
    { $sort: { jobs_completed: -1 } }
]);

// Aggregation: Parts usage report
db.work_orders.aggregate([
    { $unwind: "$service_details.parts_used" },
    {
        $group: {
            _id: "$service_details.parts_used.part_number",
            description: { $first: "$service_details.parts_used.description" },
            total_quantity: { $sum: "$service_details.parts_used.quantity" },
            total_cost: {
                $sum: {
                    $multiply: [
                        "$service_details.parts_used.quantity",
                        "$service_details.parts_used.unit_cost"
                    ]
                }
            }
        }
    },
    { $sort: { total_quantity: -1 } }
]);
