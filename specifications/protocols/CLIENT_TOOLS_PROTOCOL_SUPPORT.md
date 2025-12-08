# Client Tools Protocol Support

**Last Updated**: 2025-12-07
**Purpose**: Document protocol support across OrbitRS client tools (CLI, Desktop, SDKs)
**Related**: See [PROTOCOL_STATUS.md](./PROTOCOL_STATUS.md) for server-side protocol implementation status

---

## Executive Summary

OrbitRS provides multiple client tools for interacting with the database server. The CLI has achieved near-parity with the Desktop app for core protocols.

| Client Tool | Protocols Supported | Primary Use Case |
|-------------|---------------------|------------------|
| orbit/cli | 5 protocols (PostgreSQL, MySQL, Redis, OrbitQL, CQL) | Terminal/scripting |
| orbit/desktop | 7 protocols | GUI management |
| orbit-python-client | REST API | Python applications |
| orbit-vscode-extension | LSP | IDE integration |

---

## 1. orbit/cli (Terminal CLI)

**Location**: `orbit/cli/src/main.rs`
**Technology**: Rust CLI with `rustyline`, `tokio-postgres`, `mysql_async`, `redis`, `reqwest`, `syntect`

### Current Protocol Support

| Protocol | Status | Implementation | Default Port |
|----------|--------|----------------|--------------|
| PostgreSQL | ✅ Fully implemented | `tokio-postgres` wire protocol | 5432 |
| MySQL | ✅ Fully implemented | `mysql_async` driver | 3306 |
| Redis | ✅ Fully implemented | `redis` crate with async | 6379 |
| OrbitQL | ✅ Fully implemented | HTTP REST (`/api/v1/sql`) | 8080 |
| CQL | ✅ Implemented | HTTP REST fallback | 9042 |
| Cypher | ❌ Not supported | Not in enum | - |
| AQL | ❌ Not supported | Not in enum | - |

### Usage Examples

```bash
# PostgreSQL (default)
orbit --protocol postgres -H localhost -p 5432 -d mydb -u user

# MySQL
orbit --protocol mysql -H localhost -p 3306 -d mydb -u user

# Redis
orbit --protocol redis -H localhost -p 6379

# OrbitQL via REST
orbit --protocol orbitql -H localhost -p 8080

# CQL via REST
orbit --protocol cql -H localhost -p 9042

# Execute single command
orbit --protocol postgres -e "SELECT * FROM users;"

# Execute file
orbit --protocol postgres -f queries.sql

# JSON output format
orbit --protocol postgres -o json -e "SELECT * FROM users;"
```

### Features

- Syntax highlighting (SQL via `syntect`)
- Command history (persistent to `~/.orbit_history`)
- Multi-line query support (for SQL protocols)
- Output formats: Table, JSON, CSV, Plain
- Meta commands: `\q`, `\?`, `\d`, `\dt`, `\l`, `\format`, `\timing`
- File execution mode (`-f`)
- Single command mode (`-e`)
- Protocol-specific command handling (Redis commands execute immediately)

### Protocol-Specific Behavior

| Protocol | Query Terminator | Multi-line | Special Handling |
|----------|------------------|------------|------------------|
| PostgreSQL | Semicolon (`;`) | Yes | Standard SQL |
| MySQL | Semicolon (`;`) | Yes | Standard SQL |
| OrbitQL | Semicolon (`;`) | Yes | REST API with JSON body |
| CQL | Semicolon (`;`) | Yes | REST API with protocol hint |
| Redis | Newline | No | Commands execute immediately |

### ReplState Architecture

```rust
struct ReplState {
    protocol: Protocol,
    host: String,
    port: u16,
    database: String,
    username: String,
    password: Option<String>,
    format: OutputFormat,
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    // Protocol-specific connections
    pg_client: Option<Client>,                    // PostgreSQL
    mysql_pool: Option<mysql_async::Pool>,        // MySQL
    redis_client: Option<redis::Client>,          // Redis
    http_client: Option<reqwest::Client>,         // OrbitQL/CQL
}
```

