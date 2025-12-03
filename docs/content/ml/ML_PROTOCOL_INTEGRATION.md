# ML Protocol Integration Guide

This document describes how to use Orbit's Machine Learning capabilities across all supported protocols: Redis (RESP), PostgreSQL, MySQL, CQL (Cassandra), HTTP REST, and gRPC.

## Overview

Orbit provides unified ML capabilities across all database protocols, enabling you to:

- **Train models** directly on your data without ETL
- **Run inference** at query time with low latency
- **Manage models** (versioning, deployment, evaluation)
- **Access 140+ industry-specific models** across 28 verticals

## Protocol-Specific ML Commands

### Redis (RESP) Protocol - Port 6379

Redis commands provide a simple, fast interface for ML operations.

#### Model Management

```redis
# Create/register a model
ML.CREATE model_name algorithm [OPTIONS]
ML.CREATE fraud_detector random_forest FEATURES amount,merchant_category,hour LABEL is_fraud

# Train a model
ML.TRAIN model_name data_key [OPTIONS]
ML.TRAIN fraud_detector transactions:train EPOCHS 100

# Delete a model
ML.DELETE model_name

# List all models
ML.LIST [PATTERN]
ML.LIST fraud_*
```

#### Inference

```redis
# Run prediction
ML.PREDICT model_name input_data
ML.PREDICT fraud_detector "[100.50, 'electronics', 14]"

# Batch prediction
ML.PREDICT.BATCH model_name key_pattern [LIMIT n]
ML.PREDICT.BATCH fraud_detector transactions:pending:* LIMIT 1000

# Get prediction with confidence
ML.PREDICT.SCORE model_name input_data
```

#### Vector Operations with ML

```redis
# Generate embeddings
ML.EMBED text_or_data model_name
ML.EMBED "machine learning tutorial" sentence-transformers

# Semantic search
ML.SEARCH.SEMANTIC index_name query_text [LIMIT n]
ML.SEARCH.SEMANTIC documents "how to train models" LIMIT 10

# Similarity with ML-enhanced ranking
ML.SEARCH.RERANK index_name query_text reranker_model [LIMIT n]
```

#### Industry Models

```redis
# Healthcare
ML.HEALTHCARE.PREDICT disease_risk patient_data
ML.HEALTHCARE.PREDICT diagnosis_risk "{age: 45, symptoms: ['fatigue', 'weight_loss']}"

# Finance
ML.FINANCE.PREDICT fraud_score transaction_data
ML.FINANCE.PREDICT credit_risk customer_profile

# Retail
ML.RETAIL.PREDICT demand_forecast product_data
ML.RETAIL.PREDICT customer_churn customer_features
```

### PostgreSQL Protocol - Port 5432

SQL functions provide declarative ML integrated with your queries.

#### Statistical Functions

```sql
-- Linear regression
SELECT
    category,
    ML_LINEAR_REGRESSION(ARRAY[price, quantity], revenue) AS model
FROM sales
GROUP BY category;

-- Correlation analysis
SELECT
    ML_CORRELATION(temperature, sales) AS temp_sales_corr,
    ML_CORRELATION(humidity, sales) AS humidity_sales_corr
FROM weather_sales;
```

#### Model Training

```sql
-- Train a classification model
SELECT ML_TRAIN_MODEL(
    'customer_churn_model',          -- model name
    'random_forest',                  -- algorithm
    ARRAY[tenure, monthly_charges, total_charges],  -- features
    churned                           -- target
) FROM customer_data;

-- Train with options
SELECT ML_TRAIN_MODEL(
    'fraud_detector',
    'xgboost',
    ARRAY[amount, merchant_type, hour, day_of_week],
    is_fraud,
    '{"max_depth": 10, "n_estimators": 100}'::jsonb
) FROM transactions WHERE date > '2024-01-01';
```

#### Inference

```sql
-- Single prediction
SELECT
    customer_id,
    ML_PREDICT('customer_churn_model',
        ARRAY[tenure, monthly_charges, total_charges]) AS churn_probability
FROM customers;

-- Prediction with threshold
SELECT
    customer_id,
    ML_PREDICT('customer_churn_model', features) AS churn_prob
FROM customers
WHERE ML_PREDICT('customer_churn_model', features) > 0.7;

-- Batch prediction with explanation
SELECT
    customer_id,
    ML_PREDICT_EXPLAIN('customer_churn_model', features) AS prediction_details
FROM customers;
```

#### Clustering

```sql
-- K-means clustering
SELECT
    customer_id,
    ML_KMEANS(ARRAY[total_spent, visit_frequency, avg_order], 5) AS segment
FROM customer_metrics;

-- DBSCAN for outlier detection
SELECT
    transaction_id,
    ML_DBSCAN(ARRAY[amount, hour, merchant_risk], 0.5, 5) AS cluster
FROM transactions;
```

#### Feature Engineering

