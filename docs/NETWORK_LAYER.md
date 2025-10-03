# Orbit-RS Network Layer Documentation

## Overview
The Orbit-RS network layer provides a comprehensive gRPC-based communication infrastructure for distributed actor systems. Built on `tonic` and Protocol Buffers, it enables reliable, high-performance inter-node communication with automatic connection management, retry logic, and health monitoring.

---

## Architecture

### Components

```
orbit-proto/              # Protocol Buffer definitions and gRPC services
├── proto/
│   ├── messages.proto   # Message and invocation protocols
│   ├── node.proto       # Node information and capabilities
│   ├── addressable.proto # Actor reference types
│   ├── connection.proto # Connection service definitions
│   └── health.proto     # Health check service
├── build.rs             # tonic-build integration
└── src/
    ├── lib.rs          # Generated proto code inclusion
    ├── converters.rs   # Rust ↔ Protobuf converters
    └── services.rs     # gRPC service implementations

orbit-shared/src/
├── transport.rs         # Transaction transport layer
└── raft_transport.rs    # Raft consensus transport
```

---

## Protocol Buffer Definitions

### Message Protocol (`messages.proto`)

The message protocol defines the core communication structures for actor invocations and system messages.

```protobuf
message MessageProto {
    int64 message_id = 1;
    NodeIdProto source = 2;
    MessageTargetProto target = 3;
    MessageContentProto content = 4;
    int64 attempts = 5;
}

message MessageTargetProto {
    oneof target {
        Unicast unicast_target = 1;
        RoutedUnicast routed_unicast_target = 2;
    }
}

message MessageContentProto {
    oneof content {
        ErrorProto error = 1;
        ConnectionInfoRequestProto info_request = 2;
        ConnectionInfoResponseProto info_response = 3;
        InvocationRequestProto invocation_request = 4;
        InvocationResponseProto invocation_response = 5;
        InvocationResponseErrorProto invocation_response_error = 6;
    }
}
```

**Key Features:**
- Message routing (unicast, routed unicast)
- Request/response pattern support
- Error propagation
- Retry attempt tracking

### Node Protocol (`node.proto`)

Defines cluster node information and capabilities.

```protobuf
message NodeInfoProto {
    NodeIdProto id = 1;
    string url = 2;
    uint32 port = 3;
    NodeCapabilitiesProto capabilities = 4;
    NodeStatusProto status = 5;
    optional NodeLeaseProto lease = 6;
}

message NodeCapabilitiesProto {
    repeated string addressable_types = 1;
    optional uint32 max_addressables = 2;
    map<string, string> tags = 3;
}

enum NodeStatusProto {
    ACTIVE = 0;
    DRAINING = 1;
    STOPPED = 2;
}
```

**Use Cases:**
- Node discovery and registration
- Capability negotiation
- Health monitoring
- Lease management

### Addressable Protocol (`addressable.proto`)

Defines actor reference types and keys.

```protobuf
message KeyProto {
    oneof key {
        string string_key = 1;
        int32 int32_key = 2;
        int64 int64_key = 3;
        NoKeyProto no_key = 4;
    }
}

message AddressableReferenceProto {
    string addressable_type = 1;
    KeyProto key = 2;
}

message AddressableLeaseProto {
    AddressableReferenceProto reference = 1;
    NodeIdProto node_id = 2;
    google.protobuf.Timestamp expires_at = 3;
    google.protobuf.Timestamp renew_at = 4;
}
```

**Features:**
- Type-safe actor keys (String, Int32, Int64, NoKey)
- Actor lease management
- Namespace support

---

## gRPC Services

### ConnectionService

Bidirectional streaming service for actor communication.

```protobuf
service ConnectionService {
    rpc OpenStream(stream MessageProto) returns (stream MessageProto);
    rpc GetConnectionInfo(ConnectionInfoRequestProto) returns (ConnectionInfoResponseProto);
}
```

**Implementation** (`orbit-proto/src/services.rs`):
```rust
pub struct OrbitConnectionService {
    connections: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<MessageProto>>>>,
}

impl connection_service_server::ConnectionService for OrbitConnectionService {
    type OpenStreamStream = tokio_stream::wrappers::UnboundedReceiverStream<Result<MessageProto, Status>>;

    async fn open_stream(
        &self,
        request: Request<Streaming<MessageProto>>,
    ) -> Result<Response<Self::OpenStreamStream>, Status> {
        // Bidirectional message streaming
    }

    async fn get_connection_info(
        &self,
        _request: Request<ConnectionInfoRequestProto>,
    ) -> Result<Response<ConnectionInfoResponseProto>, Status> {
        // Return node connection information
    }
}
```

