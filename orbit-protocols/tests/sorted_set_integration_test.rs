//! Integration tests for Redis sorted set commands
//! Tests comprehensive sorted set functionality including ZADD, ZREM, ZRANGE, ZRANGEBYSCORE, etc.

use bytes::Bytes;
use orbit_client::OrbitClient;
use orbit_protocols::resp::commands::sorted_set::SortedSetCommands;
use orbit_protocols::resp::commands::traits::CommandHandler;
use orbit_protocols::resp::RespValue;
use std::sync::Arc;
use tokio;

/// Create a sorted set command handler for testing
async fn create_sorted_set_handler() -> SortedSetCommands {
    let config = orbit_client::OrbitClientConfig {
        namespace: "test".to_string(),
        server_urls: vec!["http://localhost:50051".to_string()],
        ..Default::default()
    };
    let orbit_client = Arc::new(OrbitClient::new_offline(config).await.unwrap());
    SortedSetCommands::new(orbit_client)
}

/// Helper to create RespValue strings
fn resp_str(s: &str) -> RespValue {
    RespValue::BulkString(Bytes::from(s.to_string().into_bytes()))
}

/// Helper to create RespValue integers
fn resp_int(i: i64) -> RespValue {
    RespValue::Integer(i)
}

#[tokio::test]
async fn test_sorted_set_zadd_basic() {
    let handler = create_sorted_set_handler().await;

    // ZADD myzset 1 "one" 2 "two"
    let args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
    ];

    let result = handler.handle("ZADD", &args).await;
    assert!(result.is_ok(), "ZADD should succeed: {:?}", result);

    if let Ok(RespValue::Integer(added)) = result {
        assert_eq!(added, 2, "Should add 2 elements");
    } else {
        panic!("Expected integer result for ZADD");
    }
}

#[tokio::test]
async fn test_sorted_set_zadd_with_options() {
    let handler = create_sorted_set_handler().await;

    // ZADD myzset NX 1 "one"
    let args = vec![
        resp_str("myzset"),
        resp_str("NX"),
        resp_str("1"),
        resp_str("one"),
    ];

    let result = handler.handle("ZADD", &args).await;
    assert!(result.is_ok(), "ZADD NX should succeed: {:?}", result);

    // Try to add the same element again with NX (should not add)
    let result2 = handler.handle("ZADD", &args).await;
    assert!(
        result2.is_ok(),
        "ZADD NX repeat should succeed: {:?}",
        result2
    );

    if let Ok(RespValue::Integer(added)) = result2 {
        assert_eq!(added, 0, "Should not add existing element with NX");
    }
}

#[tokio::test]
async fn test_sorted_set_zadd_incr() {
    let handler = create_sorted_set_handler().await;

    // ZADD myzset 1 "one"
    let args1 = vec![resp_str("myzset"), resp_str("1"), resp_str("one")];
    let _ = handler.handle("ZADD", &args1).await;

    // ZADD myzset INCR 2 "one" (should increment score to 3)
    let args2 = vec![
        resp_str("myzset"),
        resp_str("INCR"),
        resp_str("2"),
        resp_str("one"),
    ];

    let result = handler.handle("ZADD", &args2).await;
    assert!(result.is_ok(), "ZADD INCR should succeed: {:?}", result);

    // Result should be the new score as a string
    if let Ok(RespValue::BulkString(score_bytes)) = result {
        let score_str = String::from_utf8(score_bytes.to_vec()).unwrap();
        let score: f64 = score_str.parse().unwrap();
        assert_eq!(score, 3.0, "Incremented score should be 3");
    } else {
        panic!("Expected bulk string result for ZADD INCR: {:?}", result);
    }
}

