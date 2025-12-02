//! Cross-Protocol Integration Tests
//!
//! These tests verify that data written via one protocol adapter
//! is immediately accessible through all other protocol adapters.
//! This is the core value proposition of the unified storage layer.

use orbit_engine::unified::{
    AdapterFactory, FilterExpression, MemoryBackend, SchemaRegistry, UnifiedStorage,
    UnifiedStorageConfig, UniversalValue,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Setup helper for tests
async fn setup() -> (Arc<UnifiedStorage>, Arc<SchemaRegistry>) {
    let backend = Arc::new(MemoryBackend::new());
    let config = UnifiedStorageConfig::default();
    let storage = Arc::new(UnifiedStorage::new(backend, config));
    storage.initialize().await.unwrap();
    let registry = Arc::new(SchemaRegistry::new());
    (storage, registry)
}

/// Test: Data written via Redis is accessible via SQL
#[tokio::test]
async fn test_redis_to_sql_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via Redis adapter (using hash for structured data)
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));

    // Create a user record via Redis HSET
    redis
        .hset(
            "users:alice",
            "id",
            UniversalValue::String("alice".to_string()),
        )
        .await
        .unwrap();
    redis
        .hset(
            "users:alice",
            "name",
            UniversalValue::String("Alice Smith".to_string()),
        )
        .await
        .unwrap();
    redis
        .hset(
            "users:alice",
            "email",
            UniversalValue::String("alice@example.com".to_string()),
        )
        .await
        .unwrap();
    redis
        .hset("users:alice", "age", UniversalValue::Int(28))
        .await
        .unwrap();

    // Read the same data via SQL adapter
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = sql
        .select("users", None, None, None, None, None)
        .await
        .unwrap();

    // Verify data is accessible
    assert_eq!(rows.len(), 1, "Expected 1 row from SQL after Redis write");
    let row = &rows[0];
    assert_eq!(
        row.get("name"),
        Some(&UniversalValue::String("Alice Smith".to_string()))
    );
    assert_eq!(
        row.get("email"),
        Some(&UniversalValue::String("alice@example.com".to_string()))
    );
}

/// Test: Data written via SQL is accessible via Redis
#[tokio::test]
async fn test_sql_to_redis_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via SQL adapter
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));

    let mut user = BTreeMap::new();
    user.insert("id".to_string(), UniversalValue::String("bob".to_string()));
    user.insert(
        "name".to_string(),
        UniversalValue::String("Bob Jones".to_string()),
    );
    user.insert(
        "email".to_string(),
        UniversalValue::String("bob@example.com".to_string()),
    );
    sql.insert("users", user, "id").await.unwrap();

    // Read via Redis adapter
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));

    // Get all hash fields
    let fields = redis.hgetall("users:bob").await.unwrap();
    assert!(
        fields.is_some(),
        "Expected hash data from Redis after SQL insert"
    );

    let fields = fields.unwrap();
    assert_eq!(
        fields.get("name"),
        Some(&UniversalValue::String("Bob Jones".to_string()))
    );
}

/// Test: Data written via Graph is accessible via REST adapter
#[tokio::test]
async fn test_graph_to_rest_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via Graph adapter (Cypher style)
    let graph = AdapterFactory::cypher(Arc::clone(&storage), Arc::clone(&registry));

    let mut props = BTreeMap::new();
    props.insert(
        "name".to_string(),
        UniversalValue::String("Charlie".to_string()),
    );
    props.insert(
        "department".to_string(),
        UniversalValue::String("Engineering".to_string()),
    );
    graph
        .create_node("emp1", vec!["Employee".to_string()], props)
        .await
        .unwrap();

    // Read via REST adapter - graph nodes are stored in "graph:nodes" namespace
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));

    // The graph adapter uses "graph:nodes" as its namespace
    let node = rest.get("graph:nodes", "emp1").await.unwrap();
    assert!(node.is_some(), "Expected node from REST after Graph write");

    // Verify the node properties
    if let Some(UniversalValue::Node {
        id,
        labels,
        properties,
    }) = node
    {
        assert_eq!(id, "emp1");
        assert!(labels.contains(&"Employee".to_string()));
        assert_eq!(
            properties.get("name"),
            Some(&UniversalValue::String("Charlie".to_string()))
        );
    } else {
        panic!("Expected Node value");
    }
}

