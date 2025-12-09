# Protocol Comparison and Selection Guide

## Overview

Orbit-RS is a multi-model database engine that speaks the native languages of many popular databases. This unique capability allows you to use the drivers, tools, and libraries you already know while running on a single, unified data platform.

This document guides you through the supported protocols and helps you choose the right interface for your specific use case.

## Supported Protocols

Orbit-RS currently supports the following wire protocols:

1.  **SQL**: PostgreSQL and MySQL compatibility.
2.  **NoSQL Document**: MongoDB compatibility.
3.  **Key-Value & Data Structures**: Redis (RESP) compatibility.
4.  **Graph**: Cypher (Neo4j) and AQL (ArangoDB) compatibility.
5.  **Wide Column**: CQL (Cassandra/ScyllaDB) compatibility.
6.  **Native**: OrbitQL (ANSI SQL superset with graph, vector, and ML extensions) and gRPC for direct, high-performance access.

## Comparison Matrix

| Feature | SQL (Postgres/MySQL) | MongoDB | Redis | Graph (Cypher/AQL) | CQL (Cassandra) | OrbitQL |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Primary Data Model** | Relational Tables | JSON Documents | Key-Value, Lists, Sets | Nodes & Relationships | Wide Column / Tabular | Multi-Model (SQL + Graph + Vector) |
| **Best For** | Financials, CRM, ERP, structured data integrity. | CMS, Catalogs, Rapid Prototyping, flexible attributes. | Caching, Real-time Sessions, Leaderboards, Pub/Sub. | Social Networks, Fraud Detection, Recommendation Engines. | IoT Telemetry, Logs, High-volume writes, Timeseries. | AI/ML apps, unified queries across models, real-time subscriptions. |
| **Schema** | Rigid (Schema-on-write) | Flexible (Schema-on-read) | Schemaless | Flexible Properties | Rigid (Partition Keys) | Flexible (supports all) |
| **Query Complexity** | High (Joins, Aggregations) | Moderate (Pipelines) | Low (Key-based) | High (Pattern Matching) | Low (Key-based lookups) | Very High (SQL + Graph + ML) |
| **ACID Guarantees** | Strong | Per-Document | Per-Command/Multi | ACID Compliant | Eventual / Tunable | Strong |
| **SQL Compatibility** | Native | None | None | None | SQL-like | ANSI SQL + PostgreSQL + MySQL |
| **Orbit Status** | Stable | Stable | Stable | Beta | Beta | Stable |

## Deep Dive by Protocol

### 1. SQL (PostgreSQL & MySQL)
The backbone of business applications. Use Orbit's SQL interface when data integrity and complex relationships are paramount.
- **Use Cases**: User accounts, billing ledgers, inventory management.
- **Tools**: `psql`, `mysql`, JDBC/ODBC, DBeaver, Tableau.

### 2. MongoDB
Ideal for content management and scenarios where the data structure evolves rapidly. Orbit allows you to store complex nested documents without strict schema migrations.
- **Use Cases**: Product catalogs, user profiles, blog posts, configuration management.
- **Tools**: `mongosh`, MongoDB Compass, Mongoose ODM.

### 3. Redis
The fastest lane for hot data. Orbit implements the RESP protocol to serve as an in-memory or persisted cache and data structure store.
- **Use Cases**: Session store, shopping carts, real-time analytics, job queues, geospatial indexes.
- **Tools**: `redis-cli`, Jedis, StackExchange.Redis.

### 4. Graph (Cypher & AQL)
Unlock the value of connections. While SQL joins can model relationships, Graph languages like Cypher efficiently traverse deep hierarchies and complex webs of data.
- **Use Cases**: "People also bought", fraud rings, network topology, permission inheritance.
- **Tools**: `cypher-shell`, Neo4j Browser (via Bolt), ArangoDB Web UI.

### 5. CQL (Cassandra Query Language)
Designed for massive write throughput and timeseries data.
- **Use Cases**: Sensor readings (IoT), server logs, tick data, chat history.
- **Tools**: `cqlsh`, Datastax Drivers.

### 6. OrbitQL (Native)
OrbitRS's native query language—an ANSI SQL superset with PostgreSQL/MySQL compatibility plus graph, vector, and ML extensions.
- **Use Cases**: AI-powered applications, multi-model queries, real-time subscriptions, unified analytics.
- **Tools**: `orbit-cli`, any PostgreSQL client (wire-compatible), OrbitRS SDKs.
- **Key Advantage**: Use familiar SQL syntax while gaining graph traversal, vector search, and ML inference in a single query.