### Remaining Gaps

| Gap | Priority | Effort | Notes |
|-----|----------|--------|-------|
| Cypher support | Low | Medium | HTTP REST to Bolt endpoint |
| AQL support | Low | Medium | HTTP REST to ArangoDB API |
| Native CQL driver | Low | Medium | Replace REST with `cdrs-tokio` |

---

## 2. orbit/desktop (Tauri Desktop App)

**Location**: `orbit/desktop/src-tauri/`
**Technology**: Tauri (Rust backend + Web frontend)

### Current Protocol Support

| Protocol | Status | Implementation | Default Port |
|----------|--------|----------------|--------------|
| PostgreSQL | ✅ Implemented | `tokio-postgres` | 5432 |
| OrbitQL | ✅ Implemented | HTTP REST (`/query`) | 8080 |
| Redis | ✅ Implemented | `redis` crate | 6379 |
| MySQL | ✅ Implemented | `mysql_async` | 3306 |
| CQL | ✅ Implemented | HTTP REST | 9042 |
| Cypher | ✅ Implemented | HTTP REST (Neo4j API) | 7474 |
| AQL | ✅ Implemented | HTTP REST (ArangoDB API) | 8529 |

### Connection Manager Architecture

```rust
// orbit/desktop/src-tauri/src/connections.rs

pub enum ConnectionType {
    PostgreSQL,
    OrbitQL,
    Redis,
    MySQL,
    CQL,
    Cypher,
    AQL,
}

pub trait DatabaseConnection: Send + Sync {
    fn connection_type(&self) -> ConnectionType;
    fn is_connected(&self) -> bool;
    fn disconnect(&mut self) -> Result<(), ConnectionError>;
}
```

### Protocol-Specific Implementations

| Protocol | Struct | Query Method |
|----------|--------|--------------|
| PostgreSQL | `PostgreSQLConnection` | `execute_query()` |
| OrbitQL | `OrbitQLConnection` | `execute_orbitql()` via HTTP |
| Redis | `RedisConnection` | `execute_redis_command()` |
| MySQL | `MySQLConnection` | `execute_query()` |
| CQL | `CQLConnection` | `execute_cql()` via HTTP |
| Cypher | `CypherConnection` | `execute_cypher()` via HTTP |
| AQL | `AQLConnection` | `execute_aql()` via HTTP |

### Key Files

- `connections.rs` - Connection management (890 lines)
- `queries.rs` - Query execution
- `models.rs` - Data models
- `storage.rs` - Local storage
- `encryption.rs` - Credential encryption

---

## 3. Parser/Lexer Architecture

### Two Separate Parser Implementations

OrbitRS has **two completely independent parsers**:

#### PostgreSQL Parser (in `orbit-server`)

**Location**: `orbit/server/src/protocols/postgres_wire/sql/`

```text
orbit/server/src/protocols/postgres_wire/sql/
├── lexer.rs           # Token definitions
├── parser/
│   ├── mod.rs         # Parser coordination
│   ├── dml.rs         # SELECT, INSERT, UPDATE, DELETE
│   ├── ddl.rs         # CREATE, ALTER, DROP
│   ├── expressions.rs # Expression parsing
│   ├── select.rs      # SELECT-specific parsing
│   ├── tcl.rs         # Transaction control
│   └── dcl.rs         # Data control
├── ast.rs             # AST definitions
├── executor.rs        # Query execution
└── expression_evaluator.rs # Expression evaluation
```

**Features**:
- Standard SQL compatibility
- PostgreSQL-specific extensions (JSONB, arrays, vectors)
- Window functions, CTEs, subqueries
- Some OrbitQL extensions baked in (TRAVERSE clause)

#### OrbitQL Parser (in `orbit-shared`)

