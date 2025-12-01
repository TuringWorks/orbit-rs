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
    redis.hset("users:alice", "id", UniversalValue::String("alice".to_string())).await.unwrap();
    redis.hset("users:alice", "name", UniversalValue::String("Alice Smith".to_string())).await.unwrap();
    redis.hset("users:alice", "email", UniversalValue::String("alice@example.com".to_string())).await.unwrap();
    redis.hset("users:alice", "age", UniversalValue::Int(28)).await.unwrap();

    // Read the same data via SQL adapter
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = sql.select("users", None, None, None, None, None).await.unwrap();

    // Verify data is accessible
    assert_eq!(rows.len(), 1, "Expected 1 row from SQL after Redis write");
    let row = &rows[0];
    assert_eq!(row.get("name"), Some(&UniversalValue::String("Alice Smith".to_string())));
    assert_eq!(row.get("email"), Some(&UniversalValue::String("alice@example.com".to_string())));
}

/// Test: Data written via SQL is accessible via Redis
#[tokio::test]
async fn test_sql_to_redis_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via SQL adapter
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));

    let mut user = BTreeMap::new();
    user.insert("id".to_string(), UniversalValue::String("bob".to_string()));
    user.insert("name".to_string(), UniversalValue::String("Bob Jones".to_string()));
    user.insert("email".to_string(), UniversalValue::String("bob@example.com".to_string()));
    sql.insert("users", user, "id").await.unwrap();

    // Read via Redis adapter
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));

    // Get all hash fields
    let fields = redis.hgetall("users:bob").await.unwrap();
    assert!(fields.is_some(), "Expected hash data from Redis after SQL insert");

    let fields = fields.unwrap();
    assert_eq!(fields.get("name"), Some(&UniversalValue::String("Bob Jones".to_string())));
}

/// Test: Data written via Graph is accessible via REST adapter
#[tokio::test]
async fn test_graph_to_rest_cross_protocol() {
    let (storage, registry) = setup().await;

    // Write via Graph adapter (Cypher style)
    let graph = AdapterFactory::cypher(Arc::clone(&storage), Arc::clone(&registry));

    let mut props = BTreeMap::new();
    props.insert("name".to_string(), UniversalValue::String("Charlie".to_string()));
    props.insert("department".to_string(), UniversalValue::String("Engineering".to_string()));
    graph.create_node("emp1", vec!["Employee".to_string()], props).await.unwrap();

    // Read via REST adapter - graph nodes are stored in "graph:nodes" namespace
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));

    // The graph adapter uses "graph:nodes" as its namespace
    let node = rest.get("graph:nodes", "emp1").await.unwrap();
    assert!(node.is_some(), "Expected node from REST after Graph write");

    // Verify the node properties
    if let Some(UniversalValue::Node { id, labels, properties }) = node {
        assert_eq!(id, "emp1");
        assert!(labels.contains(&"Employee".to_string()));
        assert_eq!(properties.get("name"), Some(&UniversalValue::String("Charlie".to_string())));
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
    redis.hset("products:p1", "name", UniversalValue::String("Widget".to_string())).await.unwrap();
    redis.hset("products:p1", "price", UniversalValue::Float(29.99)).await.unwrap();
    redis.hset("products:p1", "stock", UniversalValue::Int(100)).await.unwrap();

    // Create adapters for all protocols
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let mysql = AdapterFactory::mysql(Arc::clone(&storage), Arc::clone(&registry));
    let cql = AdapterFactory::cql(Arc::clone(&storage), Arc::clone(&registry));
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));

    // Read from all protocols
    let redis_data = redis.hgetall("products:p1").await.unwrap();
    let sql_rows = sql.select("products", None, None, None, None, None).await.unwrap();
    let mysql_rows = mysql.select("products", None, None, None, None, None).await.unwrap();
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
    user.insert("id".to_string(), UniversalValue::String("update_test".to_string()));
    user.insert("status".to_string(), UniversalValue::String("active".to_string()));
    sql.insert("accounts", user, "id").await.unwrap();

    // Verify via Redis
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    let fields = redis.hgetall("accounts:update_test").await.unwrap().unwrap();
    assert_eq!(fields.get("status"), Some(&UniversalValue::String("active".to_string())));

    // Update via SQL
    let mut updates = BTreeMap::new();
    updates.insert("status".to_string(), UniversalValue::String("suspended".to_string()));
    let filter = FilterExpression::Eq("id".to_string(), UniversalValue::String("update_test".to_string()));
    sql.update("accounts", updates, Some(filter)).await.unwrap();

    // Verify update via Redis
    let fields = redis.hgetall("accounts:update_test").await.unwrap().unwrap();
    assert_eq!(fields.get("status"), Some(&UniversalValue::String("suspended".to_string())));
}

