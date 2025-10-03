//! Integration tests for RESP protocol with Redis client
//!
//! These tests use the `redis` crate to verify that our RESP implementation
//! works correctly with real Redis clients.

use redis::{Commands, Connection, RedisResult};
use orbit_protocols::resp::{RespServer, KeyValueActor, HashActor, ListActor};
use orbit_client::OrbitClient;
use tokio::time::{sleep, Duration};

const TEST_ADDR: &str = "127.0.0.1:16379";
const TEST_URL: &str = "redis://127.0.0.1:16379";

/// Helper to start test server
async fn start_test_server() -> Result<(), Box<dyn std::error::Error>> {
    // Create Orbit client
    let orbit_client = OrbitClient::builder()
        .with_namespace("resp-test")
        .build()
        .await?;

    // Start RESP server
    let server = RespServer::new(TEST_ADDR, orbit_client);
    
    tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for server to start
    sleep(Duration::from_millis(500)).await;
    
    Ok(())
}

/// Get Redis connection for tests
fn get_connection() -> RedisResult<Connection> {
    let client = redis::Client::open(TEST_URL)?;
    client.get_connection()
}

#[tokio::test]
async fn test_ping_pong() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    let pong: String = redis::cmd("PING")
        .query(&mut con)
        .expect("PING failed");
    
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn test_echo() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    let result: String = redis::cmd("ECHO")
        .arg("Hello, Orbit!")
        .query(&mut con)
        .expect("ECHO failed");
    
    assert_eq!(result, "Hello, Orbit!");
}

#[tokio::test]
async fn test_set_get() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // SET command
    let _: () = con.set("test_key", "test_value").expect("SET failed");
    
    // GET command
    let value: String = con.get("test_key").expect("GET failed");
    assert_eq!(value, "test_value");
}

#[tokio::test]
async fn test_set_with_expiration() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // SET with EX (expiration in seconds)
    let _: () = redis::cmd("SET")
        .arg("expire_key")
        .arg("expire_value")
        .arg("EX")
        .arg(10)
        .query(&mut con)
        .expect("SET EX failed");
    
    let value: String = con.get("expire_key").expect("GET failed");
    assert_eq!(value, "expire_value");
    
    // Check TTL
    let ttl: i64 = redis::cmd("TTL")
        .arg("expire_key")
        .query(&mut con)
        .expect("TTL failed");
    
    assert!(ttl > 0 && ttl <= 10);
}

#[tokio::test]
async fn test_exists() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Set a key
    let _: () = con.set("exists_key", "value").expect("SET failed");
    
    // EXISTS should return 1
    let exists: i64 = con.exists("exists_key").expect("EXISTS failed");
    assert_eq!(exists, 1);
    
    // Non-existent key should return 0
    let not_exists: i64 = con.exists("nonexistent_key").expect("EXISTS failed");
    assert_eq!(not_exists, 0);
}

#[tokio::test]
async fn test_del() {
    start_test_server().await.expect("Failed to connect");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Set keys
    let _: () = con.set("del_key1", "value1").expect("SET failed");
    let _: () = con.set("del_key2", "value2").expect("SET failed");
    
    // Delete keys
    let deleted: i64 = con.del(&["del_key1", "del_key2"]).expect("DEL failed");
    assert_eq!(deleted, 2);
    
    // Keys should not exist anymore
    let exists: i64 = con.exists("del_key1").expect("EXISTS failed");
    assert_eq!(exists, 0);
}

#[tokio::test]
async fn test_expire() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Set a key without expiration
    let _: () = con.set("expire_test", "value").expect("SET failed");
    
    // Set expiration
    let result: i64 = redis::cmd("EXPIRE")
        .arg("expire_test")
        .arg(10)
        .query(&mut con)
        .expect("EXPIRE failed");
    assert_eq!(result, 1);
    
    // Check TTL
    let ttl: i64 = redis::cmd("TTL")
        .arg("expire_test")
        .query(&mut con)
        .expect("TTL failed");
    assert!(ttl > 0 && ttl <= 10);
}