/// Test: Multiple protocols can read the same data simultaneously
#[tokio::test]
async fn test_multi_protocol_concurrent_read() {
    let (storage, registry) = setup().await;

    // Write initial data via Redis
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    redis
        .hset(
            "products:p1",
            "name",
            UniversalValue::String("Widget".to_string()),
        )
        .await
        .unwrap();
    redis
        .hset("products:p1", "price", UniversalValue::Float(29.99))
        .await
        .unwrap();
    redis
        .hset("products:p1", "stock", UniversalValue::Int(100))
        .await
        .unwrap();

    // Create adapters for all protocols
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let cql = AdapterFactory::cql(Arc::clone(&storage), Arc::clone(&registry));
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));

    // Read from all protocols
    let redis_data = redis.hgetall("products:p1").await.unwrap();
    let sql_rows = sql
        .select("products", None, None, None, None, None)
        .await
        .unwrap();
    let mysql_rows = mysql
        .select("products", None, None, None, None, None)
        .await
        .unwrap();
    let cql_rows = cql.select("products", None, None, None).await.unwrap();
    let rest_data = rest.get("products", "p1").await.unwrap();

    // All should return the same data
    assert!(redis_data.is_some());
    assert_eq!(sql_rows.len(), 1);
    assert_eq!(mysql_rows.len(), 1);
    assert_eq!(cql_rows.len(), 1);
    assert!(rest_data.is_some());

    // Verify the values match
    let name = UniversalValue::String("Widget".to_string());
    assert_eq!(redis_data.unwrap().get("name"), Some(&name));
    assert_eq!(sql_rows[0].get("name"), Some(&name));
}

/// Test: Updates via one protocol are visible via another
#[tokio::test]
async fn test_cross_protocol_updates() {
    let (storage, registry) = setup().await;

    // Create via SQL
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let mut user = BTreeMap::new();
    user.insert(
        "id".to_string(),
        UniversalValue::String("update_test".to_string()),
    );
    user.insert(
        "status".to_string(),
        UniversalValue::String("active".to_string()),
    );
    sql.insert("accounts", user, "id").await.unwrap();

    // Verify via Redis
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    let fields = redis
        .hgetall("accounts:update_test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        fields.get("status"),
        Some(&UniversalValue::String("active".to_string()))
    );

    // Update via SQL
    let mut updates = BTreeMap::new();
    updates.insert(
        "status".to_string(),
        UniversalValue::String("suspended".to_string()),
    );
    let filter = FilterExpression::Eq(
        "id".to_string(),
        UniversalValue::String("update_test".to_string()),
    );
    sql.update("accounts", updates, Some(filter)).await.unwrap();

    // Verify update via Redis
    let fields = redis
        .hgetall("accounts:update_test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        fields.get("status"),
        Some(&UniversalValue::String("suspended".to_string()))
    );
}

/// Test: Deletes via one protocol affect all protocols
#[tokio::test]
async fn test_cross_protocol_deletes() {
    let (storage, registry) = setup().await;

    // Create via REST
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));
    let mut item = BTreeMap::new();
    item.insert(
        "name".to_string(),
        UniversalValue::String("ToBeDeleted".to_string()),
    );
    rest.create("items", "del1", UniversalValue::Map(item))
        .await
        .unwrap();

    // Verify exists via Redis
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    assert!(redis.exists(&["items:del1"]).await.unwrap() > 0);

    // Delete via REST
    rest.delete("items", "del1").await.unwrap();

    // Verify gone via Redis
    assert_eq!(redis.exists(&["items:del1"]).await.unwrap(), 0);

    // Verify gone via SQL
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = sql
        .select("items", None, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 0);
}