## Polyglot Architecture: The Orbit Advantage

Building a modern application typically requires a "Polyglot Persistence" architecture—using Postgres for users, Redis for sessions, and Elastic/Mongo for search. Usually, this means operating 3+ different database clusters.

**With Orbit-RS, you run one engine.**

### Example: E-Commerce Architecture

Instead of three separate infrastructure pieces, you connect to different ports on the same Orbit cluster:

1.  **Product Catalog**: The backend connects via **MongoDB** driver to store flexible product attributes (colors, sizes).
2.  **Transactions**: The checkout microservice connects via **PostgreSQL** driver to ensure ACID compliance for orders.
3.  **Shopping Cart**: The frontend adds items to a cart stored via **Redis** commands for low-latency updates.
4.  **Recommendations**: A background job runs **Cypher** queries to analyze purchase history and update "Recommended Products".

All this data lives in the same unified storage layer, simplifying backups, scalability, and consistency.

## Multi-Protocol Integration Patterns

### Pattern 1: Write SQL, Cache in Redis
**Use Case**: Speed up read-heavy applications

```
┌─────────────┐     Write     ┌──────────────────────────────────┐
│  Application│──────────────▶│         OrbitRS                  │
│             │               │  ┌──────────┐   ┌──────────┐     │
│             │◀──Read Cache──│  │  Redis   │◀──│PostgreSQL│     │
└─────────────┘               │  │  :6379   │   │  :5432   │     │
                              │  └──────────┘   └──────────┘     │
                              └──────────────────────────────────┘
```

**Example Workflow**:
```sql
-- PostgreSQL: Authoritative write
INSERT INTO products (id, name, price) VALUES ('SKU-001', 'Widget', 29.99);
```
```redis
-- Redis: Cache for fast reads
SET product:SKU-001 '{"name":"Widget","price":29.99}' EX 3600
```

### Pattern 2: Time-Series + Real-Time Dashboard
**Use Case**: IoT monitoring with live updates

```
┌─────────────┐     Events    ┌──────────────────────────────────┐
│   Sensors   │──────────────▶│         OrbitRS                  │
│             │               │  ┌──────────┐   ┌──────────┐     │
└─────────────┘               │  │   CQL    │   │  Redis   │     │
                              │  │  :9042   │   │  :6379   │     │
┌─────────────┐               │  │(history) │   │ (live)   │     │
│  Dashboard  │◀─Live Updates─│  └──────────┘   └──────────┘     │
└─────────────┘               └──────────────────────────────────┘
```

**Example Workflow**:
```cql
-- CQL: Historical time-series
INSERT INTO sensor_readings (sensor_id, reading_time, value)
VALUES ('TEMP-001', toTimestamp(now()), 72.5);
```
```redis
-- Redis: Real-time pub/sub
PUBLISH sensor:TEMP-001 '{"value":72.5,"time":"2024-12-09T20:00:00Z"}'
```

### Pattern 3: SQL + Graph for Fraud Detection
**Use Case**: Transaction monitoring with relationship analysis

```
┌─────────────┐  Transaction  ┌──────────────────────────────────┐
│   Banking   │──────────────▶│         OrbitRS                  │
│    App      │               │  ┌──────────┐   ┌──────────┐     │
│             │◀─Fraud Alert──│  │PostgreSQL│   │  Cypher  │     │
└─────────────┘               │  │  :5432   │   │  :7687   │     │
                              │  │ (txns)   │   │ (rings)  │     │
                              │  └──────────┘   └──────────┘     │
                              └──────────────────────────────────┘
```

**Example Workflow**:
```sql
-- PostgreSQL: Record transaction
INSERT INTO transactions (id, account_id, amount, merchant)
VALUES ('TXN-001', 'ACC-123', 500.00, 'MERCHANT-456');
```
```cypher
// Cypher: Check for fraud ring
MATCH (a:Account)-[:TRANSFERRED_TO*1..3]->(target:Account)
WHERE a.id = 'ACC-123' AND target.flagged = true
RETURN COUNT(*) > 0 AS is_suspicious
```

### Pattern 4: Document Store + Search
**Use Case**: Content management with full-text search

```
┌─────────────┐    Content    ┌──────────────────────────────────┐
│     CMS     │──────────────▶│         OrbitRS                  │
│             │               │  ┌──────────┐   ┌──────────┐     │
│             │◀───Search─────│  │ MongoDB  │   │PostgreSQL│     │
└─────────────┘               │  │  :27017  │   │  :5432   │     │
                              │  │(content) │   │  (FTS)   │     │
                              └──────────────────────────────────┘
```

