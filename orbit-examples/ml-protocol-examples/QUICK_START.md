# Quick Start Guide - ML Industry Examples

## 🚀 Get Started in 5 Minutes

### Step 1: Start Orbit-RS Server

```bash
cd orbit-rs
cargo run --bin orbit-server
```

### Step 2: Connect via PostgreSQL

```bash
psql -h localhost -p 5432 -U orbit -d postgres
```

### Step 3: Run an Example

```bash
# Healthcare example
\i orbit-examples/ml-protocol-examples/01_healthcare_ml.sql

# Or from command line
psql -h localhost -p 5432 -U orbit -d postgres -f orbit-examples/ml-protocol-examples/01_healthcare_ml.sql
```

---

## 📊 Industry Examples Overview

| Industry | File | Use Cases | Lines |
|----------|------|-----------|-------|
| **Healthcare** | `01_healthcare_ml.sql` | Patient risk, medical imaging, drug discovery | 325 |
| **Finance** | `02_finance_ml.sql` | Fraud detection, credit scoring, trading | 504 |
| **Retail** | `03_retail_ecommerce_ml.sql` | Recommendations, pricing, inventory | 581 |
| **Manufacturing** | `04_manufacturing_iot_ml.sql` | Predictive maintenance, quality control | 559 |
| **Telecom** | `05_telecommunications_ml.sql` | Network optimization, churn prediction | 607 |
| **Logistics** | `06_logistics_transportation_ml.sql` | Route optimization, fleet management | 578 |

**Total:** 3,154 lines of production-ready SQL examples

---

## 🎯 Key Features Demonstrated

### 1. Vector Similarity Search

```sql
-- Find similar products
SELECT product_id, product_name, 
       1 - (product_embedding <=> query_embedding) AS similarity
FROM products
ORDER BY product_embedding <=> query_embedding
LIMIT 5;
```

**Use Cases:**
- Product recommendations (Retail)
- Similar medical cases (Healthcare)
- Fraud pattern matching (Finance)
- Defect pattern detection (Manufacturing)

### 2. Time Series Analysis

```sql
-- Moving average for trend detection
SELECT timestamp, value,
    AVG(value) OVER (
        ORDER BY timestamp 
        ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
    ) AS ma_7day
FROM sensor_readings;
```

**Use Cases:**
- Vital signs monitoring (Healthcare)
- Stock price analysis (Finance)
- Demand forecasting (Retail)
- Equipment monitoring (Manufacturing)
- Network performance (Telecom)
- Delivery demand (Logistics)

### 3. Predictive ML Models

```sql
-- Risk prediction function
CREATE FUNCTION predict_risk(
    p_age INTEGER,
    p_feature1 FLOAT,
    p_feature2 FLOAT
) RETURNS FLOAT AS $$
DECLARE
    risk_score FLOAT := 0.0;
BEGIN
    -- ML model logic
    risk_score := (p_age * 0.3) + (p_feature1 * 0.4) + (p_feature2 * 0.3);
    RETURN LEAST(risk_score, 1.0);
END;
$$ LANGUAGE plpgsql;
```

**Use Cases:**
- Patient readmission risk (Healthcare)
- Credit scoring (Finance)
- Churn prediction (Retail, Telecom)
- Equipment failure (Manufacturing)
- Delivery time prediction (Logistics)

### 4. Spatial Functions

```sql
-- Calculate distance between coordinates
SELECT calculate_distance_km(
    origin_lat, origin_lon,
    dest_lat, dest_lon
) AS distance_km;
```

**Use Cases:**
- Delivery route optimization (Logistics)
- Cell tower coverage (Telecom)
- Store location analysis (Retail)

---

## 💡 Example Queries by Use Case

### Product Recommendations (Retail)

```sql
-- Collaborative filtering
WITH customer_products AS (
    SELECT product_embedding
    FROM customer_purchases cp
    JOIN products p ON cp.product_id = p.product_id
    WHERE cp.customer_id = 1001
)
SELECT p.product_id, p.product_name, p.price,
       1 - (p.product_embedding <=> cp.product_embedding) AS similarity
FROM products p
CROSS JOIN customer_products cp
ORDER BY similarity DESC
LIMIT 10;
```

### Fraud Detection (Finance)

```sql
-- Real-time fraud scoring
SELECT transaction_id, amount, merchant_name, fraud_score,
    CASE 
        WHEN fraud_score > 0.8 THEN 'CRITICAL'
        WHEN fraud_score > 0.6 THEN 'HIGH'
        ELSE 'MEDIUM'
    END AS risk_level
FROM transactions
WHERE fraud_score > 0.5
ORDER BY fraud_score DESC;
```