**Usage:**
```rust
use orbit_proto::connection_service_client::ConnectionServiceClient;

let mut client = ConnectionServiceClient::connect("http://[::1]:50051").await?;
let (tx, rx) = mpsc::unbounded_channel();

let response = client.open_stream(tokio_stream::wrappers::UnboundedReceiverStream::new(rx)).await?;
let mut stream = response.into_inner();

// Send messages
tx.send(message)?;

// Receive responses
while let Some(response) = stream.message().await? {
    println!("Received: {:?}", response);
}
```

### HealthService

Standard health check service for monitoring.

```protobuf
service HealthService {
    rpc Check(HealthCheckRequest) returns (HealthCheckResponse);
    rpc Watch(HealthCheckRequest) returns (stream HealthCheckResponse);
}

enum ServingStatus {
    UNKNOWN = 0;
    SERVING = 1;
    NOT_SERVING = 2;
    SERVICE_UNKNOWN = 3;
}
```

**Implementation:**
```rust
pub struct OrbitHealthService {
    status: Arc<Mutex<health_check_response::ServingStatus>>,
}

impl OrbitHealthService {
    pub async fn set_serving_status(&self, status: health_check_response::ServingStatus) {
        let mut current_status = self.status.lock().await;
        *current_status = status;
    }
}
```

**Usage:**
```rust
use orbit_proto::health_service_client::HealthServiceClient;

let mut client = HealthServiceClient::connect("http://[::1]:50051").await?;
let request = HealthCheckRequest { service: "orbit".to_string() };
let response = client.check(request).await?;

match response.into_inner().status {
    ServingStatus::Serving => println!("Service is healthy"),
    _ => println!("Service is unhealthy"),
}
```

---

## Transport Layer

### Transaction Transport (`orbit-shared/src/transport.rs`)

High-performance gRPC transport for distributed transactions with connection pooling, retry logic, and metrics.

#### Configuration

```rust
use orbit_shared::transport::TransportConfig;
use std::time::Duration;

let config = TransportConfig {
    max_connections_per_endpoint: 10,
    connect_timeout: Duration::from_secs(5),
    request_timeout: Duration::from_secs(30),
    keep_alive_interval: Some(Duration::from_secs(30)),
    keep_alive_timeout: Some(Duration::from_secs(10)),
    max_message_size: 16 * 1024 * 1024, // 16MB
    retry_attempts: 3,
    retry_backoff_initial: Duration::from_millis(100),
    retry_backoff_multiplier: 2.0,
    tcp_keepalive: Some(Duration::from_secs(10)),
    http2_adaptive_window: true,
};
```

#### Connection Pool

Automatic connection management with health tracking:

```rust
use orbit_shared::transport::ConnectionPool;

let pool = ConnectionPool::new(config);

// Automatic connection creation and caching
let client = pool.get_connection("http://node1:50051").await?;

// Automatic cleanup of idle connections
pool.cleanup_idle_connections(Duration::from_secs(600)).await?;

// Get pool statistics
let stats = pool.get_stats().await;
println!("Connections: {}", stats.total_connections);
println!("Requests: {}", stats.total_requests);
println!("Errors: {}", stats.total_errors);
println!("Avg Latency: {}ms", stats.average_latency_ms);
```

**Connection Metrics:**
- Creation time tracking
- Last used timestamp
- Request count
- Error count
- Average latency (exponential moving average)

#### gRPC Transaction Sender

```rust
use orbit_shared::transport::GrpcTransactionMessageSender;
use orbit_shared::transactions::TransactionMessage;

let sender = GrpcTransactionMessageSender::new(
    node_id,
    Arc::new(node_resolver),
    config,
);

// Start background maintenance
sender.start_background_tasks().await?;

// Send transaction message
sender.send_message(&target_actor, message).await?;

// Broadcast to multiple actors
sender.broadcast_message(&targets, message).await?;

// Get transport stats
let stats = sender.get_stats().await;
```