#[tokio::test]
async fn test_hset_hget() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // HSET command
    let new_fields: i64 = redis::cmd("HSET")
        .arg("hash_key")
        .arg("field1")
        .arg("value1")
        .arg("field2")
        .arg("value2")
        .query(&mut con)
        .expect("HSET failed");
    assert_eq!(new_fields, 2);
    
    // HGET command
    let value: String = redis::cmd("HGET")
        .arg("hash_key")
        .arg("field1")
        .query(&mut con)
        .expect("HGET failed");
    assert_eq!(value, "value1");
}

#[tokio::test]
async fn test_hgetall() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Set hash fields
    let _: i64 = redis::cmd("HSET")
        .arg("hash_all")
        .arg("field1")
        .arg("value1")
        .arg("field2")
        .arg("value2")
        .query(&mut con)
        .expect("HSET failed");
    
    // Get all fields
    let all: Vec<String> = redis::cmd("HGETALL")
        .arg("hash_all")
        .query(&mut con)
        .expect("HGETALL failed");
    
    assert_eq!(all.len(), 4); // [field1, value1, field2, value2]
    assert!(all.contains(&"field1".to_string()));
    assert!(all.contains(&"value1".to_string()));
}

#[tokio::test]
async fn test_hexists() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Set hash field
    let _: i64 = redis::cmd("HSET")
        .arg("hash_exists")
        .arg("existing_field")
        .arg("value")
        .query(&mut con)
        .expect("HSET failed");
    
    // Check existing field
    let exists: i64 = redis::cmd("HEXISTS")
        .arg("hash_exists")
        .arg("existing_field")
        .query(&mut con)
        .expect("HEXISTS failed");
    assert_eq!(exists, 1);
    
    // Check non-existing field
    let not_exists: i64 = redis::cmd("HEXISTS")
        .arg("hash_exists")
        .arg("nonexistent_field")
        .query(&mut con)
        .expect("HEXISTS failed");
    assert_eq!(not_exists, 0);
}

#[tokio::test]
async fn test_hdel() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Set hash fields
    let _: i64 = redis::cmd("HSET")
        .arg("hash_del")
        .arg("field1")
        .arg("value1")
        .arg("field2")
        .arg("value2")
        .query(&mut con)
        .expect("HSET failed");
    
    // Delete one field
    let deleted: i64 = redis::cmd("HDEL")
        .arg("hash_del")
        .arg("field1")
        .query(&mut con)
        .expect("HDEL failed");
    assert_eq!(deleted, 1);
    
    // Field should not exist
    let exists: i64 = redis::cmd("HEXISTS")
        .arg("hash_del")
        .arg("field1")
        .query(&mut con)
        .expect("HEXISTS failed");
    assert_eq!(exists, 0);
}

#[tokio::test]
async fn test_hkeys_hvals() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Set hash fields
    let _: i64 = redis::cmd("HSET")
        .arg("hash_kv")
        .arg("key1")
        .arg("val1")
        .arg("key2")
        .arg("val2")
        .query(&mut con)
        .expect("HSET failed");
    
    // Get keys
    let keys: Vec<String> = redis::cmd("HKEYS")
        .arg("hash_kv")
        .query(&mut con)
        .expect("HKEYS failed");
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"key1".to_string()));
    
    // Get values
    let vals: Vec<String> = redis::cmd("HVALS")
        .arg("hash_kv")
        .query(&mut con)
        .expect("HVALS failed");
    assert_eq!(vals.len(), 2);
    assert!(vals.contains(&"val1".to_string()));
}

#[tokio::test]
async fn test_lpush_lrange() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // LPUSH command
    let len: i64 = redis::cmd("LPUSH")
        .arg("list_key")
        .arg("item1")
        .arg("item2")
        .arg("item3")
        .query(&mut con)
        .expect("LPUSH failed");
    assert_eq!(len, 3);
    
    // LRANGE command
    let items: Vec<String> = redis::cmd("LRANGE")
        .arg("list_key")
        .arg(0)
        .arg(-1)
        .query(&mut con)
        .expect("LRANGE failed");
    assert_eq!(items, vec!["item3", "item2", "item1"]);
}

