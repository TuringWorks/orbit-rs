# Redis Protocol Implementation

Orbit-RS implements the Redis Serialization Protocol (RESP) to allow standard Redis clients to interact with Orbit actors. The implementation maps Redis data structures to Orbit's distributed actors, providing a scalable and durable Redis-compatible interface.

## Architecture

The Redis implementation is built on top of Orbit's actor system:

- **RESP Adapter**: Handles the RESP2/RESP3 protocol parsing and serialization.
- **Actor Mapping**: Redis keys are mapped to specific actors based on the data type.
  - `KeyValueActor`: Handles String operations (`SET`, `GET`, etc.)
  - `HashActor`: Handles Hash operations (`HSET`, `HGET`, etc.)
  - `ListActor`: Handles List operations (`LPUSH`, `LRANGE`, etc.)
  - `SetActor`: Handles Set operations (`SADD`, `SMEMBERS`, etc.)
  - `PubSubActor`: Handles Pub/Sub channels
- **Transactions**: Supports `MULTI`/`EXEC` transactions, queuing commands for atomic execution.

## Supported Features

### Strings

Standard key-value operations are supported, including expiration and bitwise operations.

**Supported Commands:**
- `SET`, `GET`, `GETSET`, `APPEND`, `STRLEN`
- `INCR`, `DECR`, `INCRBY`
- `SETEX`, `PSETEX`, `TTL`, `PTTL`
- `GETRANGE`, `SETRANGE`
- `MGET`, `MSET` (via multiple actor calls)

### Hashes

Hash maps are implemented as `HashActor`s containing a `HashMap<String, String>`.

**Supported Commands:**
- `HSET`, `HGET`, `HMSET`, `HMGET`, `HGETALL`
- `HDEL`, `HEXISTS`
- `HKEYS`, `HVALS`, `HLEN`
- `HINCRBY`

### Lists

Lists are implemented as `ListActor`s using a double-ended queue vector.

**Supported Commands:**
- `LPUSH`, `RPUSH`, `LPOP`, `RPOP`
- `LRANGE`, `LINDEX`, `LLEN`
- `LSET`, `LREM`, `LTRIM`, `LINSERT`
- `BLPOP`, `BRPOP` (blocking operations supported via async await)

### Sets

Sets are implemented as `SetActor`s using a `HashSet`.

**Supported Commands:**
- `SADD`, `SREM`, `SISMEMBER`
- `SMEMBERS`, `SCARD`
- `SPOP`, `SRANDMEMBER`

### Pub/Sub

Complete Publish/Subscribe support using `PubSubActor`s.

**Supported Commands:**
- `PUBLISH`
- `SUBSCRIBE`
- `UNSUBSCRIBE`
- `PSUBSCRIBE` (Pattern matching)

### Transactions

ACID-compliant transactions are supported within a single actor or across multiple actors using Orbit's distributed transaction manager.

**Supported Commands:**
- `MULTI`: Start transaction
- `EXEC`: Execute transaction
- `DISCARD`: Abort transaction
- `WATCH`: Optimistic locking

### Connection Management

- `PING`: Keep-alive
- `ECHO`: Debugging
- `QUIT`: Close connection
- `SELECT`: namespace switching (mapped to actor namespaces)

## Configuration

Redis protocol settings in `orbit.toml`:

```toml
[redis]
enabled = true
port = 6379
host = "0.0.0.0"
workers = 4
```

## Implementation Notes

- **Durability**: All actor state changes are persisted to the configured storage backend (e.g., RocksDB or similar, depending on Orbit configuration).
- **Scalability**: Since each key can be an independent actor, data is naturally sharded across the cluster.
- **Consistency**: Strong consistency for single-key operations; eventual consistency for some cross-actor operations unless wrapped in a transaction.
