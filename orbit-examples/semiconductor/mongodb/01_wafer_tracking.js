// ============================================================================
// OrbitRS Semiconductor Examples - Wafer Tracking (MongoDB)
// ============================================================================
// Tracking wafer lots through fabrication with defect records
// ============================================================================

db = db.getSiblingDB('semiconductor_mes');

// Insert wafer lot documents with processing history
db.wafer_lots.insertMany([
    {
        lot_id: "LOT-2024-A1001",
        product_code: "CPU-7NM-001",
        wafer_count: 25,
        start_date: new Date("2024-03-01"),
        status: "IN_PROCESS",
        current_step: "Photolithography",
        processing_history: [
            {
                step: "Oxidation",
                equipment_id: "OX-101",
                start_time: new Date("2024-03-01T08:00:00Z"),
                end_time: new Date("2024-03-01T12:00:00Z"),
                parameters: {
                    temperature_c: 1100,
                    duration_min: 240,
                    oxide_thickness_nm: 100
                },
                result: "PASS"
            },
            {
                step: "Photolithography",
                equipment_id: "LITHO-201",
                start_time: new Date("2024-03-01T14:00:00Z"),
                end_time: null,
                parameters: {
                    exposure_dose: 50,
                    focus_offset_nm: -20,
                    mask_id: "MASK-7NM-L1"
                },
                result: "IN_PROGRESS"
            }
        ],
        defect_summary: {
            total_defects: 12,
            critical_defects: 2,
            defect_density_per_cm2: 0.15
        },
        yield_estimate: 94.2
    },
    {
        lot_id: "LOT-2024-A1002",
        product_code: "GPU-5NM-002",
        wafer_count: 20,
        start_date: new Date("2024-03-02"),
        status: "HOLD",
        current_step: "Metrology",
        processing_history: [
            {
                step: "Oxidation",
                equipment_id: "OX-102",
                start_time: new Date("2024-03-02T08:00:00Z"),
                end_time: new Date("2024-03-02T12:30:00Z"),
                parameters: {
                    temperature_c: 1050,
                    duration_min: 270,
                    oxide_thickness_nm: 80
                },
                result: "PASS"
            }
        ],
        defect_summary: {
            total_defects: 45,
            critical_defects: 15,
            defect_density_per_cm2: 0.82
        },
        hold_reason: "High defect density - requires engineering review",
        yield_estimate: 78.5
    }
]);

// Insert defect detail records
db.defect_records.insertMany([
    {
        lot_id: "LOT-2024-A1002",
        wafer_id: "W-001",
        defect_id: "DEF-2024-0001",
        detection_step: "Metrology",
        defect_type: "Particle",
        location: { x_mm: 45.2, y_mm: 78.1 },
        size_nm: 150,
        severity: "CRITICAL",
        detected_at: new Date(),
        image_url: "s3://defect-images/lot-a1002/w001/def-0001.png"
    },
    {
        lot_id: "LOT-2024-A1002",
        wafer_id: "W-001",
        defect_id: "DEF-2024-0002",
        detection_step: "Metrology",
        defect_type: "Scratch",
        location: { x_mm: 12.8, y_mm: 34.5 },
        size_nm: 2500,
        severity: "CRITICAL",
        detected_at: new Date(),
        image_url: "s3://defect-images/lot-a1002/w001/def-0002.png"
    }
]);

// Query: Find lots with high defect density
db.wafer_lots.find({
    "defect_summary.defect_density_per_cm2": { $gt: 0.5 }
});

// Query: Find lots currently on hold
db.wafer_lots.find({
    status: "HOLD"
}, {
    lot_id: 1,
    product_code: 1,
    hold_reason: 1,
    yield_estimate: 1
});

// Aggregation: Equipment utilization and yield correlation
db.wafer_lots.aggregate([
    { $unwind: "$processing_history" },
    {
        $group: {
            _id: "$processing_history.equipment_id",
            lots_processed: { $sum: 1 },
            avg_yield: { $avg: "$yield_estimate" },
            pass_rate: {
                $avg: {
                    $cond: [{ $eq: ["$processing_history.result", "PASS"] }, 1, 0]
                }
            }
        }
    },
    { $sort: { avg_yield: -1 } }
]);
