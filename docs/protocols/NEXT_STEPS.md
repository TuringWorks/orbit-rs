# Next Steps: Orbit Protocol Adapters Integration

This document outlines the concrete steps needed to complete the protocol adapter implementation.

## Phase 1: Complete RESP (Redis) Actor Integration

### 1.1 Implement State Operations in CommandHandler

**File**: `orbit-protocols/src/resp/commands.rs`

**Current State**: Commands have TODO markers for actor integration

**Action Items**:

```rust
// In cmd_get()
async fn cmd_get(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    let key = args[0].as_string()?;
    
    // TODO: Implement actor state retrieval
    // 1. Parse key to determine actor type and key
    // 2. Get actor reference via self.orbit_client.actor_reference()
    // 3. Call get_state() method on actor
    // 4. Return state value or Null
    
    // Example implementation:
    let actor_ref = self.orbit_client
        .actor_reference::<dyn KVActor>(Key::StringKey { key: key.clone() })
        .await?;
    
    let state = actor_ref.get_value(&key).await?;
    Ok(state.map(RespValue::BulkString).unwrap_or(RespValue::Null))
}
```

**Required Changes**:
1. Define `KVActor` trait in `orbit-shared` for key-value operations
2. Implement `get_value()` and `set_value()` methods
3. Update all 30+ commands to use actual actor operations
4. Add error handling for actor not found, timeout, etc.

### 1.2 Create KVActor Trait

**File**: Create `orbit-shared/src/actors/kv_actor.rs`

```rust
use async_trait::async_trait;
use crate::addressable::Addressable;
use crate::exception::OrbitResult;

#[async_trait]
pub trait KVActor: Addressable {
    async fn get_value(&self, key: &str) -> OrbitResult<Option<Vec<u8>>>;
    async fn set_value(&self, key: &str, value: Vec<u8>) -> OrbitResult<()>;
    async fn delete_value(&self, key: &str) -> OrbitResult<bool>;
    async fn exists(&self, key: &str) -> OrbitResult<bool>;
    async fn keys(&self, pattern: &str) -> OrbitResult<Vec<String>>;
}
```

### 1.3 Integration Tests

**File**: Create `orbit-protocols/tests/resp_integration_tests.rs`

```rust
use redis::Client;

#[tokio::test]
async fn test_redis_client_get_set() {
    // Start RESP server
    // Connect with redis-rs client
    // Test GET/SET operations
    // Verify actor state is updated
}
```

## Phase 2: Complete REST API Actor Integration

### 2.1 Implement Actor Handlers

**File**: `orbit-protocols/src/rest/handlers.rs`

**Action Items**:

```rust
pub async fn get_actor(
    State(state): State<ApiState>,
    Path((actor_type, key)): Path<(String, String)>,
) -> ProtocolResult<impl IntoResponse> {
    // TODO: Implement actual actor state retrieval
    
    // Parse key to appropriate Key type
    let actor_key = parse_key(&key)?;
    
    // Get actor reference dynamically based on actor_type
    let actor_ref = state.orbit_client
        .actor_reference_dynamic(&actor_type, actor_key)
        .await?;
    
    // Retrieve state via reflection or custom method
    let state_json = actor_ref.get_state_json().await?;
    
    let actor_info = ActorInfo {
        actor_type,
        key: serde_json::json!({"StringKey": {"key": key}}),
        state: state_json,
        node_id: Some(actor_ref.node_id().to_string()),
        status: "active".to_string(),
        last_activity: Some(chrono::Utc::now().to_rfc3339()),
    };
    
    Ok((StatusCode::OK, Json(SuccessResponse::new(actor_info))))
}
```

**Required Changes**:
1. Add `actor_reference_dynamic()` method to `OrbitClient`
2. Implement `get_state_json()` for state reflection
3. Add actor creation/deactivation support
4. Implement method invocation with serialization

### 2.2 Transaction Integration

**File**: `orbit-protocols/src/rest/handlers.rs`

```rust
pub async fn begin_transaction(
    State(state): State<ApiState>,
    Json(request): Json<BeginTransactionRequest>,
) -> ProtocolResult<impl IntoResponse> {
    // Get TransactionCoordinator from orbit_client
    let coordinator = state.orbit_client.transaction_coordinator().await?;
    
    // Begin transaction with timeout
    let timeout = request.timeout_ms
        .map(|ms| Duration::from_millis(ms));
    let tx_id = coordinator.begin_transaction(timeout).await?;
    
    let tx_info = TransactionInfo {
        transaction_id: tx_id.to_string(),
        status: "preparing".to_string(),
        operations: vec![],
        created_at: chrono::Utc::now().to_rfc3339(),
        completed_at: None,
    };
    
    Ok((StatusCode::CREATED, Json(SuccessResponse::new(tx_info))))
}
```

### 2.3 WebSocket Event Integration

**File**: `orbit-protocols/src/rest/websocket.rs`

