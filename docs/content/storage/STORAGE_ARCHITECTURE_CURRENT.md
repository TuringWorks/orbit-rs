# Current Orbit Data Storage Architecture

**Date**: November 2025  
**Status**: Hybrid Architecture - Actors + Direct Storage

## Quick Answer

**Orbit uses a hybrid approach:**
- ✅ **RESP/Redis Protocol**: Uses actors as an **in-memory cache layer**, backed by persistent storage (RocksDB)
- ✅ **All Other Protocols**: Use **direct storage** (RocksDB) - no actors

## Detailed Architecture

### 1. RESP/Redis Protocol - Actor-Based with Persistence

**Location**: `orbit/server/src/protocols/resp/`

**Architecture**:
```
RESP Command
    ↓
SimpleLocalRegistry (in-memory actors)
    ├─ KeyValueActor (cache)
    ├─ ListActor (cache)
    ├─ SetActor (cache)
    ├─ SortedSetActor (cache)
    └─ RedisDataProvider (RocksDB persistence)
```

**How it works**:
1. **In-Memory Actors**: `SimpleLocalRegistry` maintains in-memory actor instances (`KeyValueActor`, `ListActor`, etc.) as a cache
2. **Persistent Backing**: All data is persisted to RocksDB via `RedisDataProvider`
3. **Cache-First**: Reads check actors first, then fall back to RocksDB if not in cache
4. **Write-Through**: Writes update both actors (cache) and RocksDB (persistence)

**Code Evidence**:
```rust
// Initialization from orbit/server/src/main.rs (lines 1046-1074)
let redis_data_path = args.data_dir.join("redis").join("rocksdb");
let redis_provider = RocksDbRedisDataProvider::new(
    redis_data_path.to_str().unwrap(),
    RedisDataConfig::default(),
)?;

let redis_server = RespServer::new_with_persistence(
    bind_addr, 
    orbit_client, 
    Some(Arc::new(redis_provider))  // ← RocksDB persistence enabled
);

// Data structure from orbit/server/src/protocols/resp/simple_local.rs (lines 16-29)
pub struct SimpleLocalRegistry {
    /// KeyValue actors (in-memory cache)
    keyvalue_actors: Arc<RwLock<HashMap<String, KeyValueActor>>>,
    /// Hash actors
    hash_actors: Arc<RwLock<HashMap<String, HashActor>>>,
    /// List actors
    list_actors: Arc<RwLock<HashMap<String, ListActor>>>,
    /// Set actors
    set_actors: Arc<RwLock<HashMap<String, SetActor>>>,
    /// Sorted set actors
    sorted_set_actors: Arc<RwLock<HashMap<String, SortedSetActor>>>,
    /// Optional persistent storage provider
    persistent_storage: Option<Arc<dyn RedisDataProvider>>,  // ← RocksDB
}

// On GET: Check persistent storage first, then cache (lines 92-113)
if method == "get_value" {
    if let Some(provider) = &self.persistent_storage {
        if let Ok(Some(redis_value)) = provider.get(key).await {
            // Update in-memory cache from RocksDB
            let actor = actors.entry(key.to_string()).or_insert_with(KeyValueActor::new);
            actor.set_value(redis_value.data.clone());
            return Ok(serde_json::to_value(Some(redis_value.data))?);
        }
    }
}

// On SET: Update both cache and persistence (lines 128-148)
actor.set_value(value.clone());  // ← Update actor (in-memory cache)
if let Some(provider) = &self.persistent_storage {
    let redis_value = RedisValue::new(value);
    provider.set(key, redis_value).await?;  // ← Write to RocksDB
}
```

**Why Actors for RESP?**
- **Redis Semantics**: Keys naturally map to actors (each key is an actor instance)
- **Distributed Future**: Enables distributed actor system integration for Redis cluster mode
- **Performance**: In-memory cache provides sub-millisecond latency for hot data
- **Compatibility**: Maintains Redis-like behavior with actor lifecycle management
- **Persistence**: RocksDB ensures data durability across restarts

### 2. PostgreSQL, MySQL, CQL - Direct Storage

**Location**: `orbit/server/src/protocols/common/storage/tiered.rs`

**Architecture**:
```
SQL Query
    ↓
TieredTableStorage
    └─ RocksDB (direct storage)
```

