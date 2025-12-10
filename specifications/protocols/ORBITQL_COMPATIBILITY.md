# OrbitQL Protocol Specification

**Target**: OrbitQL Native Query Language (SurrealDB-inspired)
**Reference**: https://surrealdb.com/docs/surrealql
**Last Updated**: 2025-12-09
**Current Estimated Coverage**: ~65%

---

## Overview

OrbitQL is OrbitRS's native query language, heavily inspired by SurrealDB's SurrealQL. It provides a SQL-like syntax with extensions for graph relationships, document operations, vector search, and real-time queries.

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

### DEFINE Statements (Schema Definition)

| Statement | Status | Notes |
|-----------|--------|-------|
| DEFINE NAMESPACE | 🔶 | Basic namespace support |
| DEFINE DATABASE | ✅ | Full database creation |
| DEFINE TABLE | ✅ | Full support with SCHEMAFULL/SCHEMALESS |
| DEFINE FIELD | ✅ | Field definitions with types and constraints |
| DEFINE INDEX | ✅ | B-Tree, vector, full-text indexes |
| DEFINE EVENT | ❌ | Not implemented |
| DEFINE FUNCTION | ✅ | Full parsing with parameters, return types, languages |
| DEFINE TOKEN | ❌ | Not implemented |
| DEFINE USER | 🔶 | Basic user management |
| DEFINE SCOPE | ❌ | Not implemented |
| DEFINE ANALYZER | 🔶 | Basic text analyzers |
| DEFINE PARAM | ❌ | Not implemented |

### ALTER Statements

| Statement | Status | Notes |
|-----------|--------|-------|
| ALTER TABLE | 🔶 | Basic column operations |
| ALTER FIELD | 🔶 | Field modifications |
| ALTER INDEX | ❌ | Not implemented |

### REMOVE Statements

| Statement | Status | Notes |
|-----------|--------|-------|
| REMOVE NAMESPACE | 🔶 | Basic support |
| REMOVE DATABASE | ✅ | Full support |
| REMOVE TABLE | ✅ | Full support |
| REMOVE FIELD | ✅ | Full support |
| REMOVE INDEX | ✅ | Full support |
| REMOVE EVENT | ❌ | Not implemented |
| REMOVE FUNCTION | ✅ | Full support |
| REMOVE USER | ✅ | Full support |
| REMOVE SCOPE | ❌ | Not implemented |
| REMOVE ANALYZER | ❌ | Not implemented |

### Additional DDL Commands

| Statement | Status | Notes |
|-----------|--------|-------|
| CREATE VIEW | ✅ | Materialized views |
| CREATE FUNCTION | ✅ | PostgreSQL-style with parameters, return types, language (SQL/OrbitQL/JavaScript/Python), volatility (IMMUTABLE/STABLE/VOLATILE) |
| CREATE PROCEDURE | ✅ | Parameter modes (IN/OUT/INOUT/VARIADIC), dollar-quoted bodies |
| DROP FUNCTION | ✅ | Full support |
| DROP PROCEDURE | ✅ | Full support |
| TRUNCATE | ✅ | Full support |

### Query Statements (CRUD Operations)

| Statement | Status | Notes |
|-----------|--------|-------|
| SELECT | ✅ | Full SQL support with extensions |
| CREATE | ✅ | Create records with auto/manual IDs |
| INSERT | ✅ | Bulk insert support |
| UPDATE | ✅ | Full update with WHERE |
| UPSERT | ✅ | Update or insert |
| DELETE | ✅ | Full delete with WHERE |
| CALL | ✅ | Procedure invocation with arguments |
| RELATE | 🔶 | Graph edge creation (basic) |
| LIVE SELECT | ❌ | Real-time query subscriptions |
| KILL | ❌ | Cancel LIVE SELECT |

### Transaction Statements

| Statement | Status | Notes |
|-----------|--------|-------|
| BEGIN TRANSACTION | ✅ | Start transaction |
| COMMIT TRANSACTION | ✅ | Commit transaction |
| CANCEL TRANSACTION | ✅ | Rollback transaction |

### Control Flow Statements

