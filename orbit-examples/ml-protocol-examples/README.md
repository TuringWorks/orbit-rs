# ML Industry Examples for Orbit-RS

This directory contains comprehensive SQL examples demonstrating machine learning use cases across various industry verticals using Orbit-RS's multi-protocol database capabilities.

## Overview

These examples showcase Orbit-RS's AI-native features including:

- **Vector Search** - Similarity search with embeddings for recommendations, fraud detection, and pattern matching
- **Time Series Analysis** - Real-time monitoring, forecasting, and anomaly detection
- **Spatial Functions** - Geographic queries and distance calculations
- **ML Integration** - Predictive models, scoring functions, and intelligent analytics

## Industry Verticals

### 1. Healthcare (`01_healthcare_ml.sql`)

**Use Cases:**

- Patient risk stratification and prediction
- Medical image similarity search with CNN embeddings
- Real-time vital signs monitoring with time series
- Drug interaction prediction using molecular embeddings
- Clinical trial patient matching
- Hospital readmission risk prediction

**Key Features:**

- Vector embeddings for medical images (512-dimensional)
- JSONB for flexible vital signs storage
- Time series for continuous patient monitoring
- ML-based risk scoring functions

**Example Queries:**

```sql
-- Find similar medical images
SELECT image_id, diagnosis, similarity_score
FROM medical_images
ORDER BY image_embedding <=> query_embedding
LIMIT 5;

-- Predict patient readmission risk
SELECT patient_id, name, predict_readmission_risk(age, length_of_stay, num_procedures, risk_score)
FROM patients;
```

---

### 2. Finance & Banking (`02_finance_ml.sql`)

**Use Cases:**

- Real-time fraud detection with ML scoring
- Credit scoring and risk assessment
- Algorithmic trading with time series analysis
- Portfolio risk management and optimization
- Customer churn prediction
- Anti-money laundering (AML) pattern detection

**Key Features:**

- Transaction fraud scoring with anomaly detection
- Credit profile embeddings (128-dimensional)
- Stock price time series with moving averages
- Trading signal detection (Golden Cross/Death Cross)
- AML structuring and geographic anomaly detection

**Example Queries:**

```sql
-- Detect fraudulent transactions
SELECT transaction_id, amount, merchant_name, fraud_score
FROM transactions
WHERE fraud_score > 0.7
ORDER BY fraud_score DESC;

-- Find similar credit profiles
SELECT customer_id, name, credit_score, similarity
FROM credit_profiles
ORDER BY credit_embedding <=> query_embedding
LIMIT 5;
```

---

### 3. Retail & E-Commerce (`03_retail_ecommerce_ml.sql`)

**Use Cases:**

- Product recommendations using vector similarity
- Customer segmentation with behavioral embeddings
- Demand forecasting with time series
- Dynamic pricing optimization
- Inventory optimization with reorder points
- Shopping cart abandonment prediction
- Product review sentiment analysis

**Key Features:**

- Product embeddings (256-dimensional) for recommendations
- Customer behavioral embeddings (128-dimensional)
- Time series sales data with trend detection
- ML-based pricing functions
- Collaborative filtering for recommendations

**Example Queries:**

```sql
-- Recommend similar products
SELECT product_id, product_name, price, similarity_score
FROM products
ORDER BY product_embedding <=> purchased_product_embedding
LIMIT 5;

-- Predict cart abandonment
SELECT cart_id, customer_id, total_value, abandonment_probability
FROM shopping_carts
WHERE abandonment_probability > 0.5
ORDER BY abandonment_probability DESC;
```

---

### 4. Manufacturing & IoT (`04_manufacturing_iot_ml.sql`)

**Use Cases:**

- Predictive maintenance with sensor data
- Quality control with defect detection
- Supply chain optimization
- Production line optimization (OEE)
- Energy consumption optimization
- Sensor-quality correlation analysis

**Key Features:**

