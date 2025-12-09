// ============================================================================
// OrbitRS Travel Examples - Travel Packages MongoDB Collection
// ============================================================================
// Vacation package catalog with flexible schema for destinations and deals
// ============================================================================

// Switch to travel database
use travel;

// Create vacation packages collection
db.createCollection("vacation_packages");

// Insert sample vacation packages
db.vacation_packages.insertMany([
    {
        package_code: "PKG-HAWAII-001",
        name: "Hawaiian Paradise - 7 Nights",
        destination: {
            name: "Honolulu, Hawaii",
            country: "United States",
            region: "Pacific",
            coordinates: {
                latitude: 21.3099,
                longitude: -157.8581
            },
            timezone: "Pacific/Honolulu"
        },
        description: "Experience the ultimate Hawaiian getaway with pristine beaches, volcanic landscapes, and rich Polynesian culture. This all-inclusive package includes flights, luxury beachfront resort, rental car, and exciting activities.",
        package_type: "FLIGHT_HOTEL_CAR",
        duration: {
            nights: 7,
            days: 8
        },
        components: {
            flight: {
                included: true,
                cabin_class: "economy",
                baggage: "2 checked bags included"
            },
            hotel: {
                included: true,
                name: "Waikiki Beach Resort",
                star_rating: 4.5,
                room_type: "Ocean View Deluxe",
                meals: "Breakfast included"
            },
            car: {
                included: true,
                category: "STANDARD",
                unlimited_mileage: true
            },
            activities: [
                {
                    name: "Pearl Harbor Tour",
                    included: true,
                    duration_hours: 4
                },
                {
                    name: "Snorkeling at Hanauma Bay",
                    included: false,
                    additional_cost: 75.00
                },
                {
                    name: "Luau Dinner Show",
                    included: false,
                    additional_cost: 125.00
                }
            ]
        },
        pricing: {
            base_price_per_person: 1299.00,
            single_supplement: 450.00,
            child_discount_percent: 25,
            currency: "USD"
        },
        availability: {
            available_from: new Date("2024-01-01"),
            available_to: new Date("2024-12-31"),
            blackout_dates: [
                new Date("2024-12-20"),
                new Date("2024-12-31")
            ],
            min_travelers: 1,
            max_travelers: 8
        },
        booking_requirements: {
            min_advance_days: 14,
            max_advance_days: 365,
            cancellation_policy: "Free cancellation up to 14 days before departure"
        },
        highlights: [
            "Beachfront luxury resort",
            "Rental car for island exploration",
            "Pearl Harbor historical tour",
            "Snorkeling and water sports",
            "Traditional Hawaiian luau"
        ],
        media: {
            hero_image: "https://example.com/hawaii-beach.jpg",
            gallery: [
                "https://example.com/hawaii-1.jpg",
                "https://example.com/hawaii-2.jpg",
                "https://example.com/hawaii-3.jpg"
            ],
            video_url: "https://example.com/hawaii-tour.mp4"
        },
        is_featured: true,
        is_active: true,
        popularity_score: 95,
        average_rating: 4.7,
        total_reviews: 1243,
        created_at: new Date(),
        updated_at: new Date()
    },
    {
        package_code: "PKG-PARIS-001",
        name: "Romantic Paris Getaway - 5 Nights",
        destination: {
            name: "Paris, France",
            country: "France",
            region: "Europe",
            coordinates: {
                latitude: 48.8566,
                longitude: 2.3522
            },
            timezone: "Europe/Paris"
        },
        description: "Fall in love with the City of Light. This romantic package includes flights, a luxury hotel near the Eiffel Tower, and exclusive experiences including a Seine River cruise and skip-the-line museum passes.",
        package_type: "FLIGHT_HOTEL",
        duration: {
            nights: 5,
            days: 6
        },
        components: {
            flight: {
                included: true,
                cabin_class: "premium_economy",
                baggage: "2 checked bags included"
            },
            hotel: {
                included: true,
                name: "Le Meurice Paris",
                star_rating: 5.0,
                room_type: "Deluxe Room with Eiffel Tower View",
                meals: "Breakfast and dinner included"
            },
            car: {
                included: false
            },
            activities: [
                {
                    name: "Seine River Dinner Cruise",
                    included: true,
                    duration_hours: 3
                },
                {
                    name: "Louvre Museum Skip-the-Line",
                    included: true,
                    duration_hours: 4
                },
                {
                    name: "Versailles Palace Tour",
                    included: false,
                    additional_cost: 95.00
                }
            ]
        },
        pricing: {
            base_price_per_person: 1599.00,
            single_supplement: 550.00,
            child_discount_percent: 15,
            currency: "USD"
        },
        availability: {
            available_from: new Date("2024-03-01"),
            available_to: new Date("2024-11-30"),
            blackout_dates: [
                new Date("2024-07-14"),  // Bastille Day
                new Date("2024-12-25")
            ],
            min_travelers: 1,
            max_travelers: 4
        },
        booking_requirements: {
            min_advance_days: 21,
            max_advance_days: 365,
            cancellation_policy: "Free cancellation up to 21 days before departure"
        },
        highlights: [
            "5-star luxury hotel with Eiffel Tower views",
            "Seine River dinner cruise",
            "Skip-the-line museum access",
            "Gourmet French dining",
            "Walking tour of Montmartre"
        ],
        media: {
            hero_image: "https://example.com/paris-eiffel.jpg",
            gallery: [
                "https://example.com/paris-1.jpg",
                "https://example.com/paris-2.jpg",
                "https://example.com/paris-3.jpg"
            ]
        },
        is_featured: true,
        is_active: true,
        popularity_score: 92,
        average_rating: 4.8,
        total_reviews: 987,
        created_at: new Date(),
        updated_at: new Date()
    },
    {
        package_code: "PKG-CANCUN-001",
        name: "Cancun All-Inclusive Beach Resort",
        destination: {
            name: "Cancun, Mexico",
            country: "Mexico",
            region: "Caribbean",
            coordinates: {
                latitude: 21.1619,
                longitude: -86.8515
            },
            timezone: "America/Cancun"
        },
        description: "Ultimate all-inclusive beach vacation in Cancun's Hotel Zone. Unlimited food, drinks, and activities at a 5-star beachfront resort with crystal-clear Caribbean waters.",
        package_type: "ALL_INCLUSIVE",
        duration: {
            nights: 7,
            days: 8
        },
        components: {
            flight: {
                included: true,
                cabin_class: "economy",
                baggage: "2 checked bags included"
            },
            hotel: {
                included: true,
                name: "Hyatt Ziva Cancun",
                star_rating: 5.0,
                room_type: "Ocean View Suite",
                meals: "All-inclusive: all meals, drinks, and snacks"
            },
            car: {
                included: false
            },
            activities: [
                {
                    name: "Water sports (kayaking, paddleboarding)",
                    included: true,
                    duration_hours: null
                },
                {
                    name: "Nightly entertainment shows",
                    included: true,
                    duration_hours: 2
                },
                {
                    name: "Chichen Itza Mayan Ruins Tour",
                    included: false,
                    additional_cost: 120.00
                },
                {
                    name: "Cenote Swimming Adventure",
                    included: false,
                    additional_cost: 85.00
                }
            ]
        },
        pricing: {
            base_price_per_person: 1899.00,
            single_supplement: 600.00,
            child_discount_percent: 40,
            currency: "USD"
        },
        availability: {
            available_from: new Date("2024-01-01"),
            available_to: new Date("2024-12-31"),
            blackout_dates: [
                new Date("2024-03-15"),  // Spring Break
                new Date("2024-12-24")
            ],
            min_travelers: 1,
            max_travelers: 6
        },
        booking_requirements: {
            min_advance_days: 7,
            max_advance_days: 365,
            cancellation_policy: "Free cancellation up to 7 days before departure"
        },
        highlights: [
            "5-star all-inclusive beachfront resort",
            "Unlimited premium drinks and gourmet dining",
            "Multiple pools and water slides",
            "Kids club and family activities",
            "Spa and fitness center"
        ],
        media: {
            hero_image: "https://example.com/cancun-beach.jpg",
            gallery: [
                "https://example.com/cancun-1.jpg",
                "https://example.com/cancun-2.jpg",
                "https://example.com/cancun-3.jpg"
            ]
        },
        is_featured: true,
        is_active: true,
        popularity_score: 88,
        average_rating: 4.6,
        total_reviews: 1567,
        created_at: new Date(),
        updated_at: new Date()
    }
]);