## OrbitQL: ANSI SQL Superset

OrbitQL is OrbitRS's native query language. It's built on **ANSI SQL** with full **PostgreSQL** and **MySQL** syntax compatibility, plus powerful extensions for modern workloads.

### Standard SQL Compatibility

OrbitQL supports all standard SQL operations:

```sql
-- Standard ANSI SQL works as expected
SELECT u.name, COUNT(o.id) AS order_count, SUM(o.total) AS revenue
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
WHERE u.created_at >= '2024-01-01'
GROUP BY u.id, u.name
HAVING COUNT(o.id) > 5
ORDER BY revenue DESC
LIMIT 100;

-- PostgreSQL-style CTEs
WITH monthly_sales AS (
    SELECT DATE_TRUNC('month', order_date) AS month,
           SUM(total) AS revenue
    FROM orders
    GROUP BY 1
)
SELECT month, revenue,
       LAG(revenue) OVER (ORDER BY month) AS prev_month,
       revenue - LAG(revenue) OVER (ORDER BY month) AS growth
FROM monthly_sales;

-- MySQL-style queries also work
SELECT * FROM products
WHERE name LIKE '%widget%'
ORDER BY price ASC
LIMIT 10 OFFSET 20;
```

### OrbitQL Extensions

Beyond standard SQL, OrbitQL adds:

| Extension | Syntax | Use Case |
|-----------|--------|----------|
| Graph Traversal | `->edge->node`, `<-edge<-node` | Relationship queries |
| Vector Search | `<=>`, `vector::similarity::cosine()` | Semantic similarity |
| ML Integration | `ml::predict()`, `ml::embed_text()` | In-query inference |
| Live Queries | `LIVE SELECT...DIFF` | Real-time subscriptions |
| Record Links | `table:id` | Direct record references |
| Graph Functions | `graph::shortest_path()` | Graph algorithms |

## OrbitRS-Specific Features

OrbitRS extends standard protocols with powerful capabilities:

### Vector Search (All Protocols)
Semantic similarity search for ML applications.

```sql
-- PostgreSQL (pgvector syntax)
SELECT id, name, embedding <=> '[0.1,0.2,0.3]'::vector AS distance
FROM products ORDER BY distance LIMIT 10;
```
```redis
-- Redis (FT.SEARCH with vectors)
FT.SEARCH idx:products "*=>[KNN 10 @embedding $query_vec]"
```
```mongodb
// MongoDB ($vectorSearch)
db.products.aggregate([{
  $vectorSearch: { vector: [0.1, 0.2, 0.3], path: "embedding", numCandidates: 100, limit: 10 }
}])
```

### ML Model Integration (OrbitQL)
Run ML inference directly in queries.

```sql
-- Predict churn risk
SELECT user_id,
       ml::predict('churn_model', {days_inactive: days_since_login, ...}) AS risk
FROM users WHERE risk > 0.7;
```

### Live Queries (OrbitQL)
Real-time streaming query results.

```sql
-- Subscribe to changes
LIVE SELECT * FROM orders WHERE status = 'PENDING' DIFF;
```

### Graph Traversal (OrbitQL)
Native graph operations in SQL-like syntax.

```sql
-- Find all connections within 3 hops
SELECT ->follows->user.->follows->user.name AS friends_of_friends
FROM users:user_123;
```

## Performance Characteristics

| Protocol | Read Latency | Write Latency | Throughput | Best For |
|----------|-------------|---------------|------------|----------|
| Redis | < 1ms | < 1ms | 1M+ ops/sec | Hot data, sessions |
| PostgreSQL | 1-10ms | 5-50ms | 50K+ TPS | Transactions, joins |
| MySQL | 1-10ms | 5-50ms | 50K+ TPS | Web apps, LAMP |
| MongoDB | 1-5ms | 2-20ms | 100K+ ops/sec | Documents, catalogs |
| CQL | 2-10ms | 1-5ms | 500K+ writes/sec | Time-series, logs |
| Cypher | 5-50ms | 10-100ms | 10K+ ops/sec | Graph traversal |
| OrbitQL (SQL) | 1-10ms | 5-50ms | 50K+ TPS | Standard queries |
| OrbitQL (Graph) | 5-50ms | 10-50ms | 20K+ ops/sec | Graph + SQL combined |
| OrbitQL (Vector) | 5-20ms | 10-50ms | 30K+ ops/sec | Similarity search |
| OrbitQL (ML) | 10-100ms | N/A | 10K+ ops/sec | In-query inference |

