//! Protocol Adapters Example
//!
//! This example demonstrates how to use the protocol adapters to bridge
//! different protocols (PostgreSQL, Redis) to the unified orbit-engine.


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Orbit Engine Protocol Adapters Example ===\n");

    // TODO: This example needs to be updated to use a storage backend that implements TableStorage
    // HybridStorageManager does not implement TableStorage trait.
    // Once MemoryTableStorage is re-enabled or HybridStorageManager implements TableStorage,
    // this example can be uncommented.

    println!("⚠️  This example is currently disabled - storage backend needs updating");
    println!("    HybridStorageManager does not implement TableStorage trait");
    return Ok(());

    // Create the storage engine
    /*
    println!("1. Creating HybridStorageManager...");
    let storage = Arc::new(HybridStorageManager::new(
        "example_table".to_string(),
        vec![],
        orbit_engine::storage::HybridStorageConfig::default(),
    ));

    // Create adapter context
    let context = AdapterContext::new(storage.clone() as Arc<dyn orbit_engine::storage::TableStorage>);

    // Example 1: PostgreSQL Adapter
    println!("\n=== PostgreSQL Adapter Example ===");
    postgres_example(context.clone()).await?;

    // Example 2: Redis Adapter
    println!("\n=== Redis Adapter Example ===");
    redis_example(context.clone()).await?;

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

async fn postgres_example(context: AdapterContext) -> Result<(), Box<dyn std::error::Error>> {
    let adapter = PostgresAdapter::new(context);

    // Create a table
    println!("Creating 'users' table...");
    adapter
        .create_table(
            "users",
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
                    name: "email".to_string(),
                    data_type: PostgresDataType::Varchar,
                    nullable: true,
                },
                PostgresColumnDef {
                    name: "age".to_string(),
                    data_type: PostgresDataType::Integer,
                    nullable: true,
                },
            ],
            vec!["id".to_string()],
        )
        .await?;
    println!("✓ Table created successfully");

    // Insert some data
    println!("\nInserting users...");
    let users = vec![
        {
            let mut row = HashMap::new();
            row.insert("id".to_string(), SqlValue::Int32(1));
            row.insert("name".to_string(), SqlValue::String("Alice".to_string()));
            row.insert("email".to_string(), SqlValue::String("alice@example.com".to_string()));
            row.insert("age".to_string(), SqlValue::Int32(30));
            row
        },
        {
            let mut row = HashMap::new();
            row.insert("id".to_string(), SqlValue::Int32(2));
            row.insert("name".to_string(), SqlValue::String("Bob".to_string()));
            row.insert("email".to_string(), SqlValue::String("bob@example.com".to_string()));
            row.insert("age".to_string(), SqlValue::Int32(25));
            row
        },
        {
            let mut row = HashMap::new();
            row.insert("id".to_string(), SqlValue::Int32(3));
            row.insert("name".to_string(), SqlValue::String("Charlie".to_string()));
            row.insert("email".to_string(), SqlValue::String("charlie@example.com".to_string()));
            row.insert("age".to_string(), SqlValue::Int32(35));
            row
        },
    ];

    adapter.insert("users", users).await?;
    println!("✓ Inserted 3 users");

    // Query with filter
    println!("\nQuerying users with age > 25...");
    let filter = PostgresFilter::GreaterThan("age".to_string(), SqlValue::Int32(25));
    match adapter.select("users", None, Some(filter)).await? {
        orbit_engine::adapters::CommandResult::Rows(rows) => {
            println!("Found {} users:", rows.len());
            for row in rows {
                if let (Some(SqlValue::String(name)), Some(SqlValue::Int32(age))) =
                    (row.get("name"), row.get("age"))
                {
                    println!("  - {} (age: {})", name, age);
                }
            }
        }
        _ => println!("Unexpected result"),
    }

    // Update a user
    println!("\nUpdating Bob's age to 26...");
    let filter = PostgresFilter::Equals("id".to_string(), SqlValue::Int32(2));
    let mut updates = HashMap::new();
    updates.insert("age".to_string(), SqlValue::Int32(26));
    adapter.update("users", filter, updates).await?;
    println!("✓ Updated Bob's age");

    // Delete a user
    println!("\nDeleting user with id=3...");
    let filter = PostgresFilter::Equals("id".to_string(), SqlValue::Int32(3));
    adapter.delete("users", filter).await?;
    println!("✓ Deleted user");

    Ok(())
}

async fn redis_example(context: AdapterContext) -> Result<(), Box<dyn std::error::Error>> {
    use orbit_engine::adapters::ProtocolAdapter;

    let mut adapter = RedisAdapter::new(context);
    adapter.initialize().await?;
    println!("✓ Redis adapter initialized");

    // String operations
    println!("\nString operations:");
    adapter.set("mykey", "myvalue", None).await?;
    println!("  SET mykey myvalue");

    if let Some(value) = adapter.get("mykey").await? {
        println!("  GET mykey => {}", value);
    }

    // Hash operations
    println!("\nHash operations:");
    adapter.hset("user:1000", "name", "John").await?;
    adapter.hset("user:1000", "email", "john@example.com").await?;
    adapter.hset("user:1000", "age", "30").await?;
    println!("  HSET user:1000 name John");
    println!("  HSET user:1000 email john@example.com");
    println!("  HSET user:1000 age 30");

    if let Some(name) = adapter.hget("user:1000", "name").await? {
        println!("  HGET user:1000 name => {}", name);
    }

    let all_fields = adapter.hgetall("user:1000").await?;
    println!("  HGETALL user:1000 =>");
    for (field, value) in all_fields {
        println!("    {}: {}", field, value);
    }

    // List operations
    println!("\nList operations:");
    adapter.rpush("mylist", "first").await?;
    adapter.rpush("mylist", "second").await?;
    adapter.lpush("mylist", "zeroth").await?;
    println!("  RPUSH mylist first");
    println!("  RPUSH mylist second");
    println!("  LPUSH mylist zeroth");
    println!("  (List is now: zeroth, first, second)");

    // Check existence
    println!("\nExistence check:");
    let exists = adapter.exists("mykey").await?;
    println!("  EXISTS mykey => {}", exists);

    // Delete
    println!("\nDeletion:");
    adapter.del("mykey").await?;
    println!("  DEL mykey");

    let exists = adapter.exists("mykey").await?;
    println!("  EXISTS mykey => {}", exists);

    Ok(())
    */
}