**Features:**
- Automatic retry with exponential backoff
- Request timeout enforcement
- Node resolution integration
- Broadcast optimization (groups by node)
- Comprehensive error handling
- Non-retryable error detection (InvalidArgument, NotFound, PermissionDenied)

#### Node Resolution

Implement the `NodeResolver` trait to integrate with your directory service:

```rust
use orbit_shared::transport::NodeResolver;
use async_trait::async_trait;

struct EtcdNodeResolver {
    // etcd client
}

#[async_trait]
impl NodeResolver for EtcdNodeResolver {
    async fn resolve_node(&self, node_id: &NodeId) -> OrbitResult<String> {
        // Query etcd for node endpoint
        Ok(format!("http://{}:50051", node_id.key))
    }

    async fn resolve_addressable(&self, addressable: &AddressableReference) -> OrbitResult<NodeId> {
        // Query directory for actor location
        // ...
    }
}
```

### Raft Consensus Transport (`orbit-shared/src/raft_transport.rs`)

Specialized gRPC transport for Raft consensus protocol.

#### Setup

```rust
use orbit_shared::raft_transport::GrpcRaftTransport;
use std::collections::HashMap;

let mut node_addresses = HashMap::new();
node_addresses.insert(
    NodeId::new("node-1".to_string(), "default".to_string()),
    "http://node1:50051".to_string(),
);
node_addresses.insert(
    NodeId::new("node-2".to_string(), "default".to_string()),
    "http://node2:50051".to_string(),
);

let transport = GrpcRaftTransport::new(
    node_id,
    node_addresses,
    Duration::from_secs(5),  // connection timeout
    Duration::from_secs(10), // request timeout
);
```

#### RaftTransport Trait

```rust
#[async_trait]
pub trait RaftTransport: Send + Sync {
    async fn send_vote_request(
        &self,
        target: &NodeId,
        request: VoteRequest,
    ) -> OrbitResult<VoteResponse>;

    async fn send_append_entries(
        &self,
        target: &NodeId,
        request: AppendEntriesRequest,
    ) -> OrbitResult<AppendEntriesResponse>;

    async fn broadcast_heartbeat(
        &self,
        nodes: &[NodeId],
        request: AppendEntriesRequest,
    ) -> OrbitResult<Vec<AppendEntriesResponse>>;
}
```

#### Integration with Raft Consensus

```rust
use orbit_shared::consensus::RaftConsensus;
use orbit_shared::raft_transport::start_raft_server;

// Create consensus instance
let consensus = Arc::new(RaftConsensus::new(
    node_id.clone(),
    cluster_nodes,
    RaftConfig::default(),
));

// Start transport
let transport = Arc::new(GrpcRaftTransport::new(
    node_id,
    node_addresses,
    Duration::from_secs(5),
    Duration::from_secs(10),
));

consensus.start(transport).await?;

// Start gRPC server for incoming Raft messages
tokio::spawn(async move {
    start_raft_server(consensus.clone(), "0.0.0.0:50051").await
});
```

**Features:**
- Automatic client connection management
- Connection failure handling and reconnection
- Concurrent heartbeat broadcasting
- Dynamic node address updates
- Timeout-based failure detection

---

## Message Serialization

### Protocol Buffer Converters

The `orbit-proto/src/converters.rs` module provides bidirectional conversion between Rust domain types and Protocol Buffer messages.

#### Key Converters

```rust
use orbit_proto::*;
use orbit_shared::*;

// Key conversion
let rust_key = Key::StringKey { key: "actor-123".to_string() };
let proto_key = KeyConverter::to_proto(&rust_key);
let back_to_rust = KeyConverter::from_proto(&proto_key)?;

// NodeId conversion
let node_id = NodeId::new("node-1".to_string(), "default".to_string());
let proto_node_id = NodeIdConverter::to_proto(&node_id);
let back_to_node_id = NodeIdConverter::from_proto(&proto_node_id);

// AddressableReference conversion
let actor_ref = AddressableReference {
    addressable_type: "MyActor".to_string(),
    key: Key::StringKey { key: "actor-123".to_string() },
};
let proto_ref = AddressableReferenceConverter::to_proto(&actor_ref);
let back_to_ref = AddressableReferenceConverter::from_proto(&proto_ref)?;

// Timestamp conversion
let now = Utc::now();
let proto_timestamp = TimestampConverter::to_proto(&now);
let back_to_datetime = TimestampConverter::from_proto(&proto_timestamp);
```