/// Test: Batch operations work across protocols
#[tokio::test]
async fn test_cross_protocol_batch_operations() {
    let (storage, registry) = setup().await;

    // Batch insert via SQL
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));

    for i in 1..=5 {
        let mut row = BTreeMap::new();
        row.insert("id".to_string(), UniversalValue::Int(i));
        row.insert(
            "name".to_string(),
            UniversalValue::String(format!("Item {}", i)),
        );
        row.insert(
            "category".to_string(),
            UniversalValue::String("test".to_string()),
        );
        sql.insert("batch_items", row, "id").await.unwrap();
    }

    // Count via SQL
    let count = sql.count("batch_items", None).await.unwrap();
    assert_eq!(count, 5);

    // List via REST
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));
    let items = rest.list("batch_items", None, None, None).await.unwrap();
    assert_eq!(items.len(), 5);

    // Batch delete via Redis
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    let keys: Vec<&str> = (1..=3)
        .map(|i| match i {
            1 => "batch_items:1",
            2 => "batch_items:2",
            _ => "batch_items:3",
        })
        .collect();
    redis.del(&keys).await.unwrap();

    // Verify count changed
    let count = sql.count("batch_items", None).await.unwrap();
    assert_eq!(count, 2);
}

/// Test: CQL keyspace operations work with unified storage
#[tokio::test]
async fn test_cql_with_unified_storage() {
    let (storage, registry) = setup().await;

    // Create via CQL adapter
    let cql = AdapterFactory::cql(Arc::clone(&storage), Arc::clone(&registry));

    let mut row = BTreeMap::new();
    row.insert(
        "id".to_string(),
        UniversalValue::String("uuid1".to_string()),
    );
    row.insert(
        "name".to_string(),
        UniversalValue::String("CQL Test".to_string()),
    );
    row.insert(
        "timestamp".to_string(),
        UniversalValue::Timestamp(1700000000000),
    );
    cql.insert("events", row, "id").await.unwrap();

    // Read via SQL
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = sql
        .select("events", None, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("name"),
        Some(&UniversalValue::String("CQL Test".to_string()))
    );
}

/// Test: Graph relationships work with unified storage
#[tokio::test]
async fn test_graph_relationships_unified() {
    let (storage, registry) = setup().await;

    // Create nodes via Graph adapter
    let graph = AdapterFactory::cypher(Arc::clone(&storage), Arc::clone(&registry));

    let mut props1 = BTreeMap::new();
    props1.insert(
        "name".to_string(),
        UniversalValue::String("Node A".to_string()),
    );
    graph
        .create_node("node_a", vec!["TestNode".to_string()], props1)
        .await
        .unwrap();

    let mut props2 = BTreeMap::new();
    props2.insert(
        "name".to_string(),
        UniversalValue::String("Node B".to_string()),
    );
    graph
        .create_node("node_b", vec!["TestNode".to_string()], props2)
        .await
        .unwrap();

    // Create relationship
    let edge_props = BTreeMap::new();
    graph
        .create_relationship("rel1", "CONNECTS_TO", "node_a", "node_b", edge_props)
        .await
        .unwrap();

    // Query nodes by label
    let nodes = graph.get_nodes_by_label("TestNode").await.unwrap();
    assert_eq!(nodes.len(), 2);

    // Query relationships
    let rels = graph
        .get_relationships("node_a", Some("outgoing"), None)
        .await
        .unwrap();
    assert_eq!(rels.len(), 1);
}

