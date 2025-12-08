# OrbitQL Protocol Specification

**Target**: OrbitQL Native Query Language (SurrealDB-inspired)
**Reference**: https://surrealdb.com/docs/surrealql
**Last Updated**: 2025-12-08
**Current Estimated Coverage**: ~60%

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
| DEFINE FUNCTION | 🔶 | Parsing only, no execution |
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
| REMOVE FUNCTION | 🔶 | Basic support |
| REMOVE USER | ✅ | Full support |
| REMOVE SCOPE | ❌ | Not implemented |
| REMOVE ANALYZER | ❌ | Not implemented |

### Query Statements (CRUD Operations)

| Statement | Status | Notes |
|-----------|--------|-------|
| SELECT | ✅ | Full SQL support with extensions |
| CREATE | ✅ | Create records with auto/manual IDs |
| INSERT | ✅ | Bulk insert support |
| UPDATE | ✅ | Full update with WHERE |
| UPSERT | ✅ | Update or insert |
| DELETE | ✅ | Full delete with WHERE |
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
| ORDER BY | ✅ | ASC/DESC, NULLS |
| GROUP BY | ✅ | With ROLLUP, CUBE |
| HAVING | ✅ | Aggregate filtering |
| LIMIT/START | ✅ | Pagination |
| FETCH | 🔶 | Traverse record links |
| SPLIT | ❌ | Split results |
| VERSION | ❌ | Time-travel queries |
| TIMEOUT | ❌ | Query timeout |
| PARALLEL | ❌ | Parallel execution |

### Graph Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| RELATE (create edges) | 🔶 | Basic graph edges |
| Graph traversal | 🔶 | Basic path queries |
| Shortest path | ❌ | Not implemented |
| Path patterns | 🔶 | Basic pattern matching |
| Bidirectional edges | 🔶 | Basic support |
| Edge properties | ✅ | Full support |

### Document Operations

| Operation | Status | Notes |
|-----------|--------|-------|
| JSON Queries | ✅ | JSON path expressions |
| Nested objects | ✅ | Full support |
| Array operations | ✅ | Array manipulation |
| Dynamic fields | ✅ | Schema-less support |
| SCHEMAFULL tables | ✅ | Strict schema enforcement |
| SCHEMALESS tables | ✅ | Flexible schema |

---

## Data Types

### Scalar Types

| Type | Status | Notes |
|------|--------|-------|
| bool | ✅ | Boolean |
| int | ✅ | 64-bit integer |
| float | ✅ | 64-bit float |
| decimal | ✅ | Fixed-point |
| string | ✅ | UTF-8 string |
| datetime | ✅ | ISO 8601 datetime |
| duration | ✅ | Time duration |
| uuid | ✅ | UUID v4/v7 |
| bytes | ✅ | Binary data |
| null | ✅ | Null value |
| none | ✅ | Explicit none |

### Structured Types

| Type | Status | Notes |
|------|--------|-------|
| array | ✅ | Arrays of any type |
| object | ✅ | JSON objects |
| record | ✅ | Record IDs |
| geometry | ❌ | Geospatial types |
| set | ✅ | Unique values |

### Advanced Types

| Type | Status | Notes |
|------|--------|-------|
| option<T> | ✅ | Optional types |
| vector<F, N> | ✅ | Dense vectors |
| halfvec<N> | ✅ | Half-precision vectors |
| sparsevec | ✅ | Sparse vectors |

### Graph Types

| Type | Status | Notes |
|------|--------|-------|
| record (nodes) | ✅ | Graph nodes |
| record (edges) | 🔶 | Graph edges |
| record links | 🔶 | Record references |

---

## Functions

### Aggregate Functions

| Function | Status | Notes |
|----------|--------|-------|
| count() | ✅ | Count rows |
| sum() | ✅ | Sum values |
| avg() | ✅ | Average |
| min() | ✅ | Minimum |
| max() | ✅ | Maximum |
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
| math::abs() | ✅ | Absolute value |
| math::ceil() | ✅ | Ceiling |
| math::floor() | ✅ | Floor |
| math::round() | ✅ | Round |
| math::sqrt() | ✅ | Square root |
| math::pow() | ✅ | Power |
| math::ln() | ✅ | Natural log |
| math::log() | ✅ | Logarithm |
| math::sin() | ✅ | Sine |
| math::cos() | ✅ | Cosine |
| math::tan() | ✅ | Tangent |
| math::max() | ✅ | Maximum |
| math::min() | ✅ | Minimum |

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
| time::now() | ✅ | Current time |
| time::unix() | ✅ | Unix timestamp |
| time::day() | ✅ | Extract day |
| time::month() | ✅ | Extract month |
| time::year() | ✅ | Extract year |
| time::hour() | ✅ | Extract hour |
| time::minute() | ✅ | Extract minute |
| time::second() | ✅ | Extract second |
| time::floor() | ✅ | Floor to unit |
| time::round() | ✅ | Round to unit |
| time::format() | ✅ | Format datetime |

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

### Transaction Support

| Feature | Status | Notes |
|---------|--------|-------|
| BEGIN/COMMIT | ✅ | Transactions |
| CANCEL | ✅ | Transaction rollback |
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
| DEFINE Statements | ~60% | Core schema definition |
| Query Statements | ~90% | Full CRUD |
| Control Flow | ~30% | Basic IF/ELSE |
| Graph Operations | ~40% | Basic RELATE |
| Real-time | ~20% | WebSocket only |
| Functions | ~70% | Core functions |
| Vector Operations | ~90% | Full vector support |

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
