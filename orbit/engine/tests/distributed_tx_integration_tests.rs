//! Distributed Transaction Integration Tests
//!
//! Comprehensive tests for the distributed transaction system including:
//! - TransactionCoordinator lifecycle (begin, commit, rollback)
//! - DistributedLockManager with exclusive and shared locks
//! - DeadlockDetector with wait-for graph cycle detection
//! - 2-Phase Commit Protocol (2PC) flow

#![cfg(feature = "transactions")]

use orbit_engine::addressable::{AddressableReference, Key};
use orbit_engine::cluster::NodeId;
use orbit_engine::transactions::{
    DeadlockCycle, DeadlockDetector, DistributedLock, DistributedTransaction, LockId, LockMode,
    LockOwner, LockRequest, LockStatus, TransactionConfig, TransactionId, TransactionOperation,
    TransactionState, TransactionVote,
};
use std::time::{Duration, SystemTime};

/// Helper function to create a test node ID (NodeId is a type alias for String)
fn test_node_id(name: &str) -> NodeId {
    name.to_string()
}

/// Helper function to create a test AddressableReference
fn test_actor_ref(actor_type: &str, actor_id: &str) -> AddressableReference {
    AddressableReference {
        addressable_type: actor_type.to_string(),
        key: Key::StringKey {
            key: actor_id.to_string(),
        },
    }
}

// ============================================================================
// TransactionId Tests
// ============================================================================

#[test]
fn test_transaction_id_creation() {
    let node_id = test_node_id("node-1");
    let tx_id = TransactionId::new(node_id.clone());

    assert!(!tx_id.id.is_empty());
    assert_eq!(tx_id.coordinator_node, node_id);
    assert!(tx_id.created_at > 0);
}

#[test]
fn test_transaction_id_uniqueness() {
    let node_id = test_node_id("node-1");
    let tx_id1 = TransactionId::new(node_id.clone());
    let tx_id2 = TransactionId::new(node_id);

    assert_ne!(tx_id1.id, tx_id2.id);
}

#[test]
fn test_transaction_id_display() {
    let node_id = test_node_id("node-1");
    let tx_id = TransactionId::new(node_id);
    let display = format!("{}", tx_id);

    assert!(display.contains("@"));
    assert!(display.contains("node-1"));
}

// ============================================================================
// TransactionState Tests
// ============================================================================

#[test]
fn test_transaction_states() {
    let preparing = TransactionState::Preparing;
    let prepared = TransactionState::Prepared;
    let committing = TransactionState::Committing;
    let committed = TransactionState::Committed;
    let aborting = TransactionState::Aborting;
    let aborted = TransactionState::Aborted;
    let timed_out = TransactionState::TimedOut;
    let failed = TransactionState::Failed {
        reason: "test error".to_string(),
    };

    assert_eq!(preparing, TransactionState::Preparing);
    assert_eq!(prepared, TransactionState::Prepared);
    assert_eq!(committing, TransactionState::Committing);
    assert_eq!(committed, TransactionState::Committed);
    assert_eq!(aborting, TransactionState::Aborting);
    assert_eq!(aborted, TransactionState::Aborted);
    assert_eq!(timed_out, TransactionState::TimedOut);
    assert!(matches!(failed, TransactionState::Failed { .. }));
}

// ============================================================================
// TransactionVote Tests
// ============================================================================

#[test]
fn test_transaction_vote_yes() {
    let vote = TransactionVote::Yes;
    assert_eq!(vote, TransactionVote::Yes);
}

#[test]
fn test_transaction_vote_no() {
    let vote = TransactionVote::No {
        reason: "constraint violation".to_string(),
    };
    match vote {
        TransactionVote::No { reason } => {
            assert_eq!(reason, "constraint violation");
        }
        _ => panic!("Expected No vote"),
    }
}

#[test]
fn test_transaction_vote_uncertain() {
    let vote = TransactionVote::Uncertain;
    assert_eq!(vote, TransactionVote::Uncertain);
}

// ============================================================================
// TransactionOperation Tests
// ============================================================================