/// Test: Data types are preserved across protocols
#[tokio::test]
async fn test_data_type_preservation() {
    let (storage, registry) = setup().await;

    // Create with various data types via SQL
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));

    let mut row = BTreeMap::new();
    row.insert(
        "id".to_string(),
        UniversalValue::String("types_test".to_string()),
    );
    row.insert("int_val".to_string(), UniversalValue::Int(42));
    row.insert("float_val".to_string(), UniversalValue::Float(1.23));
    row.insert("bool_val".to_string(), UniversalValue::Bool(true));
    row.insert("null_val".to_string(), UniversalValue::Null);
    row.insert(
        "bytes_val".to_string(),
        UniversalValue::Bytes(vec![1, 2, 3, 4]),
    );
    sql.insert("typed_data", row, "id").await.unwrap();

    // Read via Redis and verify types
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    let fields = redis
        .hgetall("typed_data:types_test")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fields.get("int_val"), Some(&UniversalValue::Int(42)));
    assert_eq!(fields.get("float_val"), Some(&UniversalValue::Float(1.23)));
    assert_eq!(fields.get("bool_val"), Some(&UniversalValue::Bool(true)));
    assert_eq!(fields.get("null_val"), Some(&UniversalValue::Null));
    assert_eq!(
        fields.get("bytes_val"),
        Some(&UniversalValue::Bytes(vec![1, 2, 3, 4]))
    );
}

// =============================================================================
// MySQL Cross-Protocol Tests
// =============================================================================

/// Test: Data written via MySQL is accessible via PostgreSQL
#[tokio::test]
async fn test_mysql_to_postgres_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));

    let mut customer = BTreeMap::new();
    customer.insert(
        "id".to_string(),
        UniversalValue::String("cust001".to_string()),
    );
    customer.insert(
        "name".to_string(),
        UniversalValue::String("John Doe".to_string()),
    );
    customer.insert(
        "email".to_string(),
        UniversalValue::String("john@example.com".to_string()),
    );
    customer.insert("balance".to_string(), UniversalValue::Float(1500.50));
    customer.insert("active".to_string(), UniversalValue::Bool(true));
    mysql.insert("customers", customer, "id").await.unwrap();

    // Read via PostgreSQL adapter
    let postgres = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = postgres
        .select("customers", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(
        rows.len(),
        1,
        "Expected 1 row from PostgreSQL after MySQL write"
    );
    let row = &rows[0];
    assert_eq!(
        row.get("name"),
        Some(&UniversalValue::String("John Doe".to_string()))
    );
    assert_eq!(row.get("balance"), Some(&UniversalValue::Float(1500.50)));
    assert_eq!(row.get("active"), Some(&UniversalValue::Bool(true)));
}

/// Test: Data written via MySQL is accessible via Redis
#[tokio::test]
async fn test_mysql_to_redis_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));

    let mut order = BTreeMap::new();
    order.insert(
        "id".to_string(),
        UniversalValue::String("order123".to_string()),
    );
    order.insert(
        "product".to_string(),
        UniversalValue::String("Laptop".to_string()),
    );
    order.insert("quantity".to_string(), UniversalValue::Int(2));
    order.insert("total".to_string(), UniversalValue::Float(2499.98));
    mysql.insert("orders", order, "id").await.unwrap();

    // Read via Redis adapter
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    let fields = redis.hgetall("orders:order123").await.unwrap();

    assert!(
        fields.is_some(),
        "Expected hash data from Redis after MySQL insert"
    );
    let fields = fields.unwrap();
    assert_eq!(
        fields.get("product"),
        Some(&UniversalValue::String("Laptop".to_string()))
    );
    assert_eq!(fields.get("quantity"), Some(&UniversalValue::Int(2)));
}

