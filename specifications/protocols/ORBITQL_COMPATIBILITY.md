# OrbitQL Protocol Specification

**Target**: OrbitQL Native Query Language
**Reference**: Internal OrbitRS specification
**Last Updated**: 2025-12-08
**Current Estimated Coverage**: ~60%

---

## Overview

OrbitQL is OrbitRS's native query language designed for unified data access across multiple backends. It provides a SQL-like syntax with extensions for graph, document, and vector operations.

## Table of Contents

1. [Query Language](#query-language)
2. [Data Types](#data-types)
3. [Functions](#functions)
4. [Protocol Features](#protocol-features)
5. [Implementation Status](#implementation-status)

---

## Query Language

### Legend
- ✅ **Implemented** - Fully functional
- 🔶 **Partial** - Basic support, missing features
- ❌ **Not Implemented** - Not yet available

### Data Definition Language (DDL)

| Command | Status | Notes |
|---------|--------|-------|
| CREATE TABLE | ✅ | Full support |
| CREATE INDEX | ✅ | B-Tree, vector indexes |
| CREATE VIEW | ✅ | Materialized views |
| ALTER TABLE | 🔶 | Basic operations |
| DROP TABLE | ✅ | With IF EXISTS |
| DROP INDEX | ✅ | With IF EXISTS |
| TRUNCATE | ✅ | Full support |

### Data Manipulation Language (DML)

| Command | Status | Notes |
|---------|--------|-------|
| SELECT | ✅ | Full SQL support |
| INSERT | ✅ | VALUES, SELECT |
| UPDATE | ✅ | SET, WHERE |
| DELETE | ✅ | WHERE clause |
| MERGE | ✅ | UPSERT operations |

### Query Features

| Feature | Status | Notes |
|---------|--------|-------|
| WHERE clause | ✅ | Full expression support |
| JOIN (all types) | ✅ | INNER, LEFT, RIGHT, FULL |
| Subqueries | ✅ | Scalar, EXISTS, IN |
| CTEs (WITH) | ✅ | Recursive CTEs |
| Window Functions | 🔶 | Basic support |
| UNION/INTERSECT | ✅ | Set operations |
| ORDER BY | ✅ | ASC/DESC, NULLS |
| GROUP BY | ✅ | With ROLLUP, CUBE |
| HAVING | ✅ | Aggregate filtering |
| LIMIT/OFFSET | ✅ | Pagination |

### Vector Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| Vector Similarity Search | ✅ | L2, cosine, inner product |
| KNN Search | ✅ | K-nearest neighbors |
| Vector Indexing | ✅ | IVF-Flat, HNSW |
| Hybrid Search | ✅ | Vector + keyword |
| Distance Functions | ✅ | <->, <#>, <=> |

### Graph Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| MATCH (Cypher-like) | 🔶 | Basic pattern matching |
| CREATE (nodes/edges) | 🔶 | Graph creation |
| Path Queries | 🔶 | Shortest path |
| Graph Traversal | 🔶 | BFS/DFS |
| Property Graphs | 🔶 | Node/edge properties |

### Document Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| JSON Queries | ✅ | JSON path expressions |
| JSON Indexing | ✅ | GIN indexes |
| JSON Operators | ✅ | ->, ->>, @>, <@ |
| JSON Functions | ✅ | Extract, modify |
| Document Collections | ✅ | Schema-less storage |

---

## Data Types

### Scalar Types

| Type | Status | Notes |
|------|--------|-------|
| INTEGER | ✅ | 32-bit integer |
| BIGINT | ✅ | 64-bit integer |
| SMALLINT | ✅ | 16-bit integer |
| DECIMAL | ✅ | Fixed-point |
| FLOAT | ✅ | Single precision |
| DOUBLE | ✅ | Double precision |
| TEXT | ✅ | Variable-length string |
| VARCHAR | ✅ | Variable-length string |
| CHAR | ✅ | Fixed-length string |
| BOOLEAN | ✅ | True/false |
| DATE | ✅ | Date values |
| TIME | ✅ | Time values |
| TIMESTAMP | ✅ | Date and time |
| TIMESTAMPTZ | ✅ | With timezone |
| INTERVAL | ✅ | Time intervals |
| UUID | ✅ | Universally unique ID |
| BYTEA | ✅ | Binary data |

### Structured Types

| Type | Status | Notes |
|------|--------|-------|
| ARRAY | ✅ | Arrays of any type |
| JSON | ✅ | JSON documents |
| JSONB | ✅ | Binary JSON |
| VECTOR | ✅ | Dense vectors |
| HALFVEC | ✅ | Half-precision vectors |
| SPARSEVEC | ✅ | Sparse vectors |
| RANGE | ✅ | Range types |

### Graph Types

| Type | Status | Notes |
|------|--------|-------|
| NODE | 🔶 | Graph nodes |
| EDGE | 🔶 | Graph edges |
| PATH | 🔶 | Graph paths |
| GRAPH | 🔶 | Graph collections |

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
| ARRAY_AGG | ❌ | Aggregate to array |
| STRING_AGG | ❌ | Concatenate strings |

### String Functions

| Function | Status | Notes |
|----------|--------|-------|
| CONCAT | ✅ | Concatenate |
| SUBSTRING | ✅ | Extract substring |
| UPPER | ✅ | Uppercase |
| LOWER | ✅ | Lowercase |
| TRIM | ✅ | Remove whitespace |
| LENGTH | ✅ | String length |
| REPLACE | ✅ | Replace substring |

### Math Functions

| Function | Status | Notes |
|----------|--------|-------|
| ABS | ✅ | Absolute value |
| CEIL | ✅ | Ceiling |
| FLOOR | ✅ | Floor |
| ROUND | ✅ | Round |
| SQRT | ✅ | Square root |
| POWER | ✅ | Exponentiation |
| EXP | ✅ | Exponential |
| LN | ✅ | Natural log |
| LOG | ✅ | Logarithm |
| SIN/COS/TAN | ✅ | Trigonometric |
| FACTORIAL | ✅ | Factorial (PostgreSQL 18) |
| GCD/LCM | ✅ | GCD/LCM (PostgreSQL 18) |

### Date/Time Functions

| Function | Status | Notes |
|----------|--------|-------|
| NOW | ✅ | Current timestamp |
| CURRENT_DATE | ✅ | Current date |
| CURRENT_TIME | ✅ | Current time |
| EXTRACT | ✅ | Extract date part |
| DATE_TRUNC | ✅ | Truncate date |
| AGE | ✅ | Age calculation |

### Vector Functions

| Function | Status | Notes |
|----------|--------|-------|
| vector_dims | ✅ | Vector dimensions |
| vector_norm | ✅ | Vector norm |
| l2_distance | ✅ | L2 distance |
| cosine_distance | ✅ | Cosine distance |
| inner_product | ✅ | Inner product |

### JSON Functions

| Function | Status | Notes |
|----------|--------|-------|
| json_extract | ✅ | Extract value |
| json_array_length | ❌ | Array length |
| json_each | ❌ | Expand to rows |
| jsonb_set | ❌ | Set value |

---

## Protocol Features

### Connection

| Feature | Status | Notes |
|---------|--------|-------|
| HTTP/REST API | ✅ | RESTful interface |
| WebSocket | ✅ | Real-time queries |
| gRPC | ✅ | High-performance RPC |
| Connection Pooling | ✅ | Full support |
| SSL/TLS | ❌ | Not implemented |

### Query Execution

| Feature | Status | Notes |
|---------|--------|-------|
| Prepared Statements | ✅ | Full support |
| Parameterized Queries | ✅ | SQL injection prevention |
| Batch Execution | ✅ | Multiple queries |
| Streaming Results | ✅ | Large result sets |
| Query Cancellation | ✅ | Cancel running queries |

### Transaction Support

| Feature | Status | Notes |
|---------|--------|-------|
| BEGIN/COMMIT | ✅ | Transactions |
| ROLLBACK | ✅ | Transaction rollback |
| SAVEPOINT | ✅ | Named savepoints |
| Isolation Levels | 🔶 | Basic support |

### Advanced Features

| Feature | Status | Notes |
|---------|--------|-------|
| Query Optimization | ✅ | Cost-based optimizer |
| Query Planning | ✅ | Execution plans |
| Statistics | ✅ | Table/column stats |
| Indexes | ✅ | Multiple index types |
| Materialized Views | ✅ | Cached query results |
| Full-Text Search | 🔶 | Basic support |

---

## Implementation Status

### Overall Coverage

| Category | Coverage | Notes |
|----------|----------|-------|
| DDL Commands | ~70% | Core operations |
| DML Commands | ~90% | Full CRUD |
| Query Features | ~80% | Advanced SQL |
| Data Types | ~85% | Comprehensive types |
| Functions | ~70% | Core functions |
| Vector Operations | ~90% | Full vector support |
| Graph Operations | ~40% | Basic graph queries |
| Protocol | ~70% | Multiple interfaces |

### Priority Roadmap

**High Priority**:
1. ✅ SQL compatibility
2. ✅ Vector operations
3. ✅ gRPC protocol
4. 🔶 Graph queries
5. ❌ Advanced analytics

**Medium Priority**:
1. ❌ Time series operations
2. ❌ Geospatial queries
3. ❌ Full-text search
4. ❌ Machine learning integration

**Low Priority**:
1. ❌ Advanced graph algorithms
2. ❌ Stream processing
3. ❌ Complex event processing

---

## Known Limitations

1. **Graph Queries**: Limited Cypher support
2. **Full-Text Search**: Basic implementation
3. **Geospatial**: Not implemented
4. **Time Series**: Limited support
5. **Streaming**: Basic streaming only
6. **Advanced Analytics**: Not implemented
7. **Machine Learning**: Integration incomplete
8. **SSL/TLS**: Not implemented

---

## Client Libraries

### Supported Languages

| Language | Status | Notes |
|----------|--------|-------|
| Python | ✅ | Full support |
| JavaScript/TypeScript | ✅ | Full support |
| Rust | ✅ | Native support |
| Java | 🔶 | gRPC client |
| Go | 🔶 | gRPC client |
| C# | 🔶 | gRPC client |

---

## API Endpoints

### REST API

| Endpoint | Method | Status | Notes |
|----------|--------|--------|-------|
| /query | POST | ✅ | Execute query |
| /batch | POST | ✅ | Batch queries |
| /prepare | POST | ✅ | Prepare statement |
| /execute | POST | ✅ | Execute prepared |
| /tables | GET | ✅ | List tables |
| /schema | GET | ✅ | Get schema |
| /health | GET | ✅ | Health check |

### gRPC Services

| Service | Status | Notes |
|---------|--------|-------|
| QueryService | ✅ | Query execution |
| SchemaService | ✅ | Schema management |
| TransactionService | ✅ | Transaction control |
| StreamService | ✅ | Streaming queries |

---

## Performance Features

| Feature | Status | Notes |
|---------|--------|-------|
| Query Caching | ✅ | Result caching |
| Connection Pooling | ✅ | Connection reuse |
| Parallel Execution | ✅ | Multi-threaded |
| Vectorized Execution | ✅ | SIMD operations |
| Columnar Storage | ✅ | Efficient storage |
| Compression | ✅ | Data compression |
| Indexing | ✅ | Multiple index types |

---

## References

- [OrbitRS Documentation](https://github.com/orbitrs/orbit-rs)
- [Vector Extensions](https://github.com/pgvector/pgvector)
- [PostgreSQL Compatibility](./POSTGRESQL_18_COMPATIBILITY.md)
