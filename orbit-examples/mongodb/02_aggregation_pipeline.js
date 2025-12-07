// ============================================================================
// Orbit-RS MongoDB Protocol Examples - Aggregation Pipeline
// ============================================================================
// This file demonstrates the MongoDB aggregation framework with Orbit-RS.
// Covers stages like $match, $group, $project, $lookup, $unwind, and more.
//
// Prerequisites:
// 1. Start Orbit server: cargo run --bin orbit-server
// 2. Run with: mongosh mongodb://localhost:27017 --file 02_aggregation_pipeline.js
// ============================================================================

print("=".repeat(80));
print("MongoDB Aggregation Pipeline with Orbit-RS");
print("=".repeat(80));

const db = connect("mongodb://localhost:27017/orbit_examples");

// ============================================================================
// SETUP: Create Sample Data
// ============================================================================

print("\nSETUP: Creating sample data...");

// Drop existing collections
db.orders.drop();
db.customers.drop();
db.products.drop();

// Insert customers
db.customers.insertMany([
    { customer_id: "C001", name: "Alice Johnson", email: "alice@example.com", city: "New York", state: "NY", segment: "Premium" },
    { customer_id: "C002", name: "Bob Smith", email: "bob@example.com", city: "Los Angeles", state: "CA", segment: "Standard" },
    { customer_id: "C003", name: "Carol White", email: "carol@example.com", city: "Chicago", state: "IL", segment: "Premium" },
    { customer_id: "C004", name: "David Brown", email: "david@example.com", city: "Houston", state: "TX", segment: "Standard" },
    { customer_id: "C005", name: "Eve Davis", email: "eve@example.com", city: "Phoenix", state: "AZ", segment: "Premium" }
]);

// Insert products
db.products.insertMany([
    { product_id: "P001", name: "Laptop Pro", category: "Electronics", price: 1299.99, cost: 800 },
    { product_id: "P002", name: "Wireless Mouse", category: "Accessories", price: 29.99, cost: 15 },
    { product_id: "P003", name: "USB-C Hub", category: "Accessories", price: 49.99, cost: 25 },
    { product_id: "P004", name: "4K Monitor", category: "Electronics", price: 599.99, cost: 350 },
    { product_id: "P005", name: "Keyboard", category: "Accessories", price: 149.99, cost: 80 }
]);

// Insert orders
db.orders.insertMany([
    { order_id: "O001", customer_id: "C001", product_id: "P001", quantity: 1, order_date: new Date("2024-01-15"), status: "delivered" },
    { order_id: "O002", customer_id: "C001", product_id: "P002", quantity: 2, order_date: new Date("2024-01-16"), status: "delivered" },
    { order_id: "O003", customer_id: "C002", product_id: "P004", quantity: 1, order_date: new Date("2024-01-20"), status: "delivered" },
    { order_id: "O004", customer_id: "C003", product_id: "P001", quantity: 1, order_date: new Date("2024-02-01"), status: "shipped" },
    { order_id: "O005", customer_id: "C003", product_id: "P003", quantity: 3, order_date: new Date("2024-02-05"), status: "delivered" },
    { order_id: "O006", customer_id: "C004", product_id: "P005", quantity: 1, order_date: new Date("2024-02-10"), status: "delivered" },
    { order_id: "O007", customer_id: "C005", product_id: "P002", quantity: 5, order_date: new Date("2024-02-15"), status: "processing" },
    { order_id: "O008", customer_id: "C001", product_id: "P004", quantity: 2, order_date: new Date("2024-03-01"), status: "delivered" },
    { order_id: "O009", customer_id: "C002", product_id: "P003", quantity: 1, order_date: new Date("2024-03-05"), status: "delivered" },
    { order_id: "O010", customer_id: "C003", product_id: "P005", quantity: 2, order_date: new Date("2024-03-10"), status: "shipped" }
]);

print("Sample data created successfully!\n");

// ============================================================================
// 1. BASIC AGGREGATION - $match and $group
// ============================================================================

print("\n1. BASIC AGGREGATION");
print("-".repeat(80));

print("\n1.1 Total Orders by Status:");
db.orders.aggregate([
    { $group: { _id: "$status", count: { $sum: 1 } } },
    { $sort: { count: -1 } }
]).forEach(doc => {
    print(`  ${doc._id}: ${doc.count} orders`);
});

print("\n1.2 Total Quantity Sold per Product:");
db.orders.aggregate([
    { $group: { _id: "$product_id", total_quantity: { $sum: "$quantity" } } },
    { $sort: { total_quantity: -1 } }
]).forEach(doc => {
    print(`  ${doc._id}: ${doc.total_quantity} units`);
});

// ============================================================================
// 2. AGGREGATION WITH $lookup (JOIN)
// ============================================================================

print("\n\n2. AGGREGATION WITH $lookup (JOIN)");
print("-".repeat(80));

