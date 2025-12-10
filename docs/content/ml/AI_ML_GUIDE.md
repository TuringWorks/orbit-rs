---
layout: default
title: "AI & ML Guide"
subtitle: "Machine learning and AI-native features in Orbit-RS"
category: "ml"
---

# Orbit-RS AI & Machine Learning Guide

Comprehensive guide to AI-native database features and ML capabilities.

---

## Overview

Orbit-RS integrates AI/ML capabilities at the database level, providing:

- **8 AI Subsystems** for intelligent database operations
- **ML SQL Functions** for in-database analytics
- **GraphRAG** for knowledge graph retrieval
- **Vector Operations** for similarity search
- **Protocol-Native ML** across all supported protocols

---

## AI-Native Database Features

### AI Controller

Central orchestration of all intelligent features with 10-second control loop.

```rust
// AI subsystem registration
controller.register_subsystem(QueryOptimizer::new());
controller.register_subsystem(ResourceManager::new());
controller.register_subsystem(StorageManager::new());
```

### Intelligent Query Optimizer

- Cost-based optimization with learning
- Query pattern classification
- Automated index recommendations
- Execution plan optimization

```sql
-- AI-optimized query
EXPLAIN ANALYZE SELECT * FROM orders
WHERE customer_id = 123
AND order_date > '2025-01-01';
-- AI recommends: CREATE INDEX idx_orders_customer_date ON orders(customer_id, order_date)
```

### Predictive Resource Manager

- Workload forecasting (CPU, memory, I/O)
- Predictive scaling
- Pattern-based demand prediction

### Smart Storage Manager

- Automated hot/warm/cold tiering
- Access pattern analysis
- Data reorganization without downtime

### Adaptive Transaction Manager

- Deadlock prediction and prevention
- Dynamic isolation level adjustment
- Transaction dependency analysis

---

## ML SQL Functions

### Statistical Functions

```sql
-- Linear regression
SELECT ML_LINEAR_REGRESSION(x, y) FROM dataset;

-- Correlation analysis
SELECT ML_CORRELATION(revenue, marketing_spend) FROM sales;

-- Z-score normalization
SELECT ML_ZSCORE(value) FROM metrics;

-- Covariance
SELECT ML_COVARIANCE(x, y) FROM data_points;
```

### Clustering

```sql
-- K-means clustering
SELECT ML_KMEANS(features, k := 5) FROM customers;

-- Cluster assignment
SELECT id, ML_CLUSTER_ASSIGN(features, model := 'customer_segments')
FROM customers;
```

### Classification

```sql
-- Train classifier
SELECT ML_TRAIN_CLASSIFIER(
    model_name := 'churn_predictor',
    features := ARRAY['tenure', 'monthly_charges', 'total_charges'],
    label := 'churned'
) FROM customers;

-- Predict
SELECT id, ML_PREDICT('churn_predictor', features) as churn_risk
FROM new_customers;
```

### Anomaly Detection

```sql
-- Anomaly scoring
SELECT timestamp, value, ML_ANOMALY_SCORE(value) as score
FROM sensor_readings
WHERE ML_ANOMALY_SCORE(value) > 0.9;
```

---

## Vector Operations

### Creating Vector Columns

```sql
-- pgvector-compatible syntax
CREATE TABLE embeddings (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding VECTOR(1536)  -- OpenAI embedding dimension
);

-- Insert with vector
INSERT INTO embeddings (content, embedding)
VALUES ('Hello world', '[0.1, 0.2, ...]');
```

### Similarity Search

```sql
-- Cosine similarity (most common for text)
SELECT id, content, embedding <=> query_embedding AS distance
FROM embeddings
ORDER BY embedding <=> query_embedding
LIMIT 10;

-- L2 distance
SELECT id, content, embedding <-> query_embedding AS distance
FROM embeddings
ORDER BY embedding <-> query_embedding
LIMIT 10;

-- Inner product
SELECT id, content, embedding <#> query_embedding AS distance
FROM embeddings
ORDER BY embedding <#> query_embedding DESC
LIMIT 10;
```

### Vector Indexes

