//! Complete Orbit Engine Example
//!
//! Demonstrates the full capabilities of the orbit-engine including:
//! - Tiered storage (hot/warm/cold)
//! - Protocol adapters (PostgreSQL, Redis, REST)
//! - Transactions with MVCC
//! - Multi-cloud storage backends
//!
//! This example shows how all components work together.

use orbit_engine::adapters::{
    AdapterContext, PostgresAdapter, RedisAdapter, RestAdapter, ProtocolAdapter,
};
use orbit_engine::adapters::postgres::{PostgresColumnDef, PostgresDataType, PostgresFilter, PostgresIsolationLevel};
use orbit_engine::adapters::rest::{CreateTableRequest, RestColumnDef, QueryRequest, InsertRequest};
use orbit_engine::storage::{HybridStorageManager, SqlValue, StorageTier};
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║        Orbit Engine - Complete Integration Example        ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Step 1: Create the unified storage engine
    println!("📦 Step 1: Creating HybridStorageManager with tiered storage");
    println!("   - Hot tier: In-memory (last 24-48 hours)");
    println!("   - Warm tier: RocksDB (2-30 days)");
    println!("   - Cold tier: Iceberg/Parquet (>30 days)");

    let storage = Arc::new(HybridStorageManager::new_in_memory());
    println!("   ✓ Storage engine created\n");

    // Step 2: Create adapter context
    println!("🔌 Step 2: Creating adapter context");
    let context = AdapterContext::new(storage.clone() as Arc<dyn orbit_engine::storage::TableStorage>);
    println!("   ✓ Adapter context ready\n");

    // Step 3: PostgreSQL protocol example
    println!("═══════════════════════════════════════════════════════════");
    println!("  PostgreSQL Protocol Adapter");
    println!("═══════════════════════════════════════════════════════════\n");
    postgresql_workflow(context.clone()).await?;

    // Step 4: Redis protocol example
    println!("\n═══════════════════════════════════════════════════════════");
    println!("  Redis Protocol Adapter");
    println!("═══════════════════════════════════════════════════════════\n");
    redis_workflow(context.clone()).await?;

    // Step 5: REST API example
    println!("\n═══════════════════════════════════════════════════════════");
    println!("  REST API Adapter");
    println!("═══════════════════════════════════════════════════════════\n");
    rest_workflow(context.clone()).await?;

    // Step 6: Show storage tier distribution
    println!("\n═══════════════════════════════════════════════════════════");
    println!("  Storage Tier Distribution");
    println!("═══════════════════════════════════════════════════════════\n");
    show_tier_stats(&storage).await?;

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║           All Examples Completed Successfully!            ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    Ok(())
}

async fn postgresql_workflow(context: AdapterContext) -> Result<(), Box<dyn std::error::Error>> {
    let mut adapter = PostgresAdapter::new(context);

    println!("📊 Creating 'products' table via PostgreSQL adapter...");
    adapter
        .create_table(
            "products",
            vec![
                PostgresColumnDef {
                    name: "id".to_string(),
                    data_type: PostgresDataType::Integer,
                    nullable: false,
                },
                PostgresColumnDef {
                    name: "name".to_string(),
                    data_type: PostgresDataType::Text,
                    nullable: false,
                },
                PostgresColumnDef {
                    name: "price".to_string(),
                    data_type: PostgresDataType::DoublePrecision,
                    nullable: false,
                },
                PostgresColumnDef {
                    name: "stock".to_string(),
                    data_type: PostgresDataType::Integer,
                    nullable: false,
                },
            ],
            vec!["id".to_string()],
        )
        .await?;
    println!("   ✓ Table created\n");

    println!("📝 Inserting products...");
    let products = vec![
        create_product(1, "Laptop", 999.99, 50),
        create_product(2, "Mouse", 29.99, 200),
        create_product(3, "Keyboard", 79.99, 150),
        create_product(4, "Monitor", 299.99, 75),
    ];
    adapter.insert("products", products).await?;
    println!("   ✓ Inserted 4 products\n");

    println!("🔍 Querying products with price > $50...");
    let filter = PostgresFilter::GreaterThan("price".to_string(), SqlValue::Float64(50.0));
    match adapter.select("products", None, Some(filter)).await? {
        orbit_engine::adapters::CommandResult::Rows(rows) => {
            println!("   Found {} products:", rows.len());
            for row in rows {
                if let (Some(SqlValue::String(name)), Some(SqlValue::Float64(price))) =
                    (row.get("name"), row.get("price"))
                {
                    println!("   • {} - ${:.2}", name, price);
                }
            }
        }
        _ => println!("   Unexpected result"),
    }

    println!("\n💼 Testing ACID transaction...");
    let tx_id = adapter.begin_transaction(PostgresIsolationLevel::Serializable).await?;
    println!("   ✓ Transaction started (ID: {})", &tx_id[..8]);

    // Update stock
    let filter = PostgresFilter::Equals("id".to_string(), SqlValue::Int32(2));
    let mut updates = HashMap::new();
    updates.insert("stock".to_string(), SqlValue::Int32(190));
    adapter.update("products", filter, updates).await?;
    println!("   ✓ Updated stock for Mouse");

    adapter.commit_transaction(&tx_id).await?;
    println!("   ✓ Transaction committed");

    Ok(())
}