```sql
-- Normalize features
SELECT
    id,
    ML_NORMALIZE(features, 'minmax') AS normalized_features
FROM raw_data;

-- PCA dimensionality reduction
SELECT
    id,
    ML_PCA(high_dim_features, 3) AS reduced_features
FROM embeddings;

-- Encode categorical variables
SELECT
    id,
    ML_ENCODE_CATEGORICAL(category, 'onehot') AS encoded_category
FROM products;
```

#### Time Series

```sql
-- Forecast future values
SELECT
    date,
    actual_sales,
    ML_FORECAST(actual_sales OVER (ORDER BY date ROWS 365 PRECEDING), 30) AS forecast
FROM daily_sales;

-- Anomaly detection
SELECT
    timestamp,
    value,
    ML_ANOMALY_DETECT(value OVER (ORDER BY timestamp ROWS 100 PRECEDING)) AS is_anomaly
FROM sensor_data
WHERE ML_ANOMALY_DETECT(value OVER (ORDER BY timestamp ROWS 100 PRECEDING)) = true;
```

#### Vector Similarity with ML

```sql
-- Create table with vector column
CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    content TEXT,
    embedding vector(384)
);

-- Generate embeddings
INSERT INTO documents (content, embedding)
SELECT
    content,
    ML_EMBED_TEXT(content, 'sentence-transformers')
FROM raw_documents;

-- Semantic search with pgvector operators
SELECT
    content,
    embedding <=> ML_EMBED_TEXT('machine learning', 'sentence-transformers') AS distance
FROM documents
ORDER BY distance
LIMIT 10;
```

### MySQL Protocol - Port 3306

MySQL-compatible ML functions follow similar patterns.

```sql
-- Train model
CALL ML_TRAIN('churn_model', 'logistic_regression',
    'SELECT tenure, charges, churned FROM customers');

-- Predict
SELECT
    customer_id,
    ML_PREDICT('churn_model', tenure, charges) AS churn_risk
FROM customers;

-- Model evaluation
SELECT ML_EVALUATE('churn_model',
    'SELECT tenure, charges, churned FROM test_data');
```

### CQL (Cassandra) Protocol - Port 9042

CQL provides ML through User-Defined Functions (UDFs).

```cql
-- Create ML function
CREATE FUNCTION ml_predict(model_name text, features list<double>)
RETURNS DOUBLE
LANGUAGE orbit_ml
AS 'predict';

-- Use in query
SELECT
    customer_id,
    ml_predict('churn_model', [tenure, monthly_charges]) AS churn_prob
FROM customers
WHERE partition_key = 'region_us';

-- Batch operations with ML
SELECT
    product_id,
    ml_forecast('demand_model', historical_sales, 30) AS forecast
FROM inventory
WHERE category = 'electronics';
```

### HTTP REST API - Port 8080

RESTful endpoints for ML operations.

#### Model Management

```bash
# Create model
POST /ml/models
{
    "name": "fraud_detector",
    "algorithm": "random_forest",
    "features": ["amount", "merchant_category", "hour"],
    "target": "is_fraud"
}

# Train model
POST /ml/models/fraud_detector/train
{
    "data_source": "transactions",
    "options": {
        "epochs": 100,
        "validation_split": 0.2
    }
}

# Get model info
GET /ml/models/fraud_detector

# List models
GET /ml/models?pattern=fraud_*
```

#### Inference

```bash
# Single prediction
POST /ml/models/fraud_detector/predict
{
    "features": [100.50, "electronics", 14]
}

# Response
{
    "prediction": 0.85,
    "confidence": 0.92,
    "model_version": "v1.2.0"
}

# Batch prediction
POST /ml/models/fraud_detector/predict/batch
{
    "instances": [
        {"features": [100.50, "electronics", 14]},
        {"features": [25.00, "groceries", 10]}
    ]
}

# Streaming prediction
POST /ml/models/fraud_detector/predict/stream
Content-Type: application/x-ndjson

{"features": [100.50, "electronics", 14]}
{"features": [25.00, "groceries", 10]}
```

#### Industry Models

```bash
# Healthcare predictions
POST /ml/industry/healthcare/predict
{
    "model": "disease_risk",
    "patient": {
        "age": 45,
        "symptoms": ["fatigue", "weight_loss"],
        "lab_results": {"glucose": 126, "bmi": 28.5}
    }
}

# Financial analysis
POST /ml/industry/finance/predict
{
    "model": "credit_risk",
    "customer": {
        "income": 75000,
        "debt_ratio": 0.35,
        "credit_history_years": 8
    }
}
```

### gRPC Protocol - Port 50051

High-performance ML operations via gRPC.