/// Test: Deletes via one protocol affect all protocols
#[tokio::test]
async fn test_cross_protocol_deletes() {
    let (storage, registry) = setup().await;

    // Create via REST
    let rest = AdapterFactory::rest(Arc::clone(&storage), Arc::clone(&registry));
    let mut item = BTreeMap::new();
    item.insert("name".to_string(), UniversalValue::String("ToBeDeleted".to_string()));
    rest.create("items", "del1", UniversalValue::Map(item)).await.unwrap();

    // Verify exists via Redis
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    assert!(redis.exists(&["items:del1"]).await.unwrap() > 0);

    // Delete via REST
    rest.delete("items", "del1").await.unwrap();

    // Verify gone via Redis
    assert_eq!(redis.exists(&["items:del1"]).await.unwrap(), 0);

    // Verify gone via SQL
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = sql.select("items", None, None, None, None, None).await.unwrap();
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
        row.insert("name".to_string(), UniversalValue::String(format!("Item {}", i)));
        row.insert("category".to_string(), UniversalValue::String("test".to_string()));
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
    let keys: Vec<&str> = (1..=3).map(|i| match i {
        1 => "batch_items:1",
        2 => "batch_items:2",
        _ => "batch_items:3",
    }).collect();
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
    row.insert("id".to_string(), UniversalValue::String("uuid1".to_string()));
    row.insert("name".to_string(), UniversalValue::String("CQL Test".to_string()));
    row.insert("timestamp".to_string(), UniversalValue::Timestamp(1700000000000));
    cql.insert("events", row, "id").await.unwrap();

    // Read via SQL
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));
    let rows = sql.select("events", None, None, None, None, None).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("name"), Some(&UniversalValue::String("CQL Test".to_string())));
}

/// Test: Graph relationships work with unified storage
#[tokio::test]
async fn test_graph_relationships_unified() {
    let (storage, registry) = setup().await;

    // Create nodes via Graph adapter
    let graph = AdapterFactory::cypher(Arc::clone(&storage), Arc::clone(&registry));

    let mut props1 = BTreeMap::new();
    props1.insert("name".to_string(), UniversalValue::String("Node A".to_string()));
    graph.create_node("node_a", vec!["TestNode".to_string()], props1).await.unwrap();

    let mut props2 = BTreeMap::new();
    props2.insert("name".to_string(), UniversalValue::String("Node B".to_string()));
    graph.create_node("node_b", vec!["TestNode".to_string()], props2).await.unwrap();

    // Create relationship
    let edge_props = BTreeMap::new();
    graph.create_relationship("rel1", "CONNECTS_TO", "node_a", "node_b", edge_props).await.unwrap();

    // Query nodes by label
    let nodes = graph.get_nodes_by_label("TestNode").await.unwrap();
    assert_eq!(nodes.len(), 2);

    // Query relationships
    let rels = graph.get_relationships("node_a", Some("outgoing"), None).await.unwrap();
    assert_eq!(rels.len(), 1);
}

/// Test: Data types are preserved across protocols
#[tokio::test]
async fn test_data_type_preservation() {
    let (storage, registry) = setup().await;

    // Create with various data types via SQL
    let sql = AdapterFactory::postgres(Arc::clone(&storage), Arc::clone(&registry));

    let mut row = BTreeMap::new();
    row.insert("id".to_string(), UniversalValue::String("types_test".to_string()));
    row.insert("int_val".to_string(), UniversalValue::Int(42));
    row.insert("float_val".to_string(), UniversalValue::Float(3.14159));
    row.insert("bool_val".to_string(), UniversalValue::Bool(true));
    row.insert("null_val".to_string(), UniversalValue::Null);
    row.insert("bytes_val".to_string(), UniversalValue::Bytes(vec![1, 2, 3, 4]));
    sql.insert("typed_data", row, "id").await.unwrap();

    // Read via Redis and verify types
    let redis = AdapterFactory::redis(Arc::clone(&storage), Arc::clone(&registry));
    let fields = redis.hgetall("typed_data:types_test").await.unwrap().unwrap();

    assert_eq!(fields.get("int_val"), Some(&UniversalValue::Int(42)));
    assert_eq!(fields.get("float_val"), Some(&UniversalValue::Float(3.14159)));
    assert_eq!(fields.get("bool_val"), Some(&UniversalValue::Bool(true)));
    assert_eq!(fields.get("null_val"), Some(&UniversalValue::Null));
    assert_eq!(fields.get("bytes_val"), Some(&UniversalValue::Bytes(vec![1, 2, 3, 4])));
}
