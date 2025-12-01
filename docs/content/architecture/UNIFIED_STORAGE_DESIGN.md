# Unified Cross-Protocol Storage Architecture

## Overview

This document describes the architecture for implementing true cross-protocol data sharing in Orbit-RS, where data written via one protocol (Redis, PostgreSQL, MySQL, CQL, Cypher, AQL, REST, gRPC) is immediately accessible through all other protocols.

## Current State (Problem)

Currently, each protocol has **isolated storage**:

```
./data/
├── redis/          ← Only Redis can access
├── postgresql/     ← Only PostgreSQL can access
├── mysql/          ← Only MySQL can access
├── cql/            ← Only CQL can access
├── cypher/         ← Only Cypher can access
├── aql/            ← Only AQL can access
└── graphrag/       ← Only GraphRAG can access
```

**Code evidence** (`main.rs` lines 303-326):
```rust
// Create independent tiered storage for each protocol
let postgres_storage = Arc::new(TieredTableStorage::with_data_dir(postgres_data_dir, ...));
let redis_storage = Arc::new(TieredTableStorage::with_data_dir(redis_data_dir, ...));
// ... each protocol isolated
```

## Target State (Solution)

A **single unified storage layer** that all protocols share:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Protocol Layer                              │
├─────────┬─────────┬─────────┬─────────┬─────────┬──────────────┤
│  Redis  │PostgreSQL│  MySQL  │   CQL   │ Cypher  │  AQL/REST    │
│ Adapter │ Adapter  │ Adapter │ Adapter │ Adapter │  Adapter     │
├─────────┴─────────┴─────────┴─────────┴─────────┴──────────────┤
│                    Schema Registry                               │
├─────────────────────────────────────────────────────────────────┤
│                 Unified Query Engine                             │
├─────────────────────────────────────────────────────────────────┤
│                   Unified Storage                                │
│         (Single RocksDB + Tiered Storage Backend)                │
└─────────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. Universal Data Model

A canonical data representation that bridges all protocol paradigms:

```rust
// Location: orbit/engine/src/unified/types.rs

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Unique identifier for any record
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecordId {
    pub namespace: String,  // "users", "orders", "cache"
    pub key: String,        // Primary key/identifier
}

/// Universal value type that all protocols can map to/from
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UniversalValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<UniversalValue>),
    Map(BTreeMap<String, UniversalValue>),

    // Temporal types
    Timestamp(i64),          // Unix timestamp millis
    Date(i32),               // Days since epoch
    Time(i64),               // Nanoseconds since midnight
    Duration(i64),           // Nanoseconds

    // Graph-specific (for Cypher/AQL)
    Node {
        id: String,
        labels: Vec<String>,
        properties: BTreeMap<String, UniversalValue>,
    },
    Relationship {
        id: String,
        rel_type: String,
        start_node: String,
        end_node: String,
        properties: BTreeMap<String, UniversalValue>,
    },
    Path(Vec<UniversalValue>),  // Alternating nodes and relationships

    // Geo types
    Point { lat: f64, lon: f64 },
    Polygon(Vec<(f64, f64)>),

    // Vector embedding (for AI/ML)
    Vector(Vec<f32>),
}

/// A universal record that can be stored and retrieved
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalRecord {
    pub id: RecordId,
    pub value: UniversalValue,
    pub metadata: RecordMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordMetadata {
    pub created_at: i64,
    pub updated_at: i64,
    pub version: u64,
    pub ttl: Option<i64>,           // Expiration timestamp
    pub source_protocol: String,    // Which protocol created this
    pub schema_version: Option<u32>,
}
```

### 2. Universal Operations