### Predictive Maintenance (Manufacturing)

```sql
-- Equipment failure prediction
SELECT equipment_name, failure_probability,
    CASE 
        WHEN failure_probability > 0.7 THEN 'CRITICAL - Schedule Immediately'
        WHEN failure_probability > 0.5 THEN 'HIGH - Schedule This Week'
        ELSE 'MEDIUM - Monitor'
    END AS maintenance_priority
FROM equipment
WHERE failure_probability > 0.3
ORDER BY failure_probability DESC;
```

### Network Anomaly Detection (Telecom)

```sql
-- Detect latency spikes
WITH network_stats AS (
    SELECT tower_id, timestamp, latency_ms,
        AVG(latency_ms) OVER (
            PARTITION BY tower_id 
            ORDER BY timestamp 
            ROWS BETWEEN 59 PRECEDING AND CURRENT ROW
        ) AS avg_latency_1h
    FROM network_metrics
)
SELECT tower_name, latency_ms, avg_latency_1h
FROM network_stats ns
JOIN cell_towers ct ON ns.tower_id = ct.tower_id
WHERE ns.latency_ms > ns.avg_latency_1h * 2;
```

---

## 🔧 Performance Tips

### 1. Create Vector Indexes

```sql
-- For fast similarity search
CREATE INDEX products_embedding_idx 
ON products USING ivfflat (product_embedding vector_cosine_ops);
```

### 2. Optimize Time Series Queries

```sql
-- Index on timestamp
CREATE INDEX sensor_readings_timestamp_idx 
ON sensor_readings (timestamp);

-- Partition by date for large datasets
CREATE TABLE sensor_readings_2024_12 
PARTITION OF sensor_readings 
FOR VALUES FROM ('2024-12-01') TO ('2025-01-01');
```

### 3. Use Materialized Views

```sql
-- Pre-compute expensive aggregations
CREATE MATERIALIZED VIEW daily_sales_summary AS
SELECT product_id, sale_date, 
       SUM(units_sold) AS total_units,
       AVG(revenue) AS avg_revenue
FROM daily_sales
GROUP BY product_id, sale_date;

-- Refresh periodically
REFRESH MATERIALIZED VIEW daily_sales_summary;
```

---

## 🐍 Python Client Integration

```python
from orbit_client import OrbitClient
import numpy as np

# Connect to Orbit-RS
client = OrbitClient.postgres(
    host="127.0.0.1",
    port=5432,
    username="orbit",
    password="",
    database="postgres"
)

# Vector similarity search
query_embedding = np.random.rand(256).tolist()

results = client.execute("""
    SELECT product_id, product_name, price,
           1 - (product_embedding <=> %s::vector(256)) AS similarity
    FROM products
    ORDER BY product_embedding <=> %s::vector(256)
    LIMIT 10
""", params=(query_embedding, query_embedding))

for row in results:
    print(f"{row['product_name']}: {row['similarity']:.4f}")

# Time series query
results = client.execute("""
    SELECT timestamp, value,
           AVG(value) OVER (
               ORDER BY timestamp 
               ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
           ) AS moving_avg
    FROM sensor_readings
    WHERE equipment_id = %s
    ORDER BY timestamp DESC
    LIMIT 100
""", params=(1,))

# Predictive query
results = client.execute("""
    SELECT customer_id, name, 
           predict_customer_churn(
               account_age_months,
               num_products,
               avg_monthly_balance,
               num_transactions_monthly,
               num_customer_service_calls,
               last_login_days_ago
           ) AS churn_probability
    FROM customer_accounts
    WHERE churn_probability > 0.5
    ORDER BY churn_probability DESC
""")

client.disconnect()
```

---

## 📚 Learn More

- **Full Documentation:** `README.md`
- **Orbit-RS Architecture:** `../../docs/PRD.md`
- **Python Client Examples:** `../../orbit-python-client/examples/`
- **VS Code Extension:** `../../orbit-vscode-extension/`

---

## 🎓 Next Steps

1. **Explore Examples:** Run each industry example to see ML in action
2. **Customize:** Modify examples for your specific use case
3. **Integrate:** Use Python client or other protocols (Redis, MySQL, gRPC)
4. **Scale:** Deploy with Kubernetes using Orbit operator
5. **Monitor:** Use Prometheus metrics and AI subsystems

---

**Ready to build AI-native applications with Orbit-RS!** 🚀
