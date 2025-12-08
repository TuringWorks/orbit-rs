# Client Tools Protocol Support

**Last Updated**: 2025-12-07
**Purpose**: Document protocol support across OrbitRS client tools (CLI, Desktop, SDKs)

---

## Executive Summary

OrbitRS provides multiple client tools for interacting with the database server. Each tool has different protocol support levels, creating gaps that affect developer experience.

| Client Tool | Protocols Supported | Primary Use Case |
|-------------|---------------------|------------------|
| orbit/cli | PostgreSQL only | Terminal/scripting |
| orbit/desktop | 7 protocols | GUI management |
| orbit-python-client | REST API | Python applications |
| orbit-vscode-extension | LSP | IDE integration |

---

## 1. orbit/cli (Terminal CLI)

**Location**: `orbit/cli/src/main.rs`
**Technology**: Rust CLI with `rustyline`, `tokio-postgres`, `syntect`

### Current Protocol Support

| Protocol | Status | Implementation | Default Port |
|----------|--------|----------------|--------------|
| PostgreSQL | ✅ Fully implemented | `tokio-postgres` wire protocol | 5432 |
| MySQL | ❌ Declared only | Enum exists, returns error | 3306 |
| CQL | ❌ Declared only | Enum exists, returns error | 9042 |
| Redis | ❌ Not supported | Not in enum | - |
| OrbitQL | ❌ Not supported | Not in enum | - |
| Cypher | ❌ Not supported | Not in enum | - |
| AQL | ❌ Not supported | Not in enum | - |

### Usage Examples

```bash
# Working - PostgreSQL
orbit --protocol postgres -H localhost -p 5432 -d mydb -u user

# Declared but NOT working
orbit --protocol mysql -H localhost -p 3306   # Returns error
orbit --protocol cql -H localhost -p 9042     # Returns error
```

### Features

- Syntax highlighting (SQL via `syntect`)
- Command history (persistent)
- Multi-line query support
- Output formats: Table, JSON, CSV, Plain
- Meta commands: `\q`, `\?`, `\d`, `\dt`, `\l`
- File execution mode (`-f`)
- Single command mode (`-e`)

### Implementation Gaps

| Gap | Priority | Effort | Notes |
|-----|----------|--------|-------|
| MySQL query execution | High | Medium | Add `mysql_async` driver |
| CQL query execution | High | Medium | Add `cdrs-tokio` or HTTP REST |
| Redis commands | Medium | Low | Add `redis` crate |
| OrbitQL support | High | Medium | HTTP REST to `/api/v1/sql` |
| Cypher support | Low | Medium | HTTP REST to Bolt endpoint |
| AQL support | Low | Medium | HTTP REST to ArangoDB API |

### Recommended Implementation Order

1. **OrbitQL via REST** - Highest value, enables full query language
2. **MySQL** - Common protocol, good driver availability
3. **Redis** - Simple command interface
4. **CQL** - Enterprise use cases

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

```
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

```
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

| Protocol | Port | Parser Used | Client Support |
|----------|------|-------------|----------------|
| PostgreSQL Wire | 5432 | PostgreSQL | CLI ✅, Desktop ✅ |
| MySQL Wire | 3306 | MySQL | CLI ❌, Desktop ✅ |
| Redis RESP | 6379 | RESP | CLI ❌, Desktop ✅ |
| CQL | 9042 | CQL | CLI ❌, Desktop ✅ |
| Cypher/Bolt | 7687 | Cypher | CLI ❌, Desktop ✅ |
| AQL | 8529 | AQL | CLI ❌, Desktop ✅ |
| REST API | 8080 | PostgreSQL | SDK ✅ |
| OrbitQL | 8081 | OrbitQL | Desktop ✅ |
| gRPC | 50051 | Protobuf | Internal |

### Gap Matrix

| Feature | CLI | Desktop | Python SDK |
|---------|-----|---------|------------|
| PostgreSQL queries | ✅ | ✅ | ✅ (via REST) |
| MySQL queries | ❌ | ✅ | ❌ |
| Redis commands | ❌ | ✅ | ❌ |
| OrbitQL queries | ❌ | ✅ | ❌ |
| Graph queries | ❌ | ✅ | ❌ |
| Syntax highlighting | ✅ | ✅ | N/A |
| Connection management | Basic | Full | Basic |
| Query history | ✅ | ✅ | ❌ |
| Multiple formats | ✅ | ✅ | JSON only |

---

## 5. Implementation Roadmap

### Phase 1: CLI Protocol Parity (High Priority)

**Goal**: Match desktop protocol support in CLI

#### Task 1.1: Add OrbitQL to CLI
```rust
// Add to Protocol enum
enum Protocol {
    Postgres,
    Mysql,
    Cql,
    OrbitQL,  // NEW
}

// Implement OrbitQL via HTTP REST
impl ReplState {
    async fn execute_orbitql(&self, query: &str) -> Result<()> {
        let url = format!("http://{}:{}/api/v1/sql", self.host, self.port);
        // HTTP POST with JSON body
    }
}
```

**Effort**: Medium (2-3 days)

#### Task 1.2: Add MySQL to CLI
```rust
// Add mysql_async dependency
// Implement MySqlConnection similar to desktop

async fn connect_mysql(&mut self) -> Result<()> {
    let opts = mysql_async::OptsBuilder::default()
        .ip_or_hostname(Some(&self.host))
        .tcp_port(self.port)
        .user(Some(&self.username))
        .pass(self.password.as_deref())
        .db_name(Some(&self.database));

    let pool = mysql_async::Pool::new(opts);
    self.mysql_pool = Some(pool);
    Ok(())
}
```

**Effort**: Medium (2-3 days)

#### Task 1.3: Add Redis to CLI
```rust
// Add redis dependency
// Implement Redis REPL mode

async fn execute_redis(&self, command: &str) -> Result<()> {
    let mut conn = self.redis_client.get_async_connection().await?;
    let parts: Vec<&str> = command.split_whitespace().collect();
    let cmd = parts[0];
    let args = &parts[1..];

    let result: redis::Value = redis::cmd(cmd)
        .arg(args)
        .query_async(&mut conn)
        .await?;

    self.format_redis_result(result)?;
    Ok(())
}
```

**Effort**: Low (1-2 days)

#### Task 1.4: Add CQL to CLI
```rust
// Use cdrs-tokio or HTTP REST fallback
// Implement CQL query execution
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
async fn test_cli_orbitql_via_rest() {
    let cli = Cli::parse_from(&["orbit", "--protocol", "orbitql"]);
    let mut state = ReplState::new(&cli);
    assert!(state.connect_orbitql().await.is_ok());
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

## 7. Dependencies to Add

### orbit/cli Cargo.toml

```toml
[dependencies]
# Existing
tokio-postgres = "0.7"
rustyline = "14"
syntect = "5"

# New for protocol support
mysql_async = "0.34"           # MySQL protocol
redis = { version = "0.25", features = ["tokio-comp"] }  # Redis
reqwest = { version = "0.12", features = ["json"] }      # HTTP for OrbitQL/CQL
cdrs-tokio = "8"               # Optional: native CQL
```

---

## Version History

| Date | Changes |
|------|---------|
| 2025-12-07 | Initial specification |
