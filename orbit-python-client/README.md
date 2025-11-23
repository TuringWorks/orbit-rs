# Orbit-RS Python Client

A unified Python client library for accessing Orbit-RS multi-model database through all supported protocols.

## Features

- **Multi-Protocol Support**: Access Orbit-RS through any supported protocol
  - PostgreSQL (SQL) - Port 5432
  - MySQL (SQL) - Port 3306
  - CQL/Cassandra - Port 9042
  - Redis/RESP - Port 6379
  - Cypher/Bolt (Neo4j) - Port 7687
  - AQL/ArangoDB - Port 8529
  - MCP (Model Context Protocol) - Port 8080

- **Unified API**: Single client interface for all protocols
- **Type Safety**: Full type hints and Pydantic models
- **Context Managers**: Automatic connection management
- **Protocol-Specific Methods**: Convenience methods for each protocol

## Installation

```bash
pip install orbit-python-client
```

### Optional Dependencies

For async support:
```bash
pip install orbit-python-client[async]
```

For development:
```bash
pip install orbit-python-client[dev]
```

## Quick Start

### PostgreSQL

```python
from orbit_client import OrbitClient

# Connect to PostgreSQL
client = OrbitClient.postgres(
    host="127.0.0.1",
    port=5432,
    username="orbit",
    password="",
    database="postgres"
)

# Execute SQL query
results = client.execute("SELECT * FROM users WHERE age > %s", params=(18,))
for row in results:
    print(row)

# Context manager (automatic connection handling)
with OrbitClient.postgres() as client:
    results = client.execute("SELECT id, name FROM users")
    print(results)
```

### MySQL

```python
from orbit_client import OrbitClient

client = OrbitClient.mysql(
    host="127.0.0.1",
    port=3306,
    database="orbit"
)

results = client.execute("SELECT * FROM products WHERE price > %s", params=(100,))
```

### Redis

```python
from orbit_client import OrbitClient

client = OrbitClient.redis(host="127.0.0.1", port=6379)

# Using convenience methods
client.set("user:1", "John Doe", ex=3600)  # Set with expiration
value = client.get("user:1")
client.delete("user:1")

# Using execute for any Redis command
client.execute("SET", args=("key", "value"))
client.execute("GET", args=("key",))
client.execute("HSET", args=("user:1", "name", "John", "age", "30"))
```

### CQL/Cassandra

```python
from orbit_client import OrbitClient

client = OrbitClient.cql(
    host="127.0.0.1",
    port=9042,
    keyspace="my_keyspace"
)

results = client.execute(
    "SELECT * FROM users WHERE user_id = %s",
    params=("123",)
)
```

### Cypher/Bolt (Neo4j)

```python
from orbit_client import OrbitClient

client = OrbitClient.cypher(
    host="127.0.0.1",
    port=7687,
    database="neo4j"
)

# Create nodes
client.execute(
    "CREATE (n:Person {name: $name, age: $age})",
    params={"name": "Alice", "age": 30}
)

# Query nodes
results = client.execute(
    "MATCH (n:Person) WHERE n.age > $age RETURN n",
    params={"age": 25}
)
```

### AQL/ArangoDB

```python
from orbit_client import OrbitClient

client = OrbitClient.aql(
    host="127.0.0.1",
    port=8529,
    database="_system"
)

results = client.execute(
    "FOR doc IN users FILTER doc.age > @age RETURN doc",
    bind_vars={"age": 18}
)
```

### MCP (Model Context Protocol)

```python
from orbit_client import OrbitClient

client = OrbitClient.mcp(
    host="127.0.0.1",
    port=8080
)

# Execute natural language query
result = client.execute(
    "tools/call",
    params={
        "name": "query_data",
        "arguments": {
            "query": "Show me all users from California"
        }
    }
)
```

## Advanced Usage

### Connection Management

```python
from orbit_client import OrbitClient

client = OrbitClient.postgres()

# Manual connection management
client.connect()
try:
    results = client.execute("SELECT * FROM users")
finally:
    client.disconnect()

# Using context manager (recommended)
with OrbitClient.postgres() as client:
    results = client.execute("SELECT * FROM users")
    # Connection automatically closed on exit
```

### Batch Operations

```python
from orbit_client import OrbitClient

client = OrbitClient.postgres()

# PostgreSQL batch insert
with client._protocol_client._connection.cursor() as cursor:
    cursor.executemany(
        "INSERT INTO users (name, email) VALUES (%s, %s)",
        [("Alice", "alice@example.com"), ("Bob", "bob@example.com")]
    )
    client._protocol_client._connection.commit()
```

### Error Handling

```python
from orbit_client import OrbitClient
import psycopg2

try:
    client = OrbitClient.postgres()
    results = client.execute("SELECT * FROM users")
except psycopg2.Error as e:
    print(f"Database error: {e}")
except Exception as e:
    print(f"Error: {e}")
finally:
    client.disconnect()
```

## Protocol-Specific Features

### PostgreSQL/MySQL

- Full SQL support (DDL, DML, DCL, TCL)
- Parameterized queries
- Transaction support
- Connection pooling (via underlying drivers)

### Redis

- All Redis commands supported
- Convenience methods: `get()`, `set()`, `delete()`, `keys()`
- Pub/Sub support (via underlying redis-py)
- Pipeline support

### CQL/Cassandra

- CQL 3.x support
- Prepared statements
- Batch operations
- Keyspace management

### Cypher/Bolt

- Full Cypher query language
- Transaction support
- Parameterized queries
- Graph operations

### AQL/ArangoDB

- AQL query language
- Document operations
- Graph traversal
- Bind variables

### MCP

- Natural language queries
- Schema discovery
- Tool execution
- Resource access

## Examples

See the `examples/` directory for complete examples:

- `examples/postgres_example.py` - PostgreSQL usage
- `examples/redis_example.py` - Redis usage
- `examples/cypher_example.py` - Cypher/Bolt usage
- `examples/multi_protocol_example.py` - Using multiple protocols

## Requirements

- Python 3.8+
- Protocol-specific drivers (automatically installed):
  - `psycopg2-binary` for PostgreSQL
  - `PyMySQL` for MySQL
  - `cassandra-driver` for CQL
  - `redis` for Redis
  - `neo4j` for Cypher/Bolt
  - `python-arango` for AQL
  - `httpx` for MCP

## License

BSD-3-Clause

## Links

- [Orbit-RS Documentation](https://turingworks.github.io/orbit-rs)
- [GitHub Repository](https://github.com/turingworks/orbit-rs)

