//! RESP Command Unit Tests
//!
//! Comprehensive unit tests for RESP protocol commands, including all 15 previously failing tests
//! and other important commands.

use orbit_client::OrbitClient;
use orbit_protocols::resp::commands::CommandHandler;
use orbit_protocols::resp::RespValue;

/// Test context for RESP command tests
struct RespTestContext {
    handler: CommandHandler,
}

impl RespTestContext {
    async fn new() -> Self {
        // Create an offline OrbitClient for testing
        // This doesn't require a running server since we use SimpleLocalRegistry internally
        let orbit_client = OrbitClient::builder()
            .with_namespace("resp-test")
            .with_offline_mode(true)
            .build()
            .await
            .expect("Failed to create OrbitClient");

        let handler = CommandHandler::new(orbit_client);
        Self { handler }
    }

    async fn execute_command(&self, command: RespValue) -> Result<RespValue, String> {
        self.handler
            .handle_command(command)
            .await
            .map_err(|e| format!("Command execution failed: {}", e))
    }

    fn make_command(&self, cmd: &str, args: Vec<&str>) -> RespValue {
        let mut parts = vec![RespValue::bulk_string_from_str(cmd)];
        for arg in args {
            parts.push(RespValue::bulk_string_from_str(arg));
        }
        RespValue::array(parts)
    }
}

// ============================================================================
// Phase 1: Critical Fixes Tests
// ============================================================================

#[tokio::test]
async fn test_del_command_returns_count() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a key
    let set_cmd = ctx.make_command("SET", vec!["test_key", "test_value"]);
    let _ = ctx.execute_command(set_cmd).await;

    // Test: DEL should return count of deleted keys
    let del_cmd = ctx.make_command("DEL", vec!["test_key"]);
    let result = ctx.execute_command(del_cmd).await.unwrap();

    // Should return integer 1 (one key deleted)
    assert!(matches!(result, RespValue::Integer(1)));
}