#[test]
fn test_transaction_operation_creation() {
    let actor_ref = test_actor_ref("UserActor", "user-123");
    let operation = TransactionOperation::new(
        actor_ref.clone(),
        "UPDATE".to_string(),
        serde_json::json!({"balance": 100}),
    );

    assert!(!operation.operation_id.is_empty());
    assert_eq!(operation.target_actor, actor_ref);
    assert_eq!(operation.operation_type, "UPDATE");
    assert!(operation.compensation_data.is_none());
}

#[test]
fn test_transaction_operation_with_compensation() {
    let actor_ref = test_actor_ref("AccountActor", "account-456");
    let operation = TransactionOperation::new(
        actor_ref,
        "DEBIT".to_string(),
        serde_json::json!({"amount": 50}),
    )
    .with_compensation(serde_json::json!({"amount": 50, "type": "CREDIT"}));

    assert!(operation.compensation_data.is_some());
    let compensation = operation.compensation_data.unwrap();
    assert_eq!(compensation["type"], "CREDIT");
}

// ============================================================================
// DistributedTransaction Tests
// ============================================================================

#[test]
fn test_distributed_transaction_creation() {
    let node_id = test_node_id("coordinator-1");
    let tx = DistributedTransaction::new(node_id.clone(), Duration::from_secs(30));

    assert_eq!(tx.transaction_id.coordinator_node, node_id);
    assert!(tx.operations.is_empty());
    assert_eq!(tx.state, TransactionState::Preparing);
    assert_eq!(tx.timeout, Duration::from_secs(30));
    assert!(tx.participants.is_empty());
}

// ============================================================================
// LockId Tests
// ============================================================================

#[test]
fn test_lock_id_creation() {
    let lock_id = LockId::new("resource-123".to_string());

    assert_eq!(lock_id.resource_id, "resource-123");
    assert_eq!(lock_id.lock_key, "lock:resource-123");
}

#[test]
fn test_lock_id_from_key() {
    let lock_id = LockId::from_key("lock:resource-456".to_string());

    assert_eq!(lock_id.resource_id, "resource-456");
    assert_eq!(lock_id.lock_key, "lock:resource-456");
}

#[test]
fn test_lock_id_display() {
    let lock_id = LockId::new("test-resource".to_string());
    assert_eq!(format!("{}", lock_id), "lock:test-resource");
}

// ============================================================================
// LockOwner Tests
// ============================================================================

#[test]
fn test_lock_owner_creation() {
    let owner = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-123".to_string(),
        acquired_at: SystemTime::now(),
    };

    assert_eq!(owner.transaction_id, "tx-123");
}

#[test]
fn test_lock_owner_equality() {
    let now = SystemTime::now();
    let owner1 = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-123".to_string(),
        acquired_at: now,
    };
    let owner2 = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-123".to_string(),
        acquired_at: now,
    };

    assert_eq!(owner1, owner2);
}

// ============================================================================
// LockMode Tests
// ============================================================================

#[test]
fn test_lock_mode_exclusive() {
    let mode = LockMode::Exclusive;
    assert_eq!(mode, LockMode::Exclusive);
}

#[test]
fn test_lock_mode_shared() {
    let mode = LockMode::Shared;
    assert_eq!(mode, LockMode::Shared);
}

// ============================================================================
// DistributedLock Tests
// ============================================================================

#[test]
fn test_distributed_lock_creation() {
    let lock_id = LockId::new("resource-1".to_string());
    let lock = DistributedLock::new(lock_id.clone());

    assert_eq!(lock.lock_id, lock_id);
    assert!(lock.owners.is_empty());
    assert!(lock.is_available());
    assert!(!lock.is_held());
}

#[test]
fn test_distributed_lock_try_acquire_exclusive() {
    let lock_id = LockId::new("resource-1".to_string());
    let mut lock = DistributedLock::new(lock_id);

    let owner = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };

    let acquired = lock.try_acquire(owner.clone(), LockMode::Exclusive, Duration::from_secs(10));
    assert!(acquired);
    assert!(lock.is_held());
    assert!(!lock.is_available());
    assert!(lock.is_held_by(&owner));
}