#### Available Converters

- **KeyConverter**: Handles all key types (String, Int32, Int64, NoKey)
- **NodeIdConverter**: Node identification
- **AddressableReferenceConverter**: Actor references
- **TimestampConverter**: DateTime ↔ protobuf Timestamp
- **InvocationReasonConverter**: Invocation vs Rerouted
- **NodeStatusConverter**: Active, Draining, Stopped

### Transaction Message Serialization

Internal conversion for transaction protocol messages:

```rust
// Transaction message → Protobuf
let tx_message = TransactionMessage::Prepare {
    transaction_id: tx_id.clone(),
    operations: vec![operation],
    timeout: Duration::from_secs(30),
};

let proto_message = convert_message_to_proto(tx_message)?;

// Protobuf → Transaction message (handled in transport layer)
```

**Supported Message Types:**
- **Prepare**: Transaction initiation with operations
- **Vote**: Participant vote (Yes, No, Uncertain)
- **Commit**: Commit decision
- **Abort**: Abort with reason
- **Acknowledge**: Operation acknowledgment
- **QueryStatus**: Status inquiry
- **StatusResponse**: Current transaction state

---

## Performance Optimization

### Connection Pooling

```rust
// Configurable pool settings
let config = TransportConfig {
    max_connections_per_endpoint: 10,  // Pool size per endpoint
    ..Default::default()
};

// Automatic connection reuse
let pool = ConnectionPool::new(config);
let client1 = pool.get_connection("http://node1:50051").await?; // Creates new
let client2 = pool.get_connection("http://node1:50051").await?; // Reuses existing

// Background cleanup (runs every 5 minutes)
pool.cleanup_idle_connections(Duration::from_secs(600)).await?;
```

**Benefits:**
- Eliminates connection establishment overhead
- Reduces TCP handshake latency
- Maintains persistent HTTP/2 connections
- Automatic health-based cleanup

### Retry Logic

```rust
let config = TransportConfig {
    retry_attempts: 3,                                // Max retries
    retry_backoff_initial: Duration::from_millis(100), // Initial delay
    retry_backoff_multiplier: 2.0,                    // Exponential backoff
    ..Default::default()
};

// Automatic retry with backoff:
// Attempt 1: immediate
// Attempt 2: +100ms
// Attempt 3: +200ms
// Attempt 4: +400ms
```

**Retry Strategy:**
- Exponential backoff prevents thundering herd
- Non-retryable errors exit immediately (InvalidArgument, NotFound, PermissionDenied)
- Timeout errors trigger retry
- Network errors trigger retry

### Keep-Alive Configuration

```rust
let config = TransportConfig {
    keep_alive_interval: Some(Duration::from_secs(30)),  // Send keep-alive every 30s
    keep_alive_timeout: Some(Duration::from_secs(10)),   // Timeout after 10s
    tcp_keepalive: Some(Duration::from_secs(10)),        // TCP-level keep-alive
    http2_adaptive_window: true,                         // Adaptive flow control
    ..Default::default()
};
```

**Benefits:**
- Detects dead connections quickly
- Prevents firewall connection drops
- Maintains connection health
- HTTP/2 flow control optimization

### Message Size Limits

```rust
let config = TransportConfig {
    max_message_size: 16 * 1024 * 1024, // 16MB max message size
    ..Default::default()
};

// Applied to both encoding and decoding
let client = TransactionServiceClient::new(channel)
    .max_decoding_message_size(config.max_message_size)
    .max_encoding_message_size(config.max_message_size);
```

---

## Monitoring and Observability

### Connection Metrics

```rust
let stats = pool.get_stats().await;
println!("Total connections: {}", stats.total_connections);
println!("Total requests: {}", stats.total_requests);
println!("Total errors: {}", stats.total_errors);
println!("Average latency: {}ms", stats.average_latency_ms);
```

**Metrics Tracked:**
- Connection creation time
- Last used timestamp
- Request count per connection
- Error count per connection
- Average latency (exponential moving average)

### Health Checks

```rust
// Set service status
health_service.set_serving_status(
    health_check_response::ServingStatus::Serving
).await;

// Query health
let response = health_client.check(HealthCheckRequest {
    service: "orbit".to_string(),
}).await?;

// Watch for health changes
let mut watch_stream = health_client.watch(HealthCheckRequest {
    service: "orbit".to_string(),
}).await?.into_inner();

while let Some(status) = watch_stream.message().await? {
    println!("Health status: {:?}", status);
}
```