```rust
async fn events_websocket(socket: WebSocket, state: ApiState) {
    // Subscribe to actor events from OrbitClient
    let mut event_stream = state.orbit_client.subscribe_to_events().await?;
    
    tokio::spawn(async move {
        while let Some(event) = event_stream.next().await {
            // Transform orbit event to WebSocketMessage
            let ws_message = match event {
                OrbitEvent::ActorActivated { actor_ref, node_id } => {
                    WebSocketMessage::ActorActivated {
                        actor_type: actor_ref.actor_type().to_string(),
                        key: actor_ref.key_json(),
                        node_id: node_id.to_string(),
                    }
                }
                // Handle other event types
            };
            
            // Send to WebSocket
            let json = serde_json::to_string(&ws_message)?;
            sender.send(Message::Text(json)).await?;
        }
    });
}
```

## Phase 3: PostgreSQL Wire Protocol Implementation

### 3.1 Implement Startup Handshake

**File**: `orbit-protocols/src/postgres_wire/protocol.rs`

```rust
pub async fn handle_startup(&mut self, params: HashMap<String, String>) -> ProtocolResult<()> {
    // Extract database, user from params
    // Send AuthenticationMD5Password or AuthenticationOk
    // Send ParameterStatus messages
    // Send ReadyForQuery
}
```

### 3.2 Implement Authentication

```rust
pub async fn authenticate(&mut self, password: &[u8]) -> ProtocolResult<bool> {
    // Implement MD5 authentication
    // Compare hash with stored hash
    // Return success/failure
}
```

### 3.3 Build SQL Query Parser

**File**: `orbit-protocols/src/postgres_wire/query_engine.rs`

```rust
use sqlparser::{ast::Statement, dialect::PostgreSqlDialect, parser::Parser};

pub async fn execute(&self, query: &str) -> ProtocolResult<QueryResult> {
    let dialect = PostgreSqlDialect {};
    let ast = Parser::parse_sql(&dialect, query)?;
    
    match &ast[0] {
        Statement::Query(query) => self.execute_query(query).await,
        Statement::Insert(insert) => self.execute_insert(insert).await,
        Statement::Update(update) => self.execute_update(update).await,
        Statement::Delete(delete) => self.execute_delete(delete).await,
        _ => Err(ProtocolError::NotImplemented),
    }
}
```

**Add Dependency**: `sqlparser = "0.49"` to Cargo.toml

### 3.4 Implement Query Execution

```rust
async fn execute_query(&self, query: &Query) -> ProtocolResult<QueryResult> {
    // Parse SELECT statement
    // Map table name to actor type
    // Map WHERE clause to actor key filter
    // Execute actor queries
    // Format result as PostgreSQL RowDescription + DataRow
}
```

## Phase 4: Cypher/Bolt Protocol Implementation

### 4.1 Implement Bolt Handshake

**File**: `orbit-protocols/src/cypher/bolt.rs`

```rust
pub async fn handshake(&mut self, versions: &[u8; 16]) -> ProtocolResult<u32> {
    // Parse version proposals
    // Select highest supported version (v4 or v5)
    // Return selected version
    self.version = 4; // Bolt v4
    Ok(4)
}

pub async fn handle_hello(&mut self, extra: HashMap<String, Value>) -> ProtocolResult<()> {
    // Extract user_agent, scheme, principal, credentials
    // Authenticate user
    // Send SUCCESS message with server info
}
```

### 4.2 Build Cypher Parser

**File**: `orbit-protocols/src/cypher/cypher_parser.rs`

```rust
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case},
    IResult,
};

pub struct CypherParser;

impl CypherParser {
    pub fn parse(query: &str) -> ProtocolResult<CypherQuery> {
        // Parse MATCH, CREATE, MERGE, DELETE, RETURN clauses
        // Build AST
    }
    
    fn parse_match_clause(input: &str) -> IResult<&str, MatchClause> {
        // Parse: MATCH (n:Label {prop: value})-[r:REL_TYPE]->(m)
    }
    
    fn parse_where_clause(input: &str) -> IResult<&str, WhereClause> {
        // Parse: WHERE n.prop = value AND m.prop > 10
    }
}
```

### 4.3 Implement Graph Engine

**File**: `orbit-protocols/src/cypher/graph_engine.rs`

```rust
impl GraphEngine {
    pub async fn execute_query(&self, query: &CypherQuery) -> ProtocolResult<Vec<Record>> {
        match query {
            CypherQuery::Match { pattern, where_clause, return_clause } => {
                // Execute graph traversal
                let nodes = self.find_nodes(pattern).await?;
                let relationships = self.traverse_relationships(nodes, pattern).await?;
                let filtered = self.apply_filters(nodes, where_clause).await?;
                self.project_results(filtered, return_clause).await
            }
            CypherQuery::Create { pattern } => {
                self.create_nodes(pattern).await
            }
            // Handle other query types
        }
    }
    
    async fn find_nodes(&self, pattern: &Pattern) -> ProtocolResult<Vec<GraphNode>> {
        // Map node labels to actor types
        // Query actors matching pattern
        // Return as GraphNode instances
    }
}
```

