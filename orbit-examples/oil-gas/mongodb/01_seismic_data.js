// Seismic Exploration Data (MongoDB)
// Storing complex, nested survey data from geological exploration.

db = db.getSiblingDB('exploration_data');

// 1. Seismic Survey Log
db.surveys.insertOne({
    survey_id: "S-2024-001",
    region: "Gulf of Mexico - Block 44",
    contractor: "GeoScan Inc",
    date: new Date("2024-03-15"),
    parameters: {
        source_type: "Airgun",
        receiver_depth_m: 50,
        sample_rate_ms: 2
    },
    layers: [
        { depth_start: 0, depth_end: 1500, type: "Seawater", velocity: 1500 },
        { depth_start: 1500, depth_end: 2200, type: "Sandstone", velocity: 2800 },
        { depth_start: 2200, depth_end: 3500, type: "Shale", velocity: 3100, hydrocarbons_detected: true }
    ],
    raw_data_url: "s3://exploration/raw/s_2024_001.segy"
});

// 2. Core Sample Analysis
db.core_samples.insertOne({
    well_id: "W-99",
    depth_m: 2350,
    lithology: "Limestone",
    porosity_percent: 12.5,
    permeability_md: 450,
    images: ["img1.jpg", "img_uv.jpg"]
});

// 3. Query: Find surveys with Hydrocarbons detected in Shale layers
print("Promising Surveys:");
cursor = db.surveys.find({
    "layers": {
        $elemMatch: {
            type: "Shale",
            hydrocarbons_detected: true
        }
    }
});
while (cursor.hasNext()) {
    printjson(cursor.next());
}