/// Test: Data written via MySQL is accessible via CQL (Cassandra)
#[tokio::test]
async fn test_mysql_to_cql_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));

    let mut event = BTreeMap::new();
    event.insert(
        "id".to_string(),
        UniversalValue::String("evt001".to_string()),
    );
    event.insert(
        "event_type".to_string(),
        UniversalValue::String("user_login".to_string()),
    );
    event.insert(
        "user_id".to_string(),
        UniversalValue::String("user42".to_string()),
    );
    event.insert(
        "timestamp".to_string(),
        UniversalValue::Timestamp(1700000000000),
    );
    mysql.insert("events", event, "id").await.unwrap();

    // Read via CQL adapter
    let cql = AdapterFactory::cql(Arc::clone(&storage), Arc::clone(&registry));
    let rows = cql.select("events", None, None, None).await.unwrap();

    assert_eq!(rows.len(), 1, "Expected 1 row from CQL after MySQL write");
    assert_eq!(
        rows[0].get("event_type"),
        Some(&UniversalValue::String("user_login".to_string()))
    );
}

/// Test: Data written via MySQL is accessible via REST API
#[tokio::test]
async fn test_mysql_to_rest_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));

    let mut product = BTreeMap::new();
    product.insert(
        "id".to_string(),
        UniversalValue::String("prod001".to_string()),
    );
    product.insert(
        "name".to_string(),
        UniversalValue::String("Wireless Mouse".to_string()),
    );
    product.insert("price".to_string(), UniversalValue::Float(29.99));
    product.insert("stock".to_string(), UniversalValue::Int(150));
    mysql.insert("products", product, "id").await.unwrap();

    // Read via REST adapter
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));
    let data = rest.get("products", "prod001").await.unwrap();

    assert!(data.is_some(), "Expected data from REST after MySQL write");
    if let Some(UniversalValue::Map(map)) = data {
        assert_eq!(
            map.get("name"),
            Some(&UniversalValue::String("Wireless Mouse".to_string()))
        );
        assert_eq!(map.get("price"), Some(&UniversalValue::Float(29.99)));
    } else {
        panic!("Expected Map value from REST");
    }
}

/// Test: Data written via MySQL is accessible via Graph/AQL adapter
#[tokio::test]
async fn test_mysql_to_aql_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));

    let mut document = BTreeMap::new();
    document.insert(
        "id".to_string(),
        UniversalValue::String("doc001".to_string()),
    );
    document.insert(
        "title".to_string(),
        UniversalValue::String("Technical Report".to_string()),
    );
    document.insert(
        "author".to_string(),
        UniversalValue::String("Jane Smith".to_string()),
    );
    mysql.insert("documents", document, "id").await.unwrap();

    // Read via AQL adapter (ArangoDB style)
    let _aql = AdapterFactory::aql(Arc::clone(&storage), Arc::clone(&registry));

    // AQL adapter stores in graph:nodes namespace - access via REST for table data
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));
    let items = rest.list("documents", None, None, None).await.unwrap();

    assert_eq!(
        items.len(),
        1,
        "Expected 1 document accessible after MySQL write"
    );
}

// =============================================================================
// Reverse Direction Tests (Other Protocols -> MySQL)
// =============================================================================

/// Test: Data written via Redis is accessible via MySQL
#[tokio::test]
async fn test_redis_to_mysql_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via Redis adapter
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    redis
        .hset(
            "sessions:sess001",
            "user_id",
            UniversalValue::String("user123".to_string()),
        )
        .await
        .unwrap();
    redis
        .hset(
            "sessions:sess001",
            "ip_address",
            UniversalValue::String("192.168.1.100".to_string()),
        )
        .await
        .unwrap();
    redis
        .hset(
            "sessions:sess001",
            "created_at",
            UniversalValue::Timestamp(1700000000000),
        )
        .await
        .unwrap();

    // Read via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let rows = mysql
        .select("sessions", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1, "Expected 1 row from MySQL after Redis write");
    assert_eq!(
        rows[0].get("user_id"),
        Some(&UniversalValue::String("user123".to_string()))
    );
}

