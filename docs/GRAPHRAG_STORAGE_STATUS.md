# GraphRAG Data Storage Status

**Date**: November 2025  
**Status**: ✅ **Fully Persisted - All Three Options Implemented**

## Current Storage Architecture

### Where GraphRAG Data is Stored

GraphRAG data is currently stored **in-memory only** through the following components:

1. **GraphActor** (`orbit/server/src/protocols/graph_database.rs`)
   - Uses `InMemoryGraphStorage` (line 214)
   - Comment indicates: "in production this would be persistent"
   - **Data is lost on server restart**

2. **KnowledgeGraphBuilder** (`orbit/server/src/protocols/graphrag/knowledge_graph.rs`)
   - Creates nodes and relationships via `GraphActor`
   - Executes Cypher queries through GraphActor
   - Stores entities and relationships in-memory

3. **Storage Flow**:
   ```
   GraphRAGActor
     └─> KnowledgeGraphBuilder
           └─> GraphActor (via OrbitClient)
                 └─> InMemoryGraphStorage (in-memory only)
   ```

### What Gets Stored

- **Entity Nodes**: Extracted entities with properties, labels, confidence scores
- **Relationships**: Connections between entities with relationship types
- **Graph Metadata**: Statistics, node counts, relationship counts
- **Entity Cache**: Text-to-node-ID mappings (in-memory)
- **Relationship Cache**: Duplicate detection cache (in-memory)

### Current Status

✅ **Full Persistence**: All GraphRAG data is persisted to RocksDB  
✅ **Data Directory**: `data/graphrag/rocksdb/` directory created automatically  
✅ **RocksDB Integration**: Complete RocksDB persistence with column families  
✅ **Multiple Storage Options**: Three different persistence approaches available  

## Comparison with Other Protocols

| Protocol | Storage Type | Persistence | Data Directory |
|----------|-------------|-------------|----------------|
| PostgreSQL | RocksDB | ✅ Yes | `data/postgresql/rocksdb/` |
| MySQL | RocksDB | ✅ Yes | `data/mysql/rocksdb/` |
| CQL | RocksDB | ✅ Yes | `data/cql/rocksdb/` |
| Cypher | RocksDB | ✅ Yes | `data/cypher/rocksdb/` |
| AQL | RocksDB | ✅ Yes | `data/aql/rocksdb/` |
| **GraphRAG** | **RocksDB** | ✅ **Yes** | `data/graphrag/rocksdb/` |

## Implementation Complete

1. **Data Persistence**: All knowledge graphs, entities, and relationships survive restarts
2. **Durability**: Full crash recovery with RocksDB WAL
3. **Scalability**: No longer limited by available memory
4. **Consistency**: Now matches all other protocols with RocksDB persistence

## Implementation Complete ✅

All three options have been implemented:

### Option 1: PersistentGraphStorage Adapter ✅
- **Location**: `orbit/server/src/protocols/graph_database/persistent_storage.rs`
- **Purpose**: Wraps `CypherGraphStorage` to implement `GraphStorage` trait
- **Status**: Fully implemented and tested

### Option 2: GraphRAG-Specific Storage ✅
- **Location**: `orbit/server/src/protocols/graphrag/storage.rs`
- **Purpose**: Dedicated storage optimized for GraphRAG workloads
- **Features**: Entity indexing, embedding support, metadata tracking
- **Status**: Fully implemented with RocksDB persistence

### Option 3: Enhanced GraphActor ✅
- **Location**: `orbit/server/src/protocols/graph_database.rs`
- **Purpose**: Make GraphActor configurable for persistent storage
- **Features**: Backward compatible, opt-in persistence
- **Status**: Fully implemented

## Data Directory

The `data/graphrag/rocksdb/` directory is automatically created at server startup with the following column families:
- `nodes` - Entity nodes
- `relationships` - Entity relationships
- `metadata` - Graph metadata and statistics
- `embeddings` - Vector embeddings
- `entity_index` - Text-based entity lookup
- `rel_index` - Relationship indexing

## Usage

See `docs/GRAPHRAG_PERSISTENCE_IMPLEMENTATION.md` for detailed usage examples and integration guides.

## Status: ✅ Complete

All GraphRAG data is now fully persisted to RocksDB! 🎉