// Create indexes for efficient querying
db.vacation_packages.createIndex({ package_code: 1 }, { unique: true });
db.vacation_packages.createIndex({ "destination.name": 1 });
db.vacation_packages.createIndex({ package_type: 1 });
db.vacation_packages.createIndex({ is_active: 1, is_featured: -1 });
db.vacation_packages.createIndex({ popularity_score: -1 });
db.vacation_packages.createIndex({ "pricing.base_price_per_person": 1 });
db.vacation_packages.createIndex({ "availability.available_from": 1, "availability.available_to": 1 });

// Text index for search
db.vacation_packages.createIndex({
    name: "text",
    description: "text",
    "destination.name": "text",
    highlights: "text"
});

// Example queries
print("=== Sample Queries ===\n");

// Find all active featured packages
print("Featured packages:");
db.vacation_packages.find(
    { is_active: true, is_featured: true },
    { name: 1, destination: 1, "pricing.base_price_per_person": 1 }
).pretty();

// Find packages under $1500
print("\nPackages under $1500:");
db.vacation_packages.find(
    { "pricing.base_price_per_person": { $lt: 1500 } },
    { name: 1, "pricing.base_price_per_person": 1 }
).pretty();

// Find all-inclusive packages
print("\nAll-inclusive packages:");
db.vacation_packages.find(
    { package_type: "ALL_INCLUSIVE" },
    { name: 1, destination: 1 }
).pretty();

// Text search for "beach"
print("\nPackages mentioning 'beach':");
db.vacation_packages.find(
    { $text: { $search: "beach" } },
    { name: 1, score: { $meta: "textScore" } }
).sort({ score: { $meta: "textScore" } }).pretty();

print("\n=== Vacation packages collection created successfully! ===");
