// ============================================================================
// Orbit-RS MongoDB Protocol Examples - Basic CRUD Operations
// ============================================================================
// This file demonstrates basic Create, Read, Update, Delete operations
// using the MongoDB wire protocol with Orbit-RS.
//
// Prerequisites:
// 1. Start Orbit server: cargo run --bin orbit-server
// 2. Run with: mongosh mongodb://localhost:27017 --file 01_basic_crud.js
// ============================================================================

print("=".repeat(80));
print("MongoDB Basic CRUD Operations with Orbit-RS");
print("=".repeat(80));

// Connect to database
const db = connect("mongodb://localhost:27017/orbit_examples");

// ============================================================================
// 1. CREATE OPERATIONS
// ============================================================================

print("\n1. CREATE OPERATIONS");
print("-".repeat(80));

// Drop existing collection for clean start
db.products.drop();

// Insert a single document
print("\n1.1 Insert Single Document:");
const insertResult = db.products.insertOne({
    product_id: "PROD-001",
    name: "Laptop Pro 15",
    category: "Electronics",
    price: 1299.99,
    stock: 50,
    specs: {
        cpu: "Intel i7",
        ram: "16GB",
        storage: "512GB SSD"
    },
    tags: ["laptop", "professional", "high-performance"],
    created_at: new Date()
});

print("Inserted document ID:", insertResult.insertedId);

// Insert multiple documents
print("\n1.2 Insert Multiple Documents:");
const bulkInsertResult = db.products.insertMany([
    {
        product_id: "PROD-002",
        name: "Wireless Mouse",
        category: "Accessories",
        price: 29.99,
        stock: 200,
        specs: {
            type: "Bluetooth",
            battery: "AA x 2"
        },
        tags: ["mouse", "wireless", "ergonomic"],
        created_at: new Date()
    },
    {
        product_id: "PROD-003",
        name: "USB-C Hub",
        category: "Accessories",
        price: 49.99,
        stock: 150,
        specs: {
            ports: 7,
            power_delivery: "100W"
        },
        tags: ["hub", "usb-c", "multiport"],
        created_at: new Date()
    },
    {
        product_id: "PROD-004",
        name: "4K Monitor",
        category: "Electronics",
        price: 599.99,
        stock: 30,
        specs: {
            size: "27 inch",
            resolution: "3840x2160",
            refresh_rate: "60Hz"
        },
        tags: ["monitor", "4k", "display"],
        created_at: new Date()
    },
    {
        product_id: "PROD-005",
        name: "Mechanical Keyboard",
        category: "Accessories",
        price: 149.99,
        stock: 75,
        specs: {
            switch_type: "Cherry MX Blue",
            backlight: "RGB",
            layout: "Full-size"
        },
        tags: ["keyboard", "mechanical", "gaming"],
        created_at: new Date()
    }
]);

print("Inserted", bulkInsertResult.insertedIds.length, "documents");

// ============================================================================
// 2. READ OPERATIONS
// ============================================================================

print("\n\n2. READ OPERATIONS");
print("-".repeat(80));

// Find all documents
print("\n2.1 Find All Products:");
db.products.find().forEach(doc => {
    print(`  - ${doc.name} (${doc.category}): $${doc.price}`);
});

// Find with filter
print("\n2.2 Find Electronics:");
db.products.find({ category: "Electronics" }).forEach(doc => {
    print(`  - ${doc.name}: $${doc.price}`);
});

// Find with comparison operators
print("\n2.3 Find Products Under $100:");
db.products.find({ price: { $lt: 100 } }).forEach(doc => {
    print(`  - ${doc.name}: $${doc.price}`);
});

// Find with logical operators
print("\n2.4 Find Accessories Over $50:");
db.products.find({
    $and: [
        { category: "Accessories" },
        { price: { $gt: 50 } }
    ]
}).forEach(doc => {
    print(`  - ${doc.name}: $${doc.price}`);
});