| Statement | Status | Notes |
|-----------|--------|-------|
| FOR | ❌ | For loops |
| IF ELSE | 🔶 | Conditional logic |
| CONTINUE | ❌ | Loop continuation |
| BREAK | ❌ | Loop/scope break |
| RETURN | ✅ | Return values |
| THROW | ❌ | Error throwing |
| SLEEP | ❌ | Execution pause |

### Utility Statements

| Statement | Status | Notes |
|-----------|--------|-------|
| USE | ✅ | Switch namespace/database |
| INFO | 🔶 | Database introspection |
| SHOW | 🔶 | Show changefeeds |
| LET | ✅ | Variable assignment |
| REBUILD | ❌ | Index rebuild |
| ACCESS | ❌ | Access management |

### Query Features (SELECT Extensions)

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
| FETCH | 🔶 | Traverse record links |
| SPLIT | ❌ | Split results |
| VERSION | ❌ | Time-travel queries |
| TIMEOUT | ❌ | Query timeout |
| PARALLEL | ❌ | Parallel execution |

### Vector Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| Vector Similarity Search | ✅ | L2, cosine, inner product |
| KNN Search | ✅ | K-nearest neighbors |
| Vector Indexing | ✅ | IVF-Flat, HNSW |
| Hybrid Search | ✅ | Vector + keyword |
| Distance Functions | ✅ | <->, <#>, <=> operators |

### Graph Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| MATCH (Cypher-like) | 🔶 | Basic pattern matching |
| RELATE (create edges) | 🔶 | Basic graph edges |
| CREATE (nodes/edges) | 🔶 | Graph creation |
| Graph traversal | 🔶 | BFS/DFS |
| Path Queries | 🔶 | Shortest path |
| Path patterns | 🔶 | Basic pattern matching |
| Bidirectional edges | 🔶 | Basic support |
| Edge properties | ✅ | Full support |
| Property Graphs | 🔶 | Node/edge properties |

### Document Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| JSON Queries | ✅ | JSON path expressions |
| JSON Indexing | ✅ | GIN indexes |
| JSON Operators | ✅ | ->, ->>, @>, <@ |
| JSON Functions | ✅ | Extract, modify |
| Nested objects | ✅ | Full support |
| Array operations | ✅ | Array manipulation |
| Dynamic fields | ✅ | Schema-less support |
| Document Collections | ✅ | Schema-less storage |
| SCHEMAFULL tables | ✅ | Strict schema enforcement |
| SCHEMALESS tables | ✅ | Flexible schema |

---

## Data Types

### Scalar Types

| Type | Status | Notes |
|------|--------|-------|
| bool / BOOLEAN | ✅ | Boolean |
| int / BIGINT | ✅ | 64-bit integer |
| int32 / INTEGER | ✅ | 32-bit integer |
| int16 / SMALLINT | ✅ | 16-bit integer |
| float / DOUBLE | ✅ | 64-bit float |
| float32 / FLOAT | ✅ | Single precision |
| decimal / DECIMAL | ✅ | Fixed-point |
| string / TEXT | ✅ | UTF-8 string |
| varchar / VARCHAR | ✅ | Variable-length string |
| char / CHAR | ✅ | Fixed-length string |
| datetime / TIMESTAMP | ✅ | ISO 8601 datetime |
| datetimez / TIMESTAMPTZ | ✅ | With timezone |
| date / DATE | ✅ | Date values |
| time / TIME | ✅ | Time values |
| duration / INTERVAL | ✅ | Time duration |
| uuid / UUID | ✅ | UUID v4/v7 |
| bytes / BYTEA | ✅ | Binary data |
| null / NULL | ✅ | Null value |
| none / NONE | ✅ | Explicit none |

### Structured Types

| Type | Status | Notes |
|------|--------|-------|
| array / ARRAY | ✅ | Arrays of any type |
| object / JSON | ✅ | JSON documents |
| JSONB | ✅ | Binary JSON |
| record | ✅ | Record IDs |
| geometry | ❌ | Geospatial types |
| set | ✅ | Unique values |
| RANGE | ✅ | Range types |

### Advanced Types