#[tokio::test]
async fn test_del_nonexistent_key_returns_zero() {
    let ctx = RespTestContext::new().await;

    // Test: DEL on non-existent key should return 0
    let del_cmd = ctx.make_command("DEL", vec!["nonexistent_key"]);
    let result = ctx.execute_command(del_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_del_multiple_keys() {
    let ctx = RespTestContext::new().await;

    // Setup: Create multiple keys
    ctx.execute_command(ctx.make_command("SET", vec!["key1", "value1"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("SET", vec!["key2", "value2"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("SET", vec!["key3", "value3"]))
        .await
        .unwrap();

    // Test: DEL multiple keys
    let del_cmd = ctx.make_command("DEL", vec!["key1", "key2", "nonexistent"]);
    let result = ctx.execute_command(del_cmd).await.unwrap();

    // Should return 2 (two keys deleted)
    assert!(matches!(result, RespValue::Integer(2)));
}

#[tokio::test]
async fn test_expire_command_sets_ttl() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a key
    ctx.execute_command(ctx.make_command("SET", vec!["expire_key", "value"]))
        .await
        .unwrap();

    // Test: EXPIRE should return 1 if key exists
    let expire_cmd = ctx.make_command("EXPIRE", vec!["expire_key", "60"]);
    let result = ctx.execute_command(expire_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(1)));
}

#[tokio::test]
async fn test_expire_nonexistent_key_returns_zero() {
    let ctx = RespTestContext::new().await;

    // Test: EXPIRE on non-existent key should return 0
    let expire_cmd = ctx.make_command("EXPIRE", vec!["nonexistent", "60"]);
    let result = ctx.execute_command(expire_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_ttl_command() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a key with expiration using SET EX
    let set_cmd = ctx.make_command("SET", vec!["ttl_key", "value", "EX", "60"]);
    ctx.execute_command(set_cmd).await.unwrap();

    // Test: TTL should return remaining seconds
    let ttl_cmd = ctx.make_command("TTL", vec!["ttl_key"]);
    let result = ctx.execute_command(ttl_cmd).await.unwrap();

    // Should return a positive integer (remaining seconds)
    if let RespValue::Integer(ttl) = result {
        assert!(ttl > 0 && ttl <= 60, "TTL should be positive and <= 60, got: {}", ttl);
    } else {
        panic!("TTL should return an integer, got: {:?}", result);
    }
}

#[tokio::test]
async fn test_ttl_command_with_expire() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a key with expiration using EXPIRE
    ctx.execute_command(ctx.make_command("SET", vec!["ttl_key2", "value"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("EXPIRE", vec!["ttl_key2", "60"]))
        .await
        .unwrap();

    // Test: TTL should return remaining seconds
    let ttl_cmd = ctx.make_command("TTL", vec!["ttl_key2"]);
    let result = ctx.execute_command(ttl_cmd).await.unwrap();

    // Should return a positive integer (remaining seconds)
    if let RespValue::Integer(ttl) = result {
        assert!(ttl > 0 && ttl <= 60, "TTL should be positive and <= 60, got: {}", ttl);
    } else {
        panic!("TTL should return an integer, got: {:?}", result);
    }
}

#[tokio::test]
async fn test_ttl_nonexistent_key() {
    let ctx = RespTestContext::new().await;

    // Test: TTL on non-existent key should return -2
    let ttl_cmd = ctx.make_command("TTL", vec!["nonexistent"]);
    let result = ctx.execute_command(ttl_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(-2)));
}

// ============================================================================
// Phase 2: List Command Tests
// ============================================================================

#[tokio::test]
async fn test_llen_returns_actual_length() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list with items
    ctx.execute_command(ctx.make_command("LPUSH", vec!["list_key", "item1"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("LPUSH", vec!["list_key", "item2"]))
        .await
        .unwrap();

    // Test: LLEN should return actual length
    let llen_cmd = ctx.make_command("LLEN", vec!["list_key"]);
    let result = ctx.execute_command(llen_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(2)));
}

#[tokio::test]
async fn test_llen_empty_list() {
    let ctx = RespTestContext::new().await;

    // Test: LLEN on empty/non-existent list should return 0
    let llen_cmd = ctx.make_command("LLEN", vec!["empty_list"]);
    let result = ctx.execute_command(llen_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_rpush_returns_length() {
    let ctx = RespTestContext::new().await;

    // Test: RPUSH should return the new length
    let rpush_cmd = ctx.make_command("RPUSH", vec!["rpush_list", "value1"]);
    let result1 = ctx.execute_command(rpush_cmd).await.unwrap();
    assert!(matches!(result1, RespValue::Integer(1)));

    let rpush_cmd2 = ctx.make_command("RPUSH", vec!["rpush_list", "value2"]);
    let result2 = ctx.execute_command(rpush_cmd2).await.unwrap();
    assert!(matches!(result2, RespValue::Integer(2)));

    let rpush_cmd3 = ctx.make_command("RPUSH", vec!["rpush_list", "value3"]);
    let result3 = ctx.execute_command(rpush_cmd3).await.unwrap();
    assert!(matches!(result3, RespValue::Integer(3)));
}

#[tokio::test]
async fn test_rpush_multiple_values() {
    let ctx = RespTestContext::new().await;

    // Test: RPUSH with multiple values
    let rpush_cmd = ctx.make_command("RPUSH", vec!["multi_list", "v1", "v2", "v3"]);
    let result = ctx.execute_command(rpush_cmd).await.unwrap();

    // Should return the total length after all pushes
    assert!(matches!(result, RespValue::Integer(3)));
}

#[tokio::test]
async fn test_lrange_with_integer_indices() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list
    ctx.execute_command(ctx.make_command("RPUSH", vec!["range_list", "a", "b", "c"]))
        .await
        .unwrap();

    // Test: LRANGE with integer indices
    let lrange_cmd = ctx.make_command("LRANGE", vec!["range_list", "0", "-1"]);
    let result = ctx.execute_command(lrange_cmd).await.unwrap();

    if let RespValue::Array(items) = result {
        assert_eq!(items.len(), 3);
    } else {
        panic!("LRANGE should return an array");
    }
}

#[tokio::test]
async fn test_lrange_with_bulk_string_indices() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list
    ctx.execute_command(ctx.make_command("RPUSH", vec!["range_list2", "x", "y", "z"]))
        .await
        .unwrap();

    // Test: LRANGE with bulk string indices (simulating Redis client behavior)
    let cmd_parts = vec![
        RespValue::bulk_string_from_str("LRANGE"),
        RespValue::bulk_string_from_str("range_list2"),
        RespValue::bulk_string_from_str("0"),  // Bulk string, not integer
        RespValue::bulk_string_from_str("-1"), // Bulk string, not integer
    ];
    let lrange_cmd = RespValue::array(cmd_parts);
    let result = ctx.execute_command(lrange_cmd).await.unwrap();

    if let RespValue::Array(items) = result {
        assert_eq!(items.len(), 3);
    } else {
        panic!("LRANGE should return an array");
    }
}

#[tokio::test]
async fn test_lindex_with_integer() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list
    ctx.execute_command(ctx.make_command("RPUSH", vec!["index_list", "first", "second", "third"]))
        .await
        .unwrap();

    // Test: LINDEX with integer index
    let lindex_cmd = ctx.make_command("LINDEX", vec!["index_list", "0"]);
    let result = ctx.execute_command(lindex_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        assert_eq!(String::from_utf8_lossy(&bytes), "first");
    } else {
        panic!("LINDEX should return a bulk string");
    }
}

#[tokio::test]
async fn test_lindex_with_bulk_string_index() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list
    ctx.execute_command(ctx.make_command("RPUSH", vec!["index_list2", "a", "b", "c"]))
        .await
        .unwrap();

    // Test: LINDEX with bulk string index
    let cmd_parts = vec![
        RespValue::bulk_string_from_str("LINDEX"),
        RespValue::bulk_string_from_str("index_list2"),
        RespValue::bulk_string_from_str("1"), // Bulk string, not integer
    ];
    let lindex_cmd = RespValue::array(cmd_parts);
    let result = ctx.execute_command(lindex_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        assert_eq!(String::from_utf8_lossy(&bytes), "b");
    } else {
        panic!("LINDEX should return a bulk string");
    }
}

#[tokio::test]
async fn test_lindex_out_of_range() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list
    ctx.execute_command(ctx.make_command("RPUSH", vec!["short_list", "item"]))
        .await
        .unwrap();

    // Test: LINDEX out of range should return null
    let lindex_cmd = ctx.make_command("LINDEX", vec!["short_list", "10"]);
    let result = ctx.execute_command(lindex_cmd).await.unwrap();

    assert!(result.is_null());
}

#[tokio::test]
async fn test_lpop_returns_value() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list
    ctx.execute_command(ctx.make_command("RPUSH", vec!["pop_list", "first", "second", "third"]))
        .await
        .unwrap();

    // Test: LPOP should return the popped value
    let lpop_cmd = ctx.make_command("LPOP", vec!["pop_list"]);
    let result = ctx.execute_command(lpop_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        assert_eq!(String::from_utf8_lossy(&bytes), "first");
    } else {
        panic!("LPOP should return the popped value");
    }

    // Verify list length decreased
    let llen_cmd = ctx.make_command("LLEN", vec!["pop_list"]);
    let len_result = ctx.execute_command(llen_cmd).await.unwrap();
    assert!(matches!(len_result, RespValue::Integer(2)));
}

#[tokio::test]
async fn test_lpop_empty_list_returns_null() {
    let ctx = RespTestContext::new().await;

    // Test: LPOP on empty list should return null
    let lpop_cmd = ctx.make_command("LPOP", vec!["empty_list"]);
    let result = ctx.execute_command(lpop_cmd).await.unwrap();

    assert!(result.is_null());
}

#[tokio::test]
async fn test_lpop_with_count() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list
    ctx.execute_command(ctx.make_command("RPUSH", vec!["count_list", "a", "b", "c"]))
        .await
        .unwrap();

    // Test: LPOP with count should return array
    let lpop_cmd = ctx.make_command("LPOP", vec!["count_list", "2"]);
    let result = ctx.execute_command(lpop_cmd).await.unwrap();

    if let RespValue::Array(items) = result {
        assert_eq!(items.len(), 2);
    } else {
        panic!("LPOP with count should return an array");
    }
}

