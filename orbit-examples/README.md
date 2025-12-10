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

```text
orbit-examples/
├── README.md                          # This file
│
├── protocol/                          # Protocol-specific examples
│   ├── mongodb/                       # MongoDB wire protocol
│   ├── mysql/                         # MySQL wire protocol
│   ├── cql/                           # Cassandra CQL
│   ├── cross-protocol/                # Multi-protocol integration
│   ├── graphrag/                      # GraphRAG examples
│   └── ml-protocol-examples/          # ML/AI protocol examples
│
├── banking/                           # Banking & Financial Services
│   ├── README.md
│   ├── sql/01_schema_core.sql         # Accounts, transactions, loans
│   ├── orbitql/01_queries.orbitql     # Analytics queries
│   └── workflows/01_transaction_processing.md
│
├── financial-markets/                 # Trading & Capital Markets
│   ├── README.md
│   └── workflows/01_order_execution.md
│
├── healthcare/                        # Healthcare & Medical Records
│   ├── README.md
│   ├── sql/01_schema_ehr.sql          # Electronic Health Records
│   └── workflows/01_patient_admission.md
│
├── logistics/                         # Supply Chain & Logistics
│   ├── README.md
│   └── workflows/01_order_fulfillment.md
│
├── energy/                            # Energy & Utilities
│   ├── README.md
│   └── workflows/01_grid_management.md
│
├── media/                             # Media & Entertainment
│   ├── README.md
│   └── workflows/01_content_recommendation.md
│
├── education/                         # Education & EdTech
│   ├── README.md
│   └── workflows/01_personalized_learning.md
│
├── real-estate/                       # Real Estate & PropTech
│   ├── README.md
│   └── workflows/01_property_listing.md
│
├── agriculture/                       # Agriculture & AgTech
│   ├── README.md
│   └── workflows/01_precision_farming.md
│
│
├── entertainment/                     # Studio & Production
│   ├── README.md
│   ├── sql/                           # Production budgets
│   ├── mongodb/                       # Content catalog
│   ├── redis/                         # Streaming sessions
│   ├── cypher/                        # Knowledge graph
│   └── workflows/01_content_ingestion.md
│
├── data_center/                       # Data Center & Orbital Ops
│   ├── README.md
│   ├── sql/                           # Asset management
│   ├── cql/                           # Telemetry (Cassandra)
│   ├── redis/                         # Real-time alerts
│   ├── cypher/                        # Network topology
│   ├── mongodb/                       # Hardware inventory
│   └── workflows/01_predictive_maintenance.md
│
├── defense/                           # Defense & Aerospace
│   ├── README.md
│   └── workflows/01_mission_planning.md
│
├── space/                             # Space Operations
│   ├── README.md
│   └── workflows/01_satellite_operations.md
│
├── government/                        # Government Services
│   ├── README.md
│   └── workflows/01_permit_processing.md
│
├── insurance/                         # Insurance (9 types)
│   ├── README.md
│   ├── sql/                           # 9 insurance schemas
│   ├── redis/                         # Quote caching, risk scoring
│   ├── mongodb/                       # Policy documents
│   ├── cypher/                        # Fraud detection graphs
│   ├── cql/                           # Premium history
│   └── workflows/                     # End-to-end workflows
│
├── adtech/                            # Advertising Technology
│   ├── README.md
│   ├── sql/                           # Campaigns, Inventory (Buy/Sell side)
│   ├── redis/                         # RTB, Frequency Capping
│   ├── mongodb/                       # DMP User Profiles
│   ├── cypher/                        # Identity Graph
│   └── workflows/01_rtb_auction.md
│
├── telco/                             # Telecommunications
│   ├── README.md
│   ├── sql/                           # Network, billing schemas
│   ├── redis/                         # Real-time operations
│   ├── python/                        # Subscriber onboarding
│   └── workflows/01_subscriber_onboarding.md
│
├── retail/                            # Retail & E-Commerce
│   ├── README.md
│   ├── sql/                           # Products, orders, customers
│   ├── redis/                         # Shopping cart, inventory
│   ├── mongodb/                       # Product catalog
│   ├── cypher/                        # Recommendations
│   ├── cql/                           # Sales analytics
│   ├── python/                        # Order processing
│   └── workflows/01_order_processing.md
│
├── hospitality/                       # Coffeehouse & Restaurants
│   ├── README.md
│   ├── sql/                           # Menu, POS schemas
│   ├── redis/                         # Order queue, loyalty
│   ├── python/                        # Mobile ordering
│   └── workflows/01_mobile_ordering.md

├── travel/                            # Travel Booking Platform
│   ├── README.md
│   ├── sql/                           # Flights, hotels, cars, packages
│   ├── redis/                         # Search cache, pricing, loyalty
│   ├── mongodb/                       # Packages, preferences, reviews
│   ├── python/                        # Booking workflows
│   └── workflows/01_flight_booking_workflow.md

│
└── manufacturing/                     # Electronics Manufacturing
    ├── README.md
    ├── sql/                           # Products, BOM, production
    ├── redis/                         # ML predictions, real-time ops
    ├── orbitql/                       # Analytics queries
    ├── python/                        # Work order processing
    ├── workflows/01_work_order_processing.md
    └── run_tests.sh                   # Test automation

├── gaming/                            # Gaming & MMO
│   ├── README.md
│   ├── sql/                           # Player accounts
│   ├── redis/                         # Leaderboards
│   ├── mongodb/                       # Match history
│   ├── cypher/                        # Social graph
│   └── workflows/01_matchmaking_flow.md
│
├── automotive/                        # Connected Vehicle Platform
│   ├── README.md
│   ├── sql/                           # Vehicle registry
│   ├── cql/                           # Telemetry ingestion
│   ├── redis/                         # Fleet status
│   ├── aql/                           # Supply chain graph
│   └── workflows/01_telemetry_pipeline.md
│
├── construction/                      # Construction Management
│   ├── README.md
│   ├── sql/                           # Projects & Budgets
│   ├── mongodb/                       # Blueprints & Logs
│   ├── redis/                         # Site sensors
│   └── workflows/01_safety_incident.md
│
├── legal/                             # Legal Tech
│   ├── README.md
│   ├── sql/                           # Matters & Billing
│   ├── cypher/                        # Citation graph
│   ├── aql/                           # Discovery search
│   └── workflows/01_conflict_check.md
│
├── oil-gas/                           # Oil & Gas Exploration
│   ├── README.md
│   ├── sql/                           # Assets (Rigs/Wells)
│   ├── cql/                           # Refinery IoT
│   ├── cypher/                        # Pipeline graph
│   ├── mongodb/                       # Seismic surveys
│   └── workflows/01_preventative_maintenance.md
│
├── robotics/                          # Robotics Fleet
│   ├── README.md
│   ├── sql/                           # Robot registry
│   ├── redis/                         # Live telemetry
│   ├── mongodb/                       # Mission logs
│   └── workflows/01_navigation_mission.md
│
├── semiconductor/                     # Chip Manufacturing
│   ├── README.md
│   ├── sql/                           # MES Lot tracking
│   ├── cql/                           # FDC/SPC sensor data
│   ├── aql/                           # Yield lineage graph
│   └── workflows/01_fdc_interdiction.md
```