| Type | Status | Notes |
|------|--------|-------|
| option<T> | ✅ | Optional types |
| vector<F, N> / VECTOR | ✅ | Dense vectors |
| halfvec<N> / HALFVEC | ✅ | Half-precision vectors |
| sparsevec / SPARSEVEC | ✅ | Sparse vectors |

### Graph Types

| Type | Status | Notes |
|------|--------|-------|
| NODE | 🔶 | Graph nodes |
| EDGE | 🔶 | Graph edges |
| PATH | 🔶 | Graph paths |
| GRAPH | 🔶 | Graph collections |
| record (nodes) | ✅ | Graph nodes |
| record (edges) | 🔶 | Graph edges |
| record links | 🔶 | Record references |

---

## Functions

### Aggregate Functions

| Function | Status | Notes |
|----------|--------|-------|
| count() / COUNT | ✅ | Count rows |
| sum() / SUM | ✅ | Sum values |
| avg() / AVG | ✅ | Average |
| min() / MIN | ✅ | Minimum |
| max() / MAX | ✅ | Maximum |
| ARRAY_AGG | ✅ | Aggregate to array |
| STRING_AGG | ✅ | Concatenate strings |
| array::group() | ❌ | Group to array |
| string::join() | ❌ | Join strings |

### String Functions

| Function | Status | Notes |
|----------|--------|-------|
| string::concat() | ✅ | Concatenate |
| string::contains() | ✅ | Contains check |
| string::endsWith() | ✅ | Ends with |
| string::startsWith() | ✅ | Starts with |
| string::length() | ✅ | String length |
| string::lowercase() | ✅ | Lowercase |
| string::uppercase() | ✅ | Uppercase |
| string::trim() | ✅ | Trim whitespace |
| string::split() | ✅ | Split string |
| string::slice() | ✅ | Extract substring |
| string::reverse() | ✅ | Reverse string |
| string::replace() | ✅ | Replace substring |

### Math Functions

| Function | Status | Notes |
|----------|--------|-------|
| math::abs() / ABS | ✅ | Absolute value |
| math::ceil() / CEIL | ✅ | Ceiling |
| math::floor() / FLOOR | ✅ | Floor |
| math::round() / ROUND | ✅ | Round |
| math::sqrt() / SQRT | ✅ | Square root |
| math::pow() / POWER | ✅ | Power/Exponentiation |
| math::exp() / EXP | ✅ | Exponential |
| math::ln() / LN | ✅ | Natural log |
| math::log() / LOG | ✅ | Logarithm |
| math::sin() / SIN | ✅ | Sine |
| math::cos() / COS | ✅ | Cosine |
| math::tan() / TAN | ✅ | Tangent |
| math::max() | ✅ | Maximum |
| math::min() | ✅ | Minimum |
| FACTORIAL | ✅ | Factorial (PostgreSQL 18) |
| GCD | ✅ | Greatest common divisor |
| LCM | ✅ | Least common multiple |

### Array Functions

| Function | Status | Notes |
|----------|--------|-------|
| array::add() | ✅ | Add element |
| array::all() | ✅ | All match |
| array::any() | ✅ | Any match |
| array::append() | ✅ | Append array |
| array::combine() | ✅ | Combine arrays |
| array::complement() | ✅ | Set complement |
| array::concat() | ✅ | Concatenate |
| array::difference() | ✅ | Set difference |
| array::distinct() | ✅ | Unique values |
| array::flatten() | ✅ | Flatten nested |
| array::group() | ❌ | Group elements |
| array::insert() | ✅ | Insert at index |
| array::intersect() | ✅ | Set intersection |
| array::len() | ✅ | Array length |
| array::max() | ✅ | Maximum value |
| array::min() | ✅ | Minimum value |
| array::pop() | ✅ | Remove last |
| array::push() | ✅ | Add to end |
| array::remove() | ✅ | Remove element |
| array::reverse() | ✅ | Reverse array |
| array::slice() | ✅ | Extract slice |
| array::sort() | ✅ | Sort array |
| array::union() | ✅ | Set union |

### Time Functions