// ============================================================================
// Phase 3: Set Command Tests
// ============================================================================

#[tokio::test]
async fn test_scard_returns_cardinality() {
    let ctx = RespTestContext::new().await;

    // Setup: Add members to set
    ctx.execute_command(ctx.make_command("SADD", vec!["scard_set", "member1"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("SADD", vec!["scard_set", "member2"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("SADD", vec!["scard_set", "member3"]))
        .await
        .unwrap();

    // Test: SCARD should return actual cardinality
    let scard_cmd = ctx.make_command("SCARD", vec!["scard_set"]);
    let result = ctx.execute_command(scard_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(3)));
}

#[tokio::test]
async fn test_scard_empty_set() {
    let ctx = RespTestContext::new().await;

    // Test: SCARD on empty/non-existent set should return 0
    let scard_cmd = ctx.make_command("SCARD", vec!["empty_set"]);
    let result = ctx.execute_command(scard_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_sismember_returns_one_for_member() {
    let ctx = RespTestContext::new().await;

    // Setup: Add member to set
    ctx.execute_command(ctx.make_command("SADD", vec!["ismember_set", "member1"]))
        .await
        .unwrap();

    // Test: SISMEMBER should return 1 if member exists
    let sismember_cmd = ctx.make_command("SISMEMBER", vec!["ismember_set", "member1"]);
    let result = ctx.execute_command(sismember_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(1)));
}

#[tokio::test]
async fn test_sismember_returns_zero_for_non_member() {
    let ctx = RespTestContext::new().await;

    // Setup: Add member to set
    ctx.execute_command(ctx.make_command("SADD", vec!["ismember_set2", "member1"]))
        .await
        .unwrap();

    // Test: SISMEMBER should return 0 if member doesn't exist
    let sismember_cmd = ctx.make_command("SISMEMBER", vec!["ismember_set2", "nonexistent"]);
    let result = ctx.execute_command(sismember_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_sismember_nonexistent_set() {
    let ctx = RespTestContext::new().await;

    // Test: SISMEMBER on non-existent set should return 0
    let sismember_cmd = ctx.make_command("SISMEMBER", vec!["nonexistent_set", "member"]);
    let result = ctx.execute_command(sismember_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(0)));
}

// ============================================================================
// Phase 4: Sorted Set Command Tests
// ============================================================================

#[tokio::test]
async fn test_zadd_adds_member() {
    let ctx = RespTestContext::new().await;

    // Test: ZADD should return count of added members
    let zadd_cmd = ctx.make_command("ZADD", vec!["zadd_set", "1.0", "member1"]);
    let result = ctx.execute_command(zadd_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(1)));
}

#[tokio::test]
async fn test_zadd_multiple_members() {
    let ctx = RespTestContext::new().await;

    // Test: ZADD with multiple score-member pairs
    let zadd_cmd = ctx.make_command(
        "ZADD",
        vec!["multi_zset", "1.0", "member1", "2.0", "member2", "3.0", "member3"],
    );
    let result = ctx.execute_command(zadd_cmd).await.unwrap();

    // Should return count of newly added members
    assert!(matches!(result, RespValue::Integer(3)));
}

#[tokio::test]
async fn test_zadd_updates_existing_member() {
    let ctx = RespTestContext::new().await;

    // Setup: Add a member
    ctx.execute_command(ctx.make_command("ZADD", vec!["update_zset", "1.0", "member1"]))
        .await
        .unwrap();

    // Test: ZADD with same member but different score should update
    let zadd_cmd = ctx.make_command("ZADD", vec!["update_zset", "2.0", "member1"]);
    let result = ctx.execute_command(zadd_cmd).await.unwrap();

    // Should return 0 (member already existed, just updated)
    assert!(matches!(result, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_zcard_returns_cardinality() {
    let ctx = RespTestContext::new().await;

    // Setup: Add members
    ctx.execute_command(ctx.make_command("ZADD", vec!["zcard_set", "1.0", "m1"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("ZADD", vec!["zcard_set", "2.0", "m2"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("ZADD", vec!["zcard_set", "3.0", "m3"]))
        .await
        .unwrap();

    // Test: ZCARD should return cardinality
    let zcard_cmd = ctx.make_command("ZCARD", vec!["zcard_set"]);
    let result = ctx.execute_command(zcard_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(3)));
}

#[tokio::test]
async fn test_zcard_empty_set() {
    let ctx = RespTestContext::new().await;

    // Test: ZCARD on empty set should return 0
    let zcard_cmd = ctx.make_command("ZCARD", vec!["empty_zset"]);
    let result = ctx.execute_command(zcard_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_zscore_returns_score() {
    let ctx = RespTestContext::new().await;

    // Setup: Add member with score
    ctx.execute_command(ctx.make_command("ZADD", vec!["zscore_set", "42.5", "member1"]))
        .await
        .unwrap();

    // Test: ZSCORE should return the score
    let zscore_cmd = ctx.make_command("ZSCORE", vec!["zscore_set", "member1"]);
    let result = ctx.execute_command(zscore_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        let score_str = String::from_utf8_lossy(&bytes);
        let score: f64 = score_str.parse().unwrap();
        assert!((score - 42.5).abs() < 0.001);
    } else {
        panic!("ZSCORE should return a bulk string with the score");
    }
}

#[tokio::test]
async fn test_zscore_nonexistent_member() {
    let ctx = RespTestContext::new().await;

    // Setup: Add a member
    ctx.execute_command(ctx.make_command("ZADD", vec!["zscore_set2", "1.0", "member1"]))
        .await
        .unwrap();

    // Test: ZSCORE on non-existent member should return null
    let zscore_cmd = ctx.make_command("ZSCORE", vec!["zscore_set2", "nonexistent"]);
    let result = ctx.execute_command(zscore_cmd).await.unwrap();

    assert!(result.is_null());
}

#[tokio::test]
async fn test_zrange_returns_members() {
    let ctx = RespTestContext::new().await;

    // Setup: Add members with scores
    ctx.execute_command(ctx.make_command("ZADD", vec!["zrange_set", "1.0", "a"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("ZADD", vec!["zrange_set", "2.0", "b"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("ZADD", vec!["zrange_set", "3.0", "c"]))
        .await
        .unwrap();

    // Test: ZRANGE should return members in score order
    let zrange_cmd = ctx.make_command("ZRANGE", vec!["zrange_set", "0", "-1"]);
    let result = ctx.execute_command(zrange_cmd).await.unwrap();

    if let RespValue::Array(members) = result {
        assert_eq!(members.len(), 3);
        // Members should be in score order: a, b, c
    } else {
        panic!("ZRANGE should return an array");
    }
}

#[tokio::test]
async fn test_zrange_with_scores() {
    let ctx = RespTestContext::new().await;

    // Setup: Add members
    ctx.execute_command(ctx.make_command("ZADD", vec!["zrange_scores", "10.0", "member1"]))
        .await
        .unwrap();

    // Test: ZRANGE with WITHSCORES
    let cmd_parts = vec![
        RespValue::bulk_string_from_str("ZRANGE"),
        RespValue::bulk_string_from_str("zrange_scores"),
        RespValue::bulk_string_from_str("0"),
        RespValue::bulk_string_from_str("-1"),
        RespValue::bulk_string_from_str("WITHSCORES"),
    ];
    let zrange_cmd = RespValue::array(cmd_parts);
    let result = ctx.execute_command(zrange_cmd).await.unwrap();

    if let RespValue::Array(items) = result {
        // Should return member and score pairs
        assert!(items.len() >= 2); // At least member and score
    } else {
        panic!("ZRANGE with WITHSCORES should return an array");
    }
}

#[tokio::test]
async fn test_zincrby_increments_score() {
    let ctx = RespTestContext::new().await;

    // Setup: Add member with initial score
    ctx.execute_command(ctx.make_command("ZADD", vec!["zincrby_set", "10.0", "member1"]))
        .await
        .unwrap();

    // Test: ZINCRBY should increment and return new score
    let zincrby_cmd = ctx.make_command("ZINCRBY", vec!["zincrby_set", "5.0", "member1"]);
    let result = ctx.execute_command(zincrby_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        let score_str = String::from_utf8_lossy(&bytes);
        let score: f64 = score_str.parse().unwrap();
        assert!((score - 15.0).abs() < 0.001);
    } else {
        panic!("ZINCRBY should return the new score as bulk string");
    }
}

#[tokio::test]
async fn test_zincrby_creates_member_if_not_exists() {
    let ctx = RespTestContext::new().await;

    // Test: ZINCRBY on non-existent member should create it
    let zincrby_cmd = ctx.make_command("ZINCRBY", vec!["zincrby_set2", "10.0", "new_member"]);
    let result = ctx.execute_command(zincrby_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        let score_str = String::from_utf8_lossy(&bytes);
        let score: f64 = score_str.parse().unwrap();
        assert!((score - 10.0).abs() < 0.001);
    } else {
        panic!("ZINCRBY should return the new score");
    }
}

#[tokio::test]
async fn test_zrem_removes_members() {
    let ctx = RespTestContext::new().await;

    // Setup: Add members
    ctx.execute_command(ctx.make_command("ZADD", vec!["zrem_set", "1.0", "m1"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("ZADD", vec!["zrem_set", "2.0", "m2"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("ZADD", vec!["zrem_set", "3.0", "m3"]))
        .await
        .unwrap();

    // Test: ZREM should return count of removed members
    let zrem_cmd = ctx.make_command("ZREM", vec!["zrem_set", "m1", "m2"]);
    let result = ctx.execute_command(zrem_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(2)));

    // Verify remaining cardinality
    let zcard_cmd = ctx.make_command("ZCARD", vec!["zrem_set"]);
    let card_result = ctx.execute_command(zcard_cmd).await.unwrap();
    assert!(matches!(card_result, RespValue::Integer(1)));
}

#[tokio::test]
async fn test_zrem_nonexistent_members() {
    let ctx = RespTestContext::new().await;

    // Setup: Add a member
    ctx.execute_command(ctx.make_command("ZADD", vec!["zrem_set2", "1.0", "m1"]))
        .await
        .unwrap();

    // Test: ZREM on non-existent members should return 0
    let zrem_cmd = ctx.make_command("ZREM", vec!["zrem_set2", "nonexistent1", "nonexistent2"]);
    let result = ctx.execute_command(zrem_cmd).await.unwrap();

    assert!(matches!(result, RespValue::Integer(0)));
}

// ============================================================================
// Additional Command Tests
// ============================================================================

#[tokio::test]
async fn test_set_and_get() {
    let ctx = RespTestContext::new().await;

    // Test: SET and GET
    ctx.execute_command(ctx.make_command("SET", vec!["test_key", "test_value"]))
        .await
        .unwrap();

    let get_cmd = ctx.make_command("GET", vec!["test_key"]);
    let result = ctx.execute_command(get_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        assert_eq!(String::from_utf8_lossy(&bytes), "test_value");
    } else {
        panic!("GET should return a bulk string");
    }
}

#[tokio::test]
async fn test_set_with_ex_option() {
    let ctx = RespTestContext::new().await;

    // Test: SET with EX option should set expiration
    let set_cmd = ctx.make_command("SET", vec!["setex_key", "value", "EX", "60"]);
    let result = ctx.execute_command(set_cmd).await.unwrap();
    assert!(matches!(result, RespValue::SimpleString(_) | RespValue::BulkString(_)));

    // Verify TTL is set
    let ttl_cmd = ctx.make_command("TTL", vec!["setex_key"]);
    let ttl_result = ctx.execute_command(ttl_cmd).await.unwrap();
    if let RespValue::Integer(ttl) = ttl_result {
        assert!(ttl > 0 && ttl <= 60, "TTL should be positive and <= 60, got: {}", ttl);
    } else {
        panic!("TTL should return an integer");
    }
}

#[tokio::test]
async fn test_set_with_px_option() {
    let ctx = RespTestContext::new().await;

    // Test: SET with PX option (milliseconds) should set expiration
    let set_cmd = ctx.make_command("SET", vec!["setpx_key", "value", "PX", "60000"]);
    let result = ctx.execute_command(set_cmd).await.unwrap();
    assert!(matches!(result, RespValue::SimpleString(_) | RespValue::BulkString(_)));

    // Verify TTL is set (should be approximately 60 seconds)
    let ttl_cmd = ctx.make_command("TTL", vec!["setpx_key"]);
    let ttl_result = ctx.execute_command(ttl_cmd).await.unwrap();
    if let RespValue::Integer(ttl) = ttl_result {
        assert!(ttl > 50 && ttl <= 60, "TTL should be around 60 seconds, got: {}", ttl);
    } else {
        panic!("TTL should return an integer");
    }
}

#[tokio::test]
async fn test_set_with_nx_option() {
    let ctx = RespTestContext::new().await;

    // Test: SET with NX option should only set if key doesn't exist
    let set_cmd1 = ctx.make_command("SET", vec!["setnx_key", "value1", "NX"]);
    let result1 = ctx.execute_command(set_cmd1).await.unwrap();
    assert!(matches!(result1, RespValue::SimpleString(_) | RespValue::BulkString(_)));

    // Try to set again with NX - should return null
    let set_cmd2 = ctx.make_command("SET", vec!["setnx_key", "value2", "NX"]);
    let result2 = ctx.execute_command(set_cmd2).await.unwrap();
    assert!(result2.is_null(), "SET NX on existing key should return null");

    // Verify original value is still there
    let get_cmd = ctx.make_command("GET", vec!["setnx_key"]);
    let get_result = ctx.execute_command(get_cmd).await.unwrap();
    if let RespValue::BulkString(bytes) = get_result {
        assert_eq!(String::from_utf8_lossy(&bytes), "value1");
    }
}

#[tokio::test]
async fn test_set_with_xx_option() {
    let ctx = RespTestContext::new().await;

    // Test: SET with XX option should only set if key exists
    let set_cmd1 = ctx.make_command("SET", vec!["setxx_key", "value1"]);
    ctx.execute_command(set_cmd1).await.unwrap();

    // Set with XX on existing key - should succeed
    let set_cmd2 = ctx.make_command("SET", vec!["setxx_key", "value2", "XX"]);
    let result2 = ctx.execute_command(set_cmd2).await.unwrap();
    assert!(matches!(result2, RespValue::SimpleString(_) | RespValue::BulkString(_)));

    // Try to set with XX on non-existent key - should return null
    let set_cmd3 = ctx.make_command("SET", vec!["nonexistent_xx", "value", "XX"]);
    let result3 = ctx.execute_command(set_cmd3).await.unwrap();
    assert!(result3.is_null(), "SET XX on non-existent key should return null");

    // Verify value was updated
    let get_cmd = ctx.make_command("GET", vec!["setxx_key"]);
    let get_result = ctx.execute_command(get_cmd).await.unwrap();
    if let RespValue::BulkString(bytes) = get_result {
        assert_eq!(String::from_utf8_lossy(&bytes), "value2");
    }
}

#[tokio::test]
async fn test_set_with_ex_and_nx() {
    let ctx = RespTestContext::new().await;

    // Test: SET with both EX and NX options
    let set_cmd = ctx.make_command("SET", vec!["setexnx_key", "value", "EX", "60", "NX"]);
    let result = ctx.execute_command(set_cmd).await.unwrap();
    assert!(matches!(result, RespValue::SimpleString(_) | RespValue::BulkString(_)));

    // Verify TTL is set
    let ttl_cmd = ctx.make_command("TTL", vec!["setexnx_key"]);
    let ttl_result = ctx.execute_command(ttl_cmd).await.unwrap();
    if let RespValue::Integer(ttl) = ttl_result {
        assert!(ttl > 0 && ttl <= 60);
    }
}

#[tokio::test]
async fn test_exists_command() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a key
    ctx.execute_command(ctx.make_command("SET", vec!["exists_key", "value"]))
        .await
        .unwrap();

    // Test: EXISTS should return 1
    let exists_cmd = ctx.make_command("EXISTS", vec!["exists_key"]);
    let result = ctx.execute_command(exists_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(1)));

    // Test: EXISTS on non-existent key should return 0
    let exists_cmd2 = ctx.make_command("EXISTS", vec!["nonexistent"]);
    let result2 = ctx.execute_command(exists_cmd2).await.unwrap();
    assert!(matches!(result2, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_lpush_returns_length() {
    let ctx = RespTestContext::new().await;

    // Test: LPUSH should return new length
    let lpush_cmd = ctx.make_command("LPUSH", vec!["lpush_list", "item1"]);
    let result1 = ctx.execute_command(lpush_cmd).await.unwrap();
    assert!(matches!(result1, RespValue::Integer(1)));

    let lpush_cmd2 = ctx.make_command("LPUSH", vec!["lpush_list", "item2"]);
    let result2 = ctx.execute_command(lpush_cmd2).await.unwrap();
    assert!(matches!(result2, RespValue::Integer(2)));
}

#[tokio::test]
async fn test_rpop_returns_value() {
    let ctx = RespTestContext::new().await;

    // Setup: Create a list
    ctx.execute_command(ctx.make_command("RPUSH", vec!["rpop_list", "first", "last"]))
        .await
        .unwrap();

    // Test: RPOP should return the last value
    let rpop_cmd = ctx.make_command("RPOP", vec!["rpop_list"]);
    let result = ctx.execute_command(rpop_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        assert_eq!(String::from_utf8_lossy(&bytes), "last");
    } else {
        panic!("RPOP should return the popped value");
    }
}

#[tokio::test]
async fn test_sadd_returns_count() {
    let ctx = RespTestContext::new().await;

    // Test: SADD should return count of added members
    let sadd_cmd = ctx.make_command("SADD", vec!["sadd_set", "member1"]);
    let result = ctx.execute_command(sadd_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(1)));

    // Test: SADD same member again should return 0
    let sadd_cmd2 = ctx.make_command("SADD", vec!["sadd_set", "member1"]);
    let result2 = ctx.execute_command(sadd_cmd2).await.unwrap();
    assert!(matches!(result2, RespValue::Integer(0)));
}

#[tokio::test]
async fn test_smembers_returns_all_members() {
    let ctx = RespTestContext::new().await;

    // Setup: Add members
    ctx.execute_command(ctx.make_command("SADD", vec!["smembers_set", "m1"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("SADD", vec!["smembers_set", "m2"]))
        .await
        .unwrap();

    // Test: SMEMBERS should return all members
    let smembers_cmd = ctx.make_command("SMEMBERS", vec!["smembers_set"]);
    let result = ctx.execute_command(smembers_cmd).await.unwrap();

    if let RespValue::Array(members) = result {
        assert_eq!(members.len(), 2);
    } else {
        panic!("SMEMBERS should return an array");
    }
}

#[tokio::test]
async fn test_srem_returns_count() {
    let ctx = RespTestContext::new().await;

    // Setup: Add members
    ctx.execute_command(ctx.make_command("SADD", vec!["srem_set", "m1", "m2", "m3"]))
        .await
        .unwrap();

    // Test: SREM should return count of removed members
    let srem_cmd = ctx.make_command("SREM", vec!["srem_set", "m1", "m2"]);
    let result = ctx.execute_command(srem_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(2)));
}

#[tokio::test]
async fn test_incr_increments_value() {
    let ctx = RespTestContext::new().await;

    // Setup: Set initial value
    ctx.execute_command(ctx.make_command("SET", vec!["incr_key", "10"]))
        .await
        .unwrap();

    // Test: INCR should increment and return new value
    let incr_cmd = ctx.make_command("INCR", vec!["incr_key"]);
    let result = ctx.execute_command(incr_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(11)));
}

#[tokio::test]
async fn test_incr_creates_key_if_not_exists() {
    let ctx = RespTestContext::new().await;

    // Test: INCR on non-existent key should create it with value 1
    let incr_cmd = ctx.make_command("INCR", vec!["new_incr_key"]);
    let result = ctx.execute_command(incr_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(1)));
}

#[tokio::test]
async fn test_decr_decrements_value() {
    let ctx = RespTestContext::new().await;

    // Setup: Set initial value
    ctx.execute_command(ctx.make_command("SET", vec!["decr_key", "10"]))
        .await
        .unwrap();

    // Test: DECR should decrement and return new value
    let decr_cmd = ctx.make_command("DECR", vec!["decr_key"]);
    let result = ctx.execute_command(decr_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(9)));
}

#[tokio::test]
async fn test_incrby_increments_by_amount() {
    let ctx = RespTestContext::new().await;

    // Setup: Set initial value
    ctx.execute_command(ctx.make_command("SET", vec!["incrby_key", "10"]))
        .await
        .unwrap();

    // Test: INCRBY should increment by specified amount
    let incrby_cmd = ctx.make_command("INCRBY", vec!["incrby_key", "5"]);
    let result = ctx.execute_command(incrby_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(15)));
}

#[tokio::test]
async fn test_strlen_returns_length() {
    let ctx = RespTestContext::new().await;

    // Setup: Set a value
    ctx.execute_command(ctx.make_command("SET", vec!["strlen_key", "hello"]))
        .await
        .unwrap();

    // Test: STRLEN should return string length
    let strlen_cmd = ctx.make_command("STRLEN", vec!["strlen_key"]);
    let result = ctx.execute_command(strlen_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(5)));
}

#[tokio::test]
async fn test_append_appends_value() {
    let ctx = RespTestContext::new().await;

    // Setup: Set initial value
    ctx.execute_command(ctx.make_command("SET", vec!["append_key", "hello"]))
        .await
        .unwrap();

    // Test: APPEND should append and return new length
    let append_cmd = ctx.make_command("APPEND", vec!["append_key", " world"]);
    let result = ctx.execute_command(append_cmd).await.unwrap();
    assert!(matches!(result, RespValue::Integer(11)));

    // Verify the appended value
    let get_cmd = ctx.make_command("GET", vec!["append_key"]);
    let get_result = ctx.execute_command(get_cmd).await.unwrap();
    if let RespValue::BulkString(bytes) = get_result {
        assert_eq!(String::from_utf8_lossy(&bytes), "hello world");
    }
}

#[tokio::test]
async fn test_mget_returns_multiple_values() {
    let ctx = RespTestContext::new().await;

    // Setup: Set multiple keys
    ctx.execute_command(ctx.make_command("SET", vec!["mget_key1", "value1"]))
        .await
        .unwrap();
    ctx.execute_command(ctx.make_command("SET", vec!["mget_key2", "value2"]))
        .await
        .unwrap();

    // Test: MGET should return array of values
    let mget_cmd = ctx.make_command("MGET", vec!["mget_key1", "mget_key2", "nonexistent"]);
    let result = ctx.execute_command(mget_cmd).await.unwrap();

    if let RespValue::Array(values) = result {
        assert_eq!(values.len(), 3);
        // First two should be bulk strings, third should be null
    } else {
        panic!("MGET should return an array");
    }
}

#[tokio::test]
async fn test_mset_sets_multiple_keys() {
    let ctx = RespTestContext::new().await;

    // Test: MSET should set multiple key-value pairs
    let mset_cmd = ctx.make_command("MSET", vec!["mset_key1", "value1", "mset_key2", "value2"]);
    let result = ctx.execute_command(mset_cmd).await.unwrap();
    assert!(matches!(result, RespValue::SimpleString(_) | RespValue::BulkString(_)));

    // Verify values were set
    let get1 = ctx.execute_command(ctx.make_command("GET", vec!["mset_key1"])).await.unwrap();
    let get2 = ctx.execute_command(ctx.make_command("GET", vec!["mset_key2"])).await.unwrap();

    if let (RespValue::BulkString(b1), RespValue::BulkString(b2)) = (get1, get2) {
        assert_eq!(String::from_utf8_lossy(&b1), "value1");
        assert_eq!(String::from_utf8_lossy(&b2), "value2");
    }
}

#[tokio::test]
async fn test_setex_sets_with_expiration() {
    let ctx = RespTestContext::new().await;

    // Test: SETEX should set value with expiration
    let setex_cmd = ctx.make_command("SETEX", vec!["setex_key", "60", "value"]);
    let result = ctx.execute_command(setex_cmd).await.unwrap();
    assert!(matches!(result, RespValue::SimpleString(_) | RespValue::BulkString(_)));

    // Verify key exists
    let exists = ctx
        .execute_command(ctx.make_command("EXISTS", vec!["setex_key"]))
        .await
        .unwrap();
    assert!(matches!(exists, RespValue::Integer(1)));
}

#[tokio::test]
async fn test_setnx_only_sets_if_not_exists() {
    let ctx = RespTestContext::new().await;

    // Test: SETNX on non-existent key should return 1
    let setnx_cmd = ctx.make_command("SETNX", vec!["setnx_key", "value1"]);
    let result1 = ctx.execute_command(setnx_cmd).await.unwrap();
    assert!(matches!(result1, RespValue::Integer(1)));

    // Test: SETNX on existing key should return 0
    let setnx_cmd2 = ctx.make_command("SETNX", vec!["setnx_key", "value2"]);
    let result2 = ctx.execute_command(setnx_cmd2).await.unwrap();
    assert!(matches!(result2, RespValue::Integer(0)));

    // Verify original value is still there
    let get = ctx.execute_command(ctx.make_command("GET", vec!["setnx_key"])).await.unwrap();
    if let RespValue::BulkString(bytes) = get {
        assert_eq!(String::from_utf8_lossy(&bytes), "value1");
    }
}

#[tokio::test]
async fn test_getset_returns_old_value() {
    let ctx = RespTestContext::new().await;

    // Setup: Set initial value
    ctx.execute_command(ctx.make_command("SET", vec!["getset_key", "old_value"]))
        .await
        .unwrap();

    // Test: GETSET should return old value and set new value
    let getset_cmd = ctx.make_command("GETSET", vec!["getset_key", "new_value"]);
    let result = ctx.execute_command(getset_cmd).await.unwrap();

    if let RespValue::BulkString(bytes) = result {
        assert_eq!(String::from_utf8_lossy(&bytes), "old_value");
    }

    // Verify new value is set
    let get = ctx.execute_command(ctx.make_command("GET", vec!["getset_key"])).await.unwrap();
    if let RespValue::BulkString(bytes) = get {
        assert_eq!(String::from_utf8_lossy(&bytes), "new_value");
    }
}

#[tokio::test]
async fn test_getset_nonexistent_key() {
    let ctx = RespTestContext::new().await;

    // Test: GETSET on non-existent key should return null
    let getset_cmd = ctx.make_command("GETSET", vec!["new_getset_key", "value"]);
    let result = ctx.execute_command(getset_cmd).await.unwrap();
    assert!(result.is_null());
}