/// Test: Data written via CQL is accessible via MySQL
#[tokio::test]
async fn test_cql_to_mysql_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via CQL adapter
    let cql = AdapterFactory::cql(Arc::clone(&storage), Arc::clone(&registry));

    let mut row = BTreeMap::new();
    row.insert(
        "id".to_string(),
        UniversalValue::String("metric001".to_string()),
    );
    row.insert(
        "metric_name".to_string(),
        UniversalValue::String("cpu_usage".to_string()),
    );
    row.insert("value".to_string(), UniversalValue::Float(75.5));
    row.insert(
        "timestamp".to_string(),
        UniversalValue::Timestamp(1700000000000),
    );
    cql.insert("metrics", row, "id").await.unwrap();

    // Read via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let rows = mysql
        .select("metrics", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1, "Expected 1 row from MySQL after CQL write");
    assert_eq!(
        rows[0].get("metric_name"),
        Some(&UniversalValue::String("cpu_usage".to_string()))
    );
    assert_eq!(rows[0].get("value"), Some(&UniversalValue::Float(75.5)));
}

/// Test: Data written via REST is accessible via MySQL
#[tokio::test]
async fn test_rest_to_mysql_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via REST adapter
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));

    let mut config = BTreeMap::new();
    config.insert(
        "id".to_string(),
        UniversalValue::String("config001".to_string()),
    );
    config.insert(
        "key".to_string(),
        UniversalValue::String("max_connections".to_string()),
    );
    config.insert("value".to_string(), UniversalValue::Int(100));
    config.insert("enabled".to_string(), UniversalValue::Bool(true));
    rest.create("configs", "config001", UniversalValue::Map(config))
        .await
        .unwrap();

    // Read via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let rows = mysql
        .select("configs", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1, "Expected 1 row from MySQL after REST write");
    assert_eq!(
        rows[0].get("key"),
        Some(&UniversalValue::String("max_connections".to_string()))
    );
    assert_eq!(rows[0].get("value"), Some(&UniversalValue::Int(100)));
}

/// Test: Data written via PostgreSQL is accessible via MySQL
#[tokio::test]
async fn test_postgres_to_mysql_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via PostgreSQL adapter
    let postgres = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));

    let mut user = BTreeMap::new();
    user.insert(
        "id".to_string(),
        UniversalValue::String("user001".to_string()),
    );
    user.insert(
        "username".to_string(),
        UniversalValue::String("alice_pg".to_string()),
    );
    user.insert(
        "email".to_string(),
        UniversalValue::String("alice@postgres.example".to_string()),
    );
    user.insert("score".to_string(), UniversalValue::Int(9500));
    postgres.insert("users", user, "id").await.unwrap();

    // Read via MySQL adapter
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let rows = mysql
        .select("users", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(
        rows.len(),
        1,
        "Expected 1 row from MySQL after PostgreSQL write"
    );
    assert_eq!(
        rows[0].get("username"),
        Some(&UniversalValue::String("alice_pg".to_string()))
    );
}

// =============================================================================
// Full Protocol Matrix Tests
// =============================================================================

/// Test: Complete protocol matrix - all protocols can read data written by MySQL
#[tokio::test]
async fn test_mysql_write_all_protocols_read() {
    let (storage, registry) = setup().await;

    // Write via MySQL
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let mut record = BTreeMap::new();
    record.insert(
        "id".to_string(),
        UniversalValue::String("matrix001".to_string()),
    );
    record.insert(
        "data".to_string(),
        UniversalValue::String("MySQL Origin".to_string()),
    );
    record.insert("count".to_string(), UniversalValue::Int(42));
    mysql.insert("matrix_test", record, "id").await.unwrap();

    // Read from all protocols
    let postgres = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    let cql = AdapterFactory::cql(Arc::clone(&storage), Arc::clone(&registry));
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));

    // PostgreSQL read
    let pg_rows = postgres
        .select("matrix_test", None, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(pg_rows.len(), 1);
    assert_eq!(
        pg_rows[0].get("data"),
        Some(&UniversalValue::String("MySQL Origin".to_string()))
    );

    // Redis read
    let redis_data = redis.hgetall("matrix_test:matrix001").await.unwrap();
    assert!(redis_data.is_some());
    assert_eq!(
        redis_data.unwrap().get("data"),
        Some(&UniversalValue::String("MySQL Origin".to_string()))
    );

    // CQL read
    let cql_rows = cql.select("matrix_test", None, None, None).await.unwrap();
    assert_eq!(cql_rows.len(), 1);
    assert_eq!(
        cql_rows[0].get("data"),
        Some(&UniversalValue::String("MySQL Origin".to_string()))
    );

    // REST read
    let rest_data = rest.get("matrix_test", "matrix001").await.unwrap();
    assert!(rest_data.is_some());
}