## Protocol Selection Flowchart

```
Start
  │
  ▼
┌─────────────────────────────────────┐
│ What is your primary data pattern?  │
└─────────────────────────────────────┘
  │
  ├── Structured tables with relationships ──▶ PostgreSQL/MySQL or OrbitQL
  │
  ├── Flexible JSON documents ──▶ MongoDB
  │
  ├── Key-value with sub-millisecond access ──▶ Redis
  │
  ├── High-volume time-series writes ──▶ CQL (Cassandra)
  │
  ├── Complex relationship traversal ──▶ Cypher/AQL or OrbitQL
  │
  ├── Vector/similarity search ──▶ OrbitQL
  │
  ├── ML inference in queries ──▶ OrbitQL
  │
  ├── Real-time subscriptions ──▶ OrbitQL (LIVE queries)
  │
  └── Existing PostgreSQL/MySQL app ──▶ OrbitQL (drop-in compatible)
```

### When to Choose OrbitQL Over PostgreSQL/MySQL

| Scenario | Use PostgreSQL/MySQL | Use OrbitQL |
|----------|---------------------|-------------|
| Existing app migration | ✓ Drop-in compatible | ✓ Drop-in compatible |
| Standard SQL queries | ✓ Native | ✓ Native (same syntax) |
| Need graph traversal | ✗ Requires joins | ✓ Native `->` syntax |
| Need vector search | ✗ Requires extension | ✓ Built-in |
| Need ML inference | ✗ External service | ✓ `ml::predict()` |
| Need real-time updates | ✗ Polling required | ✓ `LIVE SELECT` |
| Third-party tool compatibility | ✓ Wide ecosystem | ✓ PostgreSQL wire-compatible |

## Migration Guide

### From Single-Protocol to Multi-Protocol

**Step 1**: Identify data patterns in your application
```
User accounts → PostgreSQL (ACID)
Session data → Redis (speed)
Audit logs → CQL (volume)
Recommendations → Cypher (relationships)
```

**Step 2**: Create appropriate schemas in each protocol

**Step 3**: Update connection strings to point to OrbitRS
```
# All pointing to same OrbitRS cluster
POSTGRES_URL=postgres://orbit:5432/mydb
REDIS_URL=redis://orbit:6379
CASSANDRA_HOSTS=orbit:9042
```

**Step 4**: Gradually migrate writes, then reads

### From Legacy Database

| From | To (OrbitRS) | Migration Notes |
|------|--------------|-----------------|
| PostgreSQL | PostgreSQL :5432 | Direct compatible, add vector extension |
| MySQL | MySQL :3306 | Direct compatible |
| Redis | Redis :6379 | Direct compatible, gains persistence |
| MongoDB | MongoDB :27017 | Direct compatible |
| Cassandra | CQL :9042 | Direct compatible |
| Neo4j | Cypher :7687 | Bolt protocol compatible |

## Industry-Specific Recommendations

### Healthcare
- **Patient Records**: PostgreSQL (HIPAA compliance, ACID)
- **Real-time Vitals**: Redis (pub/sub, low latency)
- **Audit Trail**: CQL (immutable, high volume)
- **Care Team Graph**: Cypher (relationships)
- **Similar Patients**: OrbitQL (vector search)

### Banking/Finance
- **Transactions**: PostgreSQL (ACID, regulatory)
- **Sessions/OTP**: Redis (TTL, speed)
- **Transaction History**: CQL (time-series)
- **Fraud Rings**: Cypher (graph analysis)
- **Risk Scoring**: OrbitQL (ML integration)

### Logistics
- **Orders/Inventory**: PostgreSQL (transactions)
- **Fleet Tracking**: Redis (geospatial)
- **Shipment Events**: CQL (time-series)
- **Supply Chain**: Cypher (network optimization)
- **ETA Prediction**: OrbitQL (ML models)

### Media/Streaming
- **Content Catalog**: PostgreSQL/MongoDB
- **Playback Sessions**: Redis (state, position)
- **Viewing History**: CQL (time-series)
- **Recommendations**: Cypher (collaborative)
- **Content Similarity**: OrbitQL (vectors)

## See Also

- [Healthcare Examples](healthcare/README.md)
- [Banking Examples](banking/README.md)
- [Logistics Examples](logistics/README.md)
- [Media Examples](media/README.md)
- [Protocol-Specific Examples](protocol/README.md)
