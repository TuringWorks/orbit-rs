# Orbit-RS Examples

Comprehensive examples demonstrating all 9 protocols supported by Orbit-RS: PostgreSQL, MySQL, CQL (Cassandra), Redis (RESP), Cypher (Neo4j/Bolt), AQL (ArangoDB), MongoDB, REST API, and gRPC.

## 🚀 Quick Start

### Start Orbit Server

```bash
# Clone and build
git clone https://github.com/TuringWorks/orbit-rs.git
cd orbit-rs
cargo build --release

# Start multi-protocol server (all 9 protocols active)
cargo run --bin orbit-server

# All protocols now listening:
# PostgreSQL:  localhost:5432
# MySQL:       localhost:3306
# CQL:         localhost:9042
# Redis:       localhost:6379
# Cypher/Bolt: localhost:7687
# AQL:         localhost:8529
# MongoDB:     localhost:27017
# REST API:    localhost:8080
# gRPC:        localhost:50051
```

### Test Connections

```bash
# PostgreSQL
psql -h localhost -p 5432 -U orbit -d postgres

# MySQL
mysql -h localhost -P 3306 -u orbit

# Redis
redis-cli -h localhost -p 6379

# MongoDB
mongosh mongodb://localhost:27017

# REST API
curl http://localhost:8080/health

# gRPC
grpcurl -plaintext localhost:50051 list
```

## 📁 Directory Structure

```
orbit-examples/
├── README.md                          # This file
├── QUICK_START.md                     # Quick start guide
├── PROTOCOL_COMPARISON.md             # Protocol selection guide
│
├── mongodb/                           # MongoDB wire protocol examples
│   ├── README.md
│   ├── 01_basic_crud.js
│   ├── 02_aggregation_pipeline.js
│   ├── 03_ml_integration.js
│   ├── python/
│   └── scenarios/
│
├── mysql/                             # MySQL wire protocol examples
│   ├── README.md
│   ├── 01_basic_sql.sql
│   ├── 02_ml_functions.sql
│   └── python/
│
├── cql/                               # CQL (Cassandra) protocol examples
│   ├── README.md
│   ├── 01_keyspace_setup.cql
│   ├── 02_wide_column_modeling.cql
│   └── python/
│
├── redis/                             # Redis RESP protocol examples
│   ├── README.md
│   ├── 01_data_structures.sh
│   ├── 02_streams.sh
│   └── python/
│
├── grpc/                              # gRPC protocol examples
│   ├── README.md
│   ├── rust/
│   └── python/
│
├── orbitql/                           # OrbitQL multi-model examples
│   ├── README.md
│   ├── 01_basic_queries.orbitql
│   └── python/
│
├── cross-protocol/                    # Cross-protocol integration
│   ├── README.md
│   ├── 01_write_postgres_read_redis.py
│   └── 02_multi_protocol_transaction.py
│
├── ml-protocol-examples/              # ML/AI examples (existing)
│   ├── README.md
│   ├── 01_healthcare_ml.sql
│   ├── python/
│   ├── cypher/
│   └── aql/
│
├── graphrag/                          # GraphRAG examples (existing)
│   ├── README.md
│   ├── python/
│   ├── cypher/
│   └── aql/
│
└── scenarios/                         # Complete industry scenarios
    ├── healthcare/
    ├── ecommerce/
    ├── finance/
    └── iot/
```

## 🎯 Examples by Protocol

### PostgreSQL (Port 5432)

**Full SQL database with pgvector support**

```bash
cd ml-protocol-examples
psql -h localhost -p 5432 -U orbit -d postgres -f 01_healthcare_ml.sql
```

**Features:**
- Complete SQL support (DDL, DML, DCL)
- pgvector for similarity search
- JSONB for flexible documents
- ML functions (ML_TRAIN_MODEL, ML_PREDICT)
- Spatial functions
- Time-series queries

**Examples:**
- [Healthcare ML](ml-protocol-examples/01_healthcare_ml.sql)
- [Finance ML](ml-protocol-examples/02_finance_ml.sql)
- [Retail E-commerce](ml-protocol-examples/03_retail_ecommerce_ml.sql)

---

### MySQL (Port 3306)

**MySQL-compatible SQL interface**

```bash
cd mysql
mysql -h localhost -P 3306 -u orbit < 01_basic_sql.sql
```

**Features:**
- MySQL wire protocol compatibility
- Prepared statements
- Transactions with isolation levels
- ML function integration
- Vector operations

**Examples:**
- [Basic SQL Operations](mysql/01_basic_sql.sql)
- [ML Functions](mysql/02_ml_functions.sql)
- [Finance Scenario](mysql/finance_scenario.sql)

---

### MongoDB (Port 27017)

**Document database with aggregation framework**

```bash
cd mongodb
mongosh mongodb://localhost:27017 --file 01_basic_crud.js
```

**Features:**
- Document CRUD operations
- Aggregation pipeline (34+ operators)
- Multi-document transactions
- ML function integration
- Change streams

