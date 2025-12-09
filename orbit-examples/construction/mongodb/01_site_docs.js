// Construction Example: Site Documents (MongoDB)
// Stores variable structures like Daily Logs, Inspection Reports, and Blueprint Metadata.

db = db.getSiblingDB('construction_docs');

// 1. Daily Site Log
// Captured by Site Superintendent every evening.
db.daily_logs.insertOne({
    project_id: 1, // Skyline Tower
    date: new Date("2024-05-10"),
    weather: { temp_c: 18, conditions: "Overcast", wind_mph: 12 },
    work_performed: [
        { contractor: "Sparky Electric", description: "Rough-in 5th floor", hours: 32 },
        { contractor: "SteelCo", description: "Crane lift HVAC units", hours: 8 }
    ],
    delays: [
        { type: "Material", description: "Concrete truck late 2 hours" }
    ],
    safety_incidents: 0
});

// 2. Blueprint Metadata
// Tracking revisions and sheet info for BIM/CAD files (files likely stored in S3/Blob, metadata here).
db.blueprints.insertOne({
    project_id: 1,
    sheet_number: "E-101",
    title: "5th Floor Lighting Plan",
    revision: 3,
    status: "APPROVED_FOR_CONSTRUCTION",
    tags: ["Electrical", "Lighting", "Level 5"],
    created_by: "ArchitectFunction",
    approved_by: "CityPlanning",
    file_url: "s3://blueprints/skyline/e101_r3.pdf"
});

// 3. Query: Find all logs with delays
print("Logs with Delays:");
cursor = db.daily_logs.find({ "delays": { $not: { $size: 0 } } });
while (cursor.hasNext()) {
    printjson(cursor.next());
}