```rust
// Location: orbit/engine/src/unified/operations.rs

/// Operations that any protocol can express
#[derive(Debug, Clone)]
pub enum UniversalOperation {
    // Basic CRUD
    Get {
        namespace: String,
        key: String,
    },
    Put {
        namespace: String,
        key: String,
        value: UniversalValue,
        ttl: Option<Duration>,
    },
    Delete {
        namespace: String,
        key: String,
    },

    // Batch operations
    MultiGet {
        namespace: String,
        keys: Vec<String>,
    },
    MultiPut {
        records: Vec<(RecordId, UniversalValue)>,
    },

    // Scan/Query
    Scan {
        namespace: String,
        filter: Option<FilterExpression>,
        limit: Option<usize>,
        offset: Option<usize>,
        order_by: Option<Vec<(String, SortOrder)>>,
    },

    // Aggregations
    Aggregate {
        namespace: String,
        filter: Option<FilterExpression>,
        group_by: Vec<String>,
        aggregations: Vec<AggregateOp>,
    },

    // Graph-specific
    TraverseGraph {
        start_nodes: Vec<String>,
        pattern: GraphPattern,
        max_depth: Option<usize>,
    },

    // Transactions
    BeginTransaction,
    Commit { tx_id: String },
    Rollback { tx_id: String },
}

#[derive(Debug, Clone)]
pub enum FilterExpression {
    Eq(String, UniversalValue),
    Ne(String, UniversalValue),
    Gt(String, UniversalValue),
    Gte(String, UniversalValue),
    Lt(String, UniversalValue),
    Lte(String, UniversalValue),
    In(String, Vec<UniversalValue>),
    Like(String, String),           // SQL LIKE pattern
    Contains(String, UniversalValue),
    And(Box<FilterExpression>, Box<FilterExpression>),
    Or(Box<FilterExpression>, Box<FilterExpression>),
    Not(Box<FilterExpression>),
}

#[derive(Debug, Clone)]
pub enum AggregateOp {
    Count,
    Sum(String),
    Avg(String),
    Min(String),
    Max(String),
    Collect(String),  // Collect into list
}
```

### 3. Schema Registry

```rust
// Location: orbit/engine/src/unified/schema.rs

use std::collections::HashMap;
use std::sync::RwLock;

/// Central registry for namespace schemas
pub struct SchemaRegistry {
    schemas: RwLock<HashMap<String, NamespaceSchema>>,
}

#[derive(Debug, Clone)]
pub struct NamespaceSchema {
    pub namespace: String,
    pub fields: Vec<FieldDef>,
    pub indexes: Vec<IndexDef>,
    pub constraints: Vec<Constraint>,

    // Protocol-specific projections
    pub projections: ProtocolProjections,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub nullable: bool,
    pub default: Option<UniversalValue>,
    pub primary_key: bool,
}

#[derive(Debug, Clone)]
pub enum FieldType {
    Bool,
    Int,
    Float,
    String,
    Bytes,
    Timestamp,
    Date,
    Time,
    Json,           // Nested structure
    List(Box<FieldType>),
    Map(Box<FieldType>, Box<FieldType>),
    Vector(usize),  // Fixed-dimension vector
    Ref(String),    // Reference to another namespace
}

#[derive(Debug, Clone)]
pub struct ProtocolProjections {
    pub sql: SqlProjection,
    pub redis: RedisProjection,
    pub cql: CqlProjection,
    pub cypher: CypherProjection,
    pub aql: AqlProjection,
    pub rest: RestProjection,
}

#[derive(Debug, Clone)]
pub struct SqlProjection {
    pub table_name: String,
    pub column_mappings: HashMap<String, String>, // field -> column
}

#[derive(Debug, Clone)]
pub struct RedisProjection {
    pub key_pattern: String,      // e.g., "user:{id}"
    pub data_type: RedisDataType, // STRING, HASH, LIST, SET, ZSET
    pub field_mappings: Option<HashMap<String, String>>, // For HASH
}

#[derive(Debug, Clone)]
pub enum RedisDataType {
    String,
    Hash,
    List,
    Set,
    SortedSet { score_field: String },
    Stream,
}

#[derive(Debug, Clone)]
pub struct CqlProjection {
    pub keyspace: String,
    pub table: String,
    pub partition_key: Vec<String>,
    pub clustering_key: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CypherProjection {
    pub node_label: String,
    pub relationship_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AqlProjection {
    pub collection: String,
    pub collection_type: AqlCollectionType,
}

#[derive(Debug, Clone)]
pub enum AqlCollectionType {
    Document,
    Edge,
}

#[derive(Debug, Clone)]
pub struct RestProjection {
    pub resource_path: String,     // "/api/users"
    pub item_path: String,         // "/api/users/{id}"
    pub allowed_methods: Vec<HttpMethod>,
}
```