- IoT sensor time series (temperature, vibration, pressure)
- Visual inspection embeddings (512-dimensional)
- Anomaly detection with 3-sigma rule
- OEE (Overall Equipment Effectiveness) calculation
- ML-based supplier selection

**Example Queries:**

```sql
-- Predict equipment failure
SELECT equipment_name, failure_probability, maintenance_priority
FROM equipment
WHERE failure_probability > 0.5
ORDER BY failure_probability DESC;

-- Find similar defect patterns
SELECT inspection_id, defect_types, similarity
FROM quality_inspections
ORDER BY image_embedding <=> query_embedding
LIMIT 5;
```

---

### 5. Telecommunications (`05_telecommunications_ml.sql`)

**Use Cases:**

- Network performance monitoring and optimization
- Customer churn prediction with retention strategies
- Fraud detection in call detail records (CDR)
- Network equipment predictive maintenance
- Customer experience analytics
- Data usage prediction and plan recommendations

**Key Features:**

- Network metrics time series (latency, throughput, packet loss)
- Call detail record (CDR) fraud scoring
- Equipment health monitoring
- Customer experience event tracking
- Usage pattern analysis

**Example Queries:**

```sql
-- Detect network anomalies
SELECT tower_name, latency_ms, packet_loss_pct, alert_status
FROM network_metrics
WHERE latency_ms > avg_latency * 2 OR packet_loss_pct > 1.0;

-- Predict customer churn
SELECT customer_id, account_number, churn_probability, retention_strategy
FROM customer_accounts
WHERE churn_probability > 0.5
ORDER BY churn_probability DESC;
```

---

### 6. Logistics & Transportation (`06_logistics_transportation_ml.sql`)

**Use Cases:**

- Route optimization with delivery time prediction
- Fleet management with vehicle tracking
- Demand forecasting using time series
- Warehouse optimization with ABC analysis
- Driver performance analytics
- Delivery cost optimization

**Key Features:**

- Spatial functions (Haversine distance calculation)
- Location embeddings (64-dimensional)
- Vehicle telemetry time series
- Demand forecasting by hour/day
- ML-based delivery time prediction

**Example Queries:**

```sql
-- Predict delivery time
SELECT order_id, destination, predict_delivery_time(distance_km, traffic_level, weight_kg, hour)
FROM deliveries;

-- Optimize warehouse zones
SELECT product_sku, current_zone, optimal_zone, picking_frequency
FROM warehouse_inventory
WHERE location_zone != optimal_zone
ORDER BY picking_frequency DESC;
```

---

## Running the Examples

### Prerequisites

1. **Start Orbit-RS Server:**

```bash
cargo run --bin orbit-server
```

2. **Connect via PostgreSQL Protocol:**

```bash
psql -h localhost -p 5432 -U orbit -d postgres
```

### Execute Examples

Run each SQL file in order:

```bash
# Healthcare
psql -h localhost -p 5432 -U orbit -d postgres -f 01_healthcare_ml.sql

# Finance
psql -h localhost -p 5432 -U orbit -d postgres -f 02_finance_ml.sql

# Retail
psql -h localhost -p 5432 -U orbit -d postgres -f 03_retail_ecommerce_ml.sql

# Manufacturing
psql -h localhost -p 5432 -U orbit -d postgres -f 04_manufacturing_iot_ml.sql

# Telecommunications
psql -h localhost -p 5432 -U orbit -d postgres -f 05_telecommunications_ml.sql

# Logistics
psql -h localhost -p 5432 -U orbit -d postgres -f 06_logistics_transportation_ml.sql
```

### Using Python Client

```python
from orbit_client import OrbitClient

# Connect to Orbit-RS
client = OrbitClient.postgres(
    host="127.0.0.1",
    port=5432,
    username="orbit",
    password="",
    database="postgres"
)

# Execute ML queries
results = client.execute("""
    SELECT product_id, product_name, similarity_score
    FROM products
    ORDER BY product_embedding <=> %s
    LIMIT 5
""", params=(query_embedding,))

for row in results:
    print(f"{row['product_name']}: {row['similarity_score']}")
```