### Tracing Integration

The transport layer uses `tracing` crate for structured logging:

```rust
use tracing::{info, warn, error, debug};

// Connection events
debug!("Creating new gRPC connection to: {}", endpoint_url);
info!("Created new gRPC connection to: {}", endpoint_url);

// Request failures
warn!("Vote request to {} failed: {}", target, status);
error!("Connection cleanup failed: {}", e);

// Transaction events
debug!("Sent transaction message to {} ({}ms)", target, response.processing_time_ms);
info!("Broadcast completed: {} successful sends", total_successful);
```

**Log Levels:**
- **debug**: Connection lifecycle, message routing details
- **info**: Connection creation, broadcast completion, successful operations
- **warn**: Request failures, timeouts (with automatic retry)
- **error**: Fatal errors, connection cleanup failures

---

## Error Handling

### Error Types

```rust
use orbit_shared::exception::{OrbitError, OrbitResult};

// Network errors
OrbitError::network("Connection failed")
OrbitError::timeout("Request timeout")

// Cluster errors
OrbitError::cluster("Node not found")

// Internal errors
OrbitError::internal("Invalid endpoint URL")

// Configuration errors
OrbitError::configuration("Invalid bind address")
```

### Retry Classification

**Retryable Errors:**
- Network connection failures
- Timeout errors
- Transient gRPC errors (Unavailable, DeadlineExceeded)

**Non-Retryable Errors:**
- `Code::InvalidArgument` - Invalid request format
- `Code::NotFound` - Resource doesn't exist
- `Code::PermissionDenied` - Authorization failure

### Error Recovery

```rust
// Automatic connection removal on failure
async fn remove_client(&self, target_node: &NodeId) {
    let mut clients = self.clients.write().await;
    if clients.remove(target_node).is_some() {
        debug!("Removed client connection to node {}", target_node);
    }
}

// Next request will create a fresh connection
```

---

## Usage Examples

### Complete Actor Invocation Flow

```rust
use orbit_proto::*;
use orbit_shared::*;
use orbit_shared::transport::*;

// 1. Set up transport
let config = TransportConfig::default();
let node_resolver = Arc::new(MyNodeResolver::new());
let sender = GrpcTransactionMessageSender::new(
    node_id,
    node_resolver,
    config,
);

sender.start_background_tasks().await?;

// 2. Define target actor
let target_actor = AddressableReference {
    addressable_type: "BankAccount".to_string(),
    key: Key::StringKey { key: "account-123".to_string() },
};

// 3. Create transaction message
let tx_id = TransactionId::new(node_id.clone());
let operation = TransactionOperation {
    operation_id: "op-1".to_string(),
    target_actor: target_actor.clone(),
    operation_type: "debit".to_string(),
    operation_data: serde_json::json!({"amount": 100}),
    compensation_data: Some(serde_json::json!({"action": "credit", "amount": 100})),
};

let message = TransactionMessage::Prepare {
    transaction_id: tx_id,
    operations: vec![operation],
    timeout: Duration::from_secs(30),
};

// 4. Send message
sender.send_message(&target_actor, message).await?;
```

### Setting Up a Complete Node

```rust
use orbit_proto::*;
use orbit_shared::*;
use orbit_shared::raft_transport::*;
use std::sync::Arc;
use tokio;

#[tokio::main]
async fn main() -> OrbitResult<()> {
    // Node configuration
    let node_id = NodeId::new("node-1".to_string(), "default".to_string());
    
    // Set up Raft transport
    let mut node_addresses = HashMap::new();
    node_addresses.insert(
        NodeId::new("node-2".to_string(), "default".to_string()),
        "http://node2:50051".to_string(),
    );
    
    let raft_transport = Arc::new(GrpcRaftTransport::new(
        node_id.clone(),
        node_addresses,
        Duration::from_secs(5),
        Duration::from_secs(10),
    ));
    
    // Create consensus
    let consensus = Arc::new(RaftConsensus::new(
        node_id.clone(),
        vec![node_id.clone()],
        RaftConfig::default(),
    ));
    
    // Start consensus
    consensus.start(raft_transport).await?;
    
    // Start gRPC server
    start_raft_server(consensus.clone(), "0.0.0.0:50051").await?;
    
    Ok(())
}
```

