# Protocol Adapters Integration Guide

This guide covers the integration of CQL (Cassandra) and MySQL protocol adapters with Orbit-RS.

## Overview

Orbit-RS now supports multiple database wire protocols, allowing clients from different ecosystems to connect and interact with the same underlying distributed storage system:

- **CQL (Cassandra Query Language)** - Port 9042
- **MySQL Wire Protocol** - Port 3306
- **PostgreSQL Wire Protocol** - Port 5432 (existing)

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    Client Applications                        │
├──────────────────────────────────────────────────────────────┤
│  cqlsh    │  MySQL Client  │  psql  │  Python/Node/Java/...  │
└─────┬──────────┬──────────────┬────────────────┬──────────────┘
      │          │              │                │
      │ CQL      │ MySQL        │ PostgreSQL     │ Any Driver
      │ Port     │ Port         │ Port           │
      │ 9042     │ 3306         │ 5432           │
      ▼          ▼              ▼                ▼
┌──────────────────────────────────────────────────────────────┐
│                   Protocol Adapters Layer                     │
├──────────────────────────────────────────────────────────────┤
│   CQL Adapter  │  MySQL Adapter  │  PostgreSQL Adapter        │
│  • Frame Codec │  • Packet Codec │  • Message Codec           │
│  • CQL Parser  │  • Auth Handler │  • Extended Protocol       │
│  • Type System │  • Type System  │  • Type System             │
└────────────┬──────────────┬────────────────┬──────────────────┘
             │              │                │
             ▼              ▼                ▼
┌──────────────────────────────────────────────────────────────┐
│                    Unified SQL Engine                         │
│                   (orbit-protocols/sql)                       │
├──────────────────────────────────────────────────────────────┤
│  • Query Parsing & Planning                                   │
│  • MVCC Transaction Management                                │
│  • Execution Strategies (Traditional/MVCC/Hybrid)             │
│  • Type Conversion & Validation                               │
└────────────┬─────────────────────────────────────────────────┘
             │
             ▼
┌──────────────────────────────────────────────────────────────┐
│                   Orbit Storage Engine                        │
│                    (orbit-engine)                             │
├──────────────────────────────────────────────────────────────┤
│  • Tiered Storage (Hot/Warm/Cold)                             │
│  • Columnar Format & Compression                              │
│  • Iceberg Integration                                        │
│  • Distributed Clustering (Raft)                              │
└──────────────────────────────────────────────────────────────┘
```

## Running Multiple Protocol Adapters

### Using the Multi-Protocol Server

The easiest way to run both CQL and MySQL adapters simultaneously:

```bash
cargo run --example multi_protocol_server
```

This will start:
- CQL adapter on `127.0.0.1:9042`
- MySQL adapter on `127.0.0.1:3306`

### Programmatic Usage

```rust
use orbit_protocols::cql::{CqlAdapter, CqlConfig};
use orbit_protocols::mysql::{MySqlAdapter, MySqlConfig};
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure adapters
    let cql_config = CqlConfig::default();
    let mysql_config = MySqlConfig::default();

    // Create adapters
    let cql_adapter = CqlAdapter::new(cql_config).await?;
    let mysql_adapter = MySqlAdapter::new(mysql_config).await?;

    // Run both concurrently
    let cql_handle = tokio::spawn(async move {
        cql_adapter.start().await
    });

    let mysql_handle = tokio::spawn(async move {
        mysql_adapter.start().await
    });

    // Wait for both
    tokio::try_join!(cql_handle, mysql_handle)?;

    Ok(())
}
```

## Client Connectivity

### CQL Clients

```bash
# cqlsh
cqlsh 127.0.0.1 9042

# Python (cassandra-driver)
from cassandra.cluster import Cluster
cluster = Cluster(['127.0.0.1'], port=9042)
session = cluster.connect()

# Node.js (cassandra-driver)
const cassandra = require('cassandra-driver');
const client = new cassandra.Client({
    contactPoints: ['127.0.0.1:9042']
});
```

### MySQL Clients

```bash
# mysql CLI
mysql -h 127.0.0.1 -P 3306 -u orbit

# Python (mysql-connector-python)
import mysql.connector
conn = mysql.connector.connect(
    host='127.0.0.1',
    port=3306,
    user='orbit'
)

