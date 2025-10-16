# Configurable Storage Architecture 

## Overview

The Orbit integrated server now supports configurable storage backends for both Redis and PostgreSQL, allowing you to choose between:

- **Persistent Mode**: Direct RocksDB storage for maximum performance and true persistence
- **Actor Mode**: Traditional actor-based storage for integration with Orbit's actor system

## Configuration

Set these environment variables to configure storage modes:

```bash
# Configure Redis mode (default: persistent)
export ORBIT_REDIS_MODE=persistent|actor

# Configure PostgreSQL mode (default: persistent) 
export ORBIT_POSTGRES_MODE=persistent|actor
```

## Usage Examples

### 1. Default - Both Persistent (Maximum Performance)
```bash
cargo run --package orbit-server --example integrated-server
# Redis: Persistent RocksDB storage with TTL support
# PostgreSQL: Persistent RocksDB table storage
```

### 2. Both Actor Mode (Full Integration)
```bash
ORBIT_REDIS_MODE=actor ORBIT_POSTGRES_MODE=actor \
cargo run --package orbit-server --example integrated-server
# Redis: Actor-based in-memory storage 
# PostgreSQL: Actor-based in-memory storage
```

### 3. Mixed Configurations
```bash
# Redis persistent, PostgreSQL actor
ORBIT_REDIS_MODE=persistent ORBIT_POSTGRES_MODE=actor \
cargo run --package orbit-server --example integrated-server

# Redis actor, PostgreSQL persistent  
ORBIT_REDIS_MODE=actor ORBIT_POSTGRES_MODE=persistent \
cargo run --package orbit-server --example integrated-server
```

## Architecture Comparison

### Redis Storage Architectures

#### Persistent Mode (Default)
```
Redis Client → PersistentRespServer → RedisDataProvider → RocksDB
```
- **Data Flow**: Direct storage calls, no actors involved
- **Performance**: High - no actor message passing overhead
- **Persistence**: True persistence across restarts
- **TTL Support**: Native RocksDB-based expiration with background cleanup

#### Actor Mode  
```
Redis Client → RespServer → StringCommands → LocalActorRegistry → Actors
```
- **Data Flow**: Actor-based storage via local registry
- **Performance**: Good - actor message passing overhead
- **Persistence**: Depends on actor persistence configuration
- **TTL Support**: Actor-based timers and expiration

### PostgreSQL Storage Architectures

#### Persistent Mode (Default)
```
PostgreSQL Client → PostgresServer → QueryEngine → RocksDbTableStorage → RocksDB
```
- **Data Flow**: Direct table storage, no actors for table data
- **Performance**: High - optimized for SQL workloads
- **Persistence**: True persistence across restarts
- **SQL Support**: Full DDL/DML support with persistent tables

#### Actor Mode
```
PostgreSQL Client → PostgresServer → QueryEngine → In-Memory HashMap
```
- **Data Flow**: In-memory table storage
- **Performance**: Very fast for small datasets
- **Persistence**: Lost on restart (in-memory only)
- **SQL Support**: Full DDL/DML support with ephemeral tables

## Use Cases

### When to Use Persistent Mode
- **Production deployments** requiring data durability
- **High-throughput** workloads needing maximum performance  
- **Data persistence** across server restarts is critical
- **Large datasets** that don't fit in memory

### When to Use Actor Mode
- **Development/testing** with ephemeral data needs
- **Integration scenarios** where data needs to interact with Orbit actors
- **Experimental workloads** exploring actor-based patterns
- **Small datasets** where in-memory speed is preferred

## Configuration Matrix

| Redis Mode | PostgreSQL Mode | Redis Storage | PostgreSQL Storage | Use Case |
|------------|-----------------|---------------|-------------------|----------|
| persistent | persistent | RocksDB | RocksDB | **Production** - Maximum performance and durability |
| actor | actor | Actors | In-Memory | **Development** - Full actor integration |
| persistent | actor | RocksDB | In-Memory | **Hybrid** - Persistent cache + ephemeral tables |
| actor | persistent | Actors | RocksDB | **Hybrid** - Actor patterns + persistent data |

## Performance Characteristics

### Throughput (ops/second)
- **Persistent Mode**: ~50k+ ops/sec (direct RocksDB)
- **Actor Mode**: ~20-30k ops/sec (actor message passing)

### Memory Usage
- **Persistent Mode**: Lower memory usage (data in RocksDB)
- **Actor Mode**: Higher memory usage (data in actor state)

### Startup Time
- **Persistent Mode**: Slightly slower (RocksDB initialization)
- **Actor Mode**: Faster (in-memory initialization)

## Storage Locations

When persistent mode is enabled, data is stored in:

```
./orbit_integrated_data/
├── redis/           # Redis persistent data (RocksDB)
├── postgresql/      # PostgreSQL table data (RocksDB) 
└── ...             # Other Orbit server persistence data
```

## Future Enhancements

- **Runtime switching**: Ability to switch modes without server restart
- **Per-table configuration**: PostgreSQL tables with mixed storage backends
- **Hybrid actors**: Actors that can access persistent storage directly
- **Cross-protocol integration**: Redis keys that trigger actor operations