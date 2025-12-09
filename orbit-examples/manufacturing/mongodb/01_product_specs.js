// ============================================================================
// OrbitRS Manufacturing Examples - Digital Twin (MongoDB)
// ============================================================================
// Storing complex product specifications and Bill of Materials (BOM)
// ============================================================================

db = db.getSiblingDB('factory_twin');

db.product_specs.insertMany([
    {
        sku: "ROBOT-ARM-X1",
        revision: "v2.5",
        status: "PRODUCTION",
        dimensions: { length_mm: 1200, weight_kg: 450 },
        bom: [
            { part_no: "SRV-MOTOR-55", qty: 6, supplier: "ServoDynamics" },
            { part_no: "CTRL-UNIT-A9", qty: 1, supplier: "ChipCore" }
        ],
        assembly_instructions: {
            step_1: "Mount base plate",
            step_2: "Align axis 1 servo"
        },
        quality_checks: [
            { type: "VIBRATION_TEST", limit_hz: 50 },
            { type: "THERMAL_STRESS", temp_c: 80 }
        ]
    }
]);

// Query: Find all products using 'ServoDynamics' parts
db.product_specs.find({ "bom.supplier": "ServoDynamics" });