---

## Key ML Techniques Demonstrated

### 1. Vector Similarity Search

All examples use vector embeddings for similarity search:

- **Cosine Similarity:** `embedding1 <=> embedding2`
- **Use Cases:** Product recommendations, fraud detection, similar patient cases

```sql
-- Find similar items
SELECT id, name, 1 - (embedding <=> query_embedding) AS similarity
FROM items
ORDER BY embedding <=> query_embedding
LIMIT 10;
```

### 2. Time Series Analysis

Real-time monitoring and forecasting:

- **Moving Averages:** Window functions for trend detection
- **Anomaly Detection:** Statistical methods (3-sigma rule)
- **Forecasting:** Historical pattern analysis

```sql
-- Calculate moving average
SELECT timestamp, value,
    AVG(value) OVER (ORDER BY timestamp ROWS BETWEEN 6 PRECEDING AND CURRENT ROW) AS ma_7
FROM sensor_readings;
```

### 3. Predictive Scoring

ML-based scoring functions:

- **Risk Scores:** Patient risk, credit risk, fraud risk
- **Churn Probability:** Customer retention prediction
- **Failure Probability:** Equipment maintenance prediction

```sql
-- Predict risk score
CREATE FUNCTION predict_risk(features...) RETURNS FLOAT AS $$
    -- ML model logic
$$ LANGUAGE plpgsql;
```

### 4. Spatial Analysis

Geographic queries and distance calculations:

- **Haversine Distance:** Calculate distance between coordinates
- **Nearest Neighbor:** Find closest locations
- **Route Optimization:** Delivery route planning

```sql
-- Calculate distance
SELECT calculate_distance_km(lat1, lon1, lat2, lon2) AS distance_km;
```

---

## Performance Considerations

### Vector Indexes

Create indexes for fast similarity search:

```sql
-- IVFFlat index for approximate nearest neighbor
CREATE INDEX ON products USING ivfflat (product_embedding vector_cosine_ops);

-- HNSW index for higher accuracy (if available)
CREATE INDEX ON products USING hnsw (product_embedding vector_cosine_ops);
```

### Time Series Optimization

- Use `BIGINT` timestamps (Unix milliseconds) for efficient range queries
- Create indexes on timestamp columns
- Use window functions for moving averages

### Query Optimization

- Use `EXPLAIN ANALYZE` to understand query plans
- Create appropriate indexes on frequently queried columns
- Use materialized views for complex aggregations

---

## Integration with Orbit-RS AI Subsystems

These examples leverage Orbit-RS's 8 AI-native subsystems:

1. **Intelligent Query Optimizer** - Automatic query optimization
2. **Predictive Resource Manager** - Workload forecasting
3. **Smart Storage Manager** - Hot/warm/cold tiering
4. **Learning Engine** - Model improvement over time
5. **Decision Engine** - Policy-based decisions
6. **Knowledge Base** - Pattern storage and retrieval
7. **AI Master Controller** - Central orchestration
8. **Adaptive Index Manager** - Automatic index recommendations

---

## Additional Resources

- **Orbit-RS Documentation:** `docs/PRD.md`
- **Architecture Details:** `docs/content/architecture/ORBIT_ARCHITECTURE.md`
- **Python Client Examples:** `orbit-python-client/examples/`
- **VS Code Extension:** `orbit-vscode-extension/`

---

## Contributing

To add new industry examples:

1. Create a new SQL file: `0X_industry_name_ml.sql`
2. Follow the existing structure with 6+ use cases
3. Include ML techniques: vector search, time series, predictive models
4. Add comprehensive comments and summaries
5. Update this README with the new example

---

## License

These examples are part of the Orbit-RS project and follow the same licensing terms.

---

## Orbit ML Protocol Examples

This directory contains examples demonstrating how to use Orbit's ML capabilities across different protocols.

