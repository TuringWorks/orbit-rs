// ============================================================================
// OrbitRS Aviation Examples - Flight Data Logs (MongoDB)
// ============================================================================
// Quick Access Recorder (QAR) Data Analysis
// ============================================================================

db = db.getSiblingDB('aviation_logs');

db.flight_logs.insertMany([
    {
        flight_id: "FL-501",
        tail_number: "N787BA",
        route: "SFO-LHR",
        departure_time: new Date("2024-06-01T10:00:00Z"),
        arrival_time: new Date("2024-06-01T20:30:00Z"),
        max_altitude: 39000,
        events: [
            { time_offset_sec: 1200, type: "TURBULENCE", magnitude: 1.2 },
            { time_offset_sec: 5400, type: "AUTOPILOT_DISENGAGE", duration: 5 }
        ],
        fuel_stats: {
            takeoff_kg: 45000,
            landing_kg: 8500,
            burn_kg: 36500
        },
        engine_params: {
            eng1_egt_max: 850,
            eng2_egt_max: 845
        }
    }
]);

// Query: Hard landings (Vertical G > 1.8 on touch down) - Hypothetical
// db.flight_logs.find({ "events.type": "HARD_LANDING" })

// Aggregation: Average fuel burn per route
db.flight_logs.aggregate([
    {
        $group: {
            _id: "$route",
            avg_fuel_burn: { $avg: "$fuel_stats.burn_kg" },
            flights: { $sum: 1 }
        }
    }
]);
