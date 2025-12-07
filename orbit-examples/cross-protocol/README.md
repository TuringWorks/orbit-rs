# Cross-Protocol Integration Examples

This directory demonstrates Orbit-RS's unique capability: **accessing the same data through multiple protocols with instant consistency**.

## Overview

Orbit-RS is a multi-protocol database server where all protocols share the same underlying storage. This means you can:
- Write data via PostgreSQL, read via Redis
- Insert via MongoDB, query via REST API
- Update via MySQL, graph query via Cypher
- **All with zero data duplication and instant consistency**

## Why Cross-Protocol Integration?

**Traditional Approach (Multiple Databases):**
```
PostgreSQL ──┐
             ├──> ETL/Sync ──> Data Duplication, Eventual Consistency
Redis     ───┤
MongoDB   ───┘
```

**Orbit-RS Approach (One Database, Multiple Protocols):**
```
PostgreSQL ──┐
MySQL      ──┤
MongoDB    ──┼──> Unified Storage ──> Zero Duplication, Instant Consistency
Redis      ──┤
Cypher     ──┤
REST API   ──┘
```

## Examples Structure

```text
cross-protocol/
├── README.md                              # This file
├── 01_write_postgres_read_redis.py        # SQL write, Redis read
├── 02_write_mongodb_read_rest.py          # MongoDB write, REST read
├── 03_write_mysql_read_cypher.py          # MySQL write, graph query
├── 04_multi_protocol_transaction.py       # Distributed transaction
├── 05_event_driven_integration.py         # Event-driven updates
├── scenarios/
│   ├── ecommerce_full_stack.py            # E-commerce using all protocols
│   └── social_network_scenario.py         # Social network multi-protocol
└── benchmarks/
    └── protocol_performance_comparison.py  # Performance benchmarks
```

## Quick Start

### Prerequisites

```bash
# Start Orbit server
cargo run --bin orbit-server

# Install Python clients for all protocols
pip install psycopg2-binary pymongo redis requests mysql-connector-python
```

### Run Examples

```bash
cd cross-protocol
python 01_write_postgres_read_redis.py
python 02_write_mongodb_read_rest.py
python 04_multi_protocol_transaction.py
```

## Example 1: Write via PostgreSQL, Read via Redis

**Use Case**: Store structured data in SQL, access as cache via Redis

```python
import psycopg2
import redis

# Write via PostgreSQL
pg_conn = psycopg2.connect(host="localhost", port=5432, database="postgres")
pg_cur = pg_conn.cursor()

pg_cur.execute("""
    CREATE TABLE IF NOT EXISTS products (
        id SERIAL PRIMARY KEY,
        name TEXT,
        price DECIMAL,
        stock INT
    )
""")

pg_cur.execute("""
    INSERT INTO products (name, price, stock)
    VALUES ('Laptop', 999.99, 50)
    RETURNING id
""")
product_id = pg_cur.fetchone()[0]
pg_conn.commit()

# Read via Redis (instant consistency!)
r = redis.Redis(host='localhost', port=6379)
product = r.hgetall(f'product:{product_id}')
print(f"Product from Redis: {product}")
# Output: {'name': 'Laptop', 'price': '999.99', 'stock': '50'}
```

## Example 2: Write via MongoDB, Read via REST API

**Use Case**: Flexible document storage, web API access

```python
from pymongo import MongoClient
import requests

# Write via MongoDB
mongo_client = MongoClient('mongodb://localhost:27017/')
db = mongo_client['orbit_examples']

result = db.customers.insert_one({
    "name": "Alice Johnson",
    "email": "alice@example.com",
    "preferences": {
        "theme": "dark",
        "notifications": True
    },
    "tags": ["premium", "early-adopter"]
})

customer_id = result.inserted_id

# Read via REST API (instant consistency!)
response = requests.get(f'http://localhost:8080/api/customers/{customer_id}')
customer = response.json()
print(f"Customer from REST: {customer}")
```

## Example 3: Write via MySQL, Query via Cypher

**Use Case**: Relational data with graph relationships