**Location**: `orbit/shared/src/orbitql/`

```text
orbit/shared/src/orbitql/
├── lexer.rs           # OrbitQL tokens
├── parser.rs          # OrbitQL parser
├── ast.rs             # OrbitQL AST
├── executor.rs        # Query execution
├── planner.rs         # Query planning
├── optimizer.rs       # Query optimization
├── spatial.rs         # Spatial queries
├── streaming.rs       # Real-time queries
└── distributed.rs     # Distributed execution
```

**Unique Features**:
- Graph keywords: `Node`, `Edge`, `Path`, `Traverse`, `MaxDepth`
- Time-series: `Metrics`, `Aggregate`, `Window`, `Range`, `Now`
- Live queries: `Live`, `Diff`, `Fetch`
- Position tracking for better error messages

### Parser Selection Logic

**Over PostgreSQL Wire Protocol (Port 5432)**:
- Always uses PostgreSQL parser
- No runtime switching mechanism
- OrbitQL extensions available via embedded TRAVERSE clause

**Over REST API (Port 8080)**:
- `POST /api/v1/sql` - Uses PostgreSQL parser
- OrbitQL can be added as separate endpoint

**OrbitQL Direct (Port 8081)**:
- Dedicated OrbitQL parser
- Full OrbitQL language support

---

## 4. Protocol Exposure Summary

### Server-Side Protocol Ports

| Protocol | Port | Parser Used | CLI Support | Desktop Support |
|----------|------|-------------|-------------|-----------------|
| PostgreSQL Wire | 5432 | PostgreSQL | ✅ | ✅ |
| MySQL Wire | 3306 | MySQL | ✅ | ✅ |
| Redis RESP | 6379 | RESP | ✅ | ✅ |
| CQL | 9042 | CQL | ✅ (REST) | ✅ |
| Cypher/Bolt | 7687 | Cypher | ❌ | ✅ |
| AQL | 8529 | AQL | ❌ | ✅ |
| REST API | 8080 | PostgreSQL | ✅ (OrbitQL) | ✅ |
| OrbitQL | 8081 | OrbitQL | ✅ | ✅ |
| gRPC | 50051 | Protobuf | ❌ | Internal |

### Feature Matrix

| Feature | CLI | Desktop | Python SDK |
|---------|-----|---------|------------|
| PostgreSQL queries | ✅ | ✅ | ✅ (via REST) |
| MySQL queries | ✅ | ✅ | ❌ |
| Redis commands | ✅ | ✅ | ❌ |
| OrbitQL queries | ✅ | ✅ | ❌ |
| CQL queries | ✅ | ✅ | ❌ |
| Graph queries (Cypher) | ❌ | ✅ | ❌ |
| AQL queries | ❌ | ✅ | ❌ |
| Syntax highlighting | ✅ | ✅ | N/A |
| Connection management | Basic | Full | Basic |
| Query history | ✅ | ✅ | ❌ |
| Multiple formats | ✅ | ✅ | JSON only |

---

## 5. Future Enhancements

### Phase 1: Complete CLI Protocol Parity (Low Priority)

**Goal**: Add remaining protocols to CLI

#### Task 1.1: Add Cypher to CLI
```rust
// Add to Protocol enum
enum Protocol {
    Postgres,
    Mysql,
    Cql,
    Redis,
    Orbitql,
    Cypher,  // NEW
}

// Implement Cypher via HTTP REST (Bolt endpoint)
impl ReplState {
    async fn execute_cypher(&self, query: &str) -> Result<()> {
        let url = format!("http://{}:{}/db/neo4j/tx/commit", self.host, self.port);
        // HTTP POST with Cypher query in JSON body
    }
}
```

**Effort**: Medium (2-3 days)

