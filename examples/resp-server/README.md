# RESP Server Example

This example demonstrates how to run a Redis-compatible server that accepts Redis protocol (RESP) commands and executes them on Orbit actors.

## Overview

The RESP server example creates a TCP server that:
- Listens on port 6380 for Redis protocol connections
- Accepts standard Redis commands (GET, SET, HGET, HSET, LPUSH, PUBLISH, etc.)
- Translates those commands to Orbit actor method invocations
- Returns Redis-compatible responses

## Running the Example

1. Start the RESP server:
```bash
cargo run --example resp-server
```

2. In another terminal, connect using redis-cli:
```bash
redis-cli -h 127.0.0.1 -p 6380
```

3. Try some Redis commands:
```redis
# Key-value operations
> SET mykey "hello world"
OK
> GET mykey
"hello world"

# Hash operations
> HSET myhash field1 "value1"
(integer) 1
> HGET myhash field1
"value1"

# List operations
> LPUSH mylist item1 item2
(integer) 2

# Pub/Sub operations
> PUBLISH mychannel "hello subscribers"
(integer) 0

# Connection commands
> PING
PONG
> ECHO "test message"
"test message"
```

## How It Works

### Actor Integration

The RESP server maps Redis commands to Orbit actors:

- **String commands** (GET, SET) → `KeyValueActor`
- **Hash commands** (HGET, HSET) → `HashActor`
- **List commands** (LPUSH, RPUSH) → `ListActor`
- **Pub/Sub commands** (PUBLISH, SUBSCRIBE) → `PubSubActor`

### Command Processing

1. Client sends Redis command via TCP connection
2. RESP codec parses the command into a `RespValue` array
3. `CommandHandler` identifies the command and extracts arguments
4. Handler gets the appropriate Orbit actor reference using `OrbitClient`
5. Handler invokes the corresponding actor method
6. Response is formatted as RESP and sent back to client

### Code Structure

```
examples/resp-server/
├── Cargo.toml          # Example dependencies
├── README.md           # This file
└── src/
    └── main.rs         # RESP server implementation
```

## Supported Commands

### ✅ Implemented Commands

- **Connection**: PING, ECHO, SELECT
- **Key-Value**: GET, SET (with expiration support)
- **Hash**: HGET, HSET
- **List**: LPUSH
- **Pub/Sub**: PUBLISH
- **Server**: INFO, DBSIZE, COMMAND

### 🚧 Partial/Placeholder Commands

Many Redis commands are parsed and accepted but return placeholder responses:
- DEL, EXISTS, TTL, EXPIRE
- HGETALL, HDEL, HEXISTS, HKEYS, HVALS, HLEN
- RPUSH, LPOP, RPOP, LRANGE, LLEN, LINDEX
- SUBSCRIBE, UNSUBSCRIBE, PSUBSCRIBE, PUNSUBSCRIBE
- FLUSHDB, KEYS

## Configuration

The example uses default configuration:
- **Host**: 127.0.0.1 (localhost)
- **Port**: 6380
- **Orbit Server**: Connects to local Orbit cluster

You can modify these settings in `main.rs`:

```rust
let addr = "127.0.0.1:6380";
let orbit_client = OrbitClientBuilder::new()
    .with_server_urls(vec!["http://127.0.0.1:50051".to_string()])
    .build()
    .await?;
```

## Error Handling

The server provides Redis-compatible error responses:
- Invalid command syntax → `ERR invalid command`
- Actor errors → `ERR actor error: <details>`
- Invocation failures → `ERR actor invocation failed: <details>`

## Performance Notes

This is a demonstration example. For production use, consider:
- Connection pooling for Orbit client
- Async connection handling optimization  
- Memory usage monitoring for large data sets
- Proper error recovery and failover logic

## Related Documentation

- [RESP Protocol Integration Guide](../../docs/protocols/RESP_INTEGRATION_COMPLETE.md)
- [Orbit-RS Protocol Adapters](../../docs/protocols/)
- [Main README](../../README.md#protocol-adapters)