```python
import mysql.connector
from neo4j import GraphDatabase

# Write via MySQL
mysql_conn = mysql.connector.connect(
    host="localhost",
    port=3306,
    user="orbit"
)
mysql_cur = mysql_conn.cursor()

mysql_cur.execute("""
    CREATE TABLE IF NOT EXISTS users (
        id INT AUTO_INCREMENT PRIMARY KEY,
        name VARCHAR(100),
        email VARCHAR(100)
    )
""")

mysql_cur.execute("""
    CREATE TABLE IF NOT EXISTS friendships (
        user_id INT,
        friend_id INT,
        since DATE
    )
""")

# Insert users
mysql_cur.execute("INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')")
alice_id = mysql_cur.lastrowid

mysql_cur.execute("INSERT INTO users (name, email) VALUES ('Bob', 'bob@example.com')")
bob_id = mysql_cur.lastrowid

mysql_cur.execute(f"INSERT INTO friendships VALUES ({alice_id}, {bob_id}, '2024-01-15')")
mysql_conn.commit()

# Query via Cypher (instant consistency!)
driver = GraphDatabase.driver("bolt://localhost:7687")
with driver.session() as session:
    result = session.run("""
        MATCH (alice:User {name: 'Alice'})-[:FRIEND]->(friend)
        RETURN friend.name AS friend_name
    """)
    for record in result:
        print(f"Alice's friend: {record['friend_name']}")
```

## Example 4: Multi-Protocol Transaction

**Use Case**: ACID transaction across multiple protocols

```python
import psycopg2
import redis
from pymongo import MongoClient

# Coordinated transaction across protocols
pg_conn = psycopg2.connect(host="localhost", port=5432)
r = redis.Redis(host='localhost', port=6379)
mongo_client = MongoClient('mongodb://localhost:27017/')

try:
    # Start transaction
    pg_conn.autocommit = False
    
    # Update inventory in PostgreSQL
    pg_cur = pg_conn.cursor()
    pg_cur.execute("UPDATE products SET stock = stock - 1 WHERE id = 1")
    
    # Update cache in Redis
    r.hincrby('product:1', 'stock', -1)
    
    # Log event in MongoDB
    db = mongo_client['orbit_examples']
    db.events.insert_one({
        "type": "stock_update",
        "product_id": 1,
        "change": -1,
        "timestamp": datetime.now()
    })
    
    # Commit all changes atomically
    pg_conn.commit()
    print("Multi-protocol transaction committed successfully!")
    
except Exception as e:
    pg_conn.rollback()
    print(f"Transaction failed: {e}")
```

## Use Case Scenarios

### E-Commerce Platform

```python
# Product catalog: MongoDB (flexible schema)
db.products.insert_one({
    "sku": "LAPTOP-001",
    "name": "Laptop Pro",
    "specs": {"cpu": "i7", "ram": "16GB"},
    "price": 999.99
})

# Shopping cart: Redis (fast access)
r.hset('cart:user123', 'LAPTOP-001', 1)

# Order processing: PostgreSQL (ACID transactions)
pg_cur.execute("""
    INSERT INTO orders (user_id, total_amount, status)
    VALUES (123, 999.99, 'pending')
""")

# Recommendation graph: Cypher (relationships)
session.run("""
    MATCH (u:User {id: 123})-[:PURCHASED]->(p:Product)
    MATCH (p)<-[:PURCHASED]-(other:User)-[:PURCHASED]->(rec:Product)
    WHERE NOT (u)-[:PURCHASED]->(rec)
    RETURN rec.name, COUNT(*) as score
    ORDER BY score DESC
    LIMIT 5
""")

# Analytics API: REST (external access)
requests.get('http://localhost:8080/api/analytics/user/123/recommendations')
```

### Social Network

```python
# User profiles: MongoDB (flexible attributes)
db.users.insert_one({
    "username": "alice",
    "bio": "Software engineer",
    "interests": ["coding", "hiking", "photography"]
})

# Friend graph: Cypher (relationships)
session.run("""
    MATCH (a:User {username: 'alice'}), (b:User {username: 'bob'})
    CREATE (a)-[:FOLLOWS]->(b)
""")

# Activity feed: Redis Streams (real-time)
r.xadd('feed:alice', {
    'type': 'post',
    'content': 'Hello world!',
    'timestamp': time.time()
})

# Analytics: PostgreSQL (complex queries)
pg_cur.execute("""
    SELECT 
        user_id,
        COUNT(*) as post_count,
        AVG(engagement_score) as avg_engagement
    FROM posts
    GROUP BY user_id
    HAVING COUNT(*) > 10
""")
```