### Broadcast Optimization Example

```rust
// Broadcast to multiple actors
let targets = vec![
    AddressableReference {
        addressable_type: "Counter".to_string(),
        key: Key::StringKey { key: "counter-1".to_string() },
    },
    AddressableReference {
        addressable_type: "Counter".to_string(),
        key: Key::StringKey { key: "counter-2".to_string() },
    },
    AddressableReference {
        addressable_type: "Counter".to_string(),
        key: Key::StringKey { key: "counter-3".to_string() },
    },
];

let message = TransactionMessage::Commit { transaction_id: tx_id };

// Automatically groups targets by hosting node
// Sends one broadcast request per node instead of N individual requests
sender.broadcast_message(&targets, message).await?;
```

---

## Best Practices

### 1. Connection Pool Configuration

```rust
// Production configuration
let config = TransportConfig {
    max_connections_per_endpoint: 10,
    connect_timeout: Duration::from_secs(5),
    request_timeout: Duration::from_secs(30),
    keep_alive_interval: Some(Duration::from_secs(30)),
    keep_alive_timeout: Some(Duration::from_secs(10)),
    max_message_size: 16 * 1024 * 1024,
    retry_attempts: 3,
    retry_backoff_initial: Duration::from_millis(100),
    retry_backoff_multiplier: 2.0,
    tcp_keepalive: Some(Duration::from_secs(10)),
    http2_adaptive_window: true,
};
```

**Recommendations:**
- **Development**: 2-5 connections per endpoint
- **Production**: 10-20 connections per endpoint
- **High throughput**: 20-50 connections per endpoint
- **Request timeout**: 2-3x expected P99 latency

### 2. Error Handling Patterns

```rust
// Pattern 1: Retry on network errors
match sender.send_message(&target, message.clone()).await {
    Ok(_) => println!("Message sent successfully"),
    Err(OrbitError::Network(_)) | Err(OrbitError::Timeout(_)) => {
        // Already retried automatically, log and handle
        error!("Failed to send message after retries");
    }
    Err(e) => {
        // Non-retryable error
        error!("Fatal error: {}", e);
    }
}

// Pattern 2: Circuit breaker for node failures
let mut failure_count = 0;
const MAX_FAILURES: u32 = 5;

for message in messages {
    match sender.send_message(&target, message).await {
        Ok(_) => failure_count = 0,
        Err(_) => {
            failure_count += 1;
            if failure_count >= MAX_FAILURES {
                // Open circuit, stop sending
                break;
            }
        }
    }
}
```

### 3. Monitoring Integration

```rust
// Periodic metrics collection
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        let stats = sender.get_stats().await;
        
        // Export to monitoring system
        metrics::gauge!("orbit.transport.connections", stats.total_connections as f64);
        metrics::counter!("orbit.transport.requests", stats.total_requests);
        metrics::counter!("orbit.transport.errors", stats.total_errors);
        metrics::histogram!("orbit.transport.latency_ms", stats.average_latency_ms);
    }
});
```

### 4. Dynamic Node Management

```rust
// Add new nodes dynamically
raft_transport.update_node_address(
    NodeId::new("node-3".to_string(), "default".to_string()),
    "http://node3:50051".to_string(),
).await;

// Existing connections are automatically removed and recreated
```

### 5. Graceful Shutdown

```rust
// Stop accepting new connections
health_service.set_serving_status(
    health_check_response::ServingStatus::NotServing
).await;

// Drain in-flight requests
tokio::time::sleep(Duration::from_secs(30)).await;

// Cleanup connections
pool.cleanup_idle_connections(Duration::from_secs(0)).await?;
```

---

## Testing

### Unit Tests

