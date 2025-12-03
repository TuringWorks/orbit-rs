# ML Protocol Examples

This directory contains examples demonstrating how to use Orbit's ML capabilities across different protocols.

## Directory Structure

```
ml-protocol-examples/
├── rust/           # Rust examples using orbit-client
├── python/         # Python examples (psycopg2, redis-py, requests)
├── javascript/     # JavaScript/Node.js examples
└── sql/            # Pure SQL examples for PostgreSQL protocol
```

## Prerequisites

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