| Function | Status | Notes |
|----------|--------|-------|
| time::now() / NOW | ✅ | Current time |
| CURRENT_DATE | ✅ | Current date |
| CURRENT_TIME | ✅ | Current time |
| CURRENT_TIMESTAMP | ✅ | Current timestamp |
| time::unix() | ✅ | Unix timestamp |
| time::day() / EXTRACT(DAY) | ✅ | Extract day |
| time::month() / EXTRACT(MONTH) | ✅ | Extract month |
| time::year() / EXTRACT(YEAR) | ✅ | Extract year |
| time::hour() / EXTRACT(HOUR) | ✅ | Extract hour |
| time::minute() / EXTRACT(MINUTE) | ✅ | Extract minute |
| time::second() / EXTRACT(SECOND) | ✅ | Extract second |
| time::floor() / DATE_TRUNC | ✅ | Truncate/floor to unit |
| time::round() | ✅ | Round to unit |
| time::format() | ✅ | Format datetime |
| AGE | ✅ | Age calculation |

### Type Functions

| Function | Status | Notes |
|----------|--------|-------|
| type::bool() | ✅ | Cast to bool |
| type::int() | ✅ | Cast to int |
| type::float() | ✅ | Cast to float |
| type::string() | ✅ | Cast to string |
| type::number() | ✅ | Cast to number |
| type::datetime() | ✅ | Cast to datetime |
| type::is::array() | ✅ | Check if array |
| type::is::bool() | ✅ | Check if bool |
| type::is::datetime() | ✅ | Check if datetime |
| type::is::null() | ✅ | Check if null |
| type::is::number() | ✅ | Check if number |
| type::is::object() | ✅ | Check if object |
| type::is::string() | ✅ | Check if string |
| type::is::uuid() | ✅ | Check if UUID |

### Crypto Functions

| Function | Status | Notes |
|----------|--------|-------|
| crypto::md5() | ✅ | MD5 hash |
| crypto::sha1() | ✅ | SHA-1 hash |
| crypto::sha256() | ✅ | SHA-256 hash |
| crypto::sha512() | ✅ | SHA-512 hash |
| crypto::argon2::compare() | ❌ | Argon2 verify |
| crypto::argon2::generate() | ❌ | Argon2 hash |
| crypto::bcrypt::compare() | ❌ | Bcrypt verify |
| crypto::bcrypt::generate() | ❌ | Bcrypt hash |
| crypto::pbkdf2::compare() | ❌ | PBKDF2 verify |
| crypto::pbkdf2::generate() | ❌ | PBKDF2 hash |

### Vector Functions

| Function | Status | Notes |
|----------|--------|-------|
| vector_dims | ✅ | Vector dimensions |
| vector_norm | ✅ | Vector norm |
| l2_distance / <-> | ✅ | L2 distance (operator) |
| cosine_distance / <=> | ✅ | Cosine distance (operator) |
| inner_product / <#> | ✅ | Inner product (operator) |
| vector::add() | ✅ | Vector addition |
| vector::angle() | ✅ | Angle between vectors |
| vector::cross() | ✅ | Cross product |
| vector::divide() | ✅ | Vector division |
| vector::dot() | ✅ | Dot product |
| vector::magnitude() | ✅ | Vector magnitude |
| vector::multiply() | ✅ | Scalar multiplication |
| vector::normalize() | ✅ | Normalize vector |
| vector::project() | ✅ | Vector projection |
| vector::subtract() | ✅ | Vector subtraction |
| vector::distance::cosine() | ✅ | Cosine distance |
| vector::distance::euclidean() | ✅ | L2 distance |
| vector::distance::hamming() | ✅ | Hamming distance |
| vector::distance::manhattan() | ✅ | L1 distance |
| vector::similarity::cosine() | ✅ | Cosine similarity |
| vector::similarity::jaccard() | ✅ | Jaccard similarity |

### JSON Functions

| Function | Status | Notes |
|----------|--------|-------|
| json_extract / -> | ✅ | Extract value |
| json_extract_text / ->> | ✅ | Extract as text |
| json_contains / @> | ✅ | Contains check |
| json_contained / <@ | ✅ | Is contained check |
| json_array_length | ❌ | Array length |
| json_each | ❌ | Expand to rows |
| jsonb_set | ❌ | Set value |

