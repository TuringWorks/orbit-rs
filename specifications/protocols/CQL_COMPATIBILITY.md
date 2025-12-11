# CQL (Cassandra Query Language) Compatibility Specification

**Target**: Apache Cassandra 4.x/5.x CQL Protocol
**Reference**: https://cassandra.apache.org/doc/latest/cassandra/cql/
**Last Updated**: 2025-12-11
**Current Estimated Coverage**: ~72%

---

## Overview

This document specifies the Cassandra Query Language (CQL) feature set and tracks OrbitRS implementation status. The goal is to provide CQL wire-protocol compatibility for Cassandra clients.

## Table of Contents

1. [CQL Commands](#cql-commands)
2. [Data Types](#data-types)
3. [Functions](#functions)
4. [Wire Protocol](#wire-protocol)
5. [Implementation Status](#implementation-status)

---

## CQL Commands

### Legend
- ✅ **Implemented** - Fully functional
- 🔶 **Partial** - Basic support, missing features
- ❌ **Not Implemented** - Not yet available

### Schema Definition

| Command | Status | Notes |
|---------|--------|-------|
| CREATE KEYSPACE | ✅ | Full support with IF NOT EXISTS, replication options |
| CREATE TABLE | ✅ | Full support with partition/clustering keys |
| CREATE INDEX | ✅ | Secondary indexes (incl. SASI/SAI) |
| CREATE MATERIALIZED VIEW | 🔶 | Parsed, returns schema change response |
| CREATE TYPE | 🔶 | User-defined types (parsed, schema response) |
| CREATE FUNCTION | 🔶 | User-defined functions (parsed, supports Java/JavaScript) |
| CREATE AGGREGATE | 🔶 | User-defined aggregates (parsed, schema response) |
| CREATE TRIGGER | ✅ | Triggers with JavaScript execution (js-quickjs feature) |
| CREATE ROLE | 🔶 | RBAC roles (parsed, framework exists) |
| CREATE USER | 🔶 | Legacy auth (parsed, schema response) |
| ALTER KEYSPACE | 🔶 | Replication updates (parsed) |
| ALTER TABLE | ✅ | ADD/DROP/RENAME columns, set options |
| ALTER TYPE | 🔶 | ADD/RENAME fields (parsed) |
| ALTER ROLE | 🔶 | Role modifications (parsed) |
| ALTER USER | 🔶 | User modifications (parsed) |
| DROP KEYSPACE | ✅ | With IF EXISTS |
| DROP TABLE | ✅ | With IF EXISTS |
| DROP INDEX | 🔶 | With IF EXISTS (parsed) |
| DROP MATERIALIZED VIEW | 🔶 | With IF EXISTS (parsed) |
| DROP TYPE | 🔶 | With IF EXISTS (parsed) |
| DROP FUNCTION | 🔶 | With IF EXISTS (parsed) |
| DROP AGGREGATE | 🔶 | With IF EXISTS (parsed) |
| DROP TRIGGER | ✅ | With IF EXISTS, removes from registry |
| DROP ROLE | 🔶 | With IF EXISTS (parsed) |
| DROP USER | 🔶 | With IF EXISTS (parsed) |
| TRUNCATE | ✅ | Full support |
| GRANT | 🔶 | Permission grants (parsed) |
| REVOKE | 🔶 | Permission revocations (parsed) |
| LIST ROLES | ✅ | Returns result set |
| LIST PERMISSIONS | ✅ | Returns result set |
| LIST USERS | ✅ | Returns result set |

### Data Manipulation

| Command | Status | Notes |
|---------|--------|-------|
| SELECT | ✅ | Full support with WHERE, LIMIT |
| INSERT | ✅ | VALUES, TTL, TIMESTAMP |
| UPDATE | ✅ | SET, WHERE, TTL |
| DELETE | ✅ | WHERE, IF EXISTS |
| BATCH | 🔶 | Basic support |
| USE | ✅ | Keyspace selection |

### Query Features

| Feature | Status | Notes |
|---------|--------|-------|
| WHERE clause | ✅ | Partition key, clustering key, filtering |
| ALLOW FILTERING | ✅ | Full support |
| ORDER BY | ✅ | Clustering key ordering (ASC/DESC) |
| LIMIT | ✅ | Row limit |
| PER PARTITION LIMIT | ✅ | Per-partition row limit |
| GROUP BY | ✅ | Full support |
| DISTINCT | ✅ | Distinct query results |
| Token function | ✅ | Partition key hashing |
| TTL | ✅ | Time-to-live |
| TIMESTAMP | ✅ | Write timestamp |
| IF EXISTS/IF NOT EXISTS | ✅ | Conditional operations |
| Lightweight Transactions (LWT) | ✅ | Full support (IF conditions, serial consistency) |
| SASI Index | ✅ | Full-text search (CONTAINS, LIKE) |
| SAI Index | ✅ | Storage-attached indexing |
| SELECT JSON | ✅ | JSON output format |
| ANN (Vector) Search | ✅ | Approximate nearest neighbor search |

---

## Data Types

### Basic Types

| Type | Status | Notes |
|------|--------|-------|
| ascii | ✅ | ASCII string |
| bigint | ✅ | 64-bit integer |
| blob | ✅ | Binary data |
| boolean | ✅ | True/false |
| counter | 🔶 | Basic support |
| date | ✅ | Date value |
| decimal | ✅ | Variable precision |
| double | ✅ | 64-bit float |
| duration | ✅ | Encoded duration value |
| float | ✅ | 32-bit float |
| inet | ✅ | IP address |
| int | ✅ | 32-bit integer |
| smallint | ✅ | 16-bit integer |
| text | ✅ | UTF-8 string |
| time | ✅ | Time value |
| timestamp | ✅ | Date and time |
| timeuuid | ✅ | Time-based UUID |
| tinyint | ✅ | 8-bit integer |
| uuid | ✅ | UUID |
| varchar | ✅ | UTF-8 string |
| varint | ✅ | Arbitrary precision |

### Collection Types

| Type | Status | Notes |
|------|--------|-------|
| list | ✅ | Ordered collection (append, prepend) |
| set | ✅ | Unordered unique collection (add, remove) |
| map | ✅ | Key-value pairs (put, remove) |
| tuple | ✅ | Fixed-size heterogeneous tuple |
| frozen | ✅ | Frozen collections |
| vector | ✅ | Vector embeddings for similarity search |

### User-Defined Types

| Feature | Status | Notes |
|---------|--------|-------|
| CREATE TYPE | 🔶 | Parsed, schema response |
| User-defined types | 🔶 | Parsed, type system supports UDTs |
| Tuple types | ✅ | Full support |
| Custom types | 🔶 | Custom Java class types (parsed) |

---

## Functions

### Aggregate Functions

| Function | Status | Notes |
|----------|--------|-------|
| COUNT | ✅ | Count rows |
| SUM | ✅ | Sum values |
| AVG | ✅ | Average |
| MIN | ✅ | Minimum |
| MAX | ✅ | Maximum |

### Scalar Functions

| Function | Status | Notes |
|----------|--------|-------|
| now() | ✅ | Current timestamp |
| currentTimestamp() | ✅ | Alias for now() |
| uuid() | ✅ | Random UUID (v4) |
| timeuuid() | ✅ | Time-based UUID (v7) |
| currentTimeuuid() | ✅ | Alias for timeuuid() |
| toDate() | ✅ | Convert to date |
| toTimestamp() | ✅ | Convert to timestamp |
| toUnixTimestamp() | ✅ | Convert to Unix timestamp |
| dateOf() | ✅ | Extract date from timeuuid |
| unixTimestampOf() | ✅ | Extract timestamp from timeuuid |
| minTimeuuid() | ✅ | Minimum timeuuid for timestamp |
| maxTimeuuid() | ✅ | Maximum timeuuid for timestamp |
| token() | ✅ | Partition key hash |
| ttl() | ✅ | Get TTL |
| writetime() | ✅ | Get write timestamp |

### Vector/Similarity Functions

| Function | Status | Notes |
|----------|--------|-------|
| cosine() | ✅ | Cosine similarity |
| euclidean() | ✅ | Euclidean distance (L2) |
| dot_product() | ✅ | Dot product similarity |

### Geospatial Functions

| Function | Status | Notes |
|----------|--------|-------|
| geo_distance() | ✅ | Cartesian distance |
| geo_distance_sphere() | ✅ | Great-circle (Haversine) distance |
| geo_within() | ✅ | Point-in-polygon test |
| geo_contains() | ✅ | Polygon contains point |
| geo_near() | ✅ | Point within radius |
| geo_bbox() | ✅ | Bounding box test |
| geo_intersects() | ✅ | Geometry intersection test |

### Type Conversion

| Function | Status | Notes |
|----------|--------|-------|
| CAST | ✅ | Type casting |
| toJson() | ✅ | Convert value to JSON string |
| fromJson() | ✅ | Parse JSON to value |

---

## Wire Protocol

### Protocol Features

| Feature | Status | Notes |
|---------|--------|-------|
| Native Protocol v4 | ✅ | Full support (default) |
| Native Protocol v5 | 🔶 | Advertised, partial support |
| Startup Message | ✅ | Connection handshake |
| Authentication | ✅ | SASL/SCRAM authentication |
| Query Message | ✅ | Simple queries with parameters |
| Prepare Message | ✅ | Prepared statement framework |
| Execute Message | ✅ | Execute prepared statements |
| Batch Message | ✅ | LOGGED, UNLOGGED, COUNTER batches |
| Options Message | ✅ | Protocol options |
| Register Message | 🔶 | Event registration (parsed) |
| Compression | ✅ | Snappy and LZ4 compression fully implemented |
| SSL/TLS | ✅ | TLS support via OrbitTlsAcceptor |

### Result Types

| Result Type | Status | Notes |
|-------------|--------|-------|
| Void | ✅ | No result |
| Rows | ✅ | Result set with metadata |
| Set Keyspace | ✅ | Keyspace changed |
| Prepared | ✅ | Prepared statement ID |
| Schema Change | ✅ | DDL result (CREATED, UPDATED, DROPPED) |

### System Tables

| System Table | Status | Notes |
|--------------|--------|-------|
| system.local | ✅ | Cluster metadata |
| system.peers | ✅ | Node peer information (v1) |
| system.peers_v2 | ✅ | Node peer information (v2) |
| system_schema.keyspaces | ✅ | Keyspace catalog |
| system_schema.tables | ✅ | Table definitions |
| system_schema.columns | ✅ | Column definitions |
| system_schema.types | ✅ | User-defined types |
| system_schema.functions | ✅ | User functions |
| system_schema.aggregates | ✅ | Aggregate functions |
| system_schema.views | ✅ | Materialized views |
| system_schema.indexes | ✅ | Index definitions |
| system_schema.triggers | ✅ | Trigger definitions |
| system_virtual_schema.* | 🔶 | Virtual table schema (stub) |

### Error Handling

| Error Type | Status | Notes |
|------------|--------|-------|
| Server Error | ✅ | Internal errors |
| Protocol Error | ✅ | Protocol violations |
| Authentication Error | ✅ | Auth failures |
| Unavailable | ✅ | Consistency errors |
| Overloaded | ✅ | Server overload |
| Is Bootstrapping | ✅ | Node bootstrapping |
| Truncate Error | ✅ | Truncate failures |
| Write Timeout | ✅ | Write timeouts |
| Read Timeout | ✅ | Read timeouts |
| Read Failure | ✅ | Read failures |
| Function Failure | ❌ | UDF failures |
| Write Failure | ✅ | Write failures |
| Syntax Error | ✅ | CQL syntax errors |
| Unauthorized | ✅ | Permission errors |
| Invalid | ✅ | Invalid requests |
| Config Error | ✅ | Configuration errors |
| Already Exists | ✅ | Object exists |
| Unprepared | ✅ | Statement not prepared |

---

## Consistency Levels

| Level | Status | Notes |
|-------|--------|-------|
| ANY | ✅ | Hinted handoff |
| ONE | ✅ | One replica |
| TWO | ✅ | Two replicas |
| THREE | ✅ | Three replicas |
| QUORUM | ✅ | Majority |
| ALL | ✅ | All replicas |
| LOCAL_QUORUM | ✅ | Local DC quorum |
| EACH_QUORUM | ✅ | Each DC quorum |
| LOCAL_ONE | ✅ | One local replica |
| SERIAL | 🔶 | Lightweight transactions |
| LOCAL_SERIAL | 🔶 | Local LWT |

---

## Implementation Status

### Overall Coverage

| Category | Coverage | Notes |
|----------|----------|-------|
| Schema Commands | ~85% | All DDL parsed, most executable |
| DML Commands | ~95% | Full CRUD + LWT support |
| Query Features | ~90% | All major features implemented |
| Data Types | ~95% | All types supported including vector |
| Functions | ~90% | All standard functions + geospatial |
| Wire Protocol | ~85% | Protocol v4 complete, system tables |
| Consistency | ~90% | All levels parsed and supported |
| Advanced Features | ~75% | LWT, batching, FTS, vector search |

### Priority Roadmap

**Completed (High Priority)**:
1. ✅ Basic SELECT/INSERT/UPDATE/DELETE
2. ✅ CQL protocol v4
3. ✅ Core data types (all 20+ types)
4. ✅ Prepared statements (framework complete)
5. ✅ Lightweight transactions (LWT)
6. ✅ Batch operations (LOGGED, UNLOGGED, COUNTER)
7. ✅ All scalar functions
8. ✅ Vector/ANN search
9. ✅ Geospatial functions
10. ✅ Full-text search (SASI/SAI)

**Partial (Medium Priority)**:
1. 🔶 Materialized views (parsed, not executed)
2. 🔶 User-defined types (parsed, schema support)
3. 🔶 User-defined functions (parsed, not executed)
4. 🔶 User-defined aggregates (parsed, not executed)
5. 🔶 Protocol v5 features (advertised, partial)
6. 🔶 RBAC (parsed, not enforced)

**Recently Completed (Low Priority)**:
1. ✅ Active compression (snappy, lz4) - Full Snappy + LZ4 implementation
2. ✅ Trigger execution - Full JavaScript execution with QuickJS runtime
3. ✅ JavaScript runtime integration - Shared js-quickjs framework

**Still Low Priority**:
1. ❌ Change Data Capture (CDC)
2. ❌ Virtual table system (full implementation)
3. ❌ UDF/UDA runtime execution (Aggregate functions, Java support)

---

## Known Limitations

1. **Materialized Views**: Parsed but not executed (returns schema change response)
2. **User-Defined Functions**: Parsed but JavaScript/Java runtime not integrated
3. **User-Defined Aggregates**: Parsed but SFUNC/FINALFUNC not executed
4. **RBAC Enforcement**: Permissions parsed but not enforced
5. **Event Streaming**: EVENT opcode exists but event streaming not fully implemented
6. **Virtual Table System**: Stub responses only
7. **Change Data Capture**: Not implemented
8. **Full UDT Execution**: Type system supports UDTs but full execution incomplete
9. **Java Trigger Support**: Triggers support JavaScript execution; Java class loading not yet supported

---

## Advanced Features

### Graph Algorithms (OrbitRS Extensions)

OrbitRS includes advanced graph query capabilities as CQL extensions:

| Query Type | Status | Notes |
|------------|--------|-------|
| Traverse | ✅ | Graph traversal from vertex |
| ShortestPath | ✅ | Single shortest path (weighted/unweighted) |
| AllShortestPaths | ✅ | Find all shortest paths |
| PageRank | ✅ | PageRank algorithm |
| Neighbors | ✅ | Find adjacent vertices |
| ConnectedComponents | ✅ | Weakly connected components |
| StronglyConnectedComponents | ✅ | Strongly connected components |

**Direction Types**: OUT, IN, BOTH

### Full-Text Search (SASI/SAI)

| Feature | Status | Notes |
|---------|--------|-------|
| SASI Index | ✅ | SSTable Attached Secondary Index |
| SAI Index | ✅ | Storage Attached Index |
| Analyzer Modes | ✅ | STANDARD, NON_TOKENIZING, CASE_INSENSITIVE |
| CONTAINS Operator | ✅ | Term-based search |
| CONTAINS KEY Operator | ✅ | Map key search |
| LIKE Operator | ✅ | Prefix/suffix search |
| Inverted Index | ✅ | Term indexing |
| Prefix Index | ✅ | LIKE optimization |
| Suffix Index | ✅ | LIKE optimization |

### Vector/AI Features

| Feature | Status | Notes |
|---------|--------|-------|
| Vector Type | ✅ | vector<dimension, element_type> |
| Cosine Similarity | ✅ | Similarity search |
| Euclidean Distance | ✅ | L2 distance |
| Dot Product | ✅ | Inner product similarity |
| ANN Search | ✅ | Approximate nearest neighbor |
| Vector Encoding | ✅ | Wire protocol support |

### Trigger Execution (JavaScript Runtime)

OrbitRS provides full trigger execution using the shared QuickJS JavaScript runtime:

| Feature | Status | Notes |
|---------|--------|-------|
| CREATE TRIGGER | ✅ | Register triggers with table |
| DROP TRIGGER | ✅ | Remove triggers from registry |
| INSERT Triggers | ✅ | Execute on INSERT operations |
| UPDATE Triggers | ✅ | Execute on UPDATE operations |
| DELETE Triggers | ✅ | Execute on DELETE operations |
| JavaScript Execution | ✅ | QuickJS runtime (js-quickjs feature) |
| Security Sandbox | ✅ | SecurityConfig with timeouts, memory limits |
| Row Data Access | ✅ | Full SqlValue→JsValue conversion |
| Error Handling | ✅ | Trigger failures logged and returned |

**Trigger Execution Flow**:
1. Trigger registered via CREATE TRIGGER
2. DML operation (INSERT/UPDATE/DELETE) occurs
3. Row data converted from SqlValue to JsValue
4. JavaScript context created with row, event, table
5. Trigger code executed in sandboxed QuickJS runtime
6. Result logged, errors propagated

**Security Features**:
- Execution timeouts (default: 5s)
- Memory limits (default: 16MB)
- Blocked dangerous globals (eval, process, require)
- Allowed safe built-ins (JSON, Math, Array, etc.)

**Example Trigger**:
```sql
CREATE TRIGGER audit_trigger ON users USING 'AuditTrigger.js';
INSERT INTO users (id, name) VALUES (1, 'Alice');
-- Trigger executes with:
-- row = {id: 1, name: 'Alice'}
-- event = 'INSERT'
-- table = 'keyspace.users'
```

---

## Client Compatibility

### Tested Clients

| Client | Status | Notes |
|--------|--------|-------|
| cqlsh | ✅ | Basic queries work |
| DataStax Python Driver | ✅ | Full support |
| DataStax Java Driver | 🔶 | Basic functionality |
| Node.js cassandra-driver | ✅ | Full support |
| Go gocql | 🔶 | Basic queries |

---

## Version Compatibility

| Cassandra Version | Compatibility | Notes |
|-------------------|---------------|-------|
| Cassandra 3.x | 🔶 | Most features work |
| Cassandra 4.x | ✅ | Target version |
| Cassandra 5.x | ✅ | Compatible |
| ScyllaDB | 🔶 | Basic compatibility |

---

## References

- [CQL Documentation](https://cassandra.apache.org/doc/latest/cassandra/cql/)
- [Native Protocol Specification](https://github.com/apache/cassandra/blob/trunk/doc/native_protocol_v4.spec)
- [DataStax CQL Reference](https://docs.datastax.com/en/cql-oss/3.x/cql/cql_reference/cqlReferenceTOC.html)

---

## Implementation Statistics

### Code Organization

```
orbit/server/src/protocols/cql/
├── mod.rs              (85 lines)    - Module exports, CqlConfig
├── types.rs            (731 lines)   - Type system, encoding/decoding
├── protocol.rs         (1,148 lines) - Wire protocol, system tables
├── parser.rs           (3,461 lines) - CQL statement parsing
├── adapter.rs          (2,489 lines) - Statement execution, LWT
├── fts.rs              (617 lines)   - Full-text search (SASI/SAI)
├── spatial.rs          (739 lines)   - Geospatial functions
└── graph.rs            (687 lines)   - Graph algorithms
```

**Total**: ~9,957 lines of Rust code

### Test Coverage

**Test File**: adapter.rs (lines 2140-2489, ~350 lines)

Implemented tests:
- Adapter creation and initialization
- USE statement execution
- Batch operations (empty, logged, unlogged, counter batches)
- Lightweight transactions (INSERT IF NOT EXISTS, UPDATE IF, DELETE IF)
- LWT result building and condition evaluation
- Comparison operators in LWT conditions
- Incomplete frame detection

### Feature Highlights

1. **Full LWT Support**: Complete implementation with IF conditions, serial consistency, and condition evaluation
2. **Advanced Indexing**: SASI and SAI for full-text search with multiple analyzer modes
3. **Graph Capabilities**: 7 graph algorithms including PageRank, shortest paths, and connected components
4. **Geospatial**: 7 geospatial functions for distance, containment, and intersection tests
5. **Vector Search**: Full vector type support with similarity functions and ANN search
6. **Comprehensive DDL**: All 50+ DDL commands parsed and returning appropriate responses
7. **System Tables**: Complete system catalog for driver compatibility
8. **Counter Operations**: Optimized counter batch operations

### Architecture

- **CQL-to-SQL Translation**: All DML statements translated to SQL and executed via QueryEngine
- **Keyspace Qualification**: Automatic table name resolution with current keyspace
- **Collection Storage**: Collections stored as JSON in underlying storage
- **UUID Handling**: UUID types stored as TEXT with proper encoding/decoding
- **Error Handling**: Comprehensive CQL-specific error types and frame validation
- **Connection Management**: TLS support, connection metrics, and configurable limits

---

## Version History

| Date | Changes |
|------|---------|
| 2025-12-11 | Implemented active compression (Snappy, LZ4). Implemented full trigger JavaScript execution using shared QuickJS runtime with SecurityConfig, execution guards, and SqlValue→JsValue conversion. Added all missing scalar functions (minTimeuuid, maxTimeuuid, dateOf, unixTimestampOf, toJson, fromJson, token). Updated coverage from 38% to 72%. Documented all advanced features. |
| 2025-12-09 | Initial CQL compatibility specification |