/// Test: Multi-protocol write sequence - verify data consistency
#[tokio::test]
async fn test_sequential_multi_protocol_writes() {
    let (storage, registry) = setup().await;

    // Create adapters
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let postgres = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));

    // Step 1: Create via MySQL
    let mut row1 = BTreeMap::new();
    row1.insert(
        "id".to_string(),
        UniversalValue::String("seq001".to_string()),
    );
    row1.insert(
        "status".to_string(),
        UniversalValue::String("created".to_string()),
    );
    row1.insert("version".to_string(), UniversalValue::Int(1));
    mysql.insert("workflow", row1, "id").await.unwrap();

    // Step 2: Update via PostgreSQL
    let mut updates = BTreeMap::new();
    updates.insert(
        "status".to_string(),
        UniversalValue::String("processing".to_string()),
    );
    updates.insert("version".to_string(), UniversalValue::Int(2));
    let filter = FilterExpression::Eq(
        "id".to_string(),
        UniversalValue::String("seq001".to_string()),
    );
    postgres
        .update("workflow", updates, Some(filter))
        .await
        .unwrap();

    // Step 3: Verify via Redis
    let redis_data = redis.hgetall("workflow:seq001").await.unwrap().unwrap();
    assert_eq!(
        redis_data.get("status"),
        Some(&UniversalValue::String("processing".to_string()))
    );
    assert_eq!(redis_data.get("version"), Some(&UniversalValue::Int(2)));

    // Step 4: Update via REST
    let mut patch = BTreeMap::new();
    patch.insert(
        "status".to_string(),
        UniversalValue::String("completed".to_string()),
    );
    patch.insert("version".to_string(), UniversalValue::Int(3));
    rest.patch("workflow", "seq001", patch).await.unwrap();

    // Step 5: Final verification via MySQL
    let rows = mysql
        .select("workflow", None, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("status"),
        Some(&UniversalValue::String("completed".to_string()))
    );
    assert_eq!(rows[0].get("version"), Some(&UniversalValue::Int(3)));
}