## Directory Structure

```text
ml-protocol-examples/
├── rust/           # Rust examples using orbit-client
├── python/         # Python examples (psycopg2, redis-py, requests)
├── javascript/     # JavaScript/Node.js examples
└── sql/            # Pure SQL examples for PostgreSQL protocol
```

## Setup Requirements

1. **Orbit Server Running**: Start the Orbit server with ML enabled:

   ```bash
   cargo run --bin orbit-server -- --config config/orbit-server.toml
   ```

2. **Protocol-specific clients**:
   - **Python**: `pip install psycopg2-binary redis requests`
   - **JavaScript**: `npm install pg redis axios`
   - **Rust**: Add `orbit-client` to your Cargo.toml

## Quick Start

### Python - PostgreSQL ML Functions

```python
import psycopg2

conn = psycopg2.connect(host="localhost", port=5432, database="orbit")
cur = conn.cursor()

# Train a model
cur.execute("""
    SELECT ML_TRAIN_MODEL(
        'churn_model',
        'random_forest',
        ARRAY[tenure, monthly_charges],
        churned
    ) FROM customers
""")

# Run predictions
cur.execute("""
    SELECT customer_id,
           ML_PREDICT('churn_model', ARRAY[tenure, monthly_charges]) as churn_risk
    FROM customers
""")
```

### Python - Redis ML Commands

```python
import redis

r = redis.Redis(host='localhost', port=6379)

# Create and train model
r.execute_command('ML.CREATE', 'fraud_model', 'xgboost',
                  'FEATURES', 'amount,merchant,hour', 'LABEL', 'is_fraud')
r.execute_command('ML.TRAIN', 'fraud_model', 'transactions:train')

# Predict
result = r.execute_command('ML.PREDICT', 'fraud_model', '[100.50, "electronics", 14]')
```

### SQL - Vector Similarity with ML

```sql
-- Create documents table with embeddings
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding vector(384)
);

-- Generate embeddings using ML
INSERT INTO documents (content, embedding)
SELECT content, ML_EMBED_TEXT(content, 'sentence-transformers')
FROM raw_documents;

-- Semantic search
SELECT content,
       embedding <=> ML_EMBED_TEXT('machine learning tutorial', 'sentence-transformers') AS distance
FROM documents
ORDER BY distance
LIMIT 10;
```

## Examples by Protocol

### PostgreSQL (Port 5432)

- `sql/ml_training.sql` - Model training examples
- `sql/ml_inference.sql` - Prediction and inference
- `sql/ml_vectors.sql` - Vector operations with ML
- `python/postgresql_ml.py` - Python client example

### Redis (Port 6379)

- `python/redis_ml.py` - Redis ML commands
- `javascript/redis_ml.js` - Node.js Redis example

### HTTP REST (Port 8080)

- `python/rest_ml.py` - REST API examples
- `javascript/rest_ml.js` - JavaScript fetch examples

### gRPC (Port 50051)

- `rust/grpc_ml.rs` - Rust gRPC client
- `python/grpc_ml.py` - Python gRPC example

## Industry Models

Examples for industry-specific models:

- `python/healthcare_ml.py` - Healthcare risk prediction
- `python/finance_ml.py` - Fraud detection, credit risk
- `python/retail_ml.py` - Demand forecasting, recommendations

## Running Examples

```bash
# Python examples
cd python
python postgresql_ml.py

# JavaScript examples
cd javascript
node redis_ml.js

# SQL examples (requires psql)
cd sql
psql -h localhost -p 5432 -d orbit -f ml_training.sql
```

## See Also

- [ML Protocol Integration Guide](../../docs/content/ml/ML_PROTOCOL_INTEGRATION.md)
- [ML SQL Functions Design](../../docs/content/ml/ML_SQL_FUNCTIONS_DESIGN.md)
- [Industry Models Plan](../../docs/content/ml/INDUSTRY_MODELS_PLAN.md)