#[tokio::test]
async fn test_sorted_set_zcard() {
    let handler = create_sorted_set_handler().await;

    // Add some elements first
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
        resp_str("3"),
        resp_str("three"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZCARD myzset
    let args = vec![resp_str("myzset")];
    let result = handler.handle("ZCARD", &args).await;

    assert!(result.is_ok(), "ZCARD should succeed: {:?}", result);
    if let Ok(RespValue::Integer(count)) = result {
        assert_eq!(count, 3, "Should have 3 elements");
    } else {
        panic!("Expected integer result for ZCARD");
    }
}

#[tokio::test]
async fn test_sorted_set_zscore() {
    let handler = create_sorted_set_handler().await;

    // Add an element
    let add_args = vec![resp_str("myzset"), resp_str("1.5"), resp_str("member1")];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZSCORE myzset member1
    let args = vec![resp_str("myzset"), resp_str("member1")];
    let result = handler.handle("ZSCORE", &args).await;

    assert!(result.is_ok(), "ZSCORE should succeed: {:?}", result);
    if let Ok(RespValue::BulkString(score_bytes)) = result {
        let score_str = String::from_utf8(score_bytes.to_vec()).unwrap();
        let score: f64 = score_str.parse().unwrap();
        assert_eq!(score, 1.5, "Score should be 1.5");
    } else {
        panic!("Expected bulk string result for ZSCORE");
    }

    // Test non-existing member
    let args2 = vec![resp_str("myzset"), resp_str("nonexistent")];
    let result2 = handler.handle("ZSCORE", &args2).await;
    assert!(
        result2.is_ok(),
        "ZSCORE for non-existing member should succeed"
    );

    match result2 {
        Ok(value) if value == RespValue::null() => {
            // This is expected
        }
        _ => {
            panic!(
                "Expected null result for non-existing member: {:?}",
                result2
            );
        }
    }
}

#[tokio::test]
async fn test_sorted_set_zincrby() {
    let handler = create_sorted_set_handler().await;

    // Add an element
    let add_args = vec![resp_str("myzset"), resp_str("1"), resp_str("member1")];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZINCRBY myzset 2.5 member1
    let args = vec![resp_str("myzset"), resp_str("2.5"), resp_str("member1")];
    let result = handler.handle("ZINCRBY", &args).await;

    assert!(result.is_ok(), "ZINCRBY should succeed: {:?}", result);
    if let Ok(RespValue::BulkString(score_bytes)) = result {
        let score_str = String::from_utf8(score_bytes.to_vec()).unwrap();
        let score: f64 = score_str.parse().unwrap();
        assert_eq!(score, 3.5, "New score should be 3.5");
    } else {
        panic!("Expected bulk string result for ZINCRBY");
    }
}

#[tokio::test]
async fn test_sorted_set_zrange() {
    let handler = create_sorted_set_handler().await;

    // Add elements with different scores
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
        resp_str("3"),
        resp_str("three"),
        resp_str("4"),
        resp_str("four"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZRANGE myzset 0 2
    let args = vec![resp_str("myzset"), resp_str("0"), resp_str("2")];
    let result = handler.handle("ZRANGE", &args).await;

    assert!(result.is_ok(), "ZRANGE should succeed: {:?}", result);
    if let Ok(RespValue::Array(members)) = result {
        assert_eq!(members.len(), 3, "Should return 3 members");

        // Check that members are in order
        if let RespValue::BulkString(bytes) = &members[0] {
            let member = String::from_utf8(bytes.to_vec()).unwrap();
            assert_eq!(member, "one", "First member should be 'one'");
        }
    } else {
        panic!("Expected array result for ZRANGE");
    }
}

#[tokio::test]
async fn test_sorted_set_zrange_withscores() {
    let handler = create_sorted_set_handler().await;

    // Add elements
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZRANGE myzset 0 -1 WITHSCORES
    let args = vec![
        resp_str("myzset"),
        resp_str("0"),
        resp_str("-1"),
        resp_str("WITHSCORES"),
    ];
    let result = handler.handle("ZRANGE", &args).await;

    assert!(
        result.is_ok(),
        "ZRANGE WITHSCORES should succeed: {:?}",
        result
    );
    if let Ok(RespValue::Array(elements)) = result {
        assert_eq!(
            elements.len(),
            4,
            "Should return 4 elements (2 member-score pairs)"
        );

        // First member
        if let RespValue::BulkString(bytes) = &elements[0] {
            let member = String::from_utf8(bytes.to_vec()).unwrap();
            assert_eq!(member, "one", "First member should be 'one'");
        }

        // First score
        if let RespValue::BulkString(bytes) = &elements[1] {
            let score_str = String::from_utf8(bytes.to_vec()).unwrap();
            let score: f64 = score_str.parse().unwrap();
            assert_eq!(score, 1.0, "First score should be 1.0");
        }
    } else {
        panic!("Expected array result for ZRANGE WITHSCORES");
    }
}

#[tokio::test]
async fn test_sorted_set_zrangebyscore() {
    let handler = create_sorted_set_handler().await;

    // Add elements with different scores
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2.5"),
        resp_str("two_five"),
        resp_str("3"),
        resp_str("three"),
        resp_str("10"),
        resp_str("ten"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZRANGEBYSCORE myzset 2 4
    let args = vec![resp_str("myzset"), resp_str("2"), resp_str("4")];
    let result = handler.handle("ZRANGEBYSCORE", &args).await;

    assert!(result.is_ok(), "ZRANGEBYSCORE should succeed: {:?}", result);
    if let Ok(RespValue::Array(members)) = result {
        assert_eq!(
            members.len(),
            2,
            "Should return 2 members in score range 2-4"
        );
    } else {
        panic!("Expected array result for ZRANGEBYSCORE");
    }
}

#[tokio::test]
async fn test_sorted_set_zrangebyscore_exclusive() {
    let handler = create_sorted_set_handler().await;

    // Add elements
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
        resp_str("3"),
        resp_str("three"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZRANGEBYSCORE myzset (1 (3  (exclusive bounds)
    let args = vec![resp_str("myzset"), resp_str("(1"), resp_str("(3")];
    let result = handler.handle("ZRANGEBYSCORE", &args).await;

    assert!(
        result.is_ok(),
        "ZRANGEBYSCORE with exclusive bounds should succeed: {:?}",
        result
    );
    if let Ok(RespValue::Array(members)) = result {
        assert_eq!(
            members.len(),
            1,
            "Should return 1 member in exclusive range (1,3)"
        );

        if let RespValue::BulkString(bytes) = &members[0] {
            let member = String::from_utf8(bytes.to_vec()).unwrap();
            assert_eq!(member, "two", "Member should be 'two'");
        }
    } else {
        panic!("Expected array result for ZRANGEBYSCORE");
    }
}

#[tokio::test]
async fn test_sorted_set_zcount() {
    let handler = create_sorted_set_handler().await;

    // Add elements
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
        resp_str("3"),
        resp_str("three"),
        resp_str("10"),
        resp_str("ten"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZCOUNT myzset 2 5
    let args = vec![resp_str("myzset"), resp_str("2"), resp_str("5")];
    let result = handler.handle("ZCOUNT", &args).await;

    assert!(result.is_ok(), "ZCOUNT should succeed: {:?}", result);
    if let Ok(RespValue::Integer(count)) = result {
        assert_eq!(count, 2, "Should count 2 elements in range 2-5");
    } else {
        panic!("Expected integer result for ZCOUNT");
    }
}

#[tokio::test]
async fn test_sorted_set_zrank() {
    let handler = create_sorted_set_handler().await;

    // Add elements
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
        resp_str("3"),
        resp_str("three"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZRANK myzset two
    let args = vec![resp_str("myzset"), resp_str("two")];
    let result = handler.handle("ZRANK", &args).await;

    assert!(result.is_ok(), "ZRANK should succeed: {:?}", result);
    if let Ok(RespValue::Integer(rank)) = result {
        assert_eq!(rank, 1, "Rank of 'two' should be 1 (0-indexed)");
    } else {
        panic!("Expected integer result for ZRANK");
    }
}

#[tokio::test]
async fn test_sorted_set_zrank_withscore() {
    let handler = create_sorted_set_handler().await;

    // Add elements
    let add_args = vec![resp_str("myzset"), resp_str("2.5"), resp_str("member")];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZRANK myzset member WITHSCORE
    let args = vec![
        resp_str("myzset"),
        resp_str("member"),
        resp_str("WITHSCORE"),
    ];
    let result = handler.handle("ZRANK", &args).await;

    assert!(
        result.is_ok(),
        "ZRANK WITHSCORE should succeed: {:?}",
        result
    );
    if let Ok(RespValue::Array(elements)) = result {
        assert_eq!(elements.len(), 2, "Should return rank and score");

        if let RespValue::Integer(rank) = &elements[0] {
            assert_eq!(*rank, 0, "Rank should be 0");
        }

        if let RespValue::BulkString(bytes) = &elements[1] {
            let score_str = String::from_utf8(bytes.to_vec()).unwrap();
            let score: f64 = score_str.parse().unwrap();
            assert_eq!(score, 2.5, "Score should be 2.5");
        }
    } else {
        panic!("Expected array result for ZRANK WITHSCORE");
    }
}

#[tokio::test]
async fn test_sorted_set_zrem() {
    let handler = create_sorted_set_handler().await;

    // Add elements
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
        resp_str("3"),
        resp_str("three"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZREM myzset one three
    let args = vec![resp_str("myzset"), resp_str("one"), resp_str("three")];
    let result = handler.handle("ZREM", &args).await;

    assert!(result.is_ok(), "ZREM should succeed: {:?}", result);
    if let Ok(RespValue::Integer(removed)) = result {
        assert_eq!(removed, 2, "Should remove 2 elements");
    } else {
        panic!("Expected integer result for ZREM");
    }

    // Check that elements were removed
    let card_args = vec![resp_str("myzset")];
    let card_result = handler.handle("ZCARD", &card_args).await;

    if let Ok(RespValue::Integer(count)) = card_result {
        assert_eq!(count, 1, "Should have 1 element remaining");
    } else {
        panic!("Expected integer result for ZCARD after ZREM");
    }
}

#[tokio::test]
async fn test_sorted_set_infinity_bounds() {
    let handler = create_sorted_set_handler().await;

    // Add elements
    let add_args = vec![
        resp_str("myzset"),
        resp_str("1"),
        resp_str("one"),
        resp_str("2"),
        resp_str("two"),
        resp_str("3"),
        resp_str("three"),
    ];
    let _ = handler.handle("ZADD", &add_args).await;

    // ZRANGEBYSCORE myzset -inf +inf
    let args = vec![resp_str("myzset"), resp_str("-inf"), resp_str("+inf")];
    let result = handler.handle("ZRANGEBYSCORE", &args).await;

    assert!(
        result.is_ok(),
        "ZRANGEBYSCORE with infinity bounds should succeed: {:?}",
        result
    );
    if let Ok(RespValue::Array(members)) = result {
        assert_eq!(members.len(), 3, "Should return all 3 elements");
    } else {
        panic!("Expected array result for ZRANGEBYSCORE with infinity");
    }

    // ZCOUNT myzset -inf +inf
    let count_args = vec![resp_str("myzset"), resp_str("-inf"), resp_str("+inf")];
    let count_result = handler.handle("ZCOUNT", &count_args).await;

    if let Ok(RespValue::Integer(count)) = count_result {
        assert_eq!(count, 3, "Should count all 3 elements");
    } else {
        panic!("Expected integer result for ZCOUNT with infinity");
    }
}

#[tokio::test]
async fn test_sorted_set_error_cases() {
    let handler = create_sorted_set_handler().await;

    // ZADD with wrong number of arguments
    let args = vec![resp_str("myzset"), resp_str("1")]; // Missing member
    let result = handler.handle("ZADD", &args).await;
    assert!(result.is_err(), "ZADD with missing member should fail");

    // ZSCORE with wrong number of arguments
    let args = vec![resp_str("myzset")]; // Missing member
    let result = handler.handle("ZSCORE", &args).await;
    assert!(result.is_err(), "ZSCORE with missing member should fail");

    // ZRANGE with wrong number of arguments
    let args = vec![resp_str("myzset"), resp_str("0")]; // Missing stop
    let result = handler.handle("ZRANGE", &args).await;
    assert!(result.is_err(), "ZRANGE with missing stop should fail");

    // Invalid command
    let args = vec![resp_str("myzset")];
    let result = handler.handle("ZUNKNOWN", &args).await;
    assert!(result.is_err(), "Unknown command should fail");
}
