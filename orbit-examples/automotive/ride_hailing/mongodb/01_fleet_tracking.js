// ============================================================================
// OrbitRS Ride Hailing Examples - Fleet Tracking (MongoDB)
// ============================================================================
// Real-time tracking of robo-taxi fleet using geospatial data and status logs
// ============================================================================

db = db.getSiblingDB('ride_hailing');

// 1. Vehicles Collection (Current State)
// =====================================
db.createCollection("vehicles");

// Geospatial index for "Find nearest taxi"
db.vehicles.createIndex({ location: "2dsphere" });
// Index for status queries (e.g., "Find all available cars")
db.vehicles.createIndex({ status: 1 });
db.vehicles.createIndex({ vehicle_id: 1 }, { unique: true });

// Insert active fleet
db.vehicles.insertMany([
    {
        vehicle_id: "VH-1001",
        model: "CyberCab Model 5",
        status: "AVAILABLE", // AVAILABLE, BUSY, CHARGING, MAINTENANCE, OFFLINE
        location: {
            type: "Point",
            coordinates: [-122.4194, 37.7749] // San Francisco
        },
        battery_level: 85,
        last_updated: new Date(),
        capabilities: ["wifi", "wheelchair_accessible", "pet_friendly"]
    },
    {
        vehicle_id: "VH-1002",
        model: "CyberCab Model 5",
        status: "BUSY",
        location: {
            type: "Point",
            coordinates: [-122.4090, 37.7837] // Near SF MOMA
        },
        current_trip_id: "TRIP-998877",
        battery_level: 62,
        last_updated: new Date(),
        capabilities: ["wifi"]
    },
    {
        vehicle_id: "VH-1003",
        model: "RoboVan XL",
        status: "CHARGING",
        location: {
            type: "Point",
            coordinates: [-122.3999, 37.7900] // Charging station
        },
        battery_level: 12,
        last_updated: new Date(),
        capabilities: ["6_seater", "wifi"]
    },
    {
        vehicle_id: "VH-1004",
        model: "CyberCab Model 5",
        status: "AVAILABLE",
        location: {
            type: "Point",
            coordinates: [-122.4220, 37.7650] // Mission District
        },
        battery_level: 91,
        last_updated: new Date(),
        capabilities: ["pet_friendly"]
    }
]);

// 2. Vehicle Telemetry Logs (TimeSeries)
// =====================================
// In a real scenario, this would be a TimeSeries collection
db.createCollection("vehicle_telemetry");
db.vehicle_telemetry.createIndex({ vehicle_id: 1, timestamp: -1 });

db.vehicle_telemetry.insertMany([
    {
        vehicle_id: "VH-1002",
        timestamp: new Date(new Date().getTime() - 60000), // 1 min ago
        speed_kmh: 45,
        location: { type: "Point", coordinates: [-122.4092, 37.7835] },
        battery_level: 63
    },
    {
        vehicle_id: "VH-1002",
        timestamp: new Date(), // Now
        speed_kmh: 30,
        location: { type: "Point", coordinates: [-122.4090, 37.7837] },
        battery_level: 62
    }
]);


// 3. Example Queries
// ==================

print("=== Find Available Vehicles near User (Union Square) ===");
// User at [-122.4075, 37.7879]
var userLoc = { type: "Point", coordinates: [-122.4075, 37.7879] };

var nearest = db.vehicles.find({
    status: "AVAILABLE",
    location: {
        $near: {
            $geometry: userLoc,
            $maxDistance: 2000 // 2km radius
        }
    }
}, { vehicle_id: 1, status: 1, location: 1 }).limit(3);

while (nearest.hasNext()) {
    printjson(nearest.next());
}

print("\n=== Fleet Status Summary ===");
var stats = db.vehicles.aggregate([
    {
        $group: {
            _id: "$status",
            count: { $sum: 1 },
            avg_battery: { $avg: "$battery_level" }
        }
    }
]);

while (stats.hasNext()) {
    printjson(stats.next());
}

print("\n=== Ride Hailing setup complete. ===");