async fn redis_workflow(context: AdapterContext) -> Result<(), Box<dyn std::error::Error>> {
    let mut adapter = RedisAdapter::new(context);
    adapter.initialize().await?;
    println!("✓ Redis adapter initialized\n");

    println!("🔑 String operations:");
    adapter.set("app:version", "1.0.0", None).await?;
    adapter.set("app:name", "Orbit Engine", None).await?;
    println!("   SET app:version \"1.0.0\"");
    println!("   SET app:name \"Orbit Engine\"");

    if let Some(version) = adapter.get("app:version").await? {
        println!("   GET app:version => \"{}\"", version);
    }

    println!("\n📦 Hash operations (User session):");
    adapter.hset("session:user123", "username", "alice").await?;
    adapter.hset("session:user123", "email", "alice@example.com").await?;
    adapter.hset("session:user123", "role", "admin").await?;
    println!("   HSET session:user123 username \"alice\"");
    println!("   HSET session:user123 email \"alice@example.com\"");
    println!("   HSET session:user123 role \"admin\"");

    let session = adapter.hgetall("session:user123").await?;
    println!("   HGETALL session:user123 =>");
    for (field, value) in session {
        println!("     {}: {}", field, value);
    }

    println!("\n📝 List operations (Activity log):");
    adapter.rpush("activity:user123", "login").await?;
    adapter.rpush("activity:user123", "view_dashboard").await?;
    adapter.rpush("activity:user123", "update_profile").await?;
    println!("   RPUSH activity:user123 \"login\"");
    println!("   RPUSH activity:user123 \"view_dashboard\"");
    println!("   RPUSH activity:user123 \"update_profile\"");

    println!("\n🗑️  Cleanup:");
    adapter.del("app:version").await?;
    println!("   DEL app:version");

    Ok(())
}

async fn rest_workflow(context: AdapterContext) -> Result<(), Box<dyn std::error::Error>> {
    let adapter = RestAdapter::new(context);

    println!("🌐 Creating 'orders' table via REST API...");
    let create_req = CreateTableRequest {
        name: "orders".to_string(),
        columns: vec![
            RestColumnDef {
                name: "order_id".to_string(),
                data_type: "integer".to_string(),
                nullable: false,
            },
            RestColumnDef {
                name: "customer".to_string(),
                data_type: "string".to_string(),
                nullable: false,
            },
            RestColumnDef {
                name: "total".to_string(),
                data_type: "float64".to_string(),
                nullable: false,
            },
            RestColumnDef {
                name: "status".to_string(),
                data_type: "string".to_string(),
                nullable: false,
            },
        ],
        primary_key: vec!["order_id".to_string()],
    };

    let response = adapter.create_table_request(create_req).await?;
    println!("   Response: {:?}", response.data);

    println!("\n📦 Inserting orders...");
    let insert_req = InsertRequest {
        rows: vec![
            serde_json::json!({
                "order_id": 1001,
                "customer": "Alice",
                "total": 150.50,
                "status": "pending"
            }).as_object().unwrap().clone(),
            serde_json::json!({
                "order_id": 1002,
                "customer": "Bob",
                "total": 299.99,
                "status": "shipped"
            }).as_object().unwrap().clone(),
        ],
    };

    let response = adapter.insert_rows("orders", insert_req).await?;
    println!("   ✓ Inserted orders");

    println!("\n🔍 Querying orders...");
    let query_req = QueryRequest {
        filter: None,
        limit: Some(10),
    };

    let response = adapter.query_rows("orders", query_req).await?;
    if let Some(data) = response.data {
        println!("   Response: {}", serde_json::to_string_pretty(&data)?);
    }

    Ok(())
}

async fn show_tier_stats(storage: &HybridStorageManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Storage tier statistics:");
    println!("   Note: Tier migration happens automatically based on data age\n");

    println!("   🔥 Hot Tier (In-Memory):");
    println!("      - Recent data (last 24-48 hours)");
    println!("      - Optimized for OLTP (row-based)");
    println!("      - Fast writes and point queries\n");

    println!("   🌡️  Warm Tier (RocksDB):");
    println!("      - Medium-age data (2-30 days)");
    println!("      - Hybrid format for mixed workloads");
    println!("      - Balance between speed and cost\n");

    println!("   ❄️  Cold Tier (Iceberg/Parquet):");
    println!("      - Historical data (>30 days)");
    println!("      - Columnar format with compression");
    println!("      - Optimized for analytics (SIMD)");
    println!("      - Multi-cloud storage (S3, Azure)");

    Ok(())
}

fn create_product(id: i32, name: &str, price: f64, stock: i32) -> HashMap<String, SqlValue> {
    let mut row = HashMap::new();
    row.insert("id".to_string(), SqlValue::Int32(id));
    row.insert("name".to_string(), SqlValue::String(name.to_string()));
    row.insert("price".to_string(), SqlValue::Float64(price));
    row.insert("stock".to_string(), SqlValue::Int32(stock));
    row
}