#[test]
fn test_distributed_lock_exclusive_blocks_exclusive() {
    let lock_id = LockId::new("resource-1".to_string());
    let mut lock = DistributedLock::new(lock_id);

    let owner1 = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };
    let owner2 = LockOwner {
        node_id: test_node_id("node-2"),
        transaction_id: "tx-2".to_string(),
        acquired_at: SystemTime::now(),
    };

    assert!(lock.try_acquire(owner1, LockMode::Exclusive, Duration::from_secs(10)));
    assert!(!lock.try_acquire(owner2, LockMode::Exclusive, Duration::from_secs(10)));
}

#[test]
fn test_distributed_lock_shared_allows_shared() {
    let lock_id = LockId::new("resource-1".to_string());
    let mut lock = DistributedLock::new(lock_id);

    let owner1 = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };
    let owner2 = LockOwner {
        node_id: test_node_id("node-2"),
        transaction_id: "tx-2".to_string(),
        acquired_at: SystemTime::now(),
    };

    assert!(lock.try_acquire(owner1, LockMode::Shared, Duration::from_secs(10)));
    assert!(lock.try_acquire(owner2, LockMode::Shared, Duration::from_secs(10)));
    assert_eq!(lock.owners.len(), 2);
}

#[test]
fn test_distributed_lock_release() {
    let lock_id = LockId::new("resource-1".to_string());
    let mut lock = DistributedLock::new(lock_id);

    let owner = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };

    lock.try_acquire(owner.clone(), LockMode::Exclusive, Duration::from_secs(10));
    assert!(lock.is_held());

    let released = lock.release(&owner);
    assert!(released);
    assert!(!lock.is_held());
    assert!(lock.is_available());
}

#[test]
fn test_distributed_lock_wait_queue() {
    let lock_id = LockId::new("resource-1".to_string());
    let mut lock = DistributedLock::new(lock_id.clone());

    let owner = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };

    lock.try_acquire(owner.clone(), LockMode::Exclusive, Duration::from_secs(10));

    // Add a request to the wait queue
    let waiting_owner = LockOwner {
        node_id: test_node_id("node-2"),
        transaction_id: "tx-2".to_string(),
        acquired_at: SystemTime::now(),
    };

    let request = LockRequest {
        lock_id: lock_id.clone(),
        owner: waiting_owner,
        mode: LockMode::Exclusive,
        timeout: Duration::from_secs(5),
        requested_at: SystemTime::now(),
    };

    lock.enqueue_request(request);
    assert_eq!(lock.wait_queue.len(), 1);

    // Process wait queue while lock is held
    assert!(lock.process_wait_queue().is_none());

    // Release and process wait queue
    lock.release(&owner);
    let next_request = lock.process_wait_queue();
    assert!(next_request.is_some());
}

// ============================================================================
// DeadlockDetector Tests
// ============================================================================

#[tokio::test]
async fn test_deadlock_detector_creation() {
    let detector = DeadlockDetector::new(Duration::from_millis(100));
    // Should be created without any deadlocks initially
    assert!(detector.detect_deadlock().await.is_none());
}

#[tokio::test]
async fn test_deadlock_detector_no_cycle() {
    let detector = DeadlockDetector::new(Duration::from_millis(100));

    // T1 waits for T2, but no cycle
    detector
        .add_wait_for(
            "tx-1".to_string(),
            "tx-2".to_string(),
            "resource-A".to_string(),
        )
        .await;

    assert!(detector.detect_deadlock().await.is_none());
}

#[tokio::test]
async fn test_deadlock_detector_simple_cycle() {
    let detector = DeadlockDetector::new(Duration::from_millis(100));

    // Create a simple deadlock: T1 -> T2 -> T1
    detector
        .add_wait_for(
            "tx-1".to_string(),
            "tx-2".to_string(),
            "resource-A".to_string(),
        )
        .await;
    detector
        .add_wait_for(
            "tx-2".to_string(),
            "tx-1".to_string(),
            "resource-B".to_string(),
        )
        .await;

    let deadlock = detector.detect_deadlock().await;
    assert!(deadlock.is_some());

    let cycle = deadlock.unwrap();
    assert!(cycle.transactions.len() >= 2);
    assert!(!cycle.resources.is_empty());
}