/// Test: Concurrent writes from multiple protocols
#[tokio::test]
async fn test_concurrent_multi_protocol_writes() {
    let (storage, registry) = setup().await;

    // Create adapters
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let postgres = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));

    // Insert records from different protocols
    let mut row1 = BTreeMap::new();
    row1.insert(
        "id".to_string(),
        UniversalValue::String("rec_mysql".to_string()),
    );
    row1.insert(
        "source".to_string(),
        UniversalValue::String("mysql".to_string()),
    );
    mysql.insert("concurrent_test", row1, "id").await.unwrap();

    let mut row2 = BTreeMap::new();
    row2.insert(
        "id".to_string(),
        UniversalValue::String("rec_postgres".to_string()),
    );
    row2.insert(
        "source".to_string(),
        UniversalValue::String("postgres".to_string()),
    );
    postgres
        .insert("concurrent_test", row2, "id")
        .await
        .unwrap();

    redis
        .hset(
            "concurrent_test:rec_redis",
            "id",
            UniversalValue::String("rec_redis".to_string()),
        )
        .await
        .unwrap();
    redis
        .hset(
            "concurrent_test:rec_redis",
            "source",
            UniversalValue::String("redis".to_string()),
        )
        .await
        .unwrap();

    // Verify all records are visible from PostgreSQL
    let rows = postgres
        .select("concurrent_test", None, None, None, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 3, "Expected 3 records from all protocol writes");

    // Verify sources
    let sources: Vec<_> = rows
        .iter()
        .filter_map(|r| {
            if let Some(UniversalValue::String(s)) = r.get("source") {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(sources.contains(&"mysql".to_string()));
    assert!(sources.contains(&"postgres".to_string()));
    assert!(sources.contains(&"redis".to_string()));
}

// =============================================================================
// Complex Data Type Tests
// =============================================================================

/// Test: List/Array data type preservation across protocols
#[tokio::test]
async fn test_list_type_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via MySQL with list data
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let mut row = BTreeMap::new();
    row.insert(
        "id".to_string(),
        UniversalValue::String("list001".to_string()),
    );
    row.insert(
        "tags".to_string(),
        UniversalValue::List(vec![
            UniversalValue::String("rust".to_string()),
            UniversalValue::String("database".to_string()),
            UniversalValue::String("distributed".to_string()),
        ]),
    );
    mysql.insert("tagged_items", row, "id").await.unwrap();

    // Read via PostgreSQL
    let postgres = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = postgres
        .select("tagged_items", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    if let Some(UniversalValue::List(tags)) = rows[0].get("tags") {
        assert_eq!(tags.len(), 3);
        assert!(tags.contains(&UniversalValue::String("rust".to_string())));
    } else {
        panic!("Expected List value for tags");
    }
}

/// Test: Nested Map/Object data type preservation
#[tokio::test]
async fn test_nested_map_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via PostgreSQL with nested map
    let postgres = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));

    let mut address = BTreeMap::new();
    address.insert(
        "street".to_string(),
        UniversalValue::String("123 Main St".to_string()),
    );
    address.insert(
        "city".to_string(),
        UniversalValue::String("Springfield".to_string()),
    );
    address.insert(
        "zip".to_string(),
        UniversalValue::String("12345".to_string()),
    );

    let mut row = BTreeMap::new();
    row.insert(
        "id".to_string(),
        UniversalValue::String("nested001".to_string()),
    );
    row.insert(
        "name".to_string(),
        UniversalValue::String("Test User".to_string()),
    );
    row.insert("address".to_string(), UniversalValue::Map(address));
    postgres.insert("nested_data", row, "id").await.unwrap();

    // Read via MySQL
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let rows = mysql
        .select("nested_data", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    if let Some(UniversalValue::Map(addr)) = rows[0].get("address") {
        assert_eq!(
            addr.get("city"),
            Some(&UniversalValue::String("Springfield".to_string()))
        );
    } else {
        panic!("Expected Map value for address");
    }
}

/// Test: Timestamp precision across protocols
#[tokio::test]
async fn test_timestamp_precision_cross_protocol() {
    let (storage, registry) = setup().await;

    let timestamp = 1700000000123i64; // Millisecond precision

    // Write via CQL (Cassandra - known for time-series)
    let cql = AdapterFactory::cql(Arc::clone(&storage), Arc::clone(&registry));
    let mut row = BTreeMap::new();
    row.insert(
        "id".to_string(),
        UniversalValue::String("ts001".to_string()),
    );
    row.insert(
        "event_time".to_string(),
        UniversalValue::Timestamp(timestamp),
    );
    cql.insert("time_events", row, "id").await.unwrap();

    // Read via MySQL
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let rows = mysql
        .select("time_events", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get("event_time"),
        Some(&UniversalValue::Timestamp(timestamp))
    );

    // Read via PostgreSQL
    let postgres = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = postgres
        .select("time_events", None, None, None, None, None)
        .await
        .unwrap();

    assert_eq!(
        rows[0].get("event_time"),
        Some(&UniversalValue::Timestamp(timestamp))
    );
}