#[tokio::test]
async fn test_rpush() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // RPUSH command
    let len: i64 = redis::cmd("RPUSH")
        .arg("list_rpush")
        .arg("a")
        .arg("b")
        .arg("c")
        .query(&mut con)
        .expect("RPUSH failed");
    assert_eq!(len, 3);
    
    // Items should be in order
    let items: Vec<String> = redis::cmd("LRANGE")
        .arg("list_rpush")
        .arg(0)
        .arg(-1)
        .query(&mut con)
        .expect("LRANGE failed");
    assert_eq!(items, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn test_lpop_rpop() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Setup list
    let _: i64 = redis::cmd("RPUSH")
        .arg("list_pop")
        .arg("first")
        .arg("middle")
        .arg("last")
        .query(&mut con)
        .expect("RPUSH failed");
    
    // LPOP
    let item: String = redis::cmd("LPOP")
        .arg("list_pop")
        .query(&mut con)
        .expect("LPOP failed");
    assert_eq!(item, "first");
    
    // RPOP
    let item: String = redis::cmd("RPOP")
        .arg("list_pop")
        .query(&mut con)
        .expect("RPOP failed");
    assert_eq!(item, "last");
    
    // Check remaining
    let len: i64 = redis::cmd("LLEN")
        .arg("list_pop")
        .query(&mut con)
        .expect("LLEN failed");
    assert_eq!(len, 1);
}

#[tokio::test]
async fn test_llen() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Empty list
    let len: i64 = redis::cmd("LLEN")
        .arg("empty_list")
        .query(&mut con)
        .expect("LLEN failed");
    assert_eq!(len, 0);
    
    // Add items
    let _: i64 = redis::cmd("RPUSH")
        .arg("test_list")
        .arg("a")
        .arg("b")
        .arg("c")
        .query(&mut con)
        .expect("RPUSH failed");
    
    // Check length
    let len: i64 = redis::cmd("LLEN")
        .arg("test_list")
        .query(&mut con)
        .expect("LLEN failed");
    assert_eq!(len, 3);
}

#[tokio::test]
async fn test_info() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    let info: String = redis::cmd("INFO")
        .query(&mut con)
        .expect("INFO failed");
    
    assert!(info.contains("orbit"));
    assert!(info.contains("redis_version"));
}

#[tokio::test]
async fn test_concurrent_connections() {
    start_test_server().await.expect("Failed to start server");
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            let mut con = get_connection().expect("Failed to connect");
            
            let key = format!("concurrent_key_{}", i);
            let value = format!("value_{}", i);
            
            // Set
            let _: () = con.set(&key, &value).expect("SET failed");
            
            // Get
            let retrieved: String = con.get(&key).expect("GET failed");
            assert_eq!(retrieved, value);
        });
        
        handles.push(handle);
    }
    
    for handle in handles {
        handle.await.expect("Task failed");
    }
}

#[tokio::test]
async fn test_multiple_data_types() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // String
    let _: () = con.set("string_key", "string_value").expect("SET failed");
    
    // Hash
    let _: i64 = redis::cmd("HSET")
        .arg("hash_key")
        .arg("field")
        .arg("value")
        .query(&mut con)
        .expect("HSET failed");
    
    // List
    let _: i64 = redis::cmd("LPUSH")
        .arg("list_key")
        .arg("item")
        .query(&mut con)
        .expect("LPUSH failed");
    
    // Verify all exist
    let string_val: String = con.get("string_key").expect("GET failed");
    assert_eq!(string_val, "string_value");
    
    let hash_val: String = redis::cmd("HGET")
        .arg("hash_key")
        .arg("field")
        .query(&mut con)
        .expect("HGET failed");
    assert_eq!(hash_val, "value");
    
    let list_len: i64 = redis::cmd("LLEN")
        .arg("list_key")
        .query(&mut con)
        .expect("LLEN failed");
    assert_eq!(list_len, 1);
}

#[tokio::test]
async fn test_error_handling() {
    start_test_server().await.expect("Failed to start server");
    
    let mut con = get_connection().expect("Failed to connect");
    
    // Wrong number of arguments
    let result: RedisResult<String> = redis::cmd("GET").query(&mut con);
    assert!(result.is_err());
    
    // Invalid command
    let result: RedisResult<String> = redis::cmd("INVALID_COMMAND")
        .arg("arg")
        .query(&mut con);
    assert!(result.is_err());
}
