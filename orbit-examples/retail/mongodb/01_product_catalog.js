// ============================================================================
// OrbitRS Retail Examples - MongoDB Product Catalog
// ============================================================================
// Product images, reviews, marketing content
// ============================================================================

// ============================================================================
// PRODUCT MEDIA COLLECTION
// ============================================================================

db.createCollection("product_media", {
    validator: {
        $jsonSchema: {
            bsonType: "object",
            required: ["product_id", "sku", "media_type"],
            properties: {
                product_id: { bsonType: "string" },
                sku: { bsonType: "string" },
                media_type: { enum: ["IMAGE", "VIDEO", "360_VIEW", "AR_MODEL"] },
                images: {
                    bsonType: "array",
                    items: {
                        bsonType: "object",
                        required: ["url", "type"],
                        properties: {
                            url: { bsonType: "string" },
                            thumbnail_url: { bsonType: "string" },
                            type: { enum: ["MAIN", "GALLERY", "SWATCH", "LIFESTYLE"] },
                            alt_text: { bsonType: "string" },
                            width: { bsonType: "int" },
                            height: { bsonType: "int" },
                            order: { bsonType: "int" }
                        }
                    }
                },
                videos: {
                    bsonType: "array",
                    items: {
                        bsonType: "object",
                        properties: {
                            url: { bsonType: "string" },
                            thumbnail_url: { bsonType: "string" },
                            duration_seconds: { bsonType: "int" },
                            format: { bsonType: "string" }
                        }
                    }
                }
            }
        }
    }
});

// Sample product media
db.product_media.insertMany([
    {
        product_id: "prod-001",
        sku: "SKU-001",
        media_type: "IMAGE",
        images: [
            {
                url: "s3://retail-images/products/SKU-001/main.jpg",
                thumbnail_url: "s3://retail-images/products/SKU-001/main_thumb.jpg",
                type: "MAIN",
                alt_text: "Premium Winter Jacket - Front View",
                width: 2000,
                height: 2000,
                order: 1
            },
            {
                url: "s3://retail-images/products/SKU-001/detail1.jpg",
                type: "GALLERY",
                alt_text: "Premium Winter Jacket - Detail",
                order: 2
            }
        ],
        videos: [
            {
                url: "s3://retail-videos/products/SKU-001/demo.mp4",
                thumbnail_url: "s3://retail-videos/products/SKU-001/demo_thumb.jpg",
                duration_seconds: 30,
                format: "mp4"
            }
        ],
        created_at: new Date()
    }
]);

// ============================================================================
// CUSTOMER REVIEWS COLLECTION
// ============================================================================

db.createCollection("customer_reviews", {
    validator: {
        $jsonSchema: {
            bsonType: "object",
            required: ["product_id", "customer_id", "rating"],
            properties: {
                product_id: { bsonType: "string" },
                customer_id: { bsonType: "string" },
                rating: { bsonType: "int", minimum: 1, maximum: 5 },
                title: { bsonType: "string" },
                review_text: { bsonType: "string" },
                is_verified_purchase: { bsonType: "bool" },
                helpful_votes: { bsonType: "int" },
                images: {
                    bsonType: "array",
                    items: { bsonType: "string" }
                },
                status: { enum: ["PENDING", "APPROVED", "REJECTED"] }
            }
        }
    }
});

// Sample reviews
db.customer_reviews.insertMany([
    {
        product_id: "prod-001",
        customer_id: "cust-001",
        order_id: "order-001",
        rating: 5,
        title: "Excellent quality!",
        review_text: "This jacket exceeded my expectations. Perfect fit and very warm.",
        is_verified_purchase: true,
        helpful_votes: 15,
        images: [
            "s3://retail-ugc/reviews/rev-001-1.jpg",
            "s3://retail-ugc/reviews/rev-001-2.jpg"
        ],
        status: "APPROVED",
        created_at: new Date("2024-12-01"),
        approved_at: new Date("2024-12-02")
    },
    {
        product_id: "prod-001",
        customer_id: "cust-002",
        rating: 4,
        title: "Great jacket, runs a bit large",
        review_text: "Love the quality but I'd recommend sizing down.",
        is_verified_purchase: true,
        helpful_votes: 8,
        status: "APPROVED",
        created_at: new Date("2024-12-03")
    }
]);