### Geo Functions

| Function | Status | Notes |
|----------|--------|-------|
| geo::area() | ❌ | Calculate area |
| geo::bearing() | ❌ | Calculate bearing |
| geo::centroid() | ❌ | Find centroid |
| geo::distance() | ❌ | Calculate distance |
| geo::hash::decode() | ❌ | Decode geohash |
| geo::hash::encode() | ❌ | Encode geohash |

### Parse Functions

| Function | Status | Notes |
|----------|--------|-------|
| parse::email::domain() | ❌ | Extract email domain |
| parse::email::user() | ❌ | Extract email user |
| parse::url::domain() | ❌ | Extract URL domain |
| parse::url::fragment() | ❌ | Extract URL fragment |
| parse::url::host() | ❌ | Extract URL host |
| parse::url::path() | ❌ | Extract URL path |
| parse::url::port() | ❌ | Extract URL port |
| parse::url::query() | ❌ | Extract URL query |

### Random Functions

| Function | Status | Notes |
|----------|--------|-------|
| rand() | ✅ | Random float [0,1) |
| rand::bool() | ✅ | Random boolean |
| rand::enum() | ✅ | Random from list |
| rand::float() | ✅ | Random float |
| rand::int() | ✅ | Random integer |
| rand::string() | ✅ | Random string |
| rand::time() | ✅ | Random time |
| rand::uuid() | ✅ | Random UUID |
| rand::uuid::v4() | ✅ | Random UUID v4 |
| rand::uuid::v7() | ✅ | Random UUID v7 |

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
| ROLLBACK / CANCEL | ✅ | Transaction rollback |
| SAVEPOINT | ✅ | Named savepoints |
| Isolation Levels | 🔶 | Basic support |
| Optimistic Locking | ❌ | Not implemented |

### Real-time Features

| Feature | Status | Notes |
|---------|--------|-------|
| LIVE SELECT | ❌ | Real-time subscriptions |
| Changefeeds | 🔶 | Basic support |
| WebSocket push | ✅ | Real-time updates |
| Event triggers | ❌ | Not implemented |

### Advanced Features

| Feature | Status | Notes |
|---------|--------|-------|
| Query Optimization | ✅ | Cost-based optimizer |
| Query Planning | ✅ | Execution plans |
| Statistics | ✅ | Table/column stats |
| Indexes | ✅ | Multiple index types |
| Materialized Views | ✅ | Cached query results |
| Full-Text Search | 🔶 | Basic support |
| Vector Search | ✅ | Full support |
| Graph Queries | 🔶 | Basic support |

---

## Implementation Status

### Overall Coverage

| Category | Coverage | Notes |
|----------|----------|-------|
| DDL Commands | ~75% | Core schema operations, Functions/Procedures |
| DML Commands | ~90% | Full CRUD, CALL |
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
3. 🔶 Full-text search
4. ❌ Machine learning integration

**Low Priority**:
1. ❌ Advanced graph algorithms
2. ❌ Stream processing
3. ❌ Complex event processing

### SurrealDB Compatibility

| Feature Category | Compatibility | Notes |
|------------------|---------------|-------|
| Core SQL | ~85% | Strong SQL foundation |
| Graph Features | ~40% | Basic graph support |
| Document Model | ~80% | Good document support |
| Real-time | ~20% | Limited LIVE SELECT |
| Permissions | ~30% | Basic access control |
| Functions | ~70% | Most functions implemented |

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

- [SurrealDB Documentation](https://surrealdb.com/docs)
- [SurrealQL Statements](https://surrealdb.com/docs/surrealql/statements)
- [SurrealDB GitHub](https://github.com/surrealdb/surrealdb)
- [OrbitRS Documentation](https://github.com/orbitrs/orbit-rs)
- [Vector Extensions (pgvector)](https://github.com/pgvector/pgvector)
- [PostgreSQL Compatibility](./POSTGRESQL_18_COMPATIBILITY.md)