print("\n2.1 Orders with Customer Details:");
db.orders.aggregate([
    {
        $lookup: {
            from: "customers",
            localField: "customer_id",
            foreignField: "customer_id",
            as: "customer_info"
        }
    },
    { $unwind: "$customer_info" },
    {
        $project: {
            order_id: 1,
            customer_name: "$customer_info.name",
            product_id: 1,
            quantity: 1,
            status: 1
        }
    },
    { $limit: 5 }
]).forEach(doc => {
    print(`  Order ${doc.order_id}: ${doc.customer_name} - ${doc.quantity}x ${doc.product_id} (${doc.status})`);
});

print("\n2.2 Orders with Product and Customer Details:");
db.orders.aggregate([
    {
        $lookup: {
            from: "products",
            localField: "product_id",
            foreignField: "product_id",
            as: "product_info"
        }
    },
    { $unwind: "$product_info" },
    {
        $lookup: {
            from: "customers",
            localField: "customer_id",
            foreignField: "customer_id",
            as: "customer_info"
        }
    },
    { $unwind: "$customer_info" },
    {
        $project: {
            order_id: 1,
            customer: "$customer_info.name",
            product: "$product_info.name",
            quantity: 1,
            unit_price: "$product_info.price",
            total: { $multiply: ["$quantity", "$product_info.price"] }
        }
    },
    { $limit: 5 }
]).forEach(doc => {
    print(`  ${doc.order_id}: ${doc.customer} bought ${doc.quantity}x ${doc.product} = $${doc.total.toFixed(2)}`);
});

// ============================================================================
// 3. ADVANCED GROUPING AND CALCULATIONS
// ============================================================================

print("\n\n3. ADVANCED GROUPING AND CALCULATIONS");
print("-".repeat(80));

print("\n3.1 Revenue by Product Category:");
db.orders.aggregate([
    {
        $lookup: {
            from: "products",
            localField: "product_id",
            foreignField: "product_id",
            as: "product"
        }
    },
    { $unwind: "$product" },
    {
        $group: {
            _id: "$product.category",
            total_revenue: { $sum: { $multiply: ["$quantity", "$product.price"] } },
            total_orders: { $sum: 1 },
            avg_order_value: { $avg: { $multiply: ["$quantity", "$product.price"] } }
        }
    },
    { $sort: { total_revenue: -1 } }
]).forEach(doc => {
    print(`  ${doc._id}:`);
    print(`    Revenue: $${doc.total_revenue.toFixed(2)}`);
    print(`    Orders: ${doc.total_orders}`);
    print(`    Avg Order: $${doc.avg_order_value.toFixed(2)}`);
});

print("\n3.2 Customer Lifetime Value:");
db.orders.aggregate([
    {
        $lookup: {
            from: "products",
            localField: "product_id",
            foreignField: "product_id",
            as: "product"
        }
    },
    { $unwind: "$product" },
    {
        $group: {
            _id: "$customer_id",
            total_spent: { $sum: { $multiply: ["$quantity", "$product.price"] } },
            order_count: { $sum: 1 },
            avg_order_value: { $avg: { $multiply: ["$quantity", "$product.price"] } }
        }
    },
    {
        $lookup: {
            from: "customers",
            localField: "_id",
            foreignField: "customer_id",
            as: "customer"
        }
    },
    { $unwind: "$customer" },
    {
        $project: {
            customer_name: "$customer.name",
            segment: "$customer.segment",
            total_spent: 1,
            order_count: 1,
            avg_order_value: 1
        }
    },
    { $sort: { total_spent: -1 } }
]).forEach(doc => {
    print(`  ${doc.customer_name} (${doc.segment}):`);
    print(`    Total Spent: $${doc.total_spent.toFixed(2)}`);
    print(`    Orders: ${doc.order_count}`);
    print(`    Avg Order: $${doc.avg_order_value.toFixed(2)}`);
});

// ============================================================================
// 4. TIME-BASED AGGREGATION
// ============================================================================

print("\n\n4. TIME-BASED AGGREGATION");
print("-".repeat(80));

print("\n4.1 Monthly Order Summary:");
db.orders.aggregate([
    {
        $group: {
            _id: {
                year: { $year: "$order_date" },
                month: { $month: "$order_date" }
            },
            order_count: { $sum: 1 },
            total_items: { $sum: "$quantity" }
        }
    },
    { $sort: { "_id.year": 1, "_id.month": 1 } }
]).forEach(doc => {
    print(`  ${doc._id.year}-${String(doc._id.month).padStart(2, '0')}: ${doc.order_count} orders, ${doc.total_items} items`);
});

// ============================================================================
// 5. CONDITIONAL AGGREGATION
// ============================================================================

print("\n\n5. CONDITIONAL AGGREGATION");
print("-".repeat(80));