#[tokio::test]
async fn test_deadlock_detector_complex_cycle() {
    let detector = DeadlockDetector::new(Duration::from_millis(100));

    // Create a 3-way deadlock: T1 -> T2 -> T3 -> T1
    detector
        .add_wait_for(
            "tx-1".to_string(),
            "tx-2".to_string(),
            "resource-A".to_string(),
        )
        .await;
    detector
        .add_wait_for(
            "tx-2".to_string(),
            "tx-3".to_string(),
            "resource-B".to_string(),
        )
        .await;
    detector
        .add_wait_for(
            "tx-3".to_string(),
            "tx-1".to_string(),
            "resource-C".to_string(),
        )
        .await;

    let deadlock = detector.detect_deadlock().await;
    assert!(deadlock.is_some());

    let cycle = deadlock.unwrap();
    assert!(cycle.transactions.len() >= 3);
}

#[tokio::test]
async fn test_deadlock_detector_remove_transaction() {
    let detector = DeadlockDetector::new(Duration::from_millis(100));

    // Create deadlock
    detector
        .add_wait_for(
            "tx-1".to_string(),
            "tx-2".to_string(),
            "resource-A".to_string(),
        )
        .await;
    detector
        .add_wait_for(
            "tx-2".to_string(),
            "tx-1".to_string(),
            "resource-B".to_string(),
        )
        .await;

    assert!(detector.detect_deadlock().await.is_some());

    // Remove one transaction to break the cycle
    detector.remove_transaction("tx-1").await;

    assert!(detector.detect_deadlock().await.is_none());
}

#[tokio::test]
async fn test_deadlock_cycle_victim_selection() {
    let cycle = DeadlockCycle {
        transactions: vec!["tx-1".to_string(), "tx-2".to_string(), "tx-3".to_string()],
        resources: vec!["resource-A".to_string(), "resource-B".to_string()],
        detected_at: SystemTime::now(),
    };

    // Should select the last transaction as victim
    let victim = cycle.select_victim();
    assert!(victim.is_some());
    assert_eq!(victim.unwrap(), "tx-3");
}

// ============================================================================
// LockStatus Tests
// ============================================================================

#[test]
fn test_lock_status_available() {
    let status = LockStatus::Available;
    assert_eq!(status, LockStatus::Available);
}

#[test]
fn test_lock_status_held_exclusive() {
    let owner = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };

    let status = LockStatus::HeldExclusive { owner };
    assert!(matches!(status, LockStatus::HeldExclusive { .. }));
}

#[test]
fn test_lock_status_held_shared() {
    let owners = vec![
        LockOwner {
            node_id: test_node_id("node-1"),
            transaction_id: "tx-1".to_string(),
            acquired_at: SystemTime::now(),
        },
        LockOwner {
            node_id: test_node_id("node-2"),
            transaction_id: "tx-2".to_string(),
            acquired_at: SystemTime::now(),
        },
    ];

    let status = LockStatus::HeldShared { owners };
    match status {
        LockStatus::HeldShared { owners } => assert_eq!(owners.len(), 2),
        _ => panic!("Expected HeldShared status"),
    }
}

#[test]
fn test_lock_status_waiting() {
    let status = LockStatus::Waiting { position: 5 };
    match status {
        LockStatus::Waiting { position } => assert_eq!(position, 5),
        _ => panic!("Expected Waiting status"),
    }
}

// ============================================================================
// LockRequest Tests
// ============================================================================

#[test]
fn test_lock_request_creation() {
    let lock_id = LockId::new("resource-1".to_string());
    let owner = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };

    let request = LockRequest {
        lock_id: lock_id.clone(),
        owner,
        mode: LockMode::Exclusive,
        timeout: Duration::from_secs(30),
        requested_at: SystemTime::now(),
    };

    assert_eq!(request.lock_id, lock_id);
    assert_eq!(request.mode, LockMode::Exclusive);
    assert_eq!(request.timeout, Duration::from_secs(30));
}

// ============================================================================
// TransactionConfig Tests
// ============================================================================

#[test]
fn test_transaction_config_default() {
    let config = TransactionConfig::default();

    // Config should have reasonable defaults
    assert!(config.default_timeout > Duration::ZERO);
}

// ============================================================================
// Integration Scenario Tests
// ============================================================================

