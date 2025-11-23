# Protocol Persistence Status

## Summary

This document outlines the current persistence status for all Orbit-RS protocols and identifies which protocols need RocksDB persistence implementation.

## Current Status

### ✅ Protocols WITH RocksDB Persistence

1. **PostgreSQL (5432)**
   - ✅ Uses `RocksDbTableStorage` at `data/rocksdb/`
   - ✅ Also uses `TieredTableStorage` with RocksDB at `data/postgresql/rocksdb/`
   - ✅ Full persistence implemented

2. **Redis (6379)**
   - ✅ Uses `RocksDbRedisDataProvider` at `data/redis/rocksdb/`
   - ✅ Full persistence implemented
   - ✅ **Fixed**: Now uses consistent path structure matching other protocols

3. **MySQL (3306)**
   - ✅ Uses `TieredTableStorage` with RocksDB at `data/mysql/rocksdb/`
   - ✅ Full persistence implemented (via recent fixes)

4. **CQL/Cassandra (9042)**
   - ✅ Uses `TieredTableStorage` with RocksDB at `data/cql/rocksdb/`
   - ✅ Full persistence implemented (via recent fixes)
   - **Note**: This was fixed in the recent `TieredTableStorage` RocksDB integration

### ❌ Protocols WITHOUT RocksDB Persistence

1. **Cypher/Neo4j (7687)**
   - ❌ No persistence implementation
   - ❌ Server is a stub (`CypherServer::run()` is a TODO)
   - ❌ Not initialized in `main.rs`
   - ❌ No storage backend configured
   - **Impact**: All graph data is lost on server restart

2. **AQL/ArangoDB (8529)**
   - ❌ No persistence implementation
   - ❌ No server implementation found
   - ❌ Not initialized in `main.rs`
   - ❌ No storage backend configured
   - **Impact**: All document/graph data is lost on server restart

## Recommendation

**YES, RocksDB persistence SHOULD be implemented** for Cypher and AQL protocols for the following reasons:

1. **Data Durability**: Without persistence, all data written through these protocols is lost on server restart
2. **Consistency**: All other protocols (PostgreSQL, MySQL, CQL, Redis) have persistence
3. **Production Readiness**: Production deployments require data durability
4. **User Expectations**: Users expect data to persist across restarts

## Implementation Plan

### For Cypher/Neo4j (Port 7687)

1. **Create Cypher Storage Backend**:
   - Create `orbit/server/src/protocols/cypher/storage.rs`
   - Implement graph storage using RocksDB
   - Store nodes, edges, and properties

2. **Update CypherServer**:
   - Add `TieredTableStorage` or custom graph storage
   - Initialize RocksDB at `data/cypher/rocksdb/`
   - Implement full Bolt protocol server (currently stub)

3. **Update main.rs**:
   - Add Cypher server initialization
   - Create `data/cypher/` directory
   - Pass storage to `CypherServer`

### For AQL/ArangoDB (Port 8529)

1. **Create AQL Server**:
   - Create `orbit/server/src/protocols/aql/server.rs`
   - Implement ArangoDB HTTP/WebSocket protocol

2. **Create AQL Storage Backend**:
   - Create `orbit/server/src/protocols/aql/storage.rs`
   - Implement document and graph storage using RocksDB
   - Store collections, documents, edges, and graphs

3. **Update main.rs**:
   - Add AQL server initialization
   - Create `data/aql/` directory
   - Pass storage to `AqlServer`

## Data Directory Structure

After implementation, the data directory structure should be:

```
data/
├── postgresql/
│   └── rocksdb/          # PostgreSQL RocksDB persistence
├── mysql/
│   └── rocksdb/          # MySQL RocksDB persistence
├── cql/
│   └── rocksdb/          # CQL RocksDB persistence
├── redis/
│   └── rocksdb/          # Redis RocksDB persistence
├── cypher/               # NEW: Cypher/Neo4j persistence
│   └── rocksdb/          # Graph data (nodes, edges, properties)
├── aql/                  # NEW: AQL/ArangoDB persistence
│   └── rocksdb/          # Document and graph data
├── hot/                  # Hot tier (shared)
├── warm/                 # Warm tier (shared)
├── cold/                 # Cold tier (shared)
├── wal/                  # Write-ahead log (shared)
└── rocksdb/              # Legacy PostgreSQL RocksDB
```

## Implementation Status

1. ✅ **CQL Persistence**: Already implemented via `TieredTableStorage`
2. ✅ **Cypher Persistence**: **IMPLEMENTED** - RocksDB storage at `data/cypher/rocksdb/`
3. ✅ **AQL Persistence**: **IMPLEMENTED** - RocksDB storage at `data/aql/rocksdb/`
4. ✅ **Server Initialization**: Both servers initialized in `main.rs`
5. ✅ **Directory Creation**: `data/cypher/` and `data/aql/` directories created on startup

## Implementation Details

### Cypher/Neo4j Storage (`CypherGraphStorage`)

**Location**: `orbit/server/src/protocols/cypher/storage.rs`

**Features**:
- RocksDB persistence for nodes and relationships
- In-memory caching for fast access
- Automatic data loading on startup
- Column families: `nodes`, `relationships`, `metadata`

**Storage Format**:
- Nodes: `node:{node_id}` → JSON serialized `GraphNode`
- Relationships: `rel:{rel_id}` → JSON serialized `GraphRelationship`

### AQL/ArangoDB Storage (`AqlStorage`)

**Location**: `orbit/server/src/protocols/aql/storage.rs`

**Features**:
- RocksDB persistence for collections and documents
- In-memory caching for fast access
- Automatic data loading on startup
- Column families: `collections`, `documents`, `edges`, `graphs`, `metadata`

**Storage Format**:
- Collections: `collection:{name}` → JSON serialized `AqlCollection`
- Documents: `doc:{collection}:{key}` → JSON serialized `AqlDocument`

### Server Integration

Both servers are now initialized in `main.rs`:
- **Cypher**: Port 7687, storage at `data/cypher/rocksdb/`
- **AQL**: Port 8529, storage at `data/aql/rocksdb/`

Both servers accept connections and have storage backends ready for use.

## Testing

After implementation, verify:

1. **Data Persistence**: Write data via protocol, restart server, verify data still exists
2. **Directory Creation**: Verify `data/cypher/rocksdb/` and `data/aql/rocksdb/` are created
3. **RocksDB Files**: Verify RocksDB database files (CURRENT, MANIFEST, *.sst) are created
4. **Cross-Protocol Access**: Verify data written via one protocol can be read via another (if applicable)