## Phase 5: Integration Testing

### 5.1 Create Multi-Protocol Test Suite

**File**: `orbit-protocols/tests/integration_tests.rs`

```rust

#[tokio::test]
async fn test_redis_protocol() {
    let client = redis::Client::open("redis://localhost:6379")?;
    let mut con = client.get_connection()?;
    
    redis::cmd("SET").arg("key1").arg("value1").execute(&mut con);
    let value: String = redis::cmd("GET").arg("key1").query(&mut con)?;
    assert_eq!(value, "value1");
}

#[tokio::test]
async fn test_rest_api() {
    let client = reqwest::Client::new();
    
    let response = client
        .post("http://localhost:8080/api/v1/actors")
        .json(&CreateActorRequest { /* ... */ })
        .send()
        .await?;
    
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_postgres_protocol() {
    let (client, connection) = tokio_postgres::connect(
        "host=localhost port=5432 user=orbit",
        NoTls,
    ).await?;
    
    let rows = client.query("SELECT * FROM actors WHERE type = $1", &[&"GreeterActor"]).await?;
    assert!(!rows.is_empty());
}
```

### 5.2 Create End-to-End Examples

**Files**: Create examples for each protocol showing real-world usage

## Phase 6: Performance & Production Readiness

### 6.1 Add Connection Pooling

```rust
// In RESP server
use deadpool::managed::Pool;

pub struct RespServer {
    pool: Pool<OrbitConnection>,
}
```

### 6.2 Add Metrics

```rust
use metrics::{counter, histogram};

impl CommandHandler {
    async fn handle_command(&self, cmd: RespValue) -> ProtocolResult<RespValue> {
        counter!("orbit.resp.commands.total").increment(1);
        let start = Instant::now();
        
        let result = self.execute_command(cmd).await;
        
        histogram!("orbit.resp.command.duration").record(start.elapsed());
        result
    }
}
```

### 6.3 Add Rate Limiting

```rust
use tower::limit::RateLimitLayer;

let app = Router::new()
    .route("/api/v1/actors", post(create_actor))
    .layer(RateLimitLayer::new(100, Duration::from_secs(1)));
```

## Estimated Timeline

| Phase | Estimated Time | Priority |
|-------|---------------|----------|
| Phase 1: RESP Integration | 2-3 days | HIGH |
| Phase 2: REST Integration | 2-3 days | HIGH |
| Phase 3: PostgreSQL | 5-7 days | MEDIUM |
| Phase 4: Cypher/Bolt | 5-7 days | MEDIUM |
| Phase 5: Testing | 3-5 days | HIGH |
| Phase 6: Production | 2-3 days | LOW |

**Total**: 19-28 days

## Dependencies Needed

Add to `orbit-shared/Cargo.toml`:
```toml
async-trait = "0.1"
```

Add to `orbit-client/Cargo.toml`:
```toml

# For dynamic actor references
downcast-rs = "1.2"
```

Add to `orbit-protocols/Cargo.toml`:
```toml

# For SQL parsing
sqlparser = "0.49"

# For connection pooling
deadpool = "0.12"

# For rate limiting (already have tower)
```

## Required OrbitClient Enhancements

1. **Dynamic Actor References**:
   ```rust
   pub async fn actor_reference_dynamic(
       &self,
       actor_type: &str,
       key: Key,
   ) -> OrbitResult<Box<dyn Any>>
   ```

2. **State Reflection**:
   ```rust
   pub async fn get_actor_state_json(&self, actor_ref: &dyn Addressable) -> OrbitResult<serde_json::Value>
   ```

3. **Event Subscriptions**:
   ```rust
   pub async fn subscribe_to_events(&self) -> OrbitResult<EventStream>
   ```

4. **Transaction Coordinator Access**:
   ```rust
   pub async fn transaction_coordinator(&self) -> OrbitResult<Arc<TransactionCoordinator>>
   ```

## Success Criteria

- [ ] Redis client can connect and execute commands
- [ ] REST API endpoints return 200 for all operations
- [ ] WebSocket connections remain stable
- [ ] PostgreSQL client can connect and query
- [ ] Neo4j driver can connect via Bolt
- [ ] All integration tests pass
- [ ] Performance meets requirements (>1000 req/s)
- [ ] Monitoring and metrics are functional

## Getting Started

Start with **Phase 1.1** - implementing GET/SET commands in RESP protocol. This is the smallest, most well-defined task that will validate the entire integration pattern.

```bash

# 1. Create KVActor trait
vim orbit-shared/src/actors/kv_actor.rs

# 2. Implement actor integration in RESP commands
vim orbit-protocols/src/resp/commands.rs

# 3. Test with Redis client
cargo test --package orbit-protocols --test resp_integration
```