**Examples:**
- [Basic CRUD](mongodb/01_basic_crud.js)
- [Aggregation Pipeline](mongodb/02_aggregation_pipeline.js)
- [Python PyMongo](mongodb/python/mongodb_examples.py)

---

### Redis (Port 6379)

**High-performance key-value store with persistence**

```bash
cd redis
redis-cli -h localhost -p 6379 < 01_data_structures.sh
```

**Features:**
- All Redis data types (strings, lists, sets, hashes, sorted sets)
- Redis Streams
- Pub/Sub messaging
- TTL with persistence
- ML commands
- Transactions (MULTI/EXEC)

**Examples:**
- [Data Structures](redis/01_data_structures.sh)
- [Streams](redis/02_streams.sh)
- [Python Client](redis/python/redis_advanced.py)

---

### Cypher/Bolt (Port 7687)

**Neo4j-compatible graph database**

```bash
cd ml-protocol-examples/cypher
cypher-shell -a bolt://localhost:7687 -f graph_ml.cypher
```

**Features:**
- Cypher query language
- Graph traversals
- CALL procedures
- Graph algorithms (PageRank, BFS, DFS)
- ML integration
- GraphRAG support

**Examples:**
- [Graph ML](ml-protocol-examples/cypher/graph_ml.cypher)
- [Healthcare Graph](ml-protocol-examples/cypher/hospital_end_to_end.cypher)
- [Banking Graph](ml-protocol-examples/cypher/banking_end_to_end.cypher)

---

### AQL (Port 8529)

**ArangoDB-compatible multi-model database**

```bash
cd ml-protocol-examples/aql
arangosh --server.endpoint tcp://localhost:8529 --javascript.execute graph_ml.aql
```

**Features:**
- AQL query language
- Document and graph queries
- Graph traversals
- Window functions
- ML integration
- UPSERT operations

**Examples:**
- [Graph ML](ml-protocol-examples/aql/graph_ml.aql)
- [Banking Scenario](ml-protocol-examples/aql/banking_end_to_end.aql)

---

### CQL (Port 9042)

**Cassandra-compatible wide-column store**

```bash
cd cql
cqlsh localhost 9042 -f 01_keyspace_setup.cql
```

**Features:**
- Keyspace and table management
- Wide-column data modeling
- Partition and clustering keys
- Materialized views
- ML function integration
- Time-series patterns

**Examples:**
- [Keyspace Setup](cql/01_keyspace_setup.cql)
- [Wide-Column Modeling](cql/02_wide_column_modeling.cql)
- [IoT Time-Series](cql/iot_scenario.cql)

---

### REST API (Port 8080)

**Web-friendly JSON interface**

```bash
cd rest
# Run examples
bash 01_basic_crud.sh
```

**Features:**
- RESTful CRUD operations
- Vector similarity search
- ML training and inference
- Batch operations
- OpenAPI documentation

**Examples:**
- [Basic CRUD](rest/01_basic_crud.sh)
- [Vector Search](rest/02_vector_search.sh)
- [Python Client](rest/python/rest_client.py)

---

### gRPC (Port 50051)

**High-performance actor management**

```bash
cd grpc
# Run Rust examples
cargo run --example actor_management
```

**Features:**
- Actor lifecycle management
- Distributed transactions
- Bidirectional streaming
- ML model deployment
- Health checks

**Examples:**
- [Actor Management](grpc/rust/actor_management.rs)
- [Streaming](grpc/rust/streaming_examples.rs)
- [Python Client](grpc/python/grpc_client.py)

---

### OrbitQL

**Unified multi-model query language**

```bash
cd orbitql
# Run examples via Python client
python python/orbitql_client.py
```

**Features:**
- Multi-model queries (document + graph + time-series)
- Cross-model JOINs
- Graph traversals with arrow notation
- Live query subscriptions
- Tiered storage access

**Examples:**
- [Basic Queries](orbitql/01_basic_queries.orbitql)
- [Multi-Model](orbitql/02_multi_model.orbitql)
- [Graph Traversals](orbitql/04_graph_traversals.orbitql)

---

## 🔄 Cross-Protocol Integration

**The same data is accessible through all protocols!**

### Example: Write via PostgreSQL, Read via Redis

```python
# Write via PostgreSQL
import psycopg2
conn = psycopg2.connect(host="localhost", port=5432, database="postgres")
cur = conn.cursor()
cur.execute("INSERT INTO products (name, price) VALUES ('Laptop', 999)")
conn.commit()

# Read via Redis
import redis
r = redis.Redis(host='localhost', port=6379)
product = r.hgetall('product:1')
print(product)  # {'name': 'Laptop', 'price': '999'}

# Query via MongoDB
from pymongo import MongoClient
client = MongoClient('mongodb://localhost:27017/')
db = client['postgres']
products = db.products.find({'name': 'Laptop'})

# Query via REST
import requests
response = requests.get('http://localhost:8080/api/products?name=Laptop')
print(response.json())
```

