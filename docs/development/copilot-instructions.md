# Orbit-RS Copilot Instructions

## Project Overview
Orbit-RS is a distributed virtual actor system framework in Rust, inspired by Microsoft Orleans. The system enables building scalable distributed applications using virtual actors that can be active/inactive with automatic lifecycle management, clustering, and distributed transactions.

## Architecture & Module Boundaries

### Core Modules Structure
- **`orbit-shared`**: Common types, addressable traits, transactions, clustering, error types (`OrbitResult<T>`, `OrbitError`)
- **`orbit-proto`**: gRPC Protocol Buffer definitions via `tonic::include_proto!("orbit.shared")`
- **`orbit-client`**: Client-side actor proxies, invocation management, lease handling
- **`orbit-server`**: Server-side cluster management, load balancing, actor hosting
- **`orbit-util`**: Utilities and base functionality
- **Extensions**: `orbit-server-etcd` (distributed directory), `orbit-server-prometheus` (metrics)

### Actor System Pattern
Actors implement the `Addressable` trait with these key components:
```rust
// Actor identification using typed keys
Key::{StringKey{key}, Int32Key{key}, Int64Key{key}, NoKey}
AddressableReference { addressable_type: String, key: Key }

// Actor lifecycle methods
async fn on_activate(&mut self) -> OrbitResult<()>
async fn on_deactivate(&mut self) -> OrbitResult<()>
```

### Distributed Transaction Architecture
The system implements 2-phase commit with:
- **TransactionCoordinator**: Manages distributed transaction lifecycle
- **TransactionParticipant**: Trait for services to participate (prepare, commit, abort)
- **TransactionOperation**: Individual operations with compensation data
- States: `Preparing → Prepared → Committing → Committed` (or `Aborting → Aborted`)

## Development Workflows

### Workspace Commands
```bash
# Build all modules
cargo build --workspace

# Test all modules (unit + integration + BDD)
cargo test --workspace

# Run specific test suites
cargo test --workspace --lib          # Unit tests only
cargo test --workspace --test integration  # Integration tests
cargo test --workspace --test bdd     # BDD scenarios

# Run benchmarks
cargo bench

# Examples
cargo run --package hello-world
cargo run --package distributed-counter
cargo run --package distributed-transactions
```

### Container Development
The project supports multi-environment deployment:
- `docker-compose.yml` with development/production overlays
- Environment-based configuration via `ORBIT_*` variables
- Raft consensus for leader election: `ORBIT_ELECTION_METHOD=raft`
- DNS-based service discovery: `ORBIT_DISCOVERY_DNS=orbit-server`

## Project-Specific Patterns

### Error Handling Convention
Use `OrbitResult<T>` (alias for `Result<T, OrbitError>`) consistently across all modules. Import from `orbit_shared::exception`.

### Async Patterns
- All actor methods and transaction operations are `async`
- Use `#[async_trait]` for trait definitions with async methods
- Prefer `tokio::sync::{Mutex, RwLock}` for shared state in async contexts

### Serialization Standards
- Actors and messages: `serde` with `#[derive(Serialize, Deserialize)]`
- Cross-service communication: Protocol Buffers via `tonic`
- Transaction operations: JSON-based (`serde_json::Value`) for flexibility

### Testing Architecture
- **Unit tests**: In `src/` modules using standard `#[cfg(test)]`
- **Integration tests**: In `tests/tests/integration.rs` with full actor lifecycle
- **BDD scenarios**: Cucumber tests in `tests/features/` with step definitions
- **Examples**: Working demos in `examples/` that serve as integration tests

### Observability Integration
- Structured logging: `tracing` crate with `info!`, `warn!`, `error!` macros
- Metrics: `metrics` crate with Prometheus exporter support
- Use `NodeId` for cluster node identification across components

### Module Dependencies
When working across modules:
- **Client → Shared**: For actor traits and error types
- **Server → Shared**: For cluster management and routing
- **Proto**: Shared by all for gRPC communication
- Avoid circular dependencies: extensions depend on core, not vice versa

## Key Integration Points

### Actor Registration Pattern
```rust
// In orbit-client
let client = OrbitClient::builder()
    .with_namespace("demo")
    .build().await?;

let actor_ref = client.actor_reference::<dyn GreeterActor>(
    Key::StringKey { key: "my-actor".to_string() }
).await?;
```

### Transaction Coordination Pattern
```rust
// Begin transaction with timeout
let tx_id = coordinator.begin_transaction(Some(Duration::from_secs(30))).await?;

// Add operations with compensation
let operation = TransactionOperation::new(target_actor, "debit", amount)
    .with_compensation(serde_json::json!({"action": "credit", "amount": 100}));

coordinator.add_operation(&tx_id, operation).await?;
coordinator.commit_transaction(&tx_id).await?;
```

### Cluster Node Communication
Use `AddressableReference` for routing and `NodeId` for cluster membership. All inter-node communication goes through gRPC services defined in `orbit-proto`.

## Kubernetes Operator Support

### Operator Architecture
The `orbit-operator` provides native Kubernetes support with three Custom Resource Definitions:
- **OrbitCluster**: Manages complete cluster deployments (StatefulSets, Services, ConfigMaps)
- **OrbitActor**: Manages actor-specific configurations and scaling policies
- **OrbitTransaction**: Configures distributed transaction coordination and persistence

### Operator Development Patterns
```rust
// Custom Resource Definition structure
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(group = "orbit.turingworks.com", version = "v1", kind = "OrbitCluster")]
#[kube(status = "OrbitClusterStatus")]

// Controller reconciliation loop
async fn reconcile_cluster(cluster: Arc<OrbitCluster>, ctx: Arc<ControllerContext>) -> Result<Action>
```

### Kubernetes Deployment Workflow
```bash
# Deploy operator
kubectl apply -f orbit-operator/deploy/crds.yaml
kubectl apply -f orbit-operator/deploy/rbac.yaml
kubectl apply -f orbit-operator/deploy/operator.yaml

# Deploy Orbit cluster
kubectl apply -f orbit-operator/deploy/examples.yaml
```

### Integration with Existing K8s Resources
The operator leverages existing Kubernetes patterns:
- StatefulSets for Orbit server pods with persistent storage
- Services for load balancing and service discovery
- ConfigMaps for configuration management
- RBAC for security and leader election

When implementing new features, follow the transaction system as a reference for proper async patterns, error handling, and multi-module coordination.