// Find with array operators
print("\n2.5 Find Products with 'wireless' Tag:");
db.products.find({ tags: "wireless" }).forEach(doc => {
    print(`  - ${doc.name}: ${doc.tags.join(", ")}`);
});

// Find with nested field query
print("\n2.6 Find Products with Specific Specs:");
db.products.find({ "specs.type": "Bluetooth" }).forEach(doc => {
    print(`  - ${doc.name}: ${doc.specs.type}`);
});

// Find one document
print("\n2.7 Find One Product:");
const oneProduct = db.products.findOne({ product_id: "PROD-001" });
print(`  Found: ${oneProduct.name}`);

// Find with projection (select specific fields)
print("\n2.8 Find with Projection (name and price only):");
db.products.find({}, { name: 1, price: 1, _id: 0 }).forEach(doc => {
    print(`  - ${doc.name}: $${doc.price}`);
});

// Count documents
print("\n2.9 Count Documents:");
const totalCount = db.products.countDocuments();
const electronicsCount = db.products.countDocuments({ category: "Electronics" });
print(`  Total products: ${totalCount}`);
print(`  Electronics: ${electronicsCount}`);

// Sort and limit
print("\n2.10 Top 3 Most Expensive Products:");
db.products.find().sort({ price: -1 }).limit(3).forEach(doc => {
    print(`  - ${doc.name}: $${doc.price}`);
});

// ============================================================================
// 3. UPDATE OPERATIONS
// ============================================================================

print("\n\n3. UPDATE OPERATIONS");
print("-".repeat(80));

// Update one document
print("\n3.1 Update Single Document (Increase Price):");
const updateOneResult = db.products.updateOne(
    { product_id: "PROD-002" },
    {
        $set: { price: 34.99 },
        $currentDate: { updated_at: true }
    }
);
print(`  Matched: ${updateOneResult.matchedCount}, Modified: ${updateOneResult.modifiedCount}`);

// Verify update
const updatedProduct = db.products.findOne({ product_id: "PROD-002" });
print(`  New price: $${updatedProduct.price}`);

// Update multiple documents
print("\n3.2 Update Multiple Documents (Discount on Accessories):");
const updateManyResult = db.products.updateMany(
    { category: "Accessories" },
    {
        $mul: { price: 0.9 },  // 10% discount
        $set: { on_sale: true }
    }
);
print(`  Matched: ${updateManyResult.matchedCount}, Modified: ${updateManyResult.modifiedCount}`);

// Increment stock
print("\n3.3 Increment Stock:");
db.products.updateOne(
    { product_id: "PROD-001" },
    { $inc: { stock: 25 } }
);
const stockUpdated = db.products.findOne({ product_id: "PROD-001" });
print(`  New stock for ${stockUpdated.name}: ${stockUpdated.stock}`);

// Add to array
print("\n3.4 Add Tag to Product:");
db.products.updateOne(
    { product_id: "PROD-001" },
    { $push: { tags: "bestseller" } }
);
const tagUpdated = db.products.findOne({ product_id: "PROD-001" });
print(`  Tags: ${tagUpdated.tags.join(", ")}`);

// Remove from array
print("\n3.5 Remove Tag from Product:");
db.products.updateOne(
    { product_id: "PROD-001" },
    { $pull: { tags: "high-performance" } }
);
const tagRemoved = db.products.findOne({ product_id: "PROD-001" });
print(`  Tags: ${tagRemoved.tags.join(", ")}`);

// Upsert (update or insert)
print("\n3.6 Upsert (Insert if Not Exists):");
const upsertResult = db.products.updateOne(
    { product_id: "PROD-006" },
    {
        $set: {
            name: "Webcam HD",
            category: "Accessories",
            price: 79.99,
            stock: 100,
            created_at: new Date()
        }
    },
    { upsert: true }
);
print(`  Upserted: ${upsertResult.upsertedCount > 0 ? "Yes" : "No"}`);
if (upsertResult.upsertedId) {
    print(`  Upserted ID: ${upsertResult.upsertedId}`);
}