**See [Cross-Protocol Examples](cross-protocol/) for more integration patterns.**

---

## 🏭 Industry Scenarios

Complete end-to-end scenarios using multiple protocols:

### Healthcare
- Patient risk prediction (PostgreSQL + ML)
- Real-time vital signs monitoring (Redis + Time-series)
- Medical knowledge graph (Cypher)
- **[View Healthcare Scenario](scenarios/healthcare/)**

### E-Commerce
- Product catalog (MongoDB)
- Recommendation engine (PostgreSQL + pgvector)
- Shopping cart (Redis)
- Order processing (MySQL transactions)
- **[View E-Commerce Scenario](scenarios/ecommerce/)**

### Financial Services
- Fraud detection (PostgreSQL + ML)
- Transaction graph analysis (Cypher)
- Market data time-series (CQL)
- **[View Finance Scenario](scenarios/finance/)**

### IoT & Manufacturing
- Sensor data ingestion (MongoDB + CQL)
- Predictive maintenance (PostgreSQL + ML)
- Device relationships (AQL)
- Real-time alerts (Redis Pub/Sub)
- **[View IoT Scenario](scenarios/iot/)**

---

## 📊 Protocol Comparison

| Protocol | Best For | Data Model | Query Language | Performance |
|----------|----------|------------|----------------|-------------|
| **PostgreSQL** | Complex queries, ACID transactions | Relational | SQL | High |
| **MySQL** | MySQL compatibility, web apps | Relational | SQL | High |
| **MongoDB** | Flexible schema, documents | Document | MQL | Very High |
| **Redis** | Caching, real-time, pub/sub | Key-Value | Commands | Extreme |
| **Cypher** | Graph relationships, traversals | Graph | Cypher | High |
| **AQL** | Multi-model, graph + documents | Multi-Model | AQL | High |
| **CQL** | Time-series, wide-column | Wide-Column | CQL | Very High |
| **REST** | Web APIs, external integration | Any | HTTP/JSON | Medium |
| **gRPC** | Microservices, actor system | Any | Protobuf | Very High |

**[Detailed Protocol Comparison](PROTOCOL_COMPARISON.md)**

---

## 🧪 Running Examples

### Prerequisites

1. **Start Orbit Server:**
   ```bash
   cargo run --bin orbit-server
   ```

2. **Install Client Tools:**
   ```bash
   # PostgreSQL
   brew install postgresql
   
   # MySQL
   brew install mysql-client
   
   # Redis
   brew install redis
   
   # MongoDB
   brew install mongosh
   
   # Python clients
   pip install psycopg2-binary pymongo redis requests grpcio
   
   # Node.js clients
   npm install pg mongodb redis
   ```

### Run All Examples

```bash
# Run test script
./scripts/run-all-examples.sh

# Or run specific protocol examples
cd mongodb && mongosh --file 01_basic_crud.js
cd mysql && mysql -h localhost < 01_basic_sql.sql
cd redis && redis-cli < 01_data_structures.sh
```

---

## 🎓 Learning Path

### Beginner
1. Start with [QUICK_START.md](QUICK_START.md)
2. Try [MongoDB Basic CRUD](mongodb/01_basic_crud.js)
3. Explore [PostgreSQL ML Examples](ml-protocol-examples/01_healthcare_ml.sql)
4. Test [Redis Data Structures](redis/01_data_structures.sh)

### Intermediate
1. Learn [Aggregation Pipelines](mongodb/02_aggregation_pipeline.js)
2. Explore [Graph Queries](ml-protocol-examples/cypher/graph_ml.cypher)
3. Try [Cross-Protocol Integration](cross-protocol/01_write_postgres_read_redis.py)
4. Study [Industry Scenarios](scenarios/)

### Advanced
1. Build [Complete Applications](scenarios/ecommerce/)
2. Implement [Multi-Protocol Transactions](cross-protocol/02_multi_protocol_transaction.py)
3. Optimize [Performance](PROTOCOL_COMPARISON.md#performance-tips)
4. Deploy to [Production](../docs/kubernetes_deployment.md)

---

## 🤝 Contributing

To add new examples:

1. Follow the existing directory structure
2. Include comprehensive comments
3. Add README for new directories
4. Test against running Orbit server
5. Update this main README

---

## 📚 Additional Resources

- **[Orbit-RS Documentation](../docs/README.md)**
- **[Quick Start Guide](../docs/quick_start.md)**
- **[Protocol Adapters](../docs/protocols/protocol_adapters.md)**
- **[ML Integration](../docs/content/ml/ML_PROTOCOL_INTEGRATION.md)**
- **[GraphRAG Guide](../docs/content/graphrag/GRAPHRAG_GUIDE.md)**

---

## 📝 License

These examples are part of the Orbit-RS project and follow the same dual licensing (MIT OR BSD-3-Clause).

---

**Built with Orbit-RS - One Server, All Protocols** 🚀
