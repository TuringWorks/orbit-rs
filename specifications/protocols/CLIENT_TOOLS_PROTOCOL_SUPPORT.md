# Client Tools Protocol Support

**Last Updated**: 2025-12-07
**Purpose**: Document protocol support across OrbitRS client tools (CLI, Desktop, SDKs)
**Related**: See [PROTOCOL_STATUS.md](./PROTOCOL_STATUS.md) for server-side protocol implementation status

---

## Executive Summary

OrbitRS provides multiple client tools for interacting with the database server. **The CLI has achieved full parity with the Desktop app**, supporting all 7 protocols.

| Client Tool | Protocols Supported | Primary Use Case |
|-------------|---------------------|------------------|
| orbit/cli | 7 protocols (PostgreSQL, MySQL, Redis, OrbitQL, CQL, Cypher, AQL) | Terminal/scripting |
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
| Cypher | ✅ Fully implemented | Neo4j HTTP REST API | 7474 |
| AQL | ✅ Fully implemented | ArangoDB HTTP REST API | 8529 |

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

# Cypher (Neo4j) via REST
orbit --protocol cypher -H localhost -p 7474 -u neo4j -W password

# AQL (ArangoDB) via REST
orbit --protocol aql -H localhost -p 8529 -d _system -u root

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
- Basic authentication support for Cypher and AQL

### Protocol-Specific Behavior