**How it works**:
- **No actors**: Direct RocksDB storage via `TieredTableStorage`
- **Protocol-specific directories**: Each protocol has its own RocksDB instance
  - PostgreSQL: `data/postgresql/rocksdb/`
  - MySQL: `data/mysql/rocksdb/`
  - CQL: `data/cql/rocksdb/`

**Code Evidence**:
```rust
// orbit/server/src/main.rs
let postgres_storage = Arc::new(TieredTableStorage::with_data_dir(
    postgres_data_dir,
    tiered_config.clone(),
));
// No actors - direct storage
```

### 3. Cypher/Neo4j - Direct Storage

**Location**: `orbit/server/src/protocols/cypher/storage.rs`

**Architecture**:
```
Cypher Query
    ↓
CypherGraphStorage
    └─ RocksDB (direct storage)
```

**How it works**:
- **No actors**: Direct RocksDB storage for graph data
- **Data directory**: `data/cypher/rocksdb/`

### 4. AQL/ArangoDB - Direct Storage

**Location**: `orbit/server/src/protocols/aql/storage.rs`

**Architecture**:
```
AQL Query
    ↓
AqlStorage
    └─ RocksDB (direct storage)
```

**How it works**:
- **No actors**: Direct RocksDB storage for documents and collections
- **Data directory**: `data/aql/rocksdb/`

### 5. GraphRAG - Direct Storage

**Location**: `orbit/server/src/protocols/graphrag/storage.rs`

**Architecture**:
```
GraphRAG Operation
    ↓
GraphRAGStorage
    └─ RocksDB (direct storage)
```

**How it works**:
- **No actors**: Direct RocksDB storage for knowledge graph data
- **Data directory**: `data/graphrag/rocksdb/`

## Storage Comparison

| Protocol | Storage Type | Uses Actors? | Persistence | Data Directory |
|----------|-------------|--------------|-------------|----------------|
| **RESP/Redis** | Hybrid (Actors + RocksDB) | ✅ Yes (cache layer) | ✅ RocksDB | `data/redis/rocksdb/` |
| **PostgreSQL** | Direct Storage | ❌ No | ✅ RocksDB | `data/postgresql/rocksdb/` |
| **MySQL** | Direct Storage | ❌ No | ✅ RocksDB | `data/mysql/rocksdb/` |
| **CQL** | Direct Storage | ❌ No | ✅ RocksDB | `data/cql/rocksdb/` |
| **Cypher** | Direct Storage | ❌ No | ✅ RocksDB | `data/cypher/rocksdb/` |
| **AQL** | Direct Storage | ❌ No | ✅ RocksDB | `data/aql/rocksdb/` |
| **GraphRAG** | Direct Storage | ❌ No | ✅ RocksDB | `data/graphrag/rocksdb/` |

## Why This Architecture?

### RESP Uses Actors Because:
1. **Redis Semantics**: Keys naturally map to actors
2. **Distributed Future**: Enables distributed actor system integration
3. **Performance**: In-memory cache for hot data
4. **Compatibility**: Maintains Redis-like behavior

### Other Protocols Use Direct Storage Because:
1. **SQL/Query Semantics**: Tables/collections don't map well to actors
2. **Performance**: Direct storage is more efficient for bulk operations
3. **Simplicity**: No need for actor abstraction layer
4. **Consistency**: All protocols use the same RocksDB persistence pattern

## Migration Path

**Current State**:
- RESP: Actors (cache) + RocksDB (persistence) ✅
- All others: Direct RocksDB storage ✅

**Future Considerations**:
- RESP actors could be removed if direct storage proves sufficient
- Or, other protocols could adopt actors if distributed features are needed
- Current architecture allows both approaches to coexist

## Conclusion

**Orbit uses actors for RESP/Redis protocol only**, as an in-memory cache layer backed by RocksDB. All other protocols (PostgreSQL, MySQL, CQL, Cypher, AQL, GraphRAG) use **direct RocksDB storage** without actors.

This hybrid approach provides:
- ✅ Redis compatibility (via actors)
- ✅ High performance (direct storage for SQL/graph protocols)
- ✅ Data persistence (RocksDB for all protocols)
- ✅ Flexibility (can evolve either direction)

