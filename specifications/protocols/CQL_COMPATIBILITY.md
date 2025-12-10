# CQL (Cassandra Query Language) Compatibility Specification

**Target**: Apache Cassandra 4.x/5.x CQL Protocol
**Reference**: https://cassandra.apache.org/doc/latest/cassandra/cql/
**Last Updated**: 2025-12-09
**Current Estimated Coverage**: ~38%

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
| CREATE KEYSPACE | 🔶 | Basic creation |
| CREATE TABLE | ✅ | Full support with partition/clustering keys |
| CREATE INDEX | ✅ | Secondary indexes (incl. SASI/SAI) |
| CREATE MATERIALIZED VIEW | ❌ | Not implemented |
| CREATE TYPE | ❌ | User-defined types |
| CREATE FUNCTION | ❌ | User-defined functions |
| CREATE AGGREGATE | ❌ | User-defined aggregates |
| ALTER KEYSPACE | ❌ | Not implemented |
| ALTER TABLE | 🔶 | Basic column operations |
| ALTER TYPE | ❌ | Not implemented |
| DROP KEYSPACE | ✅ | With IF EXISTS |
| DROP TABLE | ✅ | With IF EXISTS |
| DROP INDEX | ✅ | With IF EXISTS |
| DROP MATERIALIZED VIEW | ❌ | Not implemented |
| DROP TYPE | ❌ | Not implemented |
| DROP FUNCTION | ❌ | Not implemented |
| DROP AGGREGATE | ❌ | Not implemented |
| TRUNCATE | ✅ | Full support |

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
| WHERE clause | ✅ | Partition key, clustering key |
| ALLOW FILTERING | 🔶 | Basic support |
| ORDER BY | ✅ | Clustering key ordering |
| LIMIT | ✅ | Row limit |
| PER PARTITION LIMIT | ❌ | Not implemented |
| GROUP BY | ❌ | Not implemented |
| Token function | ❌ | Not implemented |
| TTL | ✅ | Time-to-live |
| TIMESTAMP | ✅ | Write timestamp |
| IF EXISTS/IF NOT EXISTS | ✅ | Conditional operations |
| Lightweight Transactions | ❌ | Not implemented |
| SASI Index | ✅ | Full-text search (CONTAINS, LIKE) |

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
| duration | ❌ | Not implemented |
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
| list | ✅ | Ordered collection |
| set | ✅ | Unordered unique collection |
| map | ✅ | Key-value pairs |
| frozen | 🔶 | Frozen collections |

### User-Defined Types

| Feature | Status | Notes |
|---------|--------|-------|
| CREATE TYPE | ❌ | Not implemented |
| User-defined types | ❌ | Not implemented |
| Tuple types | ❌ | Not implemented |

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
| uuid() | ✅ | Random UUID |
| timeuuid() | ✅ | Time-based UUID |
| toDate() | ✅ | Convert to date |
| toTimestamp() | ✅ | Convert to timestamp |
| toUnixTimestamp() | ✅ | Convert to Unix timestamp |
| dateOf() | ✅ | Extract date from timeuuid |
| unixTimestampOf() | ✅ | Extract timestamp from timeuuid |
| minTimeuuid() | ❌ | Not implemented |
| maxTimeuuid() | ❌ | Not implemented |
| token() | ❌ | Not implemented |
| ttl() | ✅ | Get TTL |
| writetime() | ✅ | Get write timestamp |

### Type Conversion

| Function | Status | Notes |
|----------|--------|-------|
| CAST | 🔶 | Basic casting |
| toJson() | ❌ | Not implemented |
| fromJson() | ❌ | Not implemented |

---

## Wire Protocol

### Protocol Features

| Feature | Status | Notes |
|---------|--------|-------|
| Native Protocol v4 | ✅ | Full support |
| Native Protocol v5 | 🔶 | Partial support |
| Startup Message | ✅ | Connection handshake |
| Authentication | ✅ | SASL authentication |
| Query Message | ✅ | Simple queries |
| Prepare Message | 🔶 | Prepared statements |
| Execute Message | 🔶 | Execute prepared |
| Batch Message | 🔶 | Batch operations |
| Options Message | ✅ | Protocol options |
| Register Message | ❌ | Event registration |
| Compression | ❌ | Not implemented |
| SSL/TLS | ❌ | Not implemented |

### Result Types

| Result Type | Status | Notes |
|-------------|--------|-------|
| Void | ✅ | No result |
| Rows | ✅ | Result set |
| Set Keyspace | ✅ | Keyspace changed |
| Prepared | 🔶 | Prepared statement |
| Schema Change | ✅ | DDL result |

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
| Schema Commands | ~60% | Basic DDL support |
| DML Commands | ~80% | Full CRUD support |
| Query Features | ~50% | Missing advanced features |
| Data Types | ~90% | All basic types supported |
| Functions | ~60% | Core functions implemented |
| Wire Protocol | ~70% | Protocol v4 complete |
| Consistency | ~80% | Most levels supported |

### Priority Roadmap

**High Priority**:
1. ✅ Basic SELECT/INSERT/UPDATE/DELETE
2. ✅ CQL protocol v4
3. ✅ Core data types
4. 🔶 Prepared statements
5. ❌ Materialized views

**Medium Priority**:
1. ❌ User-defined types
2. ❌ User-defined functions
3. ❌ Lightweight transactions
4. ❌ Protocol v5 features

**Low Priority**:
1. ❌ Advanced indexing
2. ❌ Compression
3. ❌ SSL/TLS

---

## Known Limitations

1. **Materialized Views**: Not implemented
2. **User-Defined Types**: Not supported
3. **User-Defined Functions**: Not supported
4. **Lightweight Transactions**: Not implemented
5. **Compression**: Not supported
6. **SSL/TLS**: Not supported
7. **Event Registration**: Not implemented
8. **Token Awareness**: Not implemented
9. **Advanced Batch Operations**: Limited support

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