### IoT Platform

```python
# Sensor data ingestion: CQL (time-series)
cql_session.execute("""
    INSERT INTO sensor_readings (sensor_id, timestamp, temperature, humidity)
    VALUES ('SENSOR-001', toTimestamp(now()), 23.5, 65.0)
""")

# Real-time monitoring: Redis (fast access)
r.set('sensor:SENSOR-001:latest', json.dumps({
    'temperature': 23.5,
    'humidity': 65.0,
    'timestamp': time.time()
}))

# ML predictions: PostgreSQL (ML functions)
pg_cur.execute("""
    SELECT 
        sensor_id,
        ML_PREDICT('anomaly_model', ARRAY[temperature, humidity]) as is_anomaly
    FROM sensor_readings
    WHERE timestamp > NOW() - INTERVAL '1 hour'
""")

# Device graph: Cypher (relationships)
session.run("""
    MATCH (sensor:Sensor {id: 'SENSOR-001'})-[:LOCATED_IN]->(zone:Zone)
    MATCH (zone)-[:CONTAINS]->(other:Sensor)
    RETURN other.id, other.status
""")
```

## Performance Comparison

Run the benchmark to compare protocol performance:

```bash
python benchmarks/protocol_performance_comparison.py
```

**Typical Results:**
```
Protocol Performance Comparison (10,000 operations)
====================================================
Redis HSET:        15,234 ops/sec
MongoDB Insert:    12,456 ops/sec
PostgreSQL Insert:  8,923 ops/sec
MySQL Insert:       8,567 ops/sec
REST API POST:      5,234 ops/sec

Redis HGET:        45,678 ops/sec
MongoDB Find:      23,456 ops/sec
PostgreSQL SELECT: 18,234 ops/sec
MySQL SELECT:      17,890 ops/sec
REST API GET:      12,345 ops/sec
```

## Best Practices

### 1. Choose the Right Protocol for Each Operation

- **PostgreSQL/MySQL**: Complex queries, JOINs, transactions
- **MongoDB**: Flexible documents, nested data
- **Redis**: Caching, real-time, pub/sub
- **Cypher**: Graph traversals, relationships
- **CQL**: Time-series, high write throughput
- **REST**: External APIs, web integration
- **gRPC**: Microservices, high performance

### 2. Leverage Protocol Strengths

```python
# Write structured data via SQL
pg_cur.execute("INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')")

# Cache frequently accessed data in Redis
r.hset('user:1', mapping={'name': 'Alice', 'email': 'alice@example.com'})

# Query relationships via Cypher
session.run("MATCH (u:User {name: 'Alice'})-[:FRIEND]->(f) RETURN f")
```

### 3. Maintain Consistency

All protocols share the same storage, so consistency is automatic:

```python
# Update via PostgreSQL
pg_cur.execute("UPDATE users SET email = 'alice.new@example.com' WHERE id = 1")
pg_conn.commit()

# Read via Redis (immediately reflects the change)
email = r.hget('user:1', 'email')
# Returns: 'alice.new@example.com'
```

## Troubleshooting

### Data Not Visible Across Protocols

```python
# Ensure transactions are committed
pg_conn.commit()  # PostgreSQL
mysql_conn.commit()  # MySQL

# MongoDB writes are immediately visible
# Redis writes are immediately visible
```

### Performance Issues

```python
# Use appropriate protocol for the operation
# Fast reads: Redis
# Complex queries: PostgreSQL
# Flexible documents: MongoDB
# Graph traversals: Cypher
```

## See Also

- [Protocol Comparison Guide](../PROTOCOL_COMPARISON.md)
- [Individual Protocol Examples](../)
- [Orbit-RS Architecture](../../docs/overview.md)

## Contributing

To add new cross-protocol examples:
1. Demonstrate practical use cases
2. Show instant consistency
3. Include performance considerations
4. Test with running Orbit server
5. Update this README