# Node.js (mysql2)
const mysql = require('mysql2/promise');
const connection = await mysql.createConnection({
    host: '127.0.0.1',
    port: 3306,
    user: 'orbit'
});
```

## Type System Compatibility

All protocol adapters convert to Orbit's unified `SqlValue` type system:

| Orbit SqlType | CQL Type | MySQL Type | PostgreSQL Type |
|---------------|----------|------------|-----------------|
| SmallInt | SMALLINT | SMALLINT | SMALLINT |
| Integer | INT | INT | INTEGER |
| BigInt | BIGINT | BIGINT | BIGINT |
| Real | FLOAT | FLOAT | REAL |
| DoublePrecision | DOUBLE | DOUBLE | DOUBLE PRECISION |
| Text | TEXT | VARCHAR/TEXT | TEXT |
| Bytea | BLOB | BLOB | BYTEA |
| Boolean | BOOLEAN | TINYINT | BOOLEAN |
| Date | DATE | DATE | DATE |
| Time | TIME | TIME | TIME |
| Timestamp | TIMESTAMP | TIMESTAMP | TIMESTAMP |
| Json | - | JSON | JSON/JSONB |

## Cross-Protocol Data Access

One of the key features is that data written through one protocol can be read through another:

```sql
-- Write data using MySQL
mysql> CREATE TABLE users (id INT PRIMARY KEY, name TEXT);
mysql> INSERT INTO users VALUES (1, 'Alice'), (2, 'Bob');

-- Read the same data using CQL
cqlsh> SELECT * FROM users;

 id | name
----+-------
  1 | Alice
  2 | Bob

-- Or using PostgreSQL
psql> SELECT * FROM users;
 id | name
----+-------
  1 | Alice
  2 | Bob
```

## Testing

### Running Integration Tests

```bash
# Run all protocol integration tests
cargo test -p orbit-protocols --test protocol_integration_tests

# Run CQL-specific tests
cargo test -p orbit-protocols cql_tests

# Run MySQL-specific tests
cargo test -p orbit-protocols mysql_tests
```

### Manual Testing

**CQL:**
```bash
# Start server
cargo run --example cql_server_example

# In another terminal
cqlsh 127.0.0.1 9042
cqlsh> CREATE KEYSPACE test WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};
cqlsh> USE test;
cqlsh> CREATE TABLE users (id INT PRIMARY KEY, name TEXT);
cqlsh> INSERT INTO users (id, name) VALUES (1, 'Test');
cqlsh> SELECT * FROM users;
```

**MySQL:**
```bash
# Start server
cargo run --example mysql_server_example

# In another terminal
mysql -h 127.0.0.1 -P 3306 -u orbit
mysql> CREATE TABLE products (id INT PRIMARY KEY, name VARCHAR(100), price DECIMAL(10,2));
mysql> INSERT INTO products VALUES (1, 'Laptop', 999.99);
mysql> SELECT * FROM products;
```

## Performance Considerations

### Connection Pooling

Both adapters support high concurrency:
- Default max connections: 1000
- Async I/O with tokio
- Per-connection state isolation

### Query Optimization

- Use prepared statements for repeated queries
- Batch operations when possible
- Create indexes on frequently queried columns
- Use transactions for related operations

### Monitoring

Monitor adapter performance:

```rust
// Track connection count
// Track query latency
// Track error rates
// Track throughput
```

## Configuration

### CQL Adapter Configuration

```rust
let config = CqlConfig {
    listen_addr: "127.0.0.1:9042".parse()?,
    max_connections: 1000,
    authentication_enabled: false,  // Set to true for production
    protocol_version: 4,
};
```

### MySQL Adapter Configuration

```rust
let config = MySqlConfig {
    listen_addr: "127.0.0.1:3306".parse()?,
    max_connections: 1000,
    authentication_enabled: false,  // Set to true for production
    server_version: "8.0.0-Orbit".to_string(),
};
```

## Security

### Authentication

Both adapters support authentication:

**CQL:**
- SASL authentication
- Username/password authentication
- Custom authentication plugins

**MySQL:**
- mysql_native_password (SHA256-based)
- mysql_clear_password
- caching_sha2_password (planned)

### TLS/SSL

TLS support is planned for both adapters:
- Certificate-based authentication
- Encrypted connections
- Mutual TLS

## Troubleshooting

### Connection Issues

**Problem:** Can't connect to adapter

**Solutions:**
1. Check if server is running: `netstat -an | grep <PORT>`
2. Check firewall rules
3. Verify correct port and host
4. Check logs for errors

### Type Conversion Errors

**Problem:** Data type mismatch

**Solutions:**
1. Check type mappings table above
2. Ensure consistent types across protocols
3. Use explicit casts when needed

### Performance Issues

**Problem:** Slow queries

**Solutions:**
1. Use prepared statements
2. Create indexes
3. Analyze query plans
4. Batch operations
5. Enable connection pooling in client

## Related Documentation

- [CQL Protocol Adapter](./CQL_PROTOCOL_ADAPTER.md)
- [MySQL Protocol Adapter](./MYSQL_COMPLETE_DOCUMENTATION.md)
- [PostgreSQL Wire Protocol](../orbit/protocols/src/postgres_wire/README.md)
- [Orbit SQL Engine](../orbit/protocols/src/postgres_wire/sql/README.md)

## Examples


## Contributing

Contributions welcome! Areas for improvement:

- Additional protocol support (MongoDB, Redis, etc.)
- Performance optimizations
- Enhanced type conversions
- Security features (TLS, advanced auth)
- Monitoring and metrics
- Test coverage

## License

Apache 2.0 - See LICENSE file for details.