**Total**: 27 industry examples, 120+ files, 26 comprehensive workflows


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

## 🏭 Industry Examples

Complete end-to-end industry examples using multiple protocols - **17 industries covered**:

### Banking
- Account management and transactions
- **ML-powered fraud detection** (Random Forest, 95% accuracy)
- Credit scoring (XGBoost, 92% accuracy)
- KYC/AML compliance and regulatory reporting
- **[View Banking Examples](banking/)**

### Financial Markets
- Trading and order execution (<1ms latency)
- **ML price prediction** (LSTM) and trading signals (RL)
- Risk management and VaR calculation
- **[View Financial Markets Examples](financial-markets/)**

### Healthcare & Medical Records
- Electronic Health Records (EHR) with HIPAA compliance
- **ML diagnosis prediction** (89% accuracy) and readmission risk
- Patient management and prescriptions
- **[View Healthcare Examples](healthcare/)**

### Logistics & Supply Chain
- Warehouse and transportation management
- **ML demand forecasting** (Prophet, 92% accuracy)
- Route optimization and real-time tracking
- **[View Logistics Examples](logistics/)**

### Energy & Utilities
- Smart grid management (10M+ meters, 1B+ readings/day)
- **ML load forecasting** (LSTM, 94% accuracy)
- Renewable energy and outage prediction
- **[View Energy Examples](energy/)**

### Media & Entertainment
- Streaming platform and content delivery
- **ML recommendations** (91% accuracy) and churn prediction
- Ad targeting and content moderation
- **[View Media Examples](media/)**

### Education & EdTech
- Learning Management System (LMS)
- **ML performance prediction** (87% accuracy) and dropout risk
- Personalized learning paths
- **[View Education Examples](education/)**

### Real Estate & PropTech
- Property management and smart buildings
- **ML property valuation** (92% accuracy)
- Market analytics and tenant screening
- **[View Real Estate Examples](real-estate/)**