print("\n5.1 Order Status Distribution:");
db.orders.aggregate([
    {
        $group: {
            _id: null,
            total: { $sum: 1 },
            delivered: { $sum: { $cond: [{ $eq: ["$status", "delivered"] }, 1, 0] } },
            shipped: { $sum: { $cond: [{ $eq: ["$status", "shipped"] }, 1, 0] } },
            processing: { $sum: { $cond: [{ $eq: ["$status", "processing"] }, 1, 0] } }
        }
    }
]).forEach(doc => {
    print(`  Total Orders: ${doc.total}`);
    print(`  Delivered: ${doc.delivered} (${(doc.delivered / doc.total * 100).toFixed(1)}%)`);
    print(`  Shipped: ${doc.shipped} (${(doc.shipped / doc.total * 100).toFixed(1)}%)`);
    print(`  Processing: ${doc.processing} (${(doc.processing / doc.total * 100).toFixed(1)}%)`);
});

// ============================================================================
// 6. FACETED AGGREGATION
// ============================================================================

print("\n\n6. FACETED AGGREGATION");
print("-".repeat(80));

print("\n6.1 Multi-Faceted Analysis:");
const facets = db.orders.aggregate([
    {
        $facet: {
            "by_status": [
                { $group: { _id: "$status", count: { $sum: 1 } } }
            ],
            "by_customer": [
                { $group: { _id: "$customer_id", orders: { $sum: 1 } } },
                { $sort: { orders: -1 } },
                { $limit: 3 }
            ],
            "by_product": [
                { $group: { _id: "$product_id", quantity: { $sum: "$quantity" } } },
                { $sort: { quantity: -1 } },
                { $limit: 3 }
            ]
        }
    }
]).toArray()[0];

print("  By Status:");
facets.by_status.forEach(doc => {
    print(`    ${doc._id}: ${doc.count}`);
});

print("  Top Customers:");
facets.by_customer.forEach(doc => {
    print(`    ${doc._id}: ${doc.orders} orders`);
});

print("  Top Products:");
facets.by_product.forEach(doc => {
    print(`    ${doc._id}: ${doc.quantity} units`);
});

// ============================================================================
// 7. BUCKET AGGREGATION
// ============================================================================

print("\n\n7. BUCKET AGGREGATION");
print("-".repeat(80));

print("\n7.1 Orders by Quantity Buckets:");
db.orders.aggregate([
    {
        $bucket: {
            groupBy: "$quantity",
            boundaries: [1, 2, 3, 5, 10],
            default: "10+",
            output: {
                count: { $sum: 1 },
                orders: { $push: "$order_id" }
            }
        }
    }
]).forEach(doc => {
    print(`  Quantity ${doc._id}: ${doc.count} orders`);
});

// ============================================================================
// 8. PROFIT ANALYSIS
// ============================================================================

print("\n\n8. PROFIT ANALYSIS");
print("-".repeat(80));

print("\n8.1 Profit by Product:");
db.orders.aggregate([
    {
        $lookup: {
            from: "products",
            localField: "product_id",
            foreignField: "product_id",
            as: "product"
        }
    },
    { $unwind: "$product" },
    {
        $group: {
            _id: "$product_id",
            product_name: { $first: "$product.name" },
            revenue: { $sum: { $multiply: ["$quantity", "$product.price"] } },
            cost: { $sum: { $multiply: ["$quantity", "$product.cost"] } }
        }
    },
    {
        $project: {
            product_name: 1,
            revenue: 1,
            cost: 1,
            profit: { $subtract: ["$revenue", "$cost"] },
            margin: {
                $multiply: [
                    { $divide: [{ $subtract: ["$revenue", "$cost"] }, "$revenue"] },
                    100
                ]
            }
        }
    },
    { $sort: { profit: -1 } }
]).forEach(doc => {
    print(`  ${doc.product_name}:`);
    print(`    Revenue: $${doc.revenue.toFixed(2)}`);
    print(`    Cost: $${doc.cost.toFixed(2)}`);
    print(`    Profit: $${doc.profit.toFixed(2)} (${doc.margin.toFixed(1)}% margin)`);
});

// ============================================================================
// 9. SUMMARY
// ============================================================================

print("\n\n9. SUMMARY");
print("-".repeat(80));

const summary = db.orders.aggregate([
    {
        $lookup: {
            from: "products",
            localField: "product_id",
            foreignField: "product_id",
            as: "product"
        }
    },
    { $unwind: "$product" },
    {
        $group: {
            _id: null,
            total_orders: { $sum: 1 },
            total_items: { $sum: "$quantity" },
            total_revenue: { $sum: { $multiply: ["$quantity", "$product.price"] } },
            avg_order_value: { $avg: { $multiply: ["$quantity", "$product.price"] } }
        }
    }
]).toArray()[0];

print(`Total Orders: ${summary.total_orders}`);
print(`Total Items Sold: ${summary.total_items}`);
print(`Total Revenue: $${summary.total_revenue.toFixed(2)}`);
print(`Average Order Value: $${summary.avg_order_value.toFixed(2)}`);

print("\n" + "=".repeat(80));
print("MongoDB Aggregation Pipeline Complete!");
print("=".repeat(80));
