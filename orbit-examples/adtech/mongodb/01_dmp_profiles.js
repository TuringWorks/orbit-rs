/**
 * AdTech Use Case: Data Management Platform (DMP)
 * Purpose: Store rich user profiles, aggregated behavioral data, and audience segments.
 * MongoDB is ideal here due to the flexible/sparse nature of user tracking data.
 */

// 1. Create User Profile
// Keyed by a stable persistent ID (e.g., our internal UUID)
// Mapped to various cookie IDs and Device IDs.
db.dmp_profiles.insertOne({
    "user_id": "u_555666",

    // Identity Map (synced via Graph, stored here for fast lookups)
    "identifiers": {
        "cookie_ids": ["c_abc123", "c_xyz789"],
        "device_ids": {
            "ios_idfa": "0000-0000-...",
            "android_gaid": "aaaa-bbbb-..."
        },
        "email_hashes": ["hash_of_bob@email.com"]
    },

    // Demographics (Inferred or Declared)
    "demographics": {
        "age_group": "25-34",
        "gender": "M",
        "income_bracket": "High"
    },

    // Segments (Computed nightly or real-time)
    "segments": [
        "Auto_Intender",      // Visited car sites > 3 times
        "Luxury_Shopper",     // Bought items > $500
        "Travel_Enthusiast"   // Browsed flight booking sites
    ],

    // Behavioral Signals (Rolling window)
    "recent_activity": [
        {
            "ts": ISODate("2024-12-09T10:00:00Z"),
            "category": "Automotive",
            "action": "Viewed_Product",
            "url": "carsite.com/suv-2025"
        },
        {
            "ts": ISODate("2024-12-09T10:05:00Z"),
            "category": "Finance",
            "action": "Read_Article",
            "url": "financeblog.com/car-loans"
        }
    ],

    "gdpr_consent": {
        "consent_string": "CPxyz...123",
        "purposes_allowed": [1, 2, 3, 4]
    },

    "last_updated": ISODate("2024-12-09T10:05:00Z")
});

// 2. Audience Segmentation Query
// Find users for a "Sport SUV" campaign
// Target: "Auto Intender" segment AND "Sports" interest
db.dmp_profiles.find({
    "segments": "Auto_Intender",
    "recent_activity.category": "Sports"
});

// 3. Real-time Profile Update (Pixel Fire)
// User visited a site -> Append activity and TTL the signals
db.dmp_profiles.updateOne(
    { "identifiers.cookie_ids": "c_abc123" },
    {
        $push: {
            "recent_activity": {
                $each: [{
                    "ts": new Date(),
                    "category": "Travel",
                    "action": "Search",
                    "value": "Hotels in Paris"
                }],
                $slice: -50 // Keep only last 50 actions to manage doc size
            }
        },
        $addToSet: { "segments": "Travel_Intender" }, // Real-time segment qualification
        $set: { "last_updated": new Date() }
    }
);
