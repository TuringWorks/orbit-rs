# Protocol Support in Orbit Desktop UI

## Current Status

The Orbit Desktop UI currently supports **3 out of 9 protocols** that the Orbit server provides.

### ✅ Supported Protocols (3/9)

1. **PostgreSQL** (SQL)
   - Connection type: `PostgreSQL`
   - Query type: `SQL`
   - Features:
     - Full SQL syntax highlighting
     - Query execution
     - EXPLAIN query support
     - Result set display
   - Port: 5432

2. **OrbitQL** (Native Orbit Query Language)
   - Connection type: `OrbitQL`
   - Query type: `OrbitQL`
   - Features:
     - SQL-like syntax with ML extensions
     - ML function autocompletion
     - Query execution via REST API
     - Result set display
   - Port: 8081 (or 8080 for REST)

3. **Redis** (RESP Protocol)
   - Connection type: `Redis`
   - Query type: `Redis`
   - Features:
     - Redis command autocompletion
     - Command execution
     - Result display
   - Port: 6379

### ❌ Missing Protocols (6/9)

1. **MySQL** (Wire Protocol)
   - Port: 3306
   - Status: Not supported in UI
   - Backend: ✅ Supported
   - Needed: MySQL connection type, MySQL query editor

2. **CQL** (Cassandra Query Language)
   - Port: 9042
   - Status: Not supported in UI
   - Backend: ✅ Supported
   - Needed: CQL connection type, CQL query editor with CQL syntax

3. **Cypher** (Neo4j Graph Query Language)
   - Port: 7687
   - Status: Not supported in UI
   - Backend: ✅ Supported
   - Needed: Cypher connection type, Cypher query editor

4. **AQL** (ArangoDB Query Language)
   - Port: 8529
   - Status: Not supported in UI
   - Backend: ✅ Supported
   - Needed: AQL connection type, AQL query editor

5. **gRPC** (Actor System Management)
   - Port: 50051
   - Status: Not supported in UI
   - Backend: ✅ Supported
   - Note: This is primarily for actor management, not query execution
   - Needed: gRPC client interface for actor operations

6. **REST API** (HTTP/JSON)
   - Port: 8080
   - Status: Partially supported (OrbitQL uses REST under the hood)
   - Backend: ✅ Supported
   - Note: Currently used indirectly via OrbitQL connection
   - Needed: Direct REST API query interface

## Implementation Details

### Current UI Components

1. **QueryEditor Component** (`src/components/QueryEditor.tsx`)
   - Supports: SQL, OrbitQL, Redis
   - Syntax highlighting: SQL for SQL/OrbitQL, JavaScript for Redis
   - Autocompletion: Protocol-specific keywords

2. **Connection Types** (`src/types/index.ts`)

   ```typescript
   export enum ConnectionType {
     PostgreSQL = 'PostgreSQL',
     OrbitQL = 'OrbitQL',
     Redis = 'Redis',
   }
   ```

3. **Query Types** (`src/types/index.ts`)

   ```typescript
   export enum QueryType {
     SQL = 'SQL',
     OrbitQL = 'OrbitQL',
     Redis = 'Redis',
   }
   ```

4. **Backend Support** (`src-tauri/src/connections.rs`)
   - Only implements: PostgreSQL, OrbitQL, Redis connections
   - Missing: MySQL, CQL, Cypher, AQL connection implementations

## What Needs to Be Added

### 1. Backend Connection Implementations

Add to `src-tauri/src/connections.rs`:

```rust
pub enum ConnectionType {
    PostgreSQL,
    OrbitQL,
    Redis,
    MySQL,      // ❌ Missing
    CQL,         // ❌ Missing
    Cypher,      // ❌ Missing
    AQL,         // ❌ Missing
}
```

### 2. Frontend Type Definitions

Update `src/types/index.ts`:

```typescript
export enum ConnectionType {
  PostgreSQL = 'PostgreSQL',
  OrbitQL = 'OrbitQL',
  Redis = 'Redis',
  MySQL = 'MySQL',      // ❌ Missing
  CQL = 'CQL',          // ❌ Missing
  Cypher = 'Cypher',    // ❌ Missing
  AQL = 'AQL',          // ❌ Missing
}

export enum QueryType {
  SQL = 'SQL',
  OrbitQL = 'OrbitQL',
  Redis = 'Redis',
  MySQL = 'MySQL',      // ❌ Missing
  CQL = 'CQL',          // ❌ Missing
  Cypher = 'Cypher',    // ❌ Missing
  AQL = 'AQL',          // ❌ Missing
}
```

### 3. Query Editor Enhancements

Update `src/components/QueryEditor.tsx`:

- Add syntax highlighting for CQL, Cypher, AQL
- Add autocompletion keywords for each protocol
- Add protocol-specific query formatting

### 4. Query Execution

Update `src-tauri/src/queries.rs`:

- Add query execution methods for MySQL, CQL, Cypher, AQL
- Handle protocol-specific result formats

### 5. Dependencies

Add to `src-tauri/Cargo.toml`:

- MySQL client library
- CQL client library
- Cypher client library
- AQL client library

## Recommended Priority

1. **High Priority**:
   - MySQL (widely used, similar to PostgreSQL)
   - REST API (direct HTTP interface)

2. **Medium Priority**:
   - CQL (Cassandra compatibility)
   - Cypher (graph queries)

3. **Low Priority**:
   - AQL (ArangoDB, less common)
   - gRPC (actor management, not query-focused)

## Summary

### Current Coverage: 33% (3/9 protocols)

The UI has good support for the most common protocols (PostgreSQL, OrbitQL, Redis) but is missing support for MySQL, CQL, Cypher, and AQL. To make the desktop application truly production-ready for all Orbit server capabilities, these protocols should be added.