| Protocol | Query Terminator | Multi-line | Special Handling |
|----------|------------------|------------|------------------|
| PostgreSQL | Semicolon (`;`) | Yes | Standard SQL |
| MySQL | Semicolon (`;`) | Yes | Standard SQL |
| OrbitQL | Semicolon (`;`) | Yes | REST API with JSON body |
| CQL | Semicolon (`;`) | Yes | REST API with protocol hint |
| Cypher | Semicolon (`;`) | Yes | Neo4j REST API with basic auth |
| AQL | Semicolon (`;`) | Yes | ArangoDB cursor API with basic auth |
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
    http_client: Option<reqwest::Client>,         // OrbitQL/CQL/Cypher/AQL
}
```

### Remaining Gaps

| Gap | Priority | Effort | Notes |
|-----|----------|--------|-------|
| Native CQL driver | Low | Medium | Replace REST with `cdrs-tokio` |
| Native Bolt driver | Low | High | Replace REST with native Bolt protocol |

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

### Complete Parser Inventory

OrbitRS implements **six independent parser/lexer systems**, one for each query language:

| Parser | Language | Location | Entry Point |
|--------|----------|----------|-------------|
| PostgreSQL SQL | ANSI SQL + extensions | `orbit/server/src/protocols/postgres_wire/sql/` | `SqlParser::parse()` |
| OrbitQL | Multi-model query language | `orbit/shared/src/orbitql/` | `Parser::parse()` |
| Cypher | Neo4j graph queries | `orbit/server/src/protocols/cypher/` | `CypherParser::parse()` |
| CQL | Cassandra Query Language | `orbit/server/src/protocols/cql/` | `CqlParser::parse()` |
| AQL | ArangoDB Query Language | `orbit/server/src/protocols/aql/` | `AqlParser::parse()` |
| PL/pgSQL | Stored procedures | `orbit/engine/src/procedures/` | CREATE FUNCTION |

---

### 3.1 PostgreSQL SQL Parser

**Location**: `orbit/server/src/protocols/postgres_wire/sql/`

```text
orbit/server/src/protocols/postgres_wire/sql/
├── lexer.rs              # SQL tokenizer (~1,200 lines)
├── parser/
│   ├── mod.rs            # Parser coordination (~16,000 lines)
│   ├── dml.rs            # SELECT, INSERT, UPDATE, DELETE (~71,000 lines)
│   ├── ddl.rs            # CREATE, ALTER, DROP (~81,000 lines)
│   ├── dcl.rs            # GRANT, REVOKE (~9,600 lines)
│   ├── tcl.rs            # BEGIN, COMMIT, ROLLBACK (~9,700 lines)
│   ├── expressions.rs    # Expression parsing (~59,000 lines)
│   ├── select.rs         # SELECT-specific (~25,000 lines)
│   └── utilities.rs      # Helper functions (~37,000 lines)
├── ast.rs                # AST definitions
├── executor.rs           # Query execution
└── expression_evaluator.rs
```

**Capabilities**:
- Full ANSI SQL compliance
- PostgreSQL extensions: JSONB, arrays, vectors (pgvector)
- DDL: CREATE/ALTER/DROP for tables, indexes, views, schemas, functions, triggers, sequences
- DML: SELECT with JOINs, subqueries, CTEs, window functions, MERGE, COPY
- DCL: GRANT/REVOKE permission management
- TCL: Transaction control with savepoints
- Vector operations: COSINE_DISTANCE, EUCLIDEAN_DISTANCE

---

### 3.2 OrbitQL Parser

**Location**: `orbit/shared/src/orbitql/`

```text
orbit/shared/src/orbitql/
├── lexer.rs           # OrbitQL tokenizer (~45,000 lines)
├── parser.rs          # Main parser (~2,000 lines)
├── ast.rs             # AST definitions (~27,000 lines)
├── executor.rs        # Query execution
├── planner.rs         # Query planning
├── optimizer.rs       # Query optimization
├── spatial.rs         # Spatial/geo queries
├── streaming.rs       # Real-time/live queries
└── distributed.rs     # Distributed execution
```

**Capabilities**:
- Multi-model operations (documents, graphs, time-series, key-value)
- Graph keywords: `NODE`, `EDGE`, `PATH`, `TRAVERSE`, `CONNECTED`, `MAX_DEPTH`
- Time-series: `METRICS`, `AGGREGATE`, `WINDOW`, `RANGE`, `NOW`
- Live queries: `LIVE`, `DIFF`, `FETCH`
- Cross-model JOINs
- Recursive CTEs
- GraphRAG operations

---

### 3.3 Cypher Parser (Neo4j)

**Location**: `orbit/server/src/protocols/cypher/`

```text
orbit/server/src/protocols/cypher/
├── cypher_parser.rs   # Parser + tokenizer (~3,900 lines)
├── types.rs           # Type definitions
├── graph_engine.rs    # Execution engine
└── bolt_protocol.rs   # Bolt wire protocol
```

**Capabilities**:
- Neo4j Cypher compatibility (based on ANTLR4 grammar)
- Pattern matching: `MATCH (n:Label)-[:REL]->(m)`
- Graph mutations: `CREATE`, `MERGE`, `SET`, `DELETE`, `DETACH DELETE`
- Aggregations: `COUNT`, `SUM`, `AVG`, `MIN`, `MAX`, `COLLECT`
- Path operations: Variable-length patterns `[*1..5]`
- Procedure calls: `CALL ... YIELD`
- CASE expressions and WHERE filtering
- `ORDER BY`, `LIMIT`, `SKIP`

---

### 3.4 CQL Parser (Cassandra)

**Location**: `orbit/server/src/protocols/cql/`

```text
orbit/server/src/protocols/cql/
├── parser.rs          # CQL parser (~3,500 lines)
├── types.rs           # CQL types and statements
├── adapter.rs         # Protocol adapter
└── mod.rs             # Module exports
```

**Capabilities**:
- Cassandra CQL3 compatibility
- Keyspace and table management
- SELECT with `ALLOW FILTERING`, `PER PARTITION LIMIT`
- INSERT with `IF NOT EXISTS`, TTL support
- UPDATE with counter operations, collection mutations
- Batch operations: `LOGGED`, `UNLOGGED`, `COUNTER`
- Vector search: ANN (Approximate Nearest Neighbor)
- User-defined types
- Permission management: `GRANT`, `REVOKE`

---

### 3.5 AQL Parser (ArangoDB)

**Location**: `orbit/server/src/protocols/aql/`

```text
orbit/server/src/protocols/aql/
├── aql_parser.rs      # Parser + tokenizer (~2,900 lines)
├── query_engine.rs    # Query execution
├── storage.rs         # Storage adapter
└── data_model.rs      # Data model definitions
```

**Capabilities**:
- ArangoDB AQL compatibility (based on ANTLR4 grammar)
- Iteration: `FOR doc IN collection`
- Filtering: `FILTER condition`
- Variable binding: `LET var = expression`
- Aggregation: `COLLECT`
- Projections: `RETURN`
- Data modification: `INSERT`, `UPDATE`, `REPLACE`, `REMOVE`, `UPSERT`
- Sorting and limiting: `SORT`, `LIMIT`
- Graph traversal patterns
- Debug mode for development

---

### 3.6 PL/pgSQL Parser (Stored Procedures)

**Location**: `orbit/engine/src/procedures/`

```text
orbit/engine/src/procedures/
├── ast.rs             # Procedure AST
├── executor.rs        # Procedure execution
└── mod.rs             # Module exports
```

**Capabilities**:
- Variable declarations: `DECLARE`
- Control flow: `IF-THEN-ELSE`, `LOOP`, `WHILE`
- SQL statement execution within procedures
- Exception handling: `RAISE NOTICE`, `RAISE EXCEPTION`
- Function definitions with parameters and return types
- Expression evaluation with operators

---

### Parser Selection by Protocol

| Protocol Port | Wire Protocol | Parser Used | Notes |
|---------------|---------------|-------------|-------|
| 5432 | PostgreSQL | PostgreSQL SQL | Full SQL with extensions |
| 3306 | MySQL | MySQL (adapter) | Translated to internal SQL |
| 6379 | Redis RESP | RESP Command | Direct command parsing |
| 9042 | CQL Native | CQL | Native Cassandra parsing |
| 7687 | Bolt | Cypher | Neo4j graph queries |
| 8529 | HTTP | AQL | ArangoDB queries |
| 8080 | HTTP REST | PostgreSQL SQL | `/api/v1/sql` endpoint |
| 8081 | HTTP REST | OrbitQL | Dedicated OrbitQL endpoint |

---

## 4. Protocol Exposure Summary

### Server-Side Protocol Ports

| Protocol | Port | Parser Used | CLI Support | Desktop Support |
|----------|------|-------------|-------------|-----------------|
| PostgreSQL Wire | 5432 | PostgreSQL | ✅ | ✅ |
| MySQL Wire | 3306 | MySQL | ✅ | ✅ |
| Redis RESP | 6379 | RESP | ✅ | ✅ |
| CQL | 9042 | CQL | ✅ (REST) | ✅ |
| Cypher/Bolt | 7474 | Cypher | ✅ (REST) | ✅ |
| AQL | 8529 | AQL | ✅ (REST) | ✅ |
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
| Graph queries (Cypher) | ✅ | ✅ | ❌ |
| AQL queries | ✅ | ✅ | ❌ |
| Syntax highlighting | ✅ | ✅ | N/A |
| Connection management | Basic | Full | Basic |
| Query history | ✅ | ✅ | ❌ |
| Multiple formats | ✅ | ✅ | JSON only |

---

## 5. Future Enhancements

### Phase 1: Native Protocol Drivers (Low Priority)

**Goal**: Replace REST fallbacks with native protocol drivers

#### Task 1.1: Native CQL Driver
```rust
// Replace HTTP REST with cdrs-tokio for native CQL support
// Benefits: Better performance, connection pooling, prepared statements
```

**Effort**: Medium (3-4 days)

#### Task 1.2: Native Bolt Driver
```rust
// Replace HTTP REST with native Bolt protocol for Cypher
// Benefits: Better performance, streaming results, transaction support
```

**Effort**: High (1-2 weeks)

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

    def execute_cypher(self, query):
        """Cypher via Neo4j REST API"""
        pass

    def execute_aql(self, query):
        """AQL via ArangoDB REST API"""
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

#[tokio::test]
async fn test_cli_cypher_via_rest() {
    let cli = Cli::parse_from(&["orbit", "--protocol", "cypher"]);
    let mut state = ReplState::new(&cli);
    assert!(state.connect_cypher().await.is_ok());
}

#[tokio::test]
async fn test_cli_aql_via_rest() {
    let cli = Cli::parse_from(&["orbit", "--protocol", "aql"]);
    let mut state = ReplState::new(&cli);
    assert!(state.connect_aql().await.is_ok());
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
reqwest = { version = "0.12", features = ["json"] }       # HTTP for OrbitQL/CQL/Cypher/AQL
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

---

## Version History

| Date | Changes |
|------|---------|
| 2025-12-07 | Expanded Parser/Lexer Architecture section to document all 6 parsers (PostgreSQL, OrbitQL, Cypher, CQL, AQL, PL/pgSQL) |
| 2025-12-07 | Added Cypher (Neo4j) and AQL (ArangoDB) support - CLI now has full protocol parity with Desktop |
| 2025-12-07 | Updated to reflect full CLI protocol implementation (PostgreSQL, MySQL, Redis, OrbitQL, CQL) |
| 2025-12-07 | Initial specification |
