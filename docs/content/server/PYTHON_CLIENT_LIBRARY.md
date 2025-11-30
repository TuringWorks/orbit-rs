# Orbit-RS Python Client Library

**Status**: ✅ Complete  
**Version**: 0.1.0

## Overview

The Orbit-RS Python client library provides a unified interface for accessing Orbit-RS multi-model database through all supported protocols. It wraps protocol-specific drivers and provides a clean, consistent API.

## Features

- ✅ **Multi-Protocol Support**: All 7 protocols supported
  - PostgreSQL (SQL) - Port 5432
  - MySQL (SQL) - Port 3306
  - CQL/Cassandra - Port 9042
  - Redis/RESP - Port 6379
  - Cypher/Bolt (Neo4j) - Port 7687
  - AQL/ArangoDB - Port 8529
  - MCP (Model Context Protocol) - Port 8080

- ✅ **Unified API**: Single `OrbitClient` class for all protocols
- ✅ **Type Safety**: Full type hints and Pydantic models
- ✅ **Context Managers**: Automatic connection management
- ✅ **Protocol-Specific Methods**: Convenience methods for each protocol
- ✅ **Error Handling**: Comprehensive error handling and connection management

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

# Connect to Orbit-RS PostgreSQL server
client = OrbitClient.postgres(
    host="127.0.0.1",
    port=5432,
    username="orbit",
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

## API Reference

### OrbitClient

Main client class for accessing Orbit-RS.

#### Class Methods

- `OrbitClient.postgres(**kwargs)` - Create PostgreSQL client
- `OrbitClient.mysql(**kwargs)` - Create MySQL client
- `OrbitClient.cql(**kwargs)` - Create CQL client
- `OrbitClient.redis(**kwargs)` - Create Redis client
- `OrbitClient.cypher(**kwargs)` - Create Cypher client
- `OrbitClient.aql(**kwargs)` - Create AQL client
- `OrbitClient.mcp(**kwargs)` - Create MCP client

#### Instance Methods

- `connect()` - Establish connection
- `disconnect()` - Close connection
- `is_connected()` - Check connection status
- `execute(query, **kwargs)` - Execute query/command
- `get(key)` - Get value (Redis only)
- `set(key, value, ex=None)` - Set value (Redis only)
- `delete(*keys)` - Delete keys (Redis only)
- `keys(pattern="*")` - Get keys (Redis only)

#### Properties

- `protocol` - Current protocol type
- `connected` - Connection status

## Examples

See `../orbit-python-client/examples/` for complete examples:

- `postgres_example.py` - PostgreSQL usage
- `redis_example.py` - Redis usage
- `cypher_example.py` - Cypher/Bolt usage
- `multi_protocol_example.py` - Using multiple protocols

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

## Package Structure

```
orbit-python-client/
├── orbit_client/
│   ├── __init__.py          # Package exports
│   ├── client.py            # Unified OrbitClient class
│   └── protocols.py          # Protocol-specific clients
├── examples/
│   ├── postgres_example.py
│   ├── redis_example.py
│   ├── cypher_example.py
│   └── multi_protocol_example.py
├── pyproject.toml          # Package configuration
├── README.md              # User documentation
└── .gitignore
```

## Development

### Setup

```bash
cd orbit-python-client
pip install -e ".[dev]"
```

### Testing

```bash
pytest
```

### Code Quality

```bash
black orbit_client/
ruff check orbit_client/
mypy orbit_client/
```

## Related Documentation

- [Protocol 100% Completion Report](PROTOCOL_100_PERCENT_COMPLETE.md)
- [Orbit-RS Architecture](architecture/ORBIT_ARCHITECTURE.md)
- [Quick Start Guide](quick_start.md)

---

**Last Updated**: November 2025

