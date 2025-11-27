---
layout: default
title: Advanced Transaction Features Guide
category: documentation
---

## Advanced Transaction Features Guide

## Table of Contents

1. [Distributed Locks](#distributed-locks)
2. [Metrics Integration](#metrics-integration)
3. [Security Features](#security-features)
4. [Performance Optimizations](#performance-optimizations)
5. [Saga Pattern](#saga-pattern)
6. [Best Practices](#best-practices)

## Distributed Locks

### Overview

The distributed lock system provides coordination mechanisms across the cluster with automatic deadlock detection and prevention.

### Basic Usage

```rust
use orbit_shared::transactions::{
    DistributedLockManager, LockManagerConfig, LockMode
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure lock manager
    let config = LockManagerConfig {
        default_timeout: Duration::from_secs(30),
        deadlock_detection_interval: Duration::from_secs(5),
        max_locks_per_transaction: 100,
    };
    
    // Create lock manager
    let lock_manager = DistributedLockManager::new(config);
    
    // Acquire exclusive lock
    let lock = lock_manager.acquire_lock(
        "resource-123",
        LockMode::Exclusive,
        Duration::from_secs(30)
    ).await?;
    
    // Perform protected operation
    println!("Lock acquired, performing operation...");
    
    // Lock automatically released when dropped
    drop(lock);
    
    Ok(())
}
```

### Lock Modes

#### Exclusive Lock

Prevents all other access (read or write) to the resource.

```rust
let exclusive_lock = lock_manager.acquire_lock(
    resource_id,
    LockMode::Exclusive,
    timeout
).await?;
```

#### Shared Lock

Multiple readers can hold shared locks simultaneously, but writers must wait.

```rust
let shared_lock = lock_manager.acquire_lock(
    resource_id,
    LockMode::Shared,
    timeout
).await?;
```

### Deadlock Detection

The system automatically detects deadlocks using wait-for graph analysis:

```rust
// Example scenario that could cause deadlock
// Transaction A: Lock R1, then try to lock R2
// Transaction B: Lock R2, then try to lock R1

// The deadlock detector will:
// 1. Build wait-for graph
// 2. Detect cycle using DFS
// 3. Abort one transaction to break the cycle
// 4. Return DeadlockDetected error

match lock_manager.acquire_lock(resource_id, mode, timeout).await {
    Ok(lock) => {
        // Lock acquired successfully
    },
    Err(OrbitError::Internal(msg)) if msg.contains("deadlock") => {
        // Deadlock detected - handle retry logic
        warn!("Deadlock detected, retrying transaction");
    },
    Err(e) => return Err(e),
}
```

### Lock Statistics

Query lock statistics for monitoring:

```rust
let stats = lock_manager.get_statistics().await;
println!("Active locks: {}", stats.active_locks);
println!("Waiting requests: {}", stats.waiting_requests);
println!("Deadlocks detected: {}", stats.deadlocks_detected);
println!("Deadlocks resolved: {}", stats.deadlocks_resolved);
```

## Metrics Integration

### Metrics Overview

Comprehensive Prometheus metrics for transaction monitoring and observability.

### Initializing Metrics

```rust
use orbit_shared::transactions::{TransactionMetrics, TransactionMetricsAggregator};
use orbit_shared::mesh::NodeId;

// Create metrics for a node
let node_id = NodeId::new("node-1".to_string(), "cluster-1".to_string());
let metrics = TransactionMetrics::new(node_id.clone());

// Create aggregator for cluster-wide metrics
let aggregator = TransactionMetricsAggregator::new(node_id);
```

### Recording Transaction Metrics

```rust
// Transaction lifecycle
metrics.record_transaction_started(&tx_id).await;
metrics.record_transaction_prepared(&tx_id, participant_count).await;
metrics.record_transaction_committed(&tx_id).await;

// Or on failure
metrics.record_transaction_aborted(&tx_id).await;
metrics.record_transaction_failed(&tx_id, "reason").await;
metrics.record_transaction_timeout(&tx_id).await;
```

### Recording Saga Metrics

```rust
use orbit_shared::transactions::SagaMetrics;

let saga_metrics = SagaMetrics::new(node_id);

saga_metrics.record_saga_started(&saga_id).await;
saga_metrics.record_step_executed(&saga_id, step_count).await;
saga_metrics.record_saga_completed(&saga_id, step_count).await;

// On compensation
saga_metrics.record_saga_compensating(&saga_id).await;
saga_metrics.record_step_compensated(&saga_id).await;
```

### Recording Lock Metrics

```rust
use orbit_shared::transactions::LockMetrics;

let lock_metrics = LockMetrics::new(node_id);

lock_metrics.record_lock_acquired(&lock_id, wait_duration).await;
lock_metrics.record_lock_released(&lock_id, hold_duration).await;
lock_metrics.record_deadlock_detected(&[tx_id1, tx_id2]).await;
```

### Prometheus Endpoint

Metrics are automatically exported in Prometheus format:

```

# HELP orbit_transaction_node_1_started_total Total transactions started
# TYPE orbit_transaction_node_1_started_total counter
orbit_transaction_node_1_started_total 1234

# HELP orbit_transaction_node_1_active Active transactions
# TYPE orbit_transaction_node_1_active gauge
orbit_transaction_node_1_active 5

# HELP orbit_transaction_node_1_duration_seconds Transaction duration
# TYPE orbit_transaction_node_1_duration_seconds histogram
orbit_transaction_node_1_duration_seconds_bucket{le="0.005"} 100
orbit_transaction_node_1_duration_seconds_bucket{le="0.01"} 250
```

### Grafana Dashboard

Example dashboard queries:

```promql

# Transaction throughput
rate(orbit_transaction_started_total[5m])

# Active transactions
sum(orbit_transaction_active)

# P99 transaction latency
histogram_quantile(0.99, rate(orbit_transaction_duration_seconds_bucket[5m]))

# Lock contention
rate(orbit_locks_timeout_total[5m])

# Deadlock rate
rate(orbit_locks_deadlock_detected_total[5m])
```

## Security Features

### Security Overview

Token-based authentication, scope-based authorization, and comprehensive audit logging.

### Authentication

```rust
use orbit_shared::transactions::{
    TransactionSecurityManager, SecurityConfig, AuthToken
};

// Initialize security manager
let config = SecurityConfig::default();
let security_mgr = TransactionSecurityManager::new(config);

// Create authentication token
let token = AuthToken {
    token_id: "token-123".to_string(),
    issuer: "auth-service".to_string(),
    subject: "user-alice".to_string(),
    scopes: vec!["transaction:write".to_string(), "transaction:read".to_string()],
    issued_at: SystemTime::now(),
    expires_at: SystemTime::now() + Duration::from_secs(3600),
};

// Authenticate
security_mgr.authenticate(&token).await?;
```

### Authorization

```rust
use orbit_shared::transactions::TransactionPermission;

// Check permissions before operations
security_mgr.authorize(
    &token,
    &transaction_id,
    TransactionPermission::Begin
).await?;

security_mgr.authorize(
    &token,
    &transaction_id,
    TransactionPermission::Commit
).await?;
```

### Permission Types

- `Begin`: Start a new transaction
- `Commit`: Commit a transaction
- `Abort`: Abort a transaction
- `Read`: Read transaction data
- `Write`: Write transaction data
- `Coordinate`: Act as transaction coordinator
- `Participate`: Participate in a transaction

### Audit Logging

```rust
// Audit log entries are automatically created for key operations
// but you can also create custom entries

security_mgr.audit_log_entry(
    transaction_id.clone(),
    "CUSTOM_ACTION".to_string(),
    "success".to_string()
).await?;

// Query audit logs
let entries = security_mgr.query_audit_logs(
    Some(transaction_id),
    Some(start_time),
    Some(end_time),
    100
).await?;

for entry in entries {
    println!("{:?}: {} - {}", entry.timestamp, entry.action, entry.outcome);
}
```

### Custom Authentication Provider

```rust
use orbit_shared::transactions::{AuthenticationProvider, AuthToken};
use async_trait::async_trait;

struct MyAuthProvider {
    // Your authentication backend
}

#[async_trait]
impl AuthenticationProvider for MyAuthProvider {
    async fn authenticate(&self, token: &AuthToken) -> OrbitResult<bool> {
        // Implement your authentication logic
        // e.g., validate JWT, check database, etc.
        Ok(true)
    }
    
    async fn refresh(&self, token: &AuthToken) -> OrbitResult<AuthToken> {
        // Implement token refresh logic
        Ok(token.clone())
    }
}
```

## Performance Optimizations

### Batch Processing

```rust
use orbit_shared::transactions::{BatchProcessor, BatchConfig};

// Configure batch processor
let config = BatchConfig {
    max_batch_size: 100,
    max_wait_time: Duration::from_millis(10),
    min_batch_size: 10,
    enable_adaptive_sizing: true,
};

let processor = BatchProcessor::new(config);

// Add operations
for operation in operations {
    processor.add_operation(operation, priority).await?;
}

// Flush manually or wait for automatic flush
let batch = processor.flush().await?;

// Process batch
for op in batch {
    process_operation(op).await?;
}
```

### Connection Pooling

```rust
use orbit_shared::transactions::{ConnectionPool, ConnectionPoolConfig};

// Define connection factory
async fn create_connection() -> OrbitResult<DatabaseConnection> {
    DatabaseConnection::connect("postgres://localhost/mydb").await
}

// Configure pool
let config = ConnectionPoolConfig {
    min_connections: 5,
    max_connections: 20,
    connection_timeout: Duration::from_secs(5),
    idle_timeout: Duration::from_secs(300),
    health_check_interval: Duration::from_secs(30),
};

let pool = ConnectionPool::new(config, create_connection);

// Acquire connection
let conn = pool.acquire().await?;

// Use connection
conn.execute_query("SELECT * FROM transactions").await?;

// Connection automatically returned to pool when dropped
```

### Resource Management

```rust
use orbit_shared::transactions::ResourceManager;

// Initialize resource manager
let resource_mgr = ResourceManager::new(
    1_000_000_000, // 1GB max memory
    100            // 100 max concurrent operations
);

// Acquire resources
let guard = resource_mgr.acquire(estimated_memory).await?;

// Perform operation - resources are reserved
perform_operation().await?;

// Resources automatically released when guard drops
drop(guard);

// Check current usage
let (memory, concurrent) = resource_mgr.current_usage().await;
println!("Memory: {}MB, Concurrent: {}", memory / 1_000_000, concurrent);
```

## Saga Pattern

### Saga Overview

Long-running distributed transactions with automatic compensation on failure.

### Basic Saga

```rust
use orbit_shared::saga::{SagaOrchestrator, SagaStep};

// Create saga orchestrator
let mut saga = SagaOrchestrator::new("transfer-funds");

// Add steps with compensation
saga.add_step(SagaStep {
    name: "reserve-inventory".to_string(),
    action: Box::new(|| async {
        inventory_service.reserve(item_id, quantity).await
    }),
    compensation: Some(Box::new(|| async {
        inventory_service.release(item_id, quantity).await
    })),
});

saga.add_step(SagaStep {
    name: "charge-payment".to_string(),
    action: Box::new(|| async {
        payment_service.charge(user_id, amount).await
    }),
    compensation: Some(Box::new(|| async {
        payment_service.refund(user_id, amount).await
    })),
});

saga.add_step(SagaStep {
    name: "ship-order".to_string(),
    action: Box::new(|| async {
        shipping_service.create_shipment(order_id).await
    }),
    compensation: Some(Box::new(|| async {
        shipping_service.cancel_shipment(order_id).await
    })),
});

// Execute saga
match saga.execute().await {
    Ok(_) => println!("Saga completed successfully"),
    Err(e) => {
        // Compensation automatically triggered
        println!("Saga failed, compensation completed: {:?}", e);
    }
}
```

### Saga with Metrics

```rust
let saga_metrics = SagaMetrics::new(node_id);
let saga_id = uuid::Uuid::new_v4().to_string();

saga_metrics.record_saga_started(&saga_id).await;

let mut step_count = 0;
for step in saga.steps {
    saga_metrics.record_step_executing(&saga_id).await;
    
    match step.execute().await {
        Ok(_) => {
            step_count += 1;
            saga_metrics.record_step_executed(&saga_id, step_count).await;
        }
        Err(e) => {
            saga_metrics.record_saga_compensating(&saga_id).await;
            // Trigger compensation
            break;
        }
    }
}

saga_metrics.record_saga_completed(&saga_id, step_count).await;
```

## Best Practices

### Lock Management

1. **Always use timeouts**: Never acquire locks without a timeout to prevent indefinite waits
2. **Lock ordering**: Establish a consistent lock ordering to prevent deadlocks
3. **Minimize lock hold time**: Hold locks for the shortest duration necessary
4. **Use shared locks for reads**: Maximize concurrency by using shared locks when possible

```rust
// Good: Use timeouts
let lock = lock_manager.acquire_lock(
    resource_id,
    LockMode::Exclusive,
    Duration::from_secs(30) // Always specify timeout
).await?;

// Good: Consistent lock ordering
let locks = vec![
    lock_manager.acquire_lock("resource-1", mode, timeout).await?,
    lock_manager.acquire_lock("resource-2", mode, timeout).await?,
];

// Good: Release locks early
{
    let lock = lock_manager.acquire_lock(resource_id, mode, timeout).await?;
    perform_quick_operation();
} // Lock released here

// Good: Use shared locks for reads
let lock = lock_manager.acquire_lock(
    resource_id,
    LockMode::Shared, // Allow concurrent readers
    timeout
).await?;
```

### Metrics Collection

1. **Sample high-cardinality metrics**: Avoid creating unique metrics for each transaction
2. **Use histogram buckets wisely**: Configure buckets based on expected latencies
3. **Aggregate metrics**: Use the aggregator for cluster-wide views
4. **Monitor metric cardinality**: High cardinality can impact Prometheus performance

```rust
// Good: Use node-level metrics
let metrics = TransactionMetrics::new(node_id);

// Good: Aggregate for cluster view
let aggregator = TransactionMetricsAggregator::new(node_id);
aggregator.add_node_metrics(metrics).await;

// Good: Query aggregated stats
let stats = aggregator.get_aggregated_stats().await;
```

### Security

1. **Validate tokens on every request**: Don't cache authorization decisions
2. **Use short-lived tokens**: Set appropriate expiration times
3. **Implement token refresh**: Allow seamless token renewal
4. **Audit all critical operations**: Log authentication, authorization, and transaction lifecycle events

```rust
// Good: Validate on every request
security_mgr.authenticate(&token).await?;
security_mgr.authorize(&token, &tx_id, permission).await?;

// Good: Use appropriate token lifetimes
let token = AuthToken {
    expires_at: SystemTime::now() + Duration::from_secs(300), // 5 minutes
    // ...
};

// Good: Audit critical operations
security_mgr.audit_log_entry(tx_id, "COMMIT", "success").await?;
```

### Performance

1. **Use batching for write-heavy workloads**: Reduce coordination overhead
2. **Configure pool sizes appropriately**: Match pool sizes to expected concurrency
3. **Set resource limits**: Prevent resource exhaustion with memory and concurrency limits
4. **Monitor resource usage**: Track memory and connection usage

```rust
// Good: Batch writes
let batch_processor = BatchProcessor::new(config);
for write in writes {
    batch_processor.add_operation(write, priority).await?;
}
let batch = batch_processor.flush().await?;

// Good: Size pools based on load
let pool = ConnectionPool::new(ConnectionPoolConfig {
    min_connections: expected_baseline,
    max_connections: expected_peak * 2,
    // ...
}, factory);

// Good: Set resource limits
let resource_mgr = ResourceManager::new(
    available_memory * 0.8, // Leave headroom
    max_expected_concurrency * 1.5
);
```

### Saga Design

1. **Make compensation idempotent**: Compensation may be executed multiple times
2. **Design for partial failure**: Each step should be independently compensatable
3. **Keep saga state persistent**: Store saga progress for recovery
4. **Use timeouts**: Don't let sagas run indefinitely

```rust
// Good: Idempotent compensation
async fn compensate_inventory_reservation(item_id: &str, quantity: i32) -> OrbitResult<()> {
    // Check if already compensated before releasing
    if !inventory_service.is_reserved(item_id).await? {
        return Ok(()); // Already compensated
    }
    inventory_service.release(item_id, quantity).await
}

// Good: Each step is independently compensatable
saga.add_step(SagaStep {
    name: "step-1",
    action: independent_action_1,
    compensation: Some(independent_compensation_1),
});

saga.add_step(SagaStep {
    name: "step-2",
    action: independent_action_2,
    compensation: Some(independent_compensation_2),
});
```

## Troubleshooting

### Deadlock Issues

```rust
// Enable deadlock detection logging
env::set_var("RUST_LOG", "orbit_shared::transactions::locks=debug");

// Check deadlock statistics
let stats = lock_manager.get_statistics().await;
if stats.deadlocks_detected > threshold {
    warn!("High deadlock rate: {}", stats.deadlocks_detected);
    // Consider reviewing lock acquisition order
}
```

### Performance Issues

```rust
// Monitor batch processing efficiency
let stats = batch_processor.get_stats().await;
println!("Avg batch size: {}", stats.average_batch_size);
println!("Flush rate: {}", stats.flushes_per_second);

// Monitor connection pool health
let pool_stats = pool.get_stats().await;
println!("Active connections: {}", pool_stats.active);
println!("Idle connections: {}", pool_stats.idle);
println!("Wait time: {:?}", pool_stats.average_wait_time);
```

### Security Issues

```rust
// Query audit logs for security incidents
let failed_auths = security_mgr.query_audit_logs(
    None,
    Some(start_time),
    Some(end_time),
    1000
).await?
.into_iter()
.filter(|entry| entry.outcome == "failure" && entry.action.starts_with("AUTH"))
.collect::<Vec<_>>();

if failed_auths.len() > threshold {
    alert!("High authentication failure rate");
}
```

## Additional Resources

- [Implementation Summary](wip/ADVANCED_FEATURES_IMPLEMENTATION.md)
- [Architecture Documentation](architecture/ORBIT_ARCHITECTURE.md)
- [API Documentation](https://turingworks.github.io/orbit-rs/api/)
- [Examples](../examples/)

## Support

For questions or issues:

- GitHub Issues: <https://github.com/TuringWorks/orbit-rs/issues>
- Documentation: <https://turingworks.github.io/orbit-rs/api/>
- Examples: See the `examples/` directory