#### Task 1.2: Add AQL to CLI
```rust
// Add AQL support via ArangoDB REST API
async fn execute_aql(&self, query: &str) -> Result<()> {
    let url = format!("http://{}:{}/_api/cursor", self.host, self.port);
    // HTTP POST with AQL query
}
```

**Effort**: Medium (2-3 days)

### Phase 2: Unified Query Interface

**Goal**: Single query interface that routes to appropriate protocol

```rust
// Detect query type and route appropriately
fn detect_query_type(query: &str) -> QueryType {
    let upper = query.trim().to_uppercase();

    if upper.starts_with("MATCH") || upper.starts_with("CREATE (") {
        QueryType::Cypher
    } else if upper.starts_with("FOR ") && upper.contains(" IN ") {
        QueryType::AQL
    } else if upper.starts_with("GET ") || upper.starts_with("SET ") {
        QueryType::Redis
    } else if upper.starts_with("SELECT ") && upper.contains("TRAVERSE") {
        QueryType::OrbitQL
    } else {
        QueryType::SQL  // Default to PostgreSQL
    }
}
```

### Phase 3: Python SDK Enhancement

**Goal**: Add protocol support beyond REST API

```python
# orbit-python-client enhancements
class OrbitClient:
    def connect_postgres(self, host, port, database, user, password):
        """Direct PostgreSQL wire protocol"""
        pass

    def connect_redis(self, host, port):
        """Direct Redis RESP protocol"""
        pass

    def execute_orbitql(self, query):
        """OrbitQL via REST or dedicated port"""
        pass
```

---

## 6. Testing Requirements

### CLI Protocol Tests

```rust
#[tokio::test]
async fn test_cli_postgres_connection() {
    let cli = Cli::parse_from(&["orbit", "--protocol", "postgres"]);
    let mut state = ReplState::new(&cli);
    assert!(state.connect_postgres().await.is_ok());
}

#[tokio::test]
async fn test_cli_mysql_connection() {
    let cli = Cli::parse_from(&["orbit", "--protocol", "mysql"]);
    let mut state = ReplState::new(&cli);
    assert!(state.connect_mysql().await.is_ok());
}

#[tokio::test]
async fn test_cli_redis_connection() {
    let cli = Cli::parse_from(&["orbit", "--protocol", "redis"]);
    let mut state = ReplState::new(&cli);
    assert!(state.connect_redis().await.is_ok());
}

#[tokio::test]
async fn test_cli_orbitql_via_rest() {
    let cli = Cli::parse_from(&["orbit", "--protocol", "orbitql"]);
    let mut state = ReplState::new(&cli);
    assert!(state.connect_orbitql().await.is_ok());
}

#[tokio::test]
async fn test_cli_cql_via_rest() {
    let cli = Cli::parse_from(&["orbit", "--protocol", "cql"]);
    let mut state = ReplState::new(&cli);
    assert!(state.connect_cql().await.is_ok());
}
```

### Integration Tests

```bash
# Test all protocols
./scripts/test-cli-protocols.sh

# Protocol-specific tests
cargo test -p orbit-cli -- --test-threads=1
```

---

## 7. Dependencies

### orbit/cli Cargo.toml

```toml
[dependencies]
# Core
tokio = { version = "1.48", features = ["full"] }
clap = { version = "4", features = ["derive"] }
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Terminal UI
rustyline = "14"
syntect = "5"
comfy-table = "7"
owo-colors = "4"
dirs = "5"

# Protocol drivers
tokio-postgres = "0.7"                                    # PostgreSQL
mysql_async = "0.34"                                      # MySQL
redis = { version = "0.25", features = ["tokio-comp"] }   # Redis
reqwest = { version = "0.12", features = ["json"] }       # HTTP for OrbitQL/CQL
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

## Version History

| Date | Changes |
|------|---------|
| 2025-12-07 | Updated to reflect full CLI protocol implementation (PostgreSQL, MySQL, Redis, OrbitQL, CQL) |
| 2025-12-07 | Initial specification |
