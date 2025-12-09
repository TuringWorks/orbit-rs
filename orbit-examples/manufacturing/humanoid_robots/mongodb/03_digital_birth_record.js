// ============================================================================
// OrbitRS Humanoid Robot Manufacturing - Digital Birth Record (MongoDB)
// ============================================================================
// Single source of truth for the robot's "as-built" state
// ============================================================================

db = db.getSiblingDB('robot_records');

db.birth_records.insertMany([
    {
        robot_serial: "RBT-9000-001",
        model: "Gen-1",
        manufactured_date: new Date(),
        shipping_manifest: {
            destination: "Logistics Hub Alpha",
            owner: "Internal_Fleet_Testing"
        },
        calibration_summary: {
            walk_gait_score: 0.98,
            hand_dexterity_score: 0.99,
            vision_alignment_error_mm: 0.05
        },
        // Complete immutable copy of critical params at time of shipment
        factory_settings: {
            pid_gains: { kP: 120, kI: 0.5, kD: 10 },
            safety_limits: { max_speed_mps: 2.5, max_force_n: 500 }
        },
        qa_signoffs: [
            { station: "Electrical", inspector: "Emp-44", status: "PASS", ts: new Date() },
            { station: "Final_Motion", inspector: "Emp-99", status: "PASS", ts: new Date() }
        ]
    }
]);

// Find robots with exceptional calibration scores (> 0.98)
db.birth_records.find({ "calibration_summary.walk_gait_score": { $gt: 0.98 } });