### 4. Protocol Adapters

Each protocol implements the `ProtocolAdapter` trait:

```rust
// Location: orbit/engine/src/unified/adapter.rs

use async_trait::async_trait;

#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    /// Protocol identifier (e.g., "redis", "postgresql")
    fn protocol_name(&self) -> &'static str;

    /// Convert protocol-specific command to universal operation
    fn to_universal(&self, command: &[u8]) -> Result<UniversalOperation, AdapterError>;

    /// Convert universal result to protocol-specific response
    fn from_universal(&self, result: UniversalResult) -> Result<Vec<u8>, AdapterError>;

    /// Get schema projections for this protocol
    fn get_projection(&self, schema: &NamespaceSchema) -> Box<dyn std::any::Any>;
}
```

#### Redis Adapter Mapping

| Redis Command | Universal Operation | Notes |
|---------------|---------------------|-------|
| `SET key value` | `Put { namespace: infer(key), key, value: String }` | Key pattern inference |
| `GET key` | `Get { namespace: infer(key), key }` | |
| `HSET key field value` | `Put { namespace: infer(key), key, value: Map }` | Merge into map |
| `HGET key field` | `Get + field extraction` | |
| `LPUSH key value` | `Put { ..., value: List }` | Prepend to list |
| `SADD key member` | `Put { ..., value: Set }` | Add to set |
| `ZADD key score member` | `Put { ..., value: SortedSet }` | |
| `SCAN cursor` | `Scan { namespace, ... }` | |
| `KEYS pattern` | `Scan { namespace: from_pattern }` | |

#### PostgreSQL Adapter Mapping

| SQL Statement | Universal Operation | Notes |
|---------------|---------------------|-------|
| `SELECT * FROM t WHERE ...` | `Scan { namespace: t, filter }` | |
| `SELECT * FROM t WHERE id = ?` | `Get { namespace: t, key: id }` | Optimized path |
| `INSERT INTO t (...) VALUES (...)` | `Put { namespace: t, key, value: Map }` | |
| `UPDATE t SET ... WHERE id = ?` | `Get + Merge + Put` | |
| `DELETE FROM t WHERE id = ?` | `Delete { namespace: t, key: id }` | |
| `CREATE TABLE t (...)` | Schema registration | |
| `JOIN` | Multiple Scans + merge | Complex |

#### Cypher Adapter Mapping

| Cypher Statement | Universal Operation | Notes |
|------------------|---------------------|-------|
| `CREATE (n:Label {props})` | `Put { value: Node { ... } }` | |
| `MATCH (n:Label) RETURN n` | `Scan { filter: label }` | |
| `MATCH (a)-[r:REL]->(b)` | `TraverseGraph { pattern }` | |
| `MERGE (n:Label {id: x})` | `Get + conditional Put` | |

### 5. Unified Storage Implementation