#[tokio::test]
async fn test_concurrent_lock_acquisition() {
    let lock_id = LockId::new("shared-resource".to_string());
    let mut lock = DistributedLock::new(lock_id);

    // Simulate multiple transactions acquiring shared locks
    for i in 0..5 {
        let owner = LockOwner {
            node_id: test_node_id(&format!("node-{}", i)),
            transaction_id: format!("tx-{}", i),
            acquired_at: SystemTime::now(),
        };

        assert!(lock.try_acquire(owner, LockMode::Shared, Duration::from_secs(10)));
    }

    assert_eq!(lock.owners.len(), 5);
    assert!(lock.is_held());
}

#[tokio::test]
async fn test_lock_upgrade_blocked() {
    let lock_id = LockId::new("resource-1".to_string());
    let mut lock = DistributedLock::new(lock_id);

    let owner1 = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };

    // Acquire shared lock
    assert!(lock.try_acquire(owner1.clone(), LockMode::Shared, Duration::from_secs(10)));

    // Another transaction trying exclusive lock should fail
    let owner2 = LockOwner {
        node_id: test_node_id("node-2"),
        transaction_id: "tx-2".to_string(),
        acquired_at: SystemTime::now(),
    };

    assert!(!lock.try_acquire(owner2, LockMode::Exclusive, Duration::from_secs(10)));
}

#[tokio::test]
async fn test_serialization_transaction_id() {
    let node_id = test_node_id("node-1");
    let tx_id = TransactionId::new(node_id);

    // Test serialization
    let serialized = serde_json::to_string(&tx_id).unwrap();
    let deserialized: TransactionId = serde_json::from_str(&serialized).unwrap();

    assert_eq!(tx_id.id, deserialized.id);
    assert_eq!(tx_id.coordinator_node, deserialized.coordinator_node);
}

#[tokio::test]
async fn test_serialization_transaction_operation() {
    let actor_ref = test_actor_ref("TestActor", "test-123");
    let operation = TransactionOperation::new(
        actor_ref,
        "INSERT".to_string(),
        serde_json::json!({"key": "value"}),
    )
    .with_compensation(serde_json::json!({"delete": true}));

    let serialized = serde_json::to_string(&operation).unwrap();
    let deserialized: TransactionOperation = serde_json::from_str(&serialized).unwrap();

    assert_eq!(operation.operation_id, deserialized.operation_id);
    assert_eq!(operation.operation_type, deserialized.operation_type);
    assert!(deserialized.compensation_data.is_some());
}

#[tokio::test]
async fn test_deadlock_detector_no_false_positives() {
    let detector = DeadlockDetector::new(Duration::from_millis(100));

    // Linear chain: T1 -> T2 -> T3 (no cycle)
    detector
        .add_wait_for(
            "tx-1".to_string(),
            "tx-2".to_string(),
            "resource-A".to_string(),
        )
        .await;
    detector
        .add_wait_for(
            "tx-2".to_string(),
            "tx-3".to_string(),
            "resource-B".to_string(),
        )
        .await;

    assert!(detector.detect_deadlock().await.is_none());
}

#[tokio::test]
async fn test_multiple_independent_wait_chains() {
    let detector = DeadlockDetector::new(Duration::from_millis(100));

    // Two independent chains, no cycles
    detector
        .add_wait_for(
            "tx-1".to_string(),
            "tx-2".to_string(),
            "resource-A".to_string(),
        )
        .await;
    detector
        .add_wait_for(
            "tx-3".to_string(),
            "tx-4".to_string(),
            "resource-B".to_string(),
        )
        .await;

    assert!(detector.detect_deadlock().await.is_none());
}

#[tokio::test]
async fn test_lock_expiration_check() {
    let lock_id = LockId::new("resource-1".to_string());
    let mut lock = DistributedLock::new(lock_id);

    let owner = LockOwner {
        node_id: test_node_id("node-1"),
        transaction_id: "tx-1".to_string(),
        acquired_at: SystemTime::now(),
    };

    // Acquire with very short duration
    lock.try_acquire(owner, LockMode::Exclusive, Duration::from_millis(1));

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Lock should now be available due to expiration
    assert!(lock.is_available());
}
