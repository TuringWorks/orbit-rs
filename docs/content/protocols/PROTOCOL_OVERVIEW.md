---
layout: default
title: "Protocol Overview"
subtitle: "Multi-protocol database support in Orbit-RS"
category: "protocols"
---

# Orbit-RS Protocol Support

Orbit-RS implements 9 native database protocols, allowing drop-in compatibility with existing database clients and tools.

---

## Protocol Status Summary

| Protocol | Port | Compatibility | Status |
|----------|------|---------------|--------|
| **PostgreSQL** | 5432 | 94% (PG18) | Production |
| **MySQL** | 3306 | 40% | Beta |
| **Redis RESP** | 6379 | 75% | Production |
| **CQL/Cassandra** | 9042 | 35% | Beta |
| **Cypher/Bolt** | 7687 | 70% | Production |
| **AQL** | 8529 | 60% | Beta |
| **MongoDB** | 27017 | 30% | Alpha |
| **HTTP REST** | 8080 | 100% | Production |
| **gRPC** | 50051 | 100% | Production |

---

## PostgreSQL Protocol (94% Compatible)

### Features
- Full DDL/DML support
- pgvector extension (95%)
- TimescaleDB functions (60%)
- Full-text search
- PostgreSQL 18 features

### PostgreSQL 18 Features
- OLD/NEW table references in RETURNING
- VIRTUAL generated columns
- MERGE with RETURNING
- WITHOUT OVERLAPS temporal constraints
- Range operators (8 types)
- Variable-length cancellation keys
- Protocol 3.2 negotiation

### Supported Functions (500+)
- Math: `abs`, `ceil`, `floor`, `round`, `sqrt`, `cbrt`, `factorial`, `gcd`, `lcm`
- String: `concat`, `substring`, `trim`, `upper`, `lower`, `replace`
- Date/Time: `now()`, `current_date`, `extract`, `date_trunc`
- Aggregates: `sum`, `avg`, `count`, `min`, `max`, `array_agg`
- Window: `row_number`, `rank`, `lead`, `lag`, `ntile`
- JSON: `jsonb_extract_path`, `jsonb_array_elements`, `->>`, `@>`
- Array: `array_agg`, `unnest`, `array_length`
- Full-text: `to_tsvector`, `to_tsquery`, `ts_rank`, `@@`
- Sequence: `nextval`, `currval`, `setval`, `lastval`

### Connection Example
```bash
psql -h localhost -p 5432 -U orbit -d orbit
```

---

## Redis RESP Protocol (75% Compatible)

### Supported Command Families
- **Strings**: GET, SET, MGET, MSET, INCR, DECR, APPEND
- **Hashes**: HGET, HSET, HMGET, HMSET, HDEL, HGETALL
- **Lists**: LPUSH, RPUSH, LPOP, RPOP, LRANGE, LLEN
- **Sets**: SADD, SREM, SMEMBERS, SINTER, SUNION
- **Sorted Sets**: ZADD, ZRANGE, ZSCORE, ZRANK
- **Streams**: XADD, XREAD, XGROUP, XREADGROUP, XACK
- **Pub/Sub**: SUBSCRIBE, PUBLISH, PSUBSCRIBE
- **ACL**: ACL LIST, SETUSER, GETUSER, DELUSER
- **Functions**: FUNCTION LOAD, LIST, DELETE, FCALL

### Extended Commands
- **Vector**: VECTOR.ADD, VECTOR.SEARCH, VECTOR.GET
- **Time Series**: TS.CREATE, TS.ADD, TS.RANGE, TS.GET
- **Graph**: GRAPH.QUERY, GRAPH.DELETE
- **Search**: FT.CREATE, FT.SEARCH, FT.ADD

### Connection Example
```bash
redis-cli -h localhost -p 6379
```

---

## CQL/Cassandra Protocol (35% Compatible)

### Features
- DDL: CREATE/ALTER/DROP KEYSPACE, TABLE
- DML: SELECT, INSERT, UPDATE, DELETE
- Collections: LIST, SET, MAP
- User-Defined Types
- Secondary indexes (SASI/SAI)
- Full-text search

### Connection Example
```bash
cqlsh localhost 9042
```

---

## Cypher/Bolt Protocol (70% Compatible)

### Features
- Bolt v4.4 protocol
- Full Cypher support
- Graph algorithms
- Procedures

### Supported Clauses
- MATCH, CREATE, MERGE
- DELETE, DETACH DELETE
- SET, REMOVE
- RETURN, WITH, WHERE
- ORDER BY, SKIP, LIMIT
- UNWIND, FOREACH, CASE

### Graph Algorithms
- PageRank
- Shortest path (Dijkstra, BFS)
- Community detection (Louvain)
- Centrality (Betweenness, Closeness)
- Similarity (Jaccard, Cosine)

### Connection Example
```bash
cypher-shell -a bolt://localhost:7687
```

---

## AQL Protocol (60% Compatible)

### Features
- FOR/FILTER/RETURN syntax
- Graph traversal
- Document operations
- Window functions

### Connection Example
```bash
arangosh --server.endpoint tcp://localhost:8529
```

---

## MongoDB Protocol (30% Compatible)

### Features
- Basic CRUD operations
- Document operations
- JavaScript support ($where, $function)

### Connection Example
```bash
mongosh mongodb://localhost:27017
```

---

## HTTP REST API (100% Complete)

### Endpoints
- `POST /query` - Execute queries
- `GET /tables` - List tables
- `GET /health` - Health check
- `GET /metrics` - Prometheus metrics

### Example
```bash
curl -X POST http://localhost:8080/query \
  -H "Content-Type: application/json" \
  -d '{"query": "SELECT * FROM users LIMIT 10"}'
```

---

## gRPC Protocol (100% Complete)

### Services
- ActorService - Actor management
- QueryService - Query execution
- ClusterService - Cluster operations
- StreamService - Streaming queries

### Connection Example
```bash
grpcurl -plaintext localhost:50051 list
```

---

## Configuration

### Port Configuration
```toml
# orbit-server.toml
[protocols]
postgresql_port = 5432
mysql_port = 3306
redis_port = 6379
cql_port = 9042
cypher_port = 7687
aql_port = 8529
mongodb_port = 27017
http_port = 8080
grpc_port = 50051
```

### Enable/Disable Protocols
```toml
[protocols.enabled]
postgresql = true
mysql = true
redis = true
cql = true
cypher = true
aql = false
mongodb = false
http = true
grpc = true
```

---

## Implementation Files

```
orbit/server/src/protocols/
├── postgres_wire/     # PostgreSQL protocol
├── mysql/             # MySQL protocol
├── resp/              # Redis RESP protocol
├── cql/               # CQL/Cassandra protocol
├── cypher/            # Cypher/Bolt protocol
├── aql/               # AQL protocol
├── mongodb/           # MongoDB protocol
├── rest/              # HTTP REST API
└── grpc/              # gRPC services
```

---

## Resources

- **Specifications**: See `/specifications/protocols/` for detailed compatibility specs
- **RFC**: [Multi-Protocol RFC](../rfcs/RFC_INDEX.md#rfc-006-multi-protocol-adapters)
- **Source**: `orbit/server/src/protocols/`