```rust
// Location: orbit/engine/src/unified/storage.rs

use std::sync::Arc;
use tokio::sync::RwLock;

pub struct UnifiedStorage {
    /// Single backend for all data
    backend: Arc<RocksDBBackend>,

    /// Schema registry
    schema_registry: Arc<SchemaRegistry>,

    /// Transaction manager
    tx_manager: Arc<TransactionManager>,

    /// Index manager
    index_manager: Arc<IndexManager>,

    /// Cache layer
    cache: Arc<RwLock<LruCache<RecordId, UniversalRecord>>>,
}

impl UnifiedStorage {
    pub async fn new(data_dir: PathBuf) -> Result<Self, StorageError> {
        let backend = Arc::new(RocksDBBackend::open(data_dir.join("unified"))?);

        Ok(Self {
            backend,
            schema_registry: Arc::new(SchemaRegistry::new()),
            tx_manager: Arc::new(TransactionManager::new()),
            index_manager: Arc::new(IndexManager::new()),
            cache: Arc::new(RwLock::new(LruCache::new(10_000))),
        })
    }

    /// Execute a universal operation
    pub async fn execute(&self, op: UniversalOperation) -> Result<UniversalResult, StorageError> {
        match op {
            UniversalOperation::Get { namespace, key } => {
                self.get(&namespace, &key).await
            }
            UniversalOperation::Put { namespace, key, value, ttl } => {
                self.put(&namespace, &key, value, ttl).await
            }
            UniversalOperation::Delete { namespace, key } => {
                self.delete(&namespace, &key).await
            }
            UniversalOperation::Scan { namespace, filter, limit, offset, order_by } => {
                self.scan(&namespace, filter, limit, offset, order_by).await
            }
            UniversalOperation::Aggregate { namespace, filter, group_by, aggregations } => {
                self.aggregate(&namespace, filter, group_by, aggregations).await
            }
            UniversalOperation::TraverseGraph { start_nodes, pattern, max_depth } => {
                self.traverse_graph(start_nodes, pattern, max_depth).await
            }
            // ... other operations
        }
    }

    async fn get(&self, namespace: &str, key: &str) -> Result<UniversalResult, StorageError> {
        // Check cache first
        let cache_key = RecordId {
            namespace: namespace.to_string(),
            key: key.to_string()
        };

        if let Some(record) = self.cache.read().await.get(&cache_key) {
            return Ok(UniversalResult::Record(record.clone()));
        }

        // Read from backend
        let storage_key = format!("{}:{}", namespace, key);
        match self.backend.get(&storage_key).await? {
            Some(bytes) => {
                let record: UniversalRecord = bincode::deserialize(&bytes)?;

                // Update cache
                self.cache.write().await.put(cache_key, record.clone());

                Ok(UniversalResult::Record(record))
            }
            None => Ok(UniversalResult::Empty),
        }
    }

    async fn put(
        &self,
        namespace: &str,
        key: &str,
        value: UniversalValue,
        ttl: Option<Duration>,
    ) -> Result<UniversalResult, StorageError> {
        let now = chrono::Utc::now().timestamp_millis();

        let record = UniversalRecord {
            id: RecordId {
                namespace: namespace.to_string(),
                key: key.to_string(),
            },
            value,
            metadata: RecordMetadata {
                created_at: now,
                updated_at: now,
                version: 1,
                ttl: ttl.map(|d| now + d.as_millis() as i64),
                source_protocol: "unknown".to_string(),
                schema_version: None,
            },
        };

        let storage_key = format!("{}:{}", namespace, key);
        let bytes = bincode::serialize(&record)?;

        self.backend.put(&storage_key, &bytes).await?;

        // Update indexes
        self.index_manager.index_record(namespace, &record).await?;

        // Update cache
        self.cache.write().await.put(record.id.clone(), record);

        Ok(UniversalResult::Ok)
    }
}
```

### 6. RocksDB Backend with Column Families

```rust
// Location: orbit/engine/src/unified/backend.rs

use rocksdb::{DB, Options, ColumnFamilyDescriptor};

pub struct RocksDBBackend {
    db: DB,
}

impl RocksDBBackend {
    pub fn open(path: PathBuf) -> Result<Self, StorageError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Column families for different data types
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new("data", Options::default()),      // Main data
            ColumnFamilyDescriptor::new("indexes", Options::default()),   // Secondary indexes
            ColumnFamilyDescriptor::new("schema", Options::default()),    // Schema definitions
            ColumnFamilyDescriptor::new("graph_adj", Options::default()), // Graph adjacency lists
            ColumnFamilyDescriptor::new("metadata", Options::default()),  // Record metadata
        ];

        let db = DB::open_cf_descriptors(&opts, path, cf_descriptors)?;

        Ok(Self { db })
    }
}
```

---

## Protocol-Specific Mapping Details

### Redis Key Pattern Inference

Redis keys often follow patterns like `user:123` or `cache:session:abc`. The adapter infers namespace:

```rust
fn infer_namespace(key: &str) -> (String, String) {
    // Pattern: "namespace:key" or "namespace:subtype:key"
    let parts: Vec<&str> = key.splitn(2, ':').collect();

    match parts.as_slice() {
        [namespace, rest] => (namespace.to_string(), rest.to_string()),
        [key] => ("default".to_string(), key.to_string()),
        _ => unreachable!(),
    }
}

// Examples:
// "user:123"        -> namespace="user", key="123"
// "cache:auth:abc"  -> namespace="cache", key="auth:abc"
// "simple_key"      -> namespace="default", key="simple_key"
```

### SQL Table to Namespace Mapping