// Replace entire document
print("\n3.7 Replace Document:");
db.products.replaceOne(
    { product_id: "PROD-006" },
    {
        product_id: "PROD-006",
        name: "Webcam HD Pro",
        category: "Accessories",
        price: 89.99,
        stock: 100,
        specs: {
            resolution: "1080p",
            fps: 60,
            autofocus: true
        },
        tags: ["webcam", "hd", "streaming"],
        created_at: new Date()
    }
);
print("  Document replaced");

// ============================================================================
// 4. DELETE OPERATIONS
// ============================================================================

print("\n\n4. DELETE OPERATIONS");
print("-".repeat(80));

// Delete one document
print("\n4.1 Delete Single Document:");
const deleteOneResult = db.products.deleteOne({ product_id: "PROD-006" });
print(`  Deleted: ${deleteOneResult.deletedCount} document(s)`);

// Insert some test documents for deletion
db.products.insertMany([
    { product_id: "TEMP-001", name: "Test Product 1", category: "Test", price: 10 },
    { product_id: "TEMP-002", name: "Test Product 2", category: "Test", price: 20 },
    { product_id: "TEMP-003", name: "Test Product 3", category: "Test", price: 30 }
]);

// Delete multiple documents
print("\n4.2 Delete Multiple Documents:");
const deleteManyResult = db.products.deleteMany({ category: "Test" });
print(`  Deleted: ${deleteManyResult.deletedCount} document(s)`);

// ============================================================================
// 5. ADVANCED QUERIES
// ============================================================================

print("\n\n5. ADVANCED QUERIES");
print("-".repeat(80));

// Regular expression search
print("\n5.1 Regex Search (Products with 'Pro' in name):");
db.products.find({ name: /Pro/i }).forEach(doc => {
    print(`  - ${doc.name}`);
});

// Exists operator
print("\n5.2 Products with 'on_sale' Field:");
db.products.find({ on_sale: { $exists: true } }).forEach(doc => {
    print(`  - ${doc.name}: ${doc.on_sale ? "On Sale" : "Regular Price"}`);
});

// Type operator
print("\n5.3 Products Where Price is a Number:");
const priceCount = db.products.countDocuments({ price: { $type: "double" } });
print(`  Count: ${priceCount}`);

// In operator
print("\n5.4 Products in Specific Categories:");
db.products.find({
    category: { $in: ["Electronics", "Accessories"] }
}).forEach(doc => {
    print(`  - ${doc.name} (${doc.category})`);
});

// ============================================================================
// 6. SUMMARY
// ============================================================================

print("\n\n6. SUMMARY");
print("-".repeat(80));

const finalCount = db.products.countDocuments();
print(`Total products in collection: ${finalCount}`);

print("\nProducts by category:");
db.products.aggregate([
    { $group: { _id: "$category", count: { $sum: 1 } } },
    { $sort: { count: -1 } }
]).forEach(doc => {
    print(`  - ${doc._id}: ${doc.count}`);
});

print("\nPrice statistics:");
const stats = db.products.aggregate([
    {
        $group: {
            _id: null,
            avgPrice: { $avg: "$price" },
            minPrice: { $min: "$price" },
            maxPrice: { $max: "$price" },
            totalValue: { $sum: { $multiply: ["$price", "$stock"] } }
        }
    }
]).toArray()[0];

print(`  Average price: $${stats.avgPrice.toFixed(2)}`);
print(`  Min price: $${stats.minPrice.toFixed(2)}`);
print(`  Max price: $${stats.maxPrice.toFixed(2)}`);
print(`  Total inventory value: $${stats.totalValue.toFixed(2)}`);

print("\n" + "=".repeat(80));
print("MongoDB Basic CRUD Operations Complete!");
print("=".repeat(80));
