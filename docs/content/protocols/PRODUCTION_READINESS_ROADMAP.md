# Protocol# Production Readiness Roadmap

> [!IMPORTANT]
> **CRITICAL ARCHITECTURAL DIRECTIVE**: All protocols must be embedded directly within `OrbitServer`. We are moving away from standalone protocol adapters in `orbit-protocols` and consolidating everything into `orbit-server`. `OrbitClient` must be used for in-process communication where applicable.

## 1. Protocol Standardization & Feature Parity
2025-11-29
**Status**: Active Planning

## Executive Summary

This roadmap outlines the critical path to achieving production readiness for Orbit-RS's four primary protocols: **RESP (Redis)**, **PostgreSQL**, **Neo4j (Bolt)**, and **ArangoDB (AQL)**.

| Protocol | Current Status | Target | Key Gaps |
|----------|----------------|--------|----------|
| **RESP** | 99% (Polishing) | 100% | Transactions, Blocking Ops |
| **PostgreSQL** | 95% (Polishing) | 100% | Transactions |
| **Neo4j** | 15% (Active) | MVP | Bolt Handshake, Core Graph Actors, Basic Cypher |
| **ArangoDB** | 15% (Active) | MVP | AQL Parser Completion, Document/Graph Actors |

---

## 1. RESP Protocol (Redis) - 95% Complete

**Goal**: Polish to 100% Production Readiness.

### Critical Gaps (P0)
- [x] **Fix Syntax Errors**: `commands.rs` has corruption/syntax errors that need immediate fixing.
- [x] **Storage Backends**: Support in-memory HashMap, Actors, RocksDB and other Orbit storage backends

### Feature Gaps (P1)
- [ ] **Transactions**: Implement `MULTI`, `EXEC`, `DISCARD` using Orbit's `TransactionCoordinator`.
- [ ] **Blocking Operations**: Implement `BLPOP`, `BRPOP` using Orbit's event system.
- [ ] **Pattern Matching**: Implement `KEYS` with distributed directory listing.

### Enhancements (P2)
- [ ] **Advanced Types**: Implement Sets (`SADD`) and Sorted Sets (`ZADD`).
- [ ] **Auth**: Implement `AUTH` command with Orbit security.

---

## 2. PostgreSQL Protocol - 85% Complete

**Goal**: Full SQL Support & Security.

### Critical Gaps (P0)
- [x] **OrbitClient Integration**: Replace in-memory storage with `OrbitClient` to persist data to actors.
- [x] **Authentication**: Implement MD5 or SCRAM-SHA-256 authentication (currently Trust-only).

### Feature Gaps (P1)
- [ ] **Transactions**: Implement `BEGIN`, `COMMIT`, `ROLLBACK` mapped to Orbit transactions.
- [ ] **Extended SQL**: Support `ORDER BY`, `LIMIT`, `OFFSET`, and `LIKE` operator.
- [ ] **Binary Format**: Support binary result format (currently Text-only).

### Enhancements (P2)
- [ ] **Joins**: Basic `JOIN` support between actor types.
- [ ] **Aggregates**: `COUNT`, `SUM`, `AVG`.

---

## 3. Neo4j Protocol (Bolt) - 15% Active

**Goal**: Minimum Viable Product (MVP).

### MVP Scope
- **Connection**: Bolt v4.4 Handshake & Authentication.
- **Storage**: `GraphNodeActor` and `RelationshipActor` implementation.
- **Query**: Basic Cypher execution (`MATCH`, `CREATE`, `RETURN`).

### Roadmap
1.  **Protocol Layer**: Implement Bolt handshake and message framing.
2.  **Actor Layer**: Implement `GraphNodeActor` and `RelationshipActor`.
3.  **Query Layer**: Integrate `CypherQueryActor` with basic parser.

---

## 4. ArangoDB Protocol (AQL) - 15% Active

**Goal**: Minimum Viable Product (MVP).

### MVP Scope
- **Connection**: HTTP API (ArangoDB uses HTTP/JSON).
- **Storage**: `DocumentCollectionActor` and `GraphActor`.
- **Query**: Basic AQL execution (`FOR`, `FILTER`, `RETURN`).

### Roadmap
1.  **API Layer**: Implement ArangoDB-compatible HTTP endpoints.
2.  **Actor Layer**: Implement `DocumentCollectionActor` (JSON storage).
3.  **Query Layer**: Complete AQL parser and simple execution plan.

---

## Definition of Done (Production Ready)

A protocol is considered **Production Ready** when:
1.  **Protocol Compliance**: Passes standard client connection tests (redis-cli, psql, neo4j-shell).
2.  **Persistence**: Data is persisted via `OrbitClient` -> Actors -> RocksDB (not in-memory).
3.  **Concurrency**: Handles multiple concurrent clients correctly.
4.  **Error Handling**: Returns correct protocol-specific error codes.
5.  **Testing**: Comprehensive integration test suite passing.