```sql
-- HNSW index (recommended)
CREATE INDEX ON embeddings
USING hnsw (embedding vector_cosine_ops)
WITH (m = 16, ef_construction = 200);

-- IVFFlat index
CREATE INDEX ON embeddings
USING ivfflat (embedding vector_l2_ops)
WITH (lists = 100);
```

---

## GraphRAG

### Knowledge Graph Construction

```cypher
// Create knowledge nodes
CREATE (c:Concept {name: 'Machine Learning'})
CREATE (t:Topic {name: 'Neural Networks'})
CREATE (c)-[:HAS_TOPIC]->(t)
```

### RAG Query Processing

```sql
-- Semantic search with graph context
SELECT * FROM GRAPHRAG_QUERY(
    query := 'What is machine learning?',
    max_hops := 2,
    top_k := 5
);
```

### Multi-hop Reasoning

```cypher
// Find related concepts through relationships
MATCH path = (start:Concept)-[*1..3]-(related:Concept)
WHERE start.name = 'AI'
RETURN path
```

---

## Protocol-Native ML

### PostgreSQL ML Functions

```sql
-- Train model
SELECT ML_TRAIN_MODEL(
    'fraud_detector',
    'SELECT * FROM transactions',
    'is_fraud'
);

-- Predict
SELECT *, ML_PREDICT('fraud_detector', *) as fraud_score
FROM new_transactions;
```

### Redis ML Commands

```
# Create model
ML.CREATE fraud_model CLASSIFIER FEATURES amount,merchant_type LABEL is_fraud

# Train
ML.TRAIN fraud_model FROM transactions

# Predict
ML.PREDICT fraud_model amount 100 merchant_type online
```

### CQL ML UDFs

```sql
-- Create ML function
CREATE FUNCTION ml_predict(features frozen<list<double>>)
RETURNS double
LANGUAGE java
AS 'return MLModel.predict(features);';

-- Use in query
SELECT ml_predict([amount, tenure]) as risk_score
FROM customers;
```

### HTTP REST API

```bash
# Train model
curl -X POST http://localhost:8080/ml/train \
  -H "Content-Type: application/json" \
  -d '{
    "model_name": "recommender",
    "algorithm": "collaborative_filtering",
    "data_source": "user_interactions"
  }'

# Predict
curl -X POST http://localhost:8080/ml/predict \
  -H "Content-Type: application/json" \
  -d '{
    "model_name": "recommender",
    "user_id": "user123"
  }'
```

---

## Industry Models (28 Verticals)

### Healthcare
- Disease risk prediction
- Drug interaction detection
- Readmission prediction
- Medical image analysis

### Finance
- Fraud detection
- Credit risk scoring
- AML (Anti-Money Laundering)
- Algorithmic trading

### Retail
- Demand forecasting
- Price optimization
- Product recommendations
- Inventory optimization

### Additional Verticals
- Adtech, Defense, Education, Energy
- Government, Insurance, IoT, Legal
- Logistics, Manufacturing, Media
- Real Estate, Telecom, Transportation

---

## Configuration

### Enable ML Features

```toml
# orbit-server.toml
[ml]
enabled = true
model_cache_size = "1GB"
inference_threads = 4

[ml.gpu]
enabled = true
device = "auto"  # auto, cuda, metal, cpu

[ml.models]
cache_dir = "/var/lib/orbit/models"
max_model_size = "500MB"
```

### Resource Limits

```toml
[ml.limits]
max_inference_time_ms = 5000
max_batch_size = 1000
memory_limit = "4GB"
```

---

## Implementation Files

```
orbit/server/src/ai/
├── controller.rs       # AI master controller
├── optimizer/          # Query optimizer
├── resource/           # Resource manager
├── storage/            # Storage manager
├── transaction/        # Transaction manager
├── learning.rs         # Learning engine
├── decision.rs         # Decision engine
└── knowledge.rs        # Knowledge base

orbit/ml/src/
├── models/             # ML model implementations
├── inference/          # Inference engine
├── training/           # Training infrastructure
└── transformers/       # Transformer architectures
```

---

## Resources

- **RFC**: [AI-Native Features RFC](../rfcs/RFC_INDEX.md#rfc-004-ai-native-database-features)
- **Source**: `orbit/server/src/ai/`, `orbit/ml/`
- **Examples**: `orbit-examples/ml-protocol-examples/`