```rust
impl SqlToUniversal {
    fn table_to_namespace(&self, table: &str) -> String {
        // Direct mapping: table name = namespace
        table.to_lowercase()
    }

    fn row_to_value(&self, columns: &[String], values: &[SqlValue]) -> UniversalValue {
        let mut map = BTreeMap::new();
        for (col, val) in columns.iter().zip(values.iter()) {
            map.insert(col.clone(), self.sql_value_to_universal(val));
        }
        UniversalValue::Map(map)
    }
}
```

### CQL Keyspace.Table to Namespace

```rust
fn cql_to_namespace(keyspace: &str, table: &str) -> String {
    format!("{}.{}", keyspace, table)
}
// "mykeyspace.users" -> namespace="mykeyspace.users"
```

### Graph Data (Cypher/AQL)

Graph data uses special namespaces:

```rust
// Nodes stored in: "_nodes:{label}"
// Relationships stored in: "_rels:{type}"
// Adjacency lists in: "_adj:{node_id}"

fn store_node(node: &GraphNode) -> Vec<(String, UniversalValue)> {
    vec![
        // Primary node storage
        (
            format!("_nodes:{}:{}", node.labels.join(":"), node.id),
            UniversalValue::Node { ... }
        ),
        // Adjacency list update
        (
            format!("_adj:{}", node.id),
            UniversalValue::Map { /* outgoing edges */ }
        ),
    ]
}
```

---

## Cross-Protocol Query Examples

### Example 1: Write via Redis, Read via SQL

```
# Redis: Create user
redis-cli> HSET user:alice name "Alice" email "alice@orbit.com" role "admin"

# What happens internally:
1. RedisAdapter.to_universal() ->
   UniversalOperation::Put {
       namespace: "user",
       key: "alice",
       value: Map { name: "Alice", email: "alice@...", role: "admin" }
   }
2. UnifiedStorage.put("user", "alice", Map{...})
3. IndexManager indexes fields

# PostgreSQL: Query same data
psql> SELECT * FROM user WHERE name = 'Alice';

# What happens internally:
1. PostgresAdapter.to_universal() ->
   UniversalOperation::Scan {
       namespace: "user",
       filter: Eq("name", "Alice")
   }
2. UnifiedStorage.scan("user", filter)
3. Returns same record, formatted as SQL ResultSet
```

### Example 2: Write via SQL, Read via Redis

```sql
-- PostgreSQL: Insert user
INSERT INTO users (id, name, email) VALUES ('bob', 'Bob', 'bob@orbit.com');

-- What happens internally:
1. SqlAdapter.to_universal() ->
   UniversalOperation::Put {
       namespace: "users",
       key: "bob",
       value: Map { id: "bob", name: "Bob", email: "bob@..." }
   }
2. UnifiedStorage.put("users", "bob", Map{...})
```

```
# Redis: Read same data
redis-cli> HGETALL users:bob

# What happens internally:
1. RedisAdapter.to_universal() ->
   UniversalOperation::Get { namespace: "users", key: "bob" }
2. UnifiedStorage.get("users", "bob")
3. RedisAdapter.from_universal() -> RESP array response
```

### Example 3: Graph + Relational

```cypher
// Cypher: Create relationship
CREATE (a:Person {id: 'alice'})-[:KNOWS {since: 2020}]->(b:Person {id: 'bob'})

// Stored as:
// _nodes:Person:alice -> Node{...}
// _nodes:Person:bob -> Node{...}
// _rels:KNOWS:rel_123 -> Relationship{start: alice, end: bob, ...}
// _adj:alice -> {KNOWS: [bob]}
```

```sql
-- SQL: Query relationship data
SELECT * FROM _rels WHERE rel_type = 'KNOWS' AND start_node = 'alice';
```

---

## Transaction Handling

Cross-protocol transactions use a central transaction manager:

