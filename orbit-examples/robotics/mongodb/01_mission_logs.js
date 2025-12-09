// Robotics Mission Logs (MongoDB)
// Storing complex logs from autonomous missions, including error dumps and path plans.

db = db.getSiblingDB('robotics_logs');

// 1. Mission Execution Log
db.mission_logs.insertOne({
    mission_id: "M-5501",
    robot_id: "R-101",
    start_time: new Date("2024-06-01T08:00:00Z"),
    end_time: new Date("2024-06-01T08:15:00Z"),
    type: "DELIVERY",
    targets: [
        { loc: { x: 10, y: 50 }, action: "PICK", payload: "Pallet-55" },
        { loc: { x: 80, y: 50 }, action: "DROP" }
    ],
    path_plan: [
        { x: 10, y: 50 }, { x: 15, y: 50 }, { x: 20, y: 55 } // ... simplified path
    ],
    status: "COMPLETED",
    battery_consumed: 5.2
});

// 2. Error Dump
db.error_logs.insertOne({
    robot_id: "R-102",
    timestamp: new Date(),
    code: "ERR_OBSTACLE_STUCK",
    localization_confidence: 0.45,
    lidar_snapshot: {
        resolution: 0.1,
        ranges: [1.2, 1.1, 0.2, 0.2, 1.1] // Obstacle detected close
    },
    stack_trace: "NavigationPlanner.cpp:402 - Path blocked"
});

// 3. Query: Find missions that consumed > 10% battery
print("High Consumption Missions:");
cursor = db.mission_logs.find({ "battery_consumed": { $gt: 10.0 } });
while (cursor.hasNext()) {
    printjson(cursor.next());
}
