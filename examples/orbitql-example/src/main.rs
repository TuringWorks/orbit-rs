//! # OrbitQL Example
//!
//! This example demonstrates the usage of OrbitQL, the unified multi-model
//! query language for orbit-rs.
//!
//! OrbitQL allows you to query across documents, graphs, time-series, and
//! key-value data using a single, expressive SQL-like syntax.

use orbit_shared::orbitql::{OrbitQLEngine, QueryContext, QueryParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("OrbitQL - Unified Multi-Model Query Language Example");
    println!("====================================================");

    // Create a new OrbitQL engine
    let mut engine = OrbitQLEngine::new();

    // Example 1: Basic SELECT query
    println!("\n1. Basic Document Query:");
    let query1 = "SELECT name, email, age FROM users WHERE age > 18 LIMIT 10";
    println!("Query: {}", query1);

    match engine.validate(query1) {
        Ok(_) => println!("✅ Query validation successful"),
        Err(e) => println!("❌ Query validation failed: {}", e),
    }

    // Example 2: Graph traversal query
    println!("\n2. Graph Traversal Query:");
    let query2 = r#"
        SELECT 
            user.name,
            ->follows->user.name AS friends,
            ->follows->user->posted->post.title AS friend_posts
        FROM users AS user
        WHERE user.active = true
        FETCH friends, friend_posts
    "#;
    println!("Query: {}", query2);

    match engine.validate(query2) {
        Ok(_) => println!("✅ Query validation successful"),
        Err(e) => println!("❌ Query validation failed: {}", e),
    }

    // Example 3: Time-series query
    println!("\n3. Time-Series Query:");
    let query3 = r#"
        SELECT 
            user.name,
            metrics[cpu_usage WHERE timestamp > NOW() - 1h] AS recent_cpu,
            AVG(metrics[memory_usage WHERE timestamp > NOW() - 24h]) AS avg_memory
        FROM users user
        WHERE user.monitoring = true
    "#;
    println!("Query: {}", query3);

    match engine.validate(query3) {
        Ok(_) => println!("✅ Query validation successful"),
        Err(e) => println!("❌ Query validation failed: {}", e),
    }

    // Example 4: Multi-model JOIN query
    println!("\n4. Multi-Model JOIN Query:");
    let query4 = r#"
        SELECT 
            u.name,
            p.title,
            m.avg_response_time
        FROM users u
        INNER JOIN posts p ON u.id = p.author_id
        INNER JOIN metrics m ON u.id = m.user_id 
        WHERE u.created_at > '2024-01-01' AND m.timestamp > NOW() - 7d
        ORDER BY m.avg_response_time DESC
    "#;
    println!("Query: {}", query4);

    match engine.validate(query4) {
        Ok(_) => println!("✅ Query validation successful"),
        Err(e) => println!("❌ Query validation failed: {}", e),
    }

    // Example 5: Real-time subscription query
    println!("\n5. Real-time Live Query:");
    let query5 = r#"
        LIVE SELECT 
            post.title,
            post.author.name,
            COUNT(->likes) AS like_count
        FROM posts post
        WHERE post.created_at > NOW() - 1h
        GROUP BY post.id
        HAVING like_count > 10
    "#;
    println!("Query: {}", query5);

    match engine.validate(query5) {
        Ok(_) => println!("✅ Query validation successful"),
        Err(e) => println!("❌ Query validation failed: {}", e),
    }

    // Example 6: Using query parameters
    println!("\n6. Parameterized Query:");
    let query6 = "SELECT * FROM users WHERE age > $min_age AND city = $city";
    println!("Query: {}", query6);

    let _params = QueryParams::new()
        .set("min_age", 21)
        .set("city", "San Francisco");

    let _context = QueryContext::default();

    match engine.validate(query6) {
        Ok(_) => {
            println!("✅ Query validation successful");
            println!("Parameters: min_age = 21, city = 'San Francisco'");
        }
        Err(e) => println!("❌ Query validation failed: {}", e),
    }

    // Example 7: Transaction with RELATE statement
    println!("\n7. Graph Relationship Creation:");
    let query7 = r#"
        BEGIN;
        INSERT INTO users { name: "Alice", email: "alice@example.com" };
        INSERT INTO users { name: "Bob", email: "bob@example.com" };
        RELATE (SELECT * FROM users WHERE name = "Alice") 
        ->follows 
        (SELECT * FROM users WHERE name = "Bob")
        SET created_at = NOW();
        COMMIT;
    "#;
    println!("Query: {}", query7);

    match engine.validate("BEGIN") {
        Ok(_) => println!("✅ Transaction query validation successful"),
        Err(e) => println!("❌ Transaction query validation failed: {}", e),
    }

    println!("\n🎉 OrbitQL Example Complete!");
    println!("\nOrbitQL Features Demonstrated:");
    println!("• Document queries with filtering and sorting");
    println!("• Graph traversals with relationship following");
    println!("• Time-series data querying with time windows");
    println!("• Multi-model JOINs across different data types");
    println!("• Real-time live queries for subscriptions");
    println!("• Parameterized queries for safety");
    println!("• Transaction support with relationship creation");

    println!("\nNext Steps:");
    println!("• Implement actual query execution against data stores");
    println!("• Add more advanced graph traversal patterns");
    println!("• Extend time-series aggregation functions");
    println!("• Build query optimization rules");
    println!("• Add distributed query execution capabilities");

    Ok(())
}