```rust
#[tokio::test]
async fn test_connection_pool_reuse() {
    let config = TransportConfig::default();
    let pool = ConnectionPool::new(config);
    
    let client1 = pool.get_connection("http://localhost:50051").await.unwrap();
    let client2 = pool.get_connection("http://localhost:50051").await.unwrap();
    
    // Should reuse the same connection
    let stats = pool.get_stats().await;
    assert_eq!(stats.total_connections, 1);
}

#[tokio::test]
async fn test_message_conversion() {
    let tx_id = TransactionId::new(NodeId::new("test".to_string(), "default".to_string()));
    let message = TransactionMessage::Commit { transaction_id: tx_id.clone() };
    
    let proto = convert_message_to_proto(message).unwrap();
    assert!(proto.message_type.is_some());
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_transaction_flow() {
    // Start server
    let server_handle = tokio::spawn(async {
        start_raft_server(consensus, "127.0.0.1:50051").await
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Create client
    let sender = GrpcTransactionMessageSender::new(
        node_id,
        Arc::new(mock_resolver),
        TransportConfig::default(),
    );
    
    // Send message
    let result = sender.send_message(&target, message).await;
    assert!(result.is_ok());
    
    // Cleanup
    server_handle.abort();
}
```

---

## Troubleshooting

### Connection Failures

**Symptom:** `OrbitError::Network("Connection failed")`

**Solutions:**
1. Verify endpoint URL format: `http://hostname:port`
2. Check network connectivity: `telnet hostname port`
3. Verify DNS resolution
4. Check firewall rules
5. Increase `connect_timeout`

### Timeout Errors

**Symptom:** `OrbitError::Timeout("Request timeout")`

**Solutions:**
1. Increase `request_timeout` in config
2. Check server health and load
3. Enable detailed tracing: `RUST_LOG=debug`
4. Monitor connection pool stats
5. Verify network latency

### High Latency

**Symptom:** Slow request processing

**Solutions:**
1. Check connection pool configuration
2. Enable HTTP/2 adaptive window
3. Tune TCP keepalive settings
4. Monitor connection reuse rate
5. Consider adding more connections per endpoint

### Memory Leaks

**Symptom:** Growing memory usage

**Solutions:**
1. Enable connection cleanup background task
2. Reduce `max_connections_per_endpoint`
3. Decrease connection idle timeout
4. Monitor connection pool stats
5. Check for unclosed streams

### gRPC Status Codes

Common gRPC errors and their meanings:

- **UNAVAILABLE**: Service temporarily unavailable (retryable)
- **DEADLINE_EXCEEDED**: Request timeout (retryable)
- **INVALID_ARGUMENT**: Invalid request format (not retryable)
- **NOT_FOUND**: Resource doesn't exist (not retryable)
- **PERMISSION_DENIED**: Authorization failure (not retryable)
- **RESOURCE_EXHAUSTED**: Rate limited or quota exceeded (retryable with backoff)

---

## Performance Characteristics

### Throughput

**Expected Performance:**
- Single connection: 10,000-50,000 RPS
- Connection pool (10 connections): 100,000-500,000 RPS
- Network-bound at 1Gbps: ~80,000 RPS for 1KB messages

### Latency

**P50 Latency:**
- Local network: 0.5-1ms
- Same datacenter: 1-5ms
- Cross-region: 50-200ms

**P99 Latency:**
- Local network: 2-5ms
- Same datacenter: 5-20ms
- Cross-region: 200-500ms

### Resource Usage

**Per Connection:**
- Memory: ~100KB-500KB
- File descriptors: 1
- Threads: Shared tokio runtime

**Connection Pool (10 connections):**
- Memory: ~1-5MB
- File descriptors: 10
- CPU: <1% idle, 10-50% under load

---

## Future Enhancements

### Planned Features

1. **TLS/mTLS Support**
   - Certificate-based authentication
   - Encrypted transport
   - Certificate rotation

2. **Load Balancing**
   - Client-side load balancing
   - Weighted round-robin
   - Least-connections strategy

3. **Circuit Breaker**
   - Automatic failure detection
   - Temporary endpoint isolation
   - Gradual recovery

4. **Distributed Tracing**
   - OpenTelemetry integration
   - Trace propagation
   - Span correlation

5. **Advanced Metrics**
   - Per-endpoint metrics
   - Request/response size histograms
   - Error rate tracking

---

## References

- [gRPC Documentation](https://grpc.io/docs/)
- [Tonic Rust gRPC Framework](https://github.com/hyperium/tonic)
- [Protocol Buffers](https://protobuf.dev/)
- [HTTP/2 Specification](https://http2.github.io/)
- [Orbit-RS Architecture](../ORBIT_ARCHITECTURE.md)
- [Distributed Transactions Guide](./ADVANCED_TRANSACTION_FEATURES.md)

---

**Document Version:** 1.0  
**Last Updated:** 2025-10-03  
**Status:** ✅ Production Ready