```rust
pub struct TransactionManager {
    active_transactions: RwLock<HashMap<String, Transaction>>,
}

pub struct Transaction {
    pub id: String,
    pub started_at: i64,
    pub operations: Vec<(UniversalOperation, UniversalResult)>,
    pub isolation_level: IsolationLevel,
}

impl TransactionManager {
    pub async fn begin(&self, isolation: IsolationLevel) -> String {
        let tx_id = uuid::Uuid::new_v4().to_string();
        let tx = Transaction {
            id: tx_id.clone(),
            started_at: chrono::Utc::now().timestamp_millis(),
            operations: Vec::new(),
            isolation_level: isolation,
        };
        self.active_transactions.write().await.insert(tx_id.clone(), tx);
        tx_id
    }

    pub async fn commit(&self, tx_id: &str) -> Result<(), TransactionError> {
        let tx = self.active_transactions.write().await.remove(tx_id)
            .ok_or(TransactionError::NotFound)?;

        // Apply all operations atomically
        // ... RocksDB WriteBatch

        Ok(())
    }
}
```

---

## Implementation Plan

### Phase 1: Core Types and Storage (Week 1-2)
- [ ] Define `UniversalValue`, `UniversalRecord`, `UniversalOperation`
- [ ] Implement `UnifiedStorage` with RocksDB backend
- [ ] Basic `SchemaRegistry`
- [ ] Unit tests for core types

### Phase 2: Redis Adapter (Week 3)
- [ ] Implement `RedisAdapter` with key pattern inference
- [ ] Map STRING, HASH, LIST, SET, ZSET commands
- [ ] Integration tests: Redis write -> Redis read

### Phase 3: PostgreSQL Adapter (Week 4)
- [ ] Implement `SqlAdapter` for PostgreSQL wire protocol
- [ ] Parse SELECT, INSERT, UPDATE, DELETE
- [ ] Integration tests: SQL write -> SQL read

### Phase 4: Cross-Protocol Tests (Week 5)
- [ ] Redis write -> SQL read
- [ ] SQL write -> Redis read
- [ ] Add MySQL adapter (similar to PostgreSQL)

### Phase 5: CQL and Graph Adapters (Week 6-7)
- [ ] CQL adapter for Cassandra protocol
- [ ] Cypher adapter for Neo4j protocol
- [ ] AQL adapter for ArangoDB protocol
- [ ] Graph storage with adjacency lists

### Phase 6: REST/gRPC Integration (Week 8)
- [ ] REST adapter with auto-generated endpoints
- [ ] gRPC adapter
- [ ] Update main.rs to use unified storage

### Phase 7: Performance & Polish (Week 9-10)
- [ ] Caching layer
- [ ] Index optimization
- [ ] Transaction handling
- [ ] Documentation and examples

---

## File Structure

```
orbit/engine/src/unified/
├── mod.rs                 # Module exports
├── types.rs               # UniversalValue, UniversalRecord, RecordId
├── operations.rs          # UniversalOperation, FilterExpression
├── storage.rs             # UnifiedStorage implementation
├── backend.rs             # RocksDB backend
├── schema.rs              # SchemaRegistry, NamespaceSchema
├── index.rs               # IndexManager
├── transaction.rs         # TransactionManager
├── cache.rs               # Caching layer
└── adapters/
    ├── mod.rs             # ProtocolAdapter trait
    ├── redis.rs           # Redis adapter
    ├── postgresql.rs      # PostgreSQL adapter
    ├── mysql.rs           # MySQL adapter
    ├── cql.rs             # CQL adapter
    ├── cypher.rs          # Cypher adapter
    ├── aql.rs             # AQL adapter
    └── rest.rs            # REST adapter
```

---

## Success Criteria

1. **Functional**: Data written via any protocol is readable via all others
2. **Consistent**: Same data, same values, regardless of access protocol
3. **Performant**: < 10% overhead vs. isolated storage
4. **Tested**: Cross-protocol integration tests for all combinations
5. **Documented**: Clear API docs and usage examples

---

## Open Questions

1. **Schema Evolution**: How do we handle schema changes (e.g., SQL `ALTER TABLE`)?
2. **Conflict Resolution**: What if Redis and SQL have different field names?
3. **Type Coercion**: How strict should type conversion be?
4. **Graph in SQL**: How do we expose graph data to SQL queries?
5. **Performance**: Can we avoid serialization overhead for same-protocol access?

---

## References

- [Current Storage Implementation](../../../orbit/server/src/main.rs) - Lines 303-326
- [TieredTableStorage](../../../orbit/server/src/protocols/common/storage/tiered.rs)
- [Protocol Adapters](../../../orbit/engine/src/adapters/)