```protobuf
// ML Service definition
service MLService {
    // Model management
    rpc CreateModel(CreateModelRequest) returns (ModelMetadata);
    rpc TrainModel(TrainModelRequest) returns (stream TrainingProgress);
    rpc DeleteModel(DeleteModelRequest) returns (Empty);
    rpc ListModels(ListModelsRequest) returns (ModelList);

    // Inference
    rpc Predict(PredictRequest) returns (PredictResponse);
    rpc PredictStream(stream PredictRequest) returns (stream PredictResponse);
    rpc PredictBatch(PredictBatchRequest) returns (PredictBatchResponse);

    // Model evaluation
    rpc EvaluateModel(EvaluateRequest) returns (EvaluationMetrics);
}

message PredictRequest {
    string model_name = 1;
    repeated double features = 2;
    map<string, string> options = 3;
}

message PredictResponse {
    double prediction = 1;
    double confidence = 2;
    map<string, double> feature_importance = 3;
}
```

## Industry Model Verticals

Orbit provides 140+ pre-built models across 28 industry verticals:

### Business Verticals

| Vertical | Models | Example Use Cases |
|----------|--------|-------------------|
| Healthcare | disease_risk, readmission, drug_interaction | Patient risk assessment, treatment recommendation |
| Fintech | fraud_detection, credit_risk, aml | Transaction monitoring, loan approval |
| Adtech | ctr_prediction, bid_optimization, audience_segment | Ad targeting, campaign optimization |
| Defense | threat_detection, anomaly_classify, pattern_recognition | Security monitoring, surveillance |
| Logistics | route_optimization, demand_forecast, inventory | Supply chain optimization |
| Banking | default_prediction, customer_ltv, churn | Risk management, customer retention |
| Insurance | claim_fraud, risk_pricing, underwriting | Claims processing, policy pricing |

### Advanced AI Verticals

| Vertical | Models | Example Use Cases |
|----------|--------|-------------------|
| Physical AI | robot_navigation, sensor_fusion, motion_planning | Autonomous systems, robotics |
| Drug Discovery | molecule_generation, binding_affinity, toxicity | Drug development, compound screening |
| Genomics | sequence_analysis, variant_calling, gene_expression | Genetic research, precision medicine |
| Physics | simulation_surrogate, material_property, quantum_state | Scientific computing, material science |
| Industrial AI | predictive_maintenance, quality_control, process_optimization | Manufacturing, quality assurance |
| IoT | device_anomaly, energy_optimization, predictive_failure | Smart devices, energy management |

### Retail & Consumer

| Vertical | Models | Example Use Cases |
|----------|--------|-------------------|
| Retail | demand_forecast, price_optimization, recommendation | Inventory, pricing, personalization |
| Fashion | trend_prediction, style_matching, size_recommendation | Product design, customer experience |
| FMCG/CPG | shelf_optimization, promotion_effectiveness, brand_sentiment | Marketing, distribution |

## Configuration

### Server Configuration

```toml
# orbit-server.toml
[ml]
enabled = true
model_storage_path = "/var/lib/orbit/models"
max_concurrent_training = 4
inference_cache_size = "1GB"
gpu_acceleration = true

[ml.industry_models]
healthcare = true
finance = true
retail = true
# Enable specific verticals

[ml.limits]
max_model_size = "10GB"
max_training_time = "24h"
max_batch_size = 10000
```

### Client Configuration

```python
# Python client example
from orbit import OrbitClient

client = OrbitClient(
    host="localhost",
    port=5432,
    ml_options={
        "timeout": 30,
        "retry_on_failure": True,
        "cache_predictions": True
    }
)
```

## Best Practices

### 1. Choose the Right Protocol

| Use Case | Recommended Protocol |
|----------|---------------------|
| Real-time inference | Redis (RESP) |
| Complex analytics | PostgreSQL |
| High-throughput batch | gRPC |
| REST API integration | HTTP |
| Wide-column data | CQL |

### 2. Model Lifecycle Management

```sql
-- Version your models
SELECT ML_TRAIN_MODEL('model_v2', ...);

-- A/B testing
SELECT
    id,
    ML_PREDICT('model_v1', features) AS pred_v1,
    ML_PREDICT('model_v2', features) AS pred_v2
FROM test_data;

-- Evaluate before promotion
SELECT ML_EVALUATE_MODEL('model_v2', test_features, test_labels);

-- Promote to production
SELECT ML_PROMOTE_MODEL('model_v2', 'production');
```

### 3. Performance Optimization

- Use batch predictions for bulk operations
- Enable GPU acceleration for large models
- Cache frequently-used model predictions
- Use approximate algorithms for exploratory analysis

### 4. Security

- Role-based access control for ML operations
- Audit logging for model training and predictions
- Encryption for model storage
- Input validation to prevent adversarial attacks

## Examples

See the `orbit-examples/ml-protocol-examples/` directory for complete working examples in:

- **Rust**: Native Orbit client examples
- **Python**: psycopg2, redis-py, requests examples
- **JavaScript**: Node.js client examples
- **SQL**: Pure SQL examples for PostgreSQL protocol

## See Also

- [ML SQL Functions Design](ML_SQL_FUNCTIONS_DESIGN.md)
- [Industry Models Plan](INDUSTRY_MODELS_PLAN.md)
- [GPU Acceleration](../GPU_ACCELERATION_COMPLETE.md)
- [Vector Operations](../ORBITQL_COMPLETE_DOCUMENTATION.md)