### Agriculture & AgTech
- Precision farming and crop monitoring
- **ML yield prediction** (89% accuracy) and disease detection (92%)
- IoT sensors (1M+) and supply chain
- **[View Agriculture Examples](agriculture/)**

### Defense & Aerospace
- Asset tracking and mission planning
- **ML threat detection** and predictive maintenance
- Security: RBAC, data classification
- **[View Defense Examples](defense/)**

### Space Operations
- Satellite tracking and telemetry (<10ms)
- **ML anomaly detection** and collision prediction
- Mission control and orbital mechanics
- **[View Space Examples](space/)**

### Government Services
- Citizen services and permits
- **ML fraud detection** and service optimization
- GDPR compliance and emergency response
- **[View Government Examples](government/)**

### Insurance
- **9 insurance types**: Auto, Home, Life, Health, Property, Travel, Disability, Umbrella, Industrial
- Fraud detection with graph analytics
- **[View Insurance Examples](insurance/)**

### Telecommunications
- Loyalty programs and kitchen operations
- **[View Hospitality Examples](hospitality/)**

### Travel (Booking Platform)
- Flight, hotel, and car rental bookings
- **Vacation packages** with bundled discounts
- **Dynamic pricing** and loyalty rewards
- Multi-city itinerary planning
- **[View Travel Examples](travel/)**

### Manufacturing (Electronics Assembly)
- **8 ML models**: Predictive maintenance, quality, demand forecasting
- BOM management and assembly lines
- **[View Manufacturing Examples](manufacturing/)**

### Gaming (MMO Backend)
- **4 Protocols**: SQL, Redis, Mongo, Cypher
- **Real-time Leaderboards** and matchmaking
- Social graph and match history archival
- **[View Gaming Examples](gaming/)**

### Aviation & Aerospace (New)
- **3 Protocols**: SQL, MongoDB, CQL, Redis
- **Commercial Airlines**: MRO and Flight Data
- **Private Aviation**: Fractional Ownership
- **Urban Air Mobility**: eVTOL Telemetry
- **[View Aviation Examples](aviation/)**

### Automotive (Connected Vehicles)
- **4 Protocols**: SQL, CQL, Redis, AQL
- **High-velocity telemetry** ingestion (Cassandra)
- Real-time fleet tracking and supply chain graph
- **[View Automotive Examples](automotive/)**

### Construction (Project & IoT)
- **3 Protocols**: SQL, Mongo, Redis
- Project budgets and **Blueprint management**
- Real-time site safety monitoring
- **[View Construction Examples](construction/)**

### Legal (Practice Management)
- **3 Protocols**: SQL, Cypher, AQL
- Matter billing and **Citation Graphs**
- Document discovery and search
- **[View Legal Examples](legal/)**

### Oil & Gas (Energy Platform)
- **4 Protocols**: SQL, CQL, Cypher, Mongo
- **Refinery IoT** and Seismic Data
- Pipeline network graph topology
- **[View Oil & Gas Examples](oil-gas/)**

### Robotics (Fleet Management)
- **3 Protocols**: SQL, Redis, Mongo
- **Live LiDAR Streaming** and Telemetry
- Mission logging and replay
- **[View Robotics Examples](robotics/)**

### Semiconductor (Wafer Fab)
- **3 Protocols**: SQL, CQL, AQL
- MES Lot Tracking and **FDC Sensor Data**
- Yield Lineage Graph
- **[View Semiconductor Examples](semiconductor/)**

### Car Rental Agencies
- **2 Protocols**: SQL, Redis
- **Fleet Inventory** and Booking System
- **Live Availability Cache** (Redis Sets)
- **[View Car Rental Examples](car_rental/)**

### Car Dealership & Sales
- **2 Protocols**: SQL, MongoDB
- **Vehicle Inventory** and Sales Transactions
- **Customer 360 View** (Interactions, Preferences)
- **[View Car Dealership Examples](car_dealership/)**

├── Entertainment (Studio)
│   ├── **4 Protocols**: SQL, Mongo, Redis, Cypher
│   ├── **Production Management** and Budgeting
│   ├── **Content Catalog** and Streaming Sessions
│   ├── **[View Entertainment Examples](entertainment/)**
│
├── Data Center & Orbital Ops
│   ├── **5 Protocols**: SQL, Mongo, CQL, Redis, Cypher
│   ├── **Terrestrial & Orbital** Asset Management
│   ├── **Live Telemetry** and Network Topology
│   ├── **[View Data Center Examples](data_center/)**
│
├── Field Service Management
│   ├── **2 Protocols**: SQL, Cypher
│   ├── **Work Order Scheduling** and Invoicing
│   ├── **Technician Skill Graph** (Matching skills & location)
│   ├── **[View Field Service Examples](field_service/)**

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