// ============================================================================
// MARKETING CAMPAIGNS COLLECTION
// ============================================================================

db.createCollection("marketing_campaigns");

db.marketing_campaigns.insertMany([
    {
        campaign_id: "camp-001",
        campaign_name: "Winter Sale 2024",
        campaign_type: "EMAIL",
        status: "ACTIVE",
        audience_segment: "vip_customers",
        content: {
            subject: "Exclusive Winter Sale - 30% Off",
            html_template: "s3://retail-marketing/templates/winter-sale.html",
            text_template: "s3://retail-marketing/templates/winter-sale.txt",
            images: [
                {
                    url: "s3://retail-marketing/images/winter-banner.jpg",
                    alt: "Winter Sale Banner"
                }
            ]
        },
        schedule: {
            start_date: new Date("2024-12-06"),
            end_date: new Date("2024-12-31"),
            send_time: "09:00"
        },
        metrics: {
            sent: 15420,
            opened: 8934,
            clicked: 2456,
            converted: 487,
            revenue: 48750.00
        },
        created_at: new Date()
    }
]);

// ============================================================================
// PRODUCT LOOKBOOKS (Fast Fashion)
// ============================================================================

db.createCollection("lookbooks");

db.lookbooks.insertMany([
    {
        lookbook_id: "look-001",
        collection_id: "coll-winter-2024",
        title: "Winter Essentials 2024",
        description: "Curated looks for the season",
        cover_image: "s3://retail-lookbooks/winter-2024/cover.jpg",
        looks: [
            {
                look_number: 1,
                title: "Urban Explorer",
                description: "Perfect for city adventures",
                image_url: "s3://retail-lookbooks/winter-2024/look-1.jpg",
                products: [
                    { sku: "SKU-001", product_name: "Winter Jacket" },
                    { sku: "SKU-010", product_name: "Wool Scarf" },
                    { sku: "SKU-015", product_name: "Leather Boots" }
                ]
            },
            {
                look_number: 2,
                title: "Cozy Weekend",
                description: "Comfort meets style",
                image_url: "s3://retail-lookbooks/winter-2024/look-2.jpg",
                products: [
                    { sku: "SKU-020", product_name: "Cashmere Sweater" },
                    { sku: "SKU-025", product_name: "Denim Jeans" }
                ]
            }
        ],
        is_published: true,
        published_at: new Date("2024-11-15"),
        created_at: new Date()
    }
]);

// ============================================================================
// INDEXES
// ============================================================================

// Product media indexes
db.product_media.createIndex({ product_id: 1 });
db.product_media.createIndex({ sku: 1 });

// Customer reviews indexes
db.customer_reviews.createIndex({ product_id: 1, status: 1 });
db.customer_reviews.createIndex({ customer_id: 1 });
db.customer_reviews.createIndex({ rating: 1 });
db.customer_reviews.createIndex({ created_at: -1 });

// Marketing campaigns indexes
db.marketing_campaigns.createIndex({ campaign_id: 1 });
db.marketing_campaigns.createIndex({ status: 1 });
db.marketing_campaigns.createIndex({ "schedule.start_date": 1 });

// Lookbooks indexes
db.lookbooks.createIndex({ collection_id: 1 });
db.lookbooks.createIndex({ is_published: 1 });

// ============================================================================
// AGGREGATION EXAMPLES
// ============================================================================

// Average rating by product
db.customer_reviews.aggregate([
    { $match: { status: "APPROVED" } },
    {
        $group: {
            _id: "$product_id",
            avg_rating: { $avg: "$rating" },
            review_count: { $sum: 1 },
            total_helpful_votes: { $sum: "$helpful_votes" }
        }
    },
    { $sort: { avg_rating: -1 } }
]);

// Campaign performance summary
db.marketing_campaigns.aggregate([
    { $match: { status: "ACTIVE" } },
    {
        $project: {
            campaign_name: 1,
            open_rate: {
                $multiply: [
                    { $divide: ["$metrics.opened", "$metrics.sent"] },
                    100
                ]
            },
            click_rate: {
                $multiply: [
                    { $divide: ["$metrics.clicked", "$metrics.opened"] },
                    100
                ]
            },
            conversion_rate: {
                $multiply: [
                    { $divide: ["$metrics.converted", "$metrics.clicked"] },
                    100
                ]
            },
            revenue: "$metrics.revenue"
        }
    }
]);
