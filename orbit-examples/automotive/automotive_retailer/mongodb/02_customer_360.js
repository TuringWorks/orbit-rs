// ============================================================================
// OrbitRS Car Dealership Examples - Customer 360 (MongoDB)
// ============================================================================
// Unified view of customer interactions, preferences, and history
// ============================================================================

db = db.getSiblingDB('dealership_crm');

db.createCollection("customers");
db.customers.createIndex({ "profile.email": 1 }, { unique: true });
db.customers.createIndex({ "interactions.type": 1 });

db.customers.insertMany([
    {
        customer_id: "CUST-1001", // Links to SQL customer_id
        profile: {
            first_name: "Michael",
            last_name: "Buyer",
            email: "mike@example.com",
            address: {
                street: "456 Auto Mall Dr",
                city: "Detroit",
                state: "MI",
                zip: "48201"
            },
            demographics: {
                age_range: "35-45",
                income_bracket: "100k-150k",
                family_size: 4
            }
        },
        preferences: {
            vehicle_type: ["SUV", "Crossover"],
            brands: ["Nissan", "Toyota", "Ford"],
            colors: ["White", "Silver", "Blue"],
            features_wanted: ["Leather Seats", "Sunroof", "Apple CarPlay"],
            budget_max: 40000
        },
        interaction_history: [
            {
                date: new Date("2023-11-15"),
                type: "WEB_VISIT",
                details: "Viewed 2024 Nissan Rogue page for 5 minutes"
            },
            {
                date: new Date("2023-11-20"),
                type: "TEST_DRIVE",
                vehicle_vin: "JN1AZ4EH1DM123456",
                sales_rep: "David Salesman",
                notes: "Liked the ride, concerned about cargo space."
            },
            {
                date: new Date("2023-11-25"),
                type: "PURCHASE",
                vehicle_vin: "JN1AZ4EH1DM123456",
                notes: "Decided to buy after comparing with RAV4."
            },
            {
                date: new Date("2024-05-20"),
                type: "SERVICE_VISIT",
                details: "5000 mile oil change and tire rotation",
                cost: 89.99
            }
        ],
        marketing_consent: {
            email: true,
            sms: false,
            post: true
        },
        lifetime_value: 38589.99
    },
    {
        customer_id: "CUST-1002",
        profile: {
            first_name: "Jennifer",
            last_name: "Browser",
            email: "jen@example.com"
        },
        preferences: {
            vehicle_type: ["Electric"],
            brands: ["Tesla", "Polestar", "Ford"],
            budget_max: 60000
        },
        interaction_history: [
            {
                date: new Date(),
                type: "INQUIRY",
                channel: "Phone",
                notes: "Asked about Mustang Mach-E availability."
            }
        ]
    }
]);

// Query: Find high-value customers interested in SUVs who haven't bought yet
print("=== Hot Leads for SUVs ===");
var leads = db.customers.find({
    "preferences.vehicle_type": "SUV",
    "interaction_history.type": { $ne: "PURCHASE" }
}, { "profile.first_name": 1, "profile.email": 1, "preferences.budget_max": 1 });

while (leads.hasNext()) {
    printjson(leads.next());
}

// Aggregation: Interaction Types Breakdown
print("\n=== Customer Interaction Analytics ===");
var stats = db.customers.aggregate([
    { $unwind: "$interaction_history" },
    { $group: { _id: "$interaction_history.type", count: { $sum: 1 } } },
    { $sort: { count: -1 } }
]);

while (stats.hasNext()) {
    printjson(stats.next());
